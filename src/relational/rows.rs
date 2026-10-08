//! Row verbs produce SELECT results. Checks and results share the execution snapshot.
use super::*;
use crate::execution::ValidationQuery;
use crate::parser::{JoinKey, JoinOptions, JoinSpec};
use std::collections::HashMap;

fn query(source: SqlSource, projection: Vec<SelectItem>) -> SqlQuery {
    SqlQuery {
        source,
        projection,
        filter: None,
        group_by: Vec::new(),
        order_by: Vec::new(),
        distinct: false,
        limit: None,
    }
}
fn reference(name: &str) -> Expr {
    Expr::Identifier(name.into())
}
fn scalar(expr: Expr, alias: &str) -> SelectItem {
    SelectItem {
        alias: alias.into(),
        expression: SelectExpression::Scalar {
            expr,
            partition_by: Vec::new(),
        },
    }
}

impl Planner<'_> {
    pub(super) fn rows(
        &mut self,
        input: Relation,
        name: &str,
        args: &[Expr],
    ) -> GenerationResult<Relation> {
        let (operand, options) = args
            .split_first()
            .ok_or_else(|| invalid("rows verb needs a right relation"))?;
        let right = self.relation_operand(operand)?;
        let mut opts = HashMap::new();
        for arg in options {
            let Expr::NamedArg { name, value } = arg else {
                return Err(invalid("rows options must be named"));
            };
            if !matches!(
                name.as_str(),
                "by" | "conflict" | "unmatched" | "in_place" | "copy"
            ) || opts.insert(name.as_str(), value.as_ref()).is_some()
            {
                return Err(invalid("unknown or duplicate rows option"));
            }
        }
        if opts
            .get("in_place")
            .is_some_and(|e| !matches!(e, Expr::Literal(LiteralValue::Boolean(false))))
        {
            return Err(self.unsupported("rows in_place requires a database write adapter"));
        }
        if opts
            .get("copy")
            .is_some_and(|e| !matches!(e, Expr::Literal(LiteralValue::String(v)) if v=="none"))
        {
            return Err(self.unsupported("rows copy requires a database connection adapter"));
        }
        for c in right.columns.iter().filter(|c| !c.hidden) {
            visible_column(&input.columns, &c.schema.name)?;
        }
        if name == "rows_append" {
            if opts
                .keys()
                .any(|k| matches!(*k, "by" | "conflict" | "unmatched"))
            {
                return Err(invalid("rows_append has no key options"));
            }
            return self.set_relations(input, right, &SetOperation::UnionAll);
        }
        let mut by = Vec::new();
        if let Some(expr) = opts.get("by") {
            fn keys(expr: &Expr, out: &mut Vec<String>) -> GenerationResult<()> {
                match expr {
                    Expr::Identifier(n) | Expr::Literal(LiteralValue::String(n)) => {
                        out.push(n.clone())
                    }
                    Expr::Function { name, args } if name == "c" => {
                        for e in args {
                            keys(e, out)?;
                        }
                    }
                    _ => return Err(invalid("rows by must contain column names")),
                }
                Ok(())
            }
            keys(expr, &mut by)?;
        } else {
            by.push(
                right
                    .columns
                    .iter()
                    .find(|c| !c.hidden)
                    .ok_or_else(|| invalid("empty rows input"))?
                    .schema
                    .name
                    .clone(),
            );
        }
        if by.is_empty() {
            return Err(invalid("rows needs at least one key"));
        }
        let mut unique = std::collections::HashSet::new();
        for key in &by {
            visible_column(&input.columns, key)?;
            visible_column(&right.columns, key)?;
            if !unique.insert(key) {
                return Err(invalid("duplicate rows key"));
            }
        }
        let spec = JoinSpec {
            table: String::new(),
            by: by
                .iter()
                .map(|n| JoinKey {
                    left: n.clone(),
                    right: n.clone(),
                })
                .collect(),
            on_expr: None,
            options: JoinOptions::default(),
            right_operations: Vec::new(),
        };
        let option = if name == "rows_insert" {
            "conflict"
        } else {
            "unmatched"
        };
        if opts.contains_key(if option == "conflict" {
            "unmatched"
        } else {
            "conflict"
        }) {
            return Err(invalid("rows option does not apply to this verb"));
        }
        let policy = match opts.get(option) {
            Some(Expr::Literal(LiteralValue::String(v))) if v == "error" || v == "ignore" => {
                v.as_str()
            }
            None => "error",
            _ => return Err(invalid("rows policy must be error or ignore")),
        };
        let validate_unique = matches!(name, "rows_update" | "rows_patch" | "rows_upsert");
        let validate_matching = name != "rows_upsert" && policy == "error";
        if validate_unique || validate_matching {
            if self.checks.is_none() {
                return Err(invalid(
                    "rows validation requires plan_with_schemas() and execute()",
                ));
            }
            if join::volatile_relation(&input) || join::volatile_relation(&right) {
                return Err(invalid("rows validation requires stable input"));
            }
            if validate_unique {
                let mut grouped = query(
                    SqlSource::Subquery(Box::new(lower(&right, &mut 1)?), "__rows_keys".into()),
                    by.iter().map(|n| scalar(reference(n), n)).collect(),
                );
                let count = self.hidden_name(&right.columns, "key_count");
                grouped.projection.push(SelectItem {
                    alias: count.clone(),
                    expression: SelectExpression::AggregateExpression(Expr::Function {
                        name: "n".into(),
                        args: Vec::new(),
                    }),
                });
                grouped.group_by = by.clone();
                let mut check = query(
                    SqlSource::Subquery(Box::new(grouped), "__rows_duplicates".into()),
                    vec![scalar(Expr::Literal(LiteralValue::Number(1.0)), "invalid")],
                );
                check.filter = Some(Expr::Binary {
                    left: Box::new(reference(&count)),
                    operator: BinaryOp::GreaterThan,
                    right: Box::new(Expr::Literal(LiteralValue::Number(1.0))),
                });
                check.limit = Some(1);
                let sql = check.render(self.generator)?;
                self.checks
                    .as_mut()
                    .expect("checked execution plan")
                    .push(ValidationQuery::new(sql, "rows right keys must be unique"));
            }
            if validate_matching {
                let bad = self.join_relations(
                    right.clone(),
                    input.clone(),
                    if name == "rows_insert" {
                        &JoinType::Semi
                    } else {
                        &JoinType::Anti
                    },
                    &spec,
                    false,
                )?;
                let mut check = lower(&bad, &mut 1)?;
                check.limit = Some(1);
                let sql = check.render(self.generator)?;
                self.checks
                    .as_mut()
                    .expect("checked execution plan")
                    .push(ValidationQuery::new(
                        sql,
                        format!("rows {option} policy violated"),
                    ));
            }
        }
        if name == "rows_delete" {
            return self.join_relations(input, right, &JoinType::Anti, &spec, false);
        }
        if name == "rows_insert" {
            let extra = self.join_relations(right, input.clone(), &JoinType::Anti, &spec, false)?;
            return self.set_relations(input, extra, &SetOperation::UnionAll);
        }
        let appended = if name == "rows_upsert" {
            Some(self.join_relations(
                right.clone(),
                input.clone(),
                &JoinType::Anti,
                &spec,
                false,
            )?)
        } else {
            None
        };
        let mut renamed = Vec::new();
        let mut items = Vec::new();
        let mut reserved = input.columns.clone();
        for c in &right.columns {
            if c.hidden {
                continue;
            }
            let n = self.hidden_name(&reserved, "row_value");
            let mut out = self.new_column(&n);
            out.schema = c.schema.clone();
            out.schema.name = n.clone();
            reserved.push(out.clone());
            items.push(Projection {
                column: out,
                expression: BoundExpr::Column(c.id),
            });
            renamed.push((c.schema.name.clone(), n));
        }
        let marker = self.hidden_name(&reserved, "row_present");
        let mark = self.new_column(&marker);
        items.push(Projection {
            column: mark,
            expression: BoundExpr::Literal(LiteralValue::Boolean(true)),
        });
        let marked = self.project(right, items)?;
        let keys = by
            .iter()
            .map(|n| {
                Ok((
                    n.clone(),
                    renamed
                        .iter()
                        .find(|(old, _)| old == n)
                        .ok_or_else(|| invalid("missing rows key"))?
                        .1
                        .clone(),
                ))
            })
            .collect::<GenerationResult<_>>()?;
        let projection = input
            .columns
            .iter()
            .map(|c| {
                let original = reference(&c.schema.name);
                let expr = match renamed
                    .iter()
                    .find(|(old, _)| old == &c.schema.name && !by.contains(old))
                {
                    Some((_, new)) => {
                        if name == "rows_patch" {
                            Expr::Function {
                                name: "coalesce".into(),
                                args: vec![original, reference(new)],
                            }
                        } else {
                            Expr::CaseWhen {
                                branches: vec![(
                                    Expr::Function {
                                        name: "is.na".into(),
                                        args: vec![reference(&marker)],
                                    },
                                    original,
                                )],
                                default: Some(Box::new(reference(new))),
                            }
                        }
                    }
                    None => original,
                };
                scalar(expr, &c.schema.name)
            })
            .collect();
        let q = query(
            SqlSource::Join {
                left: Box::new(lower(&input, &mut 1)?),
                right: Box::new(lower(&marked, &mut 1)?),
                join_type: JoinType::Left,
                keys,
                predicates: Vec::new(),
                closest: None,
                na_matches: false,
            },
            projection,
        );
        let schema = input
            .columns
            .iter()
            .map(|c| {
                let mut metadata = c.schema.clone();
                if name != "rows_patch"
                    && renamed
                        .iter()
                        .any(|(old, _)| old == &metadata.name && !by.contains(old))
                {
                    metadata.nullable = Some(true);
                    metadata.data_type = None;
                }
                metadata
            })
            .collect();
        let updated = self.query_relation(input, q, schema)?;
        if let Some(extra) = appended {
            self.set_relations(updated, extra, &SetOperation::UnionAll)
        } else {
            Ok(updated)
        }
    }
}
