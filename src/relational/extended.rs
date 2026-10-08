//! Additional query verbs reuse the existing relation and expression stages.
use super::*;
use crate::parser::{Assignment, ColumnExpr, SourceLocation};
use std::collections::{HashMap, HashSet};

fn arguments<'a>(
    args: &'a [Expr],
    controls: &[&str],
) -> GenerationResult<(Vec<&'a Expr>, HashMap<&'a str, &'a Expr>)> {
    let mut values = Vec::new();
    let mut options = HashMap::new();
    for arg in args {
        if let Expr::NamedArg { name, value } = arg {
            if controls.contains(&name.as_str()) {
                if options.insert(name.as_str(), value.as_ref()).is_some() {
                    return Err(invalid(format!("option {name} was given twice")));
                }
                continue;
            }
            if name.starts_with('.') {
                return Err(invalid(format!("unsupported option {name}")));
            }
        }
        values.push(arg);
    }
    Ok((values, options))
}

fn string(expr: &Expr) -> GenerationResult<&str> {
    match expr {
        Expr::Literal(LiteralValue::String(value)) => Ok(value),
        _ => Err(invalid("expected a constant string")),
    }
}
fn boolean(expr: &Expr) -> GenerationResult<bool> {
    match expr {
        Expr::Literal(LiteralValue::Boolean(value)) => Ok(*value),
        _ => Err(invalid("expected TRUE or FALSE")),
    }
}
fn integer(expr: &Expr) -> GenerationResult<usize> {
    match expr {
        Expr::Literal(LiteralValue::Number(value))
            if value.is_finite() && *value >= 0.0 && *value < usize::MAX as f64 =>
        {
            Ok(value.floor() as usize)
        }
        _ => Err(invalid("expected a finite nonnegative row count")),
    }
}
fn selectors(exprs: &[&Expr]) -> Vec<ColumnExpr> {
    exprs
        .iter()
        .flat_map(|expr| match expr {
            Expr::Function { name, args } if name == "c" || name == "list" => args
                .iter()
                .map(|e| ColumnExpr {
                    expr: e.clone(),
                    alias: None,
                })
                .collect(),
            Expr::NamedArg { name, value } => vec![ColumnExpr {
                expr: value.as_ref().clone(),
                alias: Some(name.clone()),
            }],
            expr => vec![ColumnExpr {
                expr: (*expr).clone(),
                alias: None,
            }],
        })
        .collect()
}
fn used_columns(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(name) => {
            names.insert(name.clone());
        }
        Expr::Function { args, .. } => {
            for arg in args {
                used_columns(arg, names);
            }
        }
        Expr::NamedArg { value, .. }
        | Expr::Unary { expr: value, .. }
        | Expr::In { expr: value, .. } => used_columns(value, names),
        Expr::Binary { left, right, .. } => {
            used_columns(left, names);
            used_columns(right, names);
        }
        Expr::CaseWhen { branches, default } => {
            for (a, b) in branches {
                used_columns(a, names);
                used_columns(b, names);
            }
            if let Some(e) = default {
                used_columns(e, names);
            }
        }
        _ => {}
    }
}
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

impl Planner<'_> {
    fn selected_names(&self, input: &Relation, values: &[&Expr]) -> GenerationResult<Vec<String>> {
        let schema = input
            .columns
            .iter()
            .filter(|c| !c.hidden)
            .map(|c| c.schema.clone())
            .collect::<Vec<_>>();
        selection::resolve(&selectors(values), &schema)?
            .into_iter()
            .map(|c| match c.expr {
                Expr::Identifier(name) => Ok(name),
                _ => Err(invalid("selector must resolve to a column")),
            })
            .collect()
    }
    fn temporary_groups(&self, input: &mut Relation, by: Option<&&Expr>) -> GenerationResult<bool> {
        if let Some(by) = by {
            if !input.groups.is_empty() {
                return Err(invalid(".by cannot be used on grouped input"));
            }
            input.groups = self
                .selected_names(input, &[*by])?
                .iter()
                .map(|name| visible_column(&input.columns, name).map(|c| c.id))
                .collect::<GenerationResult<_>>()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn assignments(&self, input: &Relation, args: &[&Expr]) -> GenerationResult<Vec<Assignment>> {
        let mut out = Vec::new();
        for arg in args {
            match arg {
                Expr::NamedArg { name, value } => out.push(Assignment {
                    column: name.clone(),
                    expr: self.expand_predicates(value, input)?,
                }),
                Expr::Function { name, .. } if name == "across" => {
                    out.extend(across::expand(arg, &self.across_schema(input))?)
                }
                _ => {
                    return Err(invalid(
                        "mutate entries must be named expressions or across()",
                    ))
                }
            }
        }
        Ok(out)
    }
    pub(super) fn expand_predicates(
        &self,
        expr: &Expr,
        input: &Relation,
    ) -> GenerationResult<Expr> {
        Ok(match expr {
            Expr::Function { name, args } if name == "if_any" || name == "if_all" => {
                let across = Expr::Function {
                    name: "across".into(),
                    args: args.clone(),
                };
                let mut values = across::expand(&across, &self.across_schema(input))?
                    .into_iter()
                    .map(|a| a.expr);
                let operator = if name == "if_any" {
                    BinaryOp::Or
                } else {
                    BinaryOp::And
                };
                let first = values
                    .next()
                    .unwrap_or(Expr::Literal(LiteralValue::Boolean(name == "if_all")));
                values.fold(first, |left, right| Expr::Binary {
                    left: Box::new(left),
                    operator: operator.clone(),
                    right: Box::new(right),
                })
            }
            Expr::Function { name, args } => Expr::Function {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|e| self.expand_predicates(e, input))
                    .collect::<GenerationResult<_>>()?,
            },
            Expr::NamedArg { name, value } => Expr::NamedArg {
                name: name.clone(),
                value: Box::new(self.expand_predicates(value, input)?),
            },
            Expr::Unary { operator, expr } => Expr::Unary {
                operator: operator.clone(),
                expr: Box::new(self.expand_predicates(expr, input)?),
            },
            Expr::Binary {
                left,
                operator,
                right,
            } => Expr::Binary {
                left: Box::new(self.expand_predicates(left, input)?),
                operator: operator.clone(),
                right: Box::new(self.expand_predicates(right, input)?),
            },
            Expr::In { expr, values } => Expr::In {
                expr: Box::new(self.expand_predicates(expr, input)?),
                values: values.clone(),
            },
            Expr::CaseWhen { branches, default } => Expr::CaseWhen {
                branches: branches
                    .iter()
                    .map(|(a, b)| {
                        Ok((
                            self.expand_predicates(a, input)?,
                            self.expand_predicates(b, input)?,
                        ))
                    })
                    .collect::<GenerationResult<_>>()?,
                default: default
                    .as_ref()
                    .map(|e| self.expand_predicates(e, input).map(Box::new))
                    .transpose()?,
            },
            other => other.clone(),
        })
    }
    fn ordered_query(&self, input: &Relation) -> GenerationResult<SqlQuery> {
        let mut result = lower(input, &mut 1)?;
        result.order_by = input
            .order
            .iter()
            .map(|(id, direction)| {
                Ok(OrderExpr {
                    column: column_by_id(&input.columns, *id)?.schema.name.clone(),
                    direction: direction.clone(),
                })
            })
            .collect::<GenerationResult<_>>()?;
        Ok(result)
    }
    pub(super) fn query_relation(
        &mut self,
        input: Relation,
        query: SqlQuery,
        schema: Vec<SchemaColumn>,
    ) -> GenerationResult<Relation> {
        let group_names = names_for_ids(&input.columns, &input.groups)?;
        let order = input
            .order
            .iter()
            .map(|(id, d)| {
                Ok((
                    column_by_id(&input.columns, *id)?.schema.name.clone(),
                    d.clone(),
                ))
            })
            .collect::<GenerationResult<Vec<_>>>()?;
        let columns = schema
            .into_iter()
            .map(|metadata| {
                let mut c = input
                    .columns
                    .iter()
                    .find(|c| c.schema.name == metadata.name)
                    .cloned()
                    .unwrap_or_else(|| self.new_column(&metadata.name));
                c.schema = metadata;
                c
            })
            .collect::<Vec<_>>();
        self.validate_output(&columns)?;
        let groups = group_names
            .iter()
            .filter_map(|name| {
                columns
                    .iter()
                    .find(|c| &c.schema.name == name)
                    .map(|c| c.id)
            })
            .collect();
        let order = order
            .into_iter()
            .filter_map(|(name, d)| {
                columns
                    .iter()
                    .find(|c| c.schema.name == name)
                    .map(|c| (c.id, d))
            })
            .collect();
        self.stage()?;
        Ok(Relation {
            node: RelNode::Query(Box::new(query)),
            columns,
            groups,
            order,
            frame: input.frame,
        })
    }
    pub(super) fn relation_operand(&mut self, expr: &Expr) -> GenerationResult<Relation> {
        match expr {
            Expr::Identifier(name) | Expr::Literal(LiteralValue::String(name)) => self.scan(name),
            Expr::Function { name, args } if name == "__pipeline" => {
                let (source, ops) = args
                    .split_first()
                    .ok_or_else(|| invalid("empty relation pipeline"))?;
                let mut input = self.relation_operand(source)?;
                for operation in ops {
                    let Expr::Function { name, args } = operation else {
                        return Err(invalid("invalid relation operation"));
                    };
                    input = self.extended(input, name, args)?;
                }
                Ok(input)
            }
            _ => Err(invalid("expected a table or a table pipeline")),
        }
    }
    pub(super) fn extended(
        &mut self,
        mut input: Relation,
        name: &str,
        args: &[Expr],
    ) -> GenerationResult<Relation> {
        let location = SourceLocation::unknown();
        let relational = matches!(
            name,
            "union"
                | "union_all"
                | "intersect"
                | "setdiff"
                | "cross_join"
                | "bind_queries"
                | "rows_insert"
                | "rows_append"
                | "rows_update"
                | "rows_patch"
                | "rows_upsert"
                | "rows_delete"
        );
        let resolved = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                if (relational && (index == 0 || name == "bind_queries"))
                    || (matches!(name, "mutate" | "transmute")
                        && matches!(arg,Expr::NamedArg{name,..} if !name.starts_with('.')))
                {
                    Ok(arg.clone())
                } else {
                    self.resolve_expression(arg, &input)
                }
            })
            .collect::<GenerationResult<Vec<_>>>()?;
        let args = resolved.as_slice();
        match name {
            "select" => self.apply(
                input,
                &DplyrOperation::Select {
                    columns: selectors(&args.iter().collect::<Vec<_>>()),
                    location,
                },
            ),
            "mutate" | "transmute" => {
                let (values, opts) = arguments(
                    args,
                    &[".by", ".keep", ".before", ".after", ".order", ".frame"],
                )?;
                let temporary = self.temporary_groups(&mut input, opts.get(".by"))?;
                let original_order = input.order.clone();
                let original_frame = input.frame;
                if let Some(order) = opts.get(".order") {
                    input = self.arrange_expressions(input, &[order], false)?;
                }
                if let Some(frame) = opts.get(".frame") {
                    input.frame = Some(parse_frame(frame)?);
                }
                let original = input
                    .columns
                    .iter()
                    .filter(|c| !c.hidden)
                    .map(|c| c.schema.name.clone())
                    .collect::<Vec<_>>();
                let mut used = HashSet::new();
                let mut assigned = HashSet::new();
                for value in &values {
                    let expanded = self.assignments(&input, &[*value])?;
                    for a in &expanded {
                        used_columns(&a.expr, &mut used);
                        assigned.insert(a.column.clone());
                    }
                    input = self.mutate_assignments(input, &expanded)?;
                }
                let keep = opts
                    .get(".keep")
                    .map(|e| string(e))
                    .transpose()?
                    .unwrap_or(if name == "transmute" { "none" } else { "all" });
                if !["all", "used", "unused", "none"].contains(&keep) {
                    return Err(invalid(".keep must be all, used, unused or none"));
                }
                if keep != "all" {
                    let items = input
                        .columns
                        .iter()
                        .filter(|c| {
                            !c.hidden
                                && (assigned.contains(&c.schema.name)
                                    || input.groups.contains(&c.id)
                                    || match keep {
                                        "used" => used.contains(&c.schema.name),
                                        "unused" => !used.contains(&c.schema.name),
                                        _ => false,
                                    })
                        })
                        .map(|c| Projection {
                            column: c.clone(),
                            expression: BoundExpr::Column(c.id),
                        })
                        .collect();
                    input = self.project(input, items)?;
                }
                if opts.contains_key(".before") && opts.contains_key(".after") {
                    return Err(invalid("use only one of .before and .after"));
                }
                if let Some((target, after)) = opts
                    .get(".before")
                    .map(|e| (*e, false))
                    .or_else(|| opts.get(".after").map(|e| (*e, true)))
                {
                    let selected = self.selected_names(&input, &[target])?;
                    let target = selected
                        .first()
                        .ok_or_else(|| invalid("placement selector is empty"))?;
                    let mut items = Self::identities(&input.columns);
                    let mut moved = Vec::new();
                    items.retain(|i| {
                        if assigned.contains(&i.column.schema.name)
                            && !original.contains(&i.column.schema.name)
                        {
                            moved.push(Projection {
                                column: i.column.clone(),
                                expression: BoundExpr::Column(i.column.id),
                            });
                            false
                        } else {
                            true
                        }
                    });
                    let index = items
                        .iter()
                        .position(|i| &i.column.schema.name == target)
                        .ok_or_else(|| invalid("placement target was removed"))?
                        + usize::from(after);
                    items.splice(index..index, moved);
                    input = self.project(input, items)?;
                }
                if temporary {
                    input.groups.clear();
                }
                if opts.contains_key(".order") {
                    input.order = original_order;
                }
                if opts.contains_key(".frame") {
                    input.frame = original_frame;
                }
                Ok(input)
            }
            "filter" | "filter_out" => {
                let (values, opts) = arguments(args, &[".by", ".preserve"])?;
                if let Some(preserve) = opts.get(".preserve") {
                    if boolean(preserve)? {
                        return Err(self.unsupported("filter(.preserve=TRUE)"));
                    }
                }
                let temporary = self.temporary_groups(&mut input, opts.get(".by"))?;
                let mut predicate = Expr::Literal(LiteralValue::Boolean(true));
                for value in values {
                    predicate = Expr::Binary {
                        left: Box::new(predicate),
                        operator: BinaryOp::And,
                        right: Box::new(self.expand_predicates(value, &input)?),
                    };
                }
                if name == "filter_out" {
                    predicate = Expr::Unary {
                        operator: UnaryOp::Not,
                        expr: Box::new(Expr::Function {
                            name: "coalesce".into(),
                            args: vec![predicate, Expr::Literal(LiteralValue::Boolean(false))],
                        }),
                    };
                }
                if !args.is_empty() {
                    input = self.apply(
                        input,
                        &DplyrOperation::Filter {
                            condition: predicate,
                            location,
                        },
                    )?;
                }
                if temporary {
                    input.groups.clear();
                }
                Ok(input)
            }
            "summarise" | "summarize" => {
                let (values, opts) = arguments(args, &[".by", ".groups"])?;
                if opts.contains_key(".by") && opts.contains_key(".groups") {
                    return Err(invalid(".by and .groups cannot be combined"));
                }
                let temporary = self.temporary_groups(&mut input, opts.get(".by"))?;
                let groups = input.groups.clone();
                let mut assignments = Vec::new();
                for value in values {
                    if let Expr::Function { name, .. } = value {
                        if name == "across" {
                            assignments.extend(
                                across::expand(value, &self.across_schema(&input))?
                                    .into_iter()
                                    .map(|a| (a.column, a.expr)),
                            );
                            continue;
                        }
                    }
                    match value {
                        Expr::NamedArg { name, value } => {
                            assignments.push((name.clone(), value.as_ref().clone()))
                        }
                        e => assignments.push((e.to_string(), e.clone())),
                    }
                }
                let policy = opts
                    .get(".groups")
                    .map(|e| string(e))
                    .transpose()?
                    .unwrap_or(if temporary { "drop" } else { "drop_last" });
                input = self.summarise(input, assignments)?;
                input.groups = match policy {
                    "drop" => Vec::new(),
                    "keep" => groups,
                    "drop_last" => groups.into_iter().take(input.groups.len()).collect(),
                    _ => return Err(invalid(".groups must be drop, drop_last or keep")),
                };
                Ok(input)
            }
            "group_by" => {
                let (values, opts) = arguments(args, &[".add", ".drop"])?;
                if let Some(drop) = opts.get(".drop") {
                    if !boolean(drop)? {
                        return Err(self
                            .unsupported("group_by(.drop=FALSE) needs explicit domain metadata"));
                    }
                }
                let add = opts
                    .get(".add")
                    .map(|e| boolean(e))
                    .transpose()?
                    .unwrap_or(false);
                let original = names_for_ids(&input.columns, &input.groups)?;
                input.groups.clear();
                let mut names = Vec::new();
                for value in values {
                    match value {
                        Expr::Identifier(name) => names.push(name.clone()),
                        Expr::NamedArg { name, value } => {
                            input = self.mutate_assignments(
                                input,
                                &[Assignment {
                                    column: name.clone(),
                                    expr: value.as_ref().clone(),
                                }],
                            )?;
                            names.push(name.clone());
                        }
                        _ => {
                            return Err(invalid(
                                "group_by entries must be columns or named expressions",
                            ))
                        }
                    }
                }
                if add {
                    let mut all = original;
                    all.extend(names);
                    names = all;
                }
                for name in names {
                    let id = visible_column(&input.columns, &name)?.id;
                    if !input.groups.contains(&id) {
                        input.groups.push(id);
                    }
                }
                Ok(input)
            }
            "ungroup" => {
                if args.is_empty() {
                    input.groups.clear();
                } else {
                    let names = self.selected_names(&input, &args.iter().collect::<Vec<_>>())?;
                    input.groups.retain(|id| {
                        input
                            .columns
                            .iter()
                            .any(|c| c.id == *id && !names.contains(&c.schema.name))
                    });
                }
                Ok(input)
            }
            "arrange" | "window_order" => {
                let (values, opts) = arguments(args, &[".by_group"])?;
                let by_group = opts
                    .get(".by_group")
                    .map(|e| boolean(e))
                    .transpose()?
                    .unwrap_or(false);
                self.arrange_expressions(input, &values, by_group)
            }
            "window_frame" => {
                if args.len() != 2 {
                    return Err(invalid("window_frame needs two bounds"));
                }
                input.frame = Some(frame_bounds(&args[0], &args[1])?);
                Ok(input)
            }
            "head" => {
                let (values, opts) = arguments(args, &["n"])?;
                if values.len() > 1 {
                    return Err(invalid("head has extra arguments"));
                }
                let n = opts
                    .get("n")
                    .copied()
                    .or_else(|| values.first().copied())
                    .map(integer)
                    .transpose()?
                    .unwrap_or(6);
                let mut q = self.ordered_query(&input)?;
                q.limit = Some(n);
                let schema = input.columns.iter().map(|c| c.schema.clone()).collect();
                self.query_relation(input, q, schema)
            }
            "relocate" => {
                let (values, opts) = arguments(args, &[".before", ".after"])?;
                if opts.contains_key(".before") && opts.contains_key(".after") {
                    return Err(invalid("use only one placement option"));
                }
                let selected = self.selected_names(&input, &values)?;
                let mut kept = Self::identities(&input.columns);
                let mut moved = Vec::new();
                for name in &selected {
                    let index = kept
                        .iter()
                        .position(|p| &p.column.schema.name == name)
                        .ok_or_else(|| invalid("missing relocate column"))?;
                    moved.push(kept.remove(index));
                }
                let position = if let Some((target, after)) = opts
                    .get(".before")
                    .map(|e| (*e, false))
                    .or_else(|| opts.get(".after").map(|e| (*e, true)))
                {
                    let targets = self.selected_names(&input, &[target])?;
                    let target = targets
                        .first()
                        .ok_or_else(|| invalid("empty placement selector"))?;
                    kept.iter()
                        .position(|p| &p.column.schema.name == target)
                        .ok_or_else(|| invalid("placement target is itself relocated"))?
                        + usize::from(after)
                } else {
                    0
                };
                kept.splice(position..position, moved);
                self.project(input, kept)
            }
            "rename" => {
                let renames = args
                    .iter()
                    .map(|arg| match arg {
                        Expr::NamedArg { name, value } => match value.as_ref() {
                            Expr::Identifier(old) => Ok(crate::parser::RenameSpec {
                                new_name: name.clone(),
                                old_name: old.clone(),
                            }),
                            _ => Err(invalid("rename source must be a column")),
                        },
                        _ => Err(invalid("rename needs named entries")),
                    })
                    .collect::<GenerationResult<_>>()?;
                self.apply(input, &DplyrOperation::Rename { renames, location })
            }
            "rename_with" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(invalid(
                        "rename_with needs a function and optional selector",
                    ));
                }
                let Expr::Identifier(function) = &args[0] else {
                    return Err(self.unsupported("rename_with requires a known name transform"));
                };
                let selected = if args.len() == 2 {
                    self.selected_names(&input, &[&args[1]])?
                } else {
                    input
                        .columns
                        .iter()
                        .filter(|c| !c.hidden)
                        .map(|c| c.schema.name.clone())
                        .collect()
                };
                let renames = selected
                    .into_iter()
                    .map(|old_name| {
                        let new_name = match function.as_str() {
                            "toupper" => old_name.to_uppercase(),
                            "tolower" => old_name.to_lowercase(),
                            _ => return Err(invalid("rename_with supports toupper and tolower")),
                        };
                        Ok(crate::parser::RenameSpec { new_name, old_name })
                    })
                    .collect::<GenerationResult<_>>()?;
                self.apply(input, &DplyrOperation::Rename { renames, location })
            }
            "slice" | "slice_head" | "slice_tail" | "tail" => self.positional(input, name, args),
            "slice_sample" => self.sample_options(input, args),
            "slice_min" | "slice_max" => self.ordered_slice(input, name, args),
            "cross_join" => {
                let (values, opts) = arguments(args, &["suffix"])?;
                if values.len() != 1 {
                    return Err(invalid("cross_join needs one right relation"));
                }
                let right = self.relation_operand(values[0])?;
                let mut options = crate::parser::JoinOptions::default();
                if let Some(suffix) = opts.get("suffix") {
                    let Expr::Function { name, args } = suffix else {
                        return Err(invalid("suffix needs c(left,right)"));
                    };
                    if name != "c" || args.len() != 2 {
                        return Err(invalid("suffix needs c(left,right)"));
                    }
                    options.suffix = (string(&args[0])?.into(), string(&args[1])?.into());
                }
                self.join_relations(
                    input,
                    right,
                    &JoinType::Inner,
                    &crate::parser::JoinSpec {
                        table: String::new(),
                        by: Vec::new(),
                        on_expr: None,
                        options,
                        right_operations: Vec::new(),
                    },
                    true,
                )
            }
            "count" | "tally" | "add_count" | "add_tally" => self.count_options(input, name, args),
            "distinct" => self.distinct_options(input, args),
            "union" | "union_all" | "intersect" | "setdiff" | "bind_queries" => {
                let (values, opts) = arguments(args, &["all"])?;
                let all = opts
                    .get("all")
                    .map(|e| boolean(e))
                    .transpose()?
                    .unwrap_or(name == "union_all" || name == "bind_queries");
                let operation = match name {
                    "union" | "union_all" | "bind_queries" => {
                        if all {
                            SetOperation::UnionAll
                        } else {
                            SetOperation::Union
                        }
                    }
                    "intersect" => {
                        if all {
                            return Err(self.unsupported("INTERSECT ALL"));
                        }
                        SetOperation::Intersect
                    }
                    _ => {
                        if all {
                            return Err(self.unsupported("EXCEPT ALL"));
                        }
                        SetOperation::SetDiff
                    }
                };
                for (index, value) in values.into_iter().enumerate() {
                    if name == "bind_queries"
                        && index == 0
                        && matches!(value,Expr::Identifier(source) if self.schemas.first().is_some_and(|s|&s.source==source))
                    {
                        continue;
                    }
                    let right = self.relation_operand(value)?;
                    input = self.set_relations(input, right, &operation)?;
                }
                Ok(input)
            }
            "rows_append" | "rows_insert" | "rows_update" | "rows_patch" | "rows_upsert"
            | "rows_delete" => self.rows(input, name, args),
            "dbplyr_uncount" => self.uncount(input, args),
            "pivot_longer" | "pivot_wider" | "fill" | "expand" | "complete" | "replace_na" => {
                let columns = input
                    .columns
                    .iter()
                    .filter(|c| !c.hidden || matches!(name, "fill" | "replace_na"))
                    .map(|c| c.schema.clone())
                    .collect::<Vec<_>>();
                let mut fill_args = Vec::new();
                if name == "fill" {
                    for arg in args {
                        if matches!(arg, Expr::NamedArg { .. }) {
                            fill_args.push(arg.clone());
                        } else {
                            fill_args.extend(
                                self.selected_names(&input, &[arg])?
                                    .into_iter()
                                    .map(Expr::Identifier),
                            );
                        }
                    }
                }
                let args = if name == "fill" {
                    fill_args.as_slice()
                } else {
                    args
                };
                let groups = names_for_ids(&input.columns, &input.groups)?;
                let q = self.ordered_query(&input)?;
                let order = q.order_by.clone();
                let (q, schema) = tidyr::lower(q, name, args, &columns, &groups, &order)?;
                self.query_relation(input, q, schema)
            }
            _ => Err(self.unsupported(&format!("{name}()"))),
        }
    }
    fn uncount(&mut self, input: Relation, args: &[Expr]) -> GenerationResult<Relation> {
        let (values, opts) = arguments(args, &["weights", ".remove", ".id"])?;
        if values.len() > 1 {
            return Err(invalid("uncount accepts one weights expression"));
        }
        let weights = opts
            .get("weights")
            .copied()
            .or_else(|| values.first().copied())
            .ok_or_else(|| invalid("uncount requires weights"))?;
        self.validate_weights(&input, weights, true)?;
        let remove = opts
            .get(".remove")
            .map(|e| boolean(e))
            .transpose()?
            .unwrap_or(true);
        let id = opts.get(".id").map(|e| string(e)).transpose()?;
        let instance = id
            .map(str::to_owned)
            .unwrap_or_else(|| self.hidden_name(&input.columns, "instance"));
        if input.columns.iter().any(|c| c.schema.name == instance) {
            return Err(invalid("uncount id would overwrite an existing column"));
        }
        let columns = input
            .columns
            .iter()
            .map(|c| c.schema.name.clone())
            .collect::<Vec<_>>();
        let schema = input
            .columns
            .iter()
            .filter(|c| {
                !(remove && matches!(weights,Expr::Identifier(name) if name==&c.schema.name))
            })
            .map(|c| c.schema.clone())
            .collect::<Vec<_>>();
        let mut projection = schema
            .iter()
            .map(|c| SelectItem {
                alias: c.name.clone(),
                expression: SelectExpression::Scalar {
                    expr: Expr::Identifier(c.name.clone()),
                    partition_by: Vec::new(),
                },
            })
            .collect::<Vec<_>>();
        let mut schema = schema;
        if id.is_some() {
            schema.push(SchemaColumn::new(&instance));
            projection.push(SelectItem {
                alias: instance.clone(),
                expression: SelectExpression::Scalar {
                    expr: Expr::Identifier(instance.clone()),
                    partition_by: Vec::new(),
                },
            });
        }
        let q = query(
            SqlSource::Repeat {
                input: Box::new(self.ordered_query(&input)?),
                columns,
                weights: weights.clone(),
                instance_column: instance,
            },
            projection,
        );
        self.query_relation(input, q, schema)
    }
    fn validate_weights(
        &mut self,
        input: &Relation,
        weight: &Expr,
        integer: bool,
    ) -> GenerationResult<()> {
        self.validate_expression(weight)?;
        if join::volatile_relation(input) || join::volatile_ast(weight) {
            return Err(invalid(
                "validated weights require stable input expressions",
            ));
        }
        if self.has_window(weight) {
            return Err(invalid("weights cannot contain window functions"));
        }
        BoundExpr::bind(weight, &input.columns)?;
        if let Ok(n) = signed_number(weight) {
            if n < 0.0 || (integer && n.fract() != 0.0) {
                return Err(invalid(
                    "weights must be nonnegative, and uncount weights must be integers",
                ));
            }
            return Ok(());
        }
        if self.checks.is_none() {
            return Err(invalid(
                "data-dependent weights require plan_with_schemas() and execute() for validation",
            ));
        }
        let missing = Expr::Function {
            name: "is.na".into(),
            args: vec![weight.clone()],
        };
        let negative = Expr::Binary {
            left: Box::new(weight.clone()),
            operator: BinaryOp::LessThan,
            right: Box::new(Expr::Literal(LiteralValue::Number(0.0))),
        };
        let too_large = Expr::Binary {
            left: Box::new(weight.clone()),
            operator: BinaryOp::GreaterThan,
            right: Box::new(Expr::Literal(LiteralValue::Number(f64::MAX))),
        };
        let mut invalid_weight = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(missing),
                operator: BinaryOp::Or,
                right: Box::new(negative),
            }),
            operator: BinaryOp::Or,
            right: Box::new(too_large),
        };
        if integer {
            invalid_weight = Expr::Binary {
                left: Box::new(invalid_weight),
                operator: BinaryOp::Or,
                right: Box::new(Expr::Binary {
                    left: Box::new(weight.clone()),
                    operator: BinaryOp::NotEqual,
                    right: Box::new(Expr::Function {
                        name: "floor".into(),
                        args: vec![weight.clone()],
                    }),
                }),
            };
        }
        let mut check = query(
            SqlSource::Subquery(Box::new(lower(input, &mut 1)?), "__weight_input".into()),
            vec![SelectItem {
                alias: "invalid_weight".into(),
                expression: SelectExpression::Scalar {
                    expr: Expr::Literal(LiteralValue::Number(1.0)),
                    partition_by: Vec::new(),
                },
            }],
        );
        check.filter = Some(invalid_weight);
        check.limit = Some(1);
        let sql = check.render(self.generator)?;
        if let Some(checks) = &mut self.checks {
            checks.push(crate::execution::ValidationQuery::new(sql,"invalid weights: values must be nonmissing and nonnegative; uncount also requires integers"));
        }
        Ok(())
    }
    fn positional(
        &mut self,
        mut input: Relation,
        name: &str,
        args: &[Expr],
    ) -> GenerationResult<Relation> {
        let (values, opts) = arguments(args, &["n", "prop", "by", ".by"])?;
        if opts.contains_key("by") && opts.contains_key(".by") {
            return Err(invalid("use one by option"));
        }
        let temporary =
            self.temporary_groups(&mut input, opts.get("by").or_else(|| opts.get(".by")))?;
        let positions = if name == "slice" {
            if let [value] = values.as_slice() {
                if let Some((start, end)) = position_range(value, 1)? {
                    slice::Positions::Range { start, end }
                } else {
                    let mut positions = Vec::new();
                    position_values(value, 1, &mut positions)?;
                    slice::Positions::Indices(positions)
                }
            } else {
                let mut positions = Vec::new();
                for value in values {
                    position_values(value, 1, &mut positions)?;
                }
                slice::Positions::Indices(positions)
            }
        } else {
            if opts.contains_key("n") && opts.contains_key("prop") {
                return Err(invalid("slice takes n or prop, not both"));
            }
            if values.len() > 1 {
                return Err(invalid("too many positional slice arguments"));
            }
            let proportional = opts.contains_key("prop");
            let amount = opts
                .get("n")
                .copied()
                .or_else(|| opts.get("prop").copied())
                .or_else(|| values.first().copied())
                .map(signed_number)
                .transpose()?
                .unwrap_or(if name == "tail" { 6.0 } else { 1.0 });
            if name == "slice_head" {
                slice::Positions::Head {
                    amount,
                    proportional,
                }
            } else {
                slice::Positions::Tail {
                    amount,
                    proportional,
                }
            }
        };
        let request = self.hidden_name(&input.columns, "request_order");
        let columns = input
            .columns
            .iter()
            .map(|c| c.schema.name.clone())
            .collect();
        let groups = names_for_ids(&input.columns, &input.groups)?;
        let (q, requested_order) = slice::lower_positional(
            self.ordered_query(&input)?,
            positions,
            groups,
            columns,
            &request,
        )?;
        let mut schema = input
            .columns
            .iter()
            .map(|c| c.schema.clone())
            .collect::<Vec<_>>();
        if requested_order {
            schema.push(SchemaColumn::new(&request));
        }
        let mut result = self.query_relation(input, q, schema)?;
        if requested_order {
            let column = result
                .columns
                .iter_mut()
                .find(|c| c.schema.name == request)
                .ok_or_else(|| invalid("missing slice request order"))?;
            column.hidden = true;
            result.order = vec![(column.id, OrderDirection::Asc)];
        }
        if temporary {
            result.groups.clear();
        }
        Ok(result)
    }
    fn sample_options(&mut self, mut input: Relation, args: &[Expr]) -> GenerationResult<Relation> {
        let (values, opts) = arguments(args, &["n", "prop", "weight_by", "replace", "by", ".by"])?;
        if !values.is_empty() {
            return Err(invalid("slice_sample arguments must be named"));
        }
        if opts.contains_key("n") && opts.contains_key("prop") {
            return Err(invalid("slice_sample takes n or prop, not both"));
        }
        if opts.contains_key("by") && opts.contains_key(".by") {
            return Err(invalid("use one by option"));
        }
        let temporary =
            self.temporary_groups(&mut input, opts.get("by").or_else(|| opts.get(".by")))?;
        let replace = opts
            .get("replace")
            .map(|e| boolean(e))
            .transpose()?
            .unwrap_or(false);
        let n = opts.get("n").map(|e| integer(e)).transpose()?;
        let prop = opts.get("prop").map(|e| signed_number(e)).transpose()?;
        if prop.is_some_and(|p| p < 0.0) {
            return Err(invalid("sample proportion must be nonnegative"));
        }
        let weight = opts.get("weight_by").copied();
        if let Some(weight) = weight {
            self.validate_weights(&input, weight, false)?;
        }
        if let Some(weight) = weight {
            if n.unwrap_or(1) > 0 && prop != Some(0.0) {
                if signed_number(weight).is_ok_and(|n| n == 0.0) {
                    return Err(invalid("sampling requires a positive weight"));
                }
                if signed_number(weight).is_err() {
                    let groups = names_for_ids(&input.columns, &input.groups)?;
                    let total = self.hidden_name(&input.columns, "weight_total");
                    let mut sums = query(
                        SqlSource::Subquery(
                            Box::new(lower(&input, &mut 1)?),
                            "__weight_domain".into(),
                        ),
                        vec![SelectItem {
                            alias: total.clone(),
                            expression: SelectExpression::AggregateExpression(Expr::Function {
                                name: "sum".into(),
                                args: vec![weight.clone()],
                            }),
                        }],
                    );
                    sums.group_by = groups;
                    let mut check = query(
                        SqlSource::Subquery(Box::new(sums), "__weight_totals".into()),
                        vec![SelectItem {
                            alias: "invalid".into(),
                            expression: SelectExpression::Scalar {
                                expr: Expr::Literal(LiteralValue::Number(1.0)),
                                partition_by: Vec::new(),
                            },
                        }],
                    );
                    check.filter = Some(Expr::Binary {
                        left: Box::new(Expr::Identifier(total)),
                        operator: BinaryOp::LessThanOrEqual,
                        right: Box::new(Expr::Literal(LiteralValue::Number(0.0))),
                    });
                    check.limit = Some(1);
                    let sql = check.render(self.generator)?;
                    if let Some(checks) = &mut self.checks {
                        checks.push(crate::execution::ValidationQuery::new(
                            sql,
                            "sampling requires a positive total weight in each nonempty group",
                        ));
                    }
                }
            }
        }
        let spec = SliceSpec {
            kind: crate::parser::SliceKind::Sample,
            order_by: None,
            n: if n.is_none() && prop.is_none() {
                Some(1)
            } else {
                n
            },
            prop,
            with_ties: false,
            na_rm: false,
            by: Vec::new(),
        };
        let columns = input
            .columns
            .iter()
            .map(|c| c.schema.name.clone())
            .collect();
        let groups = names_for_ids(&input.columns, &input.groups)?;
        let q = slice::lower_sample(
            self.ordered_query(&input)?,
            &spec,
            weight,
            replace,
            groups,
            columns,
            self.generator.dialect().dialect_name(),
        )?;
        let schema = input.columns.iter().map(|c| c.schema.clone()).collect();
        let mut result = self.query_relation(input, q, schema)?;
        result.order.clear();
        if temporary {
            result.groups.clear();
        }
        Ok(result)
    }
    fn ordered_slice(
        &mut self,
        input: Relation,
        name: &str,
        args: &[Expr],
    ) -> GenerationResult<Relation> {
        let (values, opts) = arguments(
            args,
            &["order_by", "n", "prop", "with_ties", "na_rm", "by", ".by"],
        )?;
        if values.len() > 1 {
            return Err(invalid("slice order accepts one expression"));
        }
        let order = opts
            .get("order_by")
            .copied()
            .or_else(|| values.first().copied())
            .ok_or_else(|| invalid("slice requires order_by"))?;
        let spec = SliceSpec {
            kind: if name == "slice_min" {
                crate::parser::SliceKind::Min
            } else {
                crate::parser::SliceKind::Max
            },
            order_by: Some(order.clone()),
            n: opts.get("n").map(|e| integer(e)).transpose()?,
            prop: opts.get("prop").map(|e| signed_number(e)).transpose()?,
            with_ties: opts
                .get("with_ties")
                .map(|e| boolean(e))
                .transpose()?
                .unwrap_or(true),
            na_rm: opts
                .get("na_rm")
                .map(|e| boolean(e))
                .transpose()?
                .unwrap_or(true),
            by: opts
                .get("by")
                .or_else(|| opts.get(".by"))
                .map(|e| selectors(&[*e]))
                .unwrap_or_default(),
        };
        self.slice(input, &spec)
    }
    fn arrange_expressions(
        &mut self,
        mut input: Relation,
        values: &[&Expr],
        by_group: bool,
    ) -> GenerationResult<Relation> {
        let mut order = if by_group {
            input
                .groups
                .iter()
                .map(|id| (*id, OrderDirection::Asc))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        for value in values {
            let (expr, direction) = match value {
                Expr::Function { name, args }
                    if matches!(name.as_str(), "desc" | "asc") && args.len() == 1 =>
                {
                    (
                        &args[0],
                        if name == "desc" {
                            OrderDirection::Desc
                        } else {
                            OrderDirection::Asc
                        },
                    )
                }
                e => (*e, OrderDirection::Asc),
            };
            let id = if let Expr::Identifier(name) = expr {
                visible_column(&input.columns, name)?.id
            } else {
                self.validate_expression(expr)?;
                let name = self.hidden_name(&input.columns, "sort");
                let mut c = self.new_column(&name);
                let id = c.id;
                c.hidden = true;
                let expression = BoundExpr::bind(expr, &input.columns)?;
                let mut items = Self::identities(&input.columns);
                items.push(Projection {
                    column: c,
                    expression,
                });
                input = self.project(input, items)?;
                id
            };
            order.push((id, direction));
        }
        input.order = order;
        Ok(input)
    }
    fn count_options(
        &mut self,
        mut input: Relation,
        name: &str,
        args: &[Expr],
    ) -> GenerationResult<Relation> {
        let (values, opts) = arguments(args, &["wt", "sort", "name", ".drop"])?;
        if let Some(drop) = opts.get(".drop") {
            if !boolean(drop)? {
                return Err(self.unsupported("count(.drop=FALSE)"));
            }
        }
        if name.ends_with("tally") && !values.is_empty() {
            return Err(invalid("tally accepts no grouping columns"));
        }
        let original = names_for_ids(&input.columns, &input.groups)?;
        for column in self.selected_names(&input, &values)? {
            let id = visible_column(&input.columns, &column)?.id;
            if !input.groups.contains(&id) {
                input.groups.push(id);
            }
        }
        let expression = match opts.get("wt") {
            Some(Expr::Literal(LiteralValue::Null)) | None => Expr::Function {
                name: "n".into(),
                args: Vec::new(),
            },
            Some(weight) => Expr::Function {
                name: "sum".into(),
                args: vec![(*weight).clone()],
            },
        };
        let mut output = opts
            .get("name")
            .map(|e| string(e).map(str::to_owned))
            .transpose()?
            .unwrap_or_else(|| "n".into());
        if !opts.contains_key("name") {
            while input
                .columns
                .iter()
                .any(|c| input.groups.contains(&c.id) && c.schema.name == output)
            {
                output.push('n');
            }
        }
        if name.starts_with("add_") {
            input = self.mutate_assignments(
                input,
                &[Assignment {
                    column: output.clone(),
                    expr: expression,
                }],
            )?;
        } else {
            input = self.summarise(input, vec![(output.clone(), expression)])?;
        }
        input.groups = original
            .iter()
            .map(|name| visible_column(&input.columns, name).map(|c| c.id))
            .collect::<GenerationResult<_>>()?;
        if opts
            .get("sort")
            .map(|e| boolean(e))
            .transpose()?
            .unwrap_or(false)
        {
            input.order = vec![(
                visible_column(&input.columns, &output)?.id,
                OrderDirection::Desc,
            )];
        }
        Ok(input)
    }
    fn distinct_options(
        &mut self,
        mut input: Relation,
        args: &[Expr],
    ) -> GenerationResult<Relation> {
        let (values, opts) = arguments(args, &[".keep_all"])?;
        let keep = opts
            .get(".keep_all")
            .map(|e| boolean(e))
            .transpose()?
            .unwrap_or(false);
        let mut names = names_for_ids(&input.columns, &input.groups)?;
        for value in values {
            let name = match value {
                Expr::Identifier(name) => name.clone(),
                Expr::NamedArg { name, value } => {
                    input = self.mutate_assignments(
                        input,
                        &[Assignment {
                            column: name.clone(),
                            expr: value.as_ref().clone(),
                        }],
                    )?;
                    name.clone()
                }
                _ => return Err(invalid("distinct needs columns or named expressions")),
            };
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if keep && !names.is_empty() {
            let schema = input
                .columns
                .iter()
                .map(|c| c.schema.clone())
                .collect::<Vec<_>>();
            let rank = self.hidden_name(&input.columns, "distinct");
            let base = self.ordered_query(&input)?;
            let mut projection = identity_select(&input.columns);
            projection.push(SelectItem {
                alias: rank.clone(),
                expression: SelectExpression::WindowRank {
                    function: "ROW_NUMBER()".into(),
                    partition_by: names,
                    order_by: input
                        .order
                        .iter()
                        .map(|(id, d)| {
                            Ok(sql::SqlOrderTerm::Value {
                                expr: Expr::Identifier(
                                    column_by_id(&input.columns, *id)?.schema.name.clone(),
                                ),
                                descending: matches!(d, OrderDirection::Desc),
                            })
                        })
                        .collect::<GenerationResult<_>>()?,
                },
            });
            let ranked = query(
                SqlSource::Subquery(Box::new(base), "__distinct_input".into()),
                projection,
            );
            let mut result = query(
                SqlSource::Subquery(Box::new(ranked), "__distinct_ranked".into()),
                identity_select(&input.columns),
            );
            result.filter = Some(Expr::Binary {
                left: Box::new(Expr::Identifier(rank)),
                operator: BinaryOp::Equal,
                right: Box::new(Expr::Literal(LiteralValue::Number(1.0))),
            });
            self.query_relation(input, result, schema)
        } else {
            input.order.clear();
            if input.columns.iter().any(|c| c.hidden) {
                let visible = input
                    .columns
                    .iter()
                    .filter(|c| !c.hidden)
                    .cloned()
                    .collect::<Vec<_>>();
                input = self.project(input, Self::identities(&visible))?;
            }
            self.apply(
                input,
                &DplyrOperation::Distinct {
                    columns: names,
                    location: SourceLocation::unknown(),
                },
            )
        }
    }
    pub(super) fn set_relations(
        &mut self,
        mut left: Relation,
        mut right: Relation,
        operation: &SetOperation,
    ) -> GenerationResult<Relation> {
        left.order.clear();
        right.order.clear();
        let mut schema = left
            .columns
            .iter()
            .filter(|c| !c.hidden)
            .map(|c| c.schema.clone())
            .collect::<Vec<_>>();
        for column in right.columns.iter().filter(|c| !c.hidden) {
            if !schema.iter().any(|c| c.name == column.schema.name) {
                schema.push(column.schema.clone());
            }
        }
        for branch in [&mut left, &mut right] {
            let mut items = Vec::new();
            for metadata in &schema {
                if let Some(c) = branch
                    .columns
                    .iter()
                    .find(|c| !c.hidden && c.schema.name == metadata.name)
                {
                    items.push(Projection {
                        column: c.clone(),
                        expression: BoundExpr::Column(c.id),
                    });
                } else {
                    let mut c = self.new_column(&metadata.name);
                    c.schema = metadata.clone();
                    c.schema.nullable = Some(true);
                    items.push(Projection {
                        column: c,
                        expression: BoundExpr::Literal(LiteralValue::Null),
                    });
                }
            }
            let empty = Relation {
                node: RelNode::Scan(String::new()),
                columns: Vec::new(),
                groups: Vec::new(),
                order: Vec::new(),
                frame: None,
            };
            let old = std::mem::replace(branch, empty);
            *branch = self.project(old, items)?;
        }
        for c in &mut left.columns {
            let other = visible_column(&right.columns, &c.schema.name)?;
            if c.schema.data_type != other.schema.data_type {
                c.schema.data_type = None;
            }
            if c.schema.nullable != Some(false) || other.schema.nullable != Some(false) {
                c.schema.nullable = Some(true);
            }
        }
        let columns = left.columns.clone();
        let groups = left.groups.clone();
        self.stage()?;
        Ok(Relation {
            node: RelNode::Set {
                left: Box::new(left),
                right: Box::new(right),
                operation: operation.clone(),
            },
            columns,
            groups,
            order: Vec::new(),
            frame: None,
        })
    }
}

fn frame_bounds(from: &Expr, to: &Expr) -> GenerationResult<(i64, i64)> {
    fn bound(expr: &Expr) -> GenerationResult<i64> {
        match expr {
            Expr::Literal(LiteralValue::Number(n))
                if n.is_finite()
                    && n.fract() == 0.0
                    && *n >= i64::MIN as f64
                    && *n < i64::MAX as f64 =>
            {
                Ok(*n as i64)
            }
            Expr::Unary {
                operator: UnaryOp::Minus,
                expr,
            } if matches!(expr.as_ref(),Expr::Identifier(name) if name=="Inf") => Ok(i64::MIN),
            Expr::Unary {
                operator: UnaryOp::Minus,
                expr,
            } => bound(expr)?
                .checked_neg()
                .ok_or_else(|| invalid("frame bound overflow")),
            Expr::Identifier(name) if name == "Inf" => Ok(i64::MAX),
            Expr::Unary {
                operator: UnaryOp::Plus,
                expr,
            } => bound(expr),
            _ => Err(invalid("frame bounds must be integers or Inf")),
        }
    }
    let bounds = (bound(from)?, bound(to)?);
    if bounds.0 > bounds.1 {
        return Err(invalid("frame start must not exceed end"));
    }
    Ok(bounds)
}
fn parse_frame(expr: &Expr) -> GenerationResult<(i64, i64)> {
    match expr {
        Expr::Function { name, args } if name == "c" && args.len() == 2 => {
            frame_bounds(&args[0], &args[1])
        }
        _ => Err(invalid(".frame needs c(from,to)")),
    }
}

fn signed_number(expr: &Expr) -> GenerationResult<f64> {
    match expr {
        Expr::Literal(LiteralValue::Number(n)) if n.is_finite() => Ok(*n),
        Expr::Unary {
            operator: UnaryOp::Minus,
            expr,
        } => signed_number(expr).map(|n| -n),
        Expr::Unary {
            operator: UnaryOp::Plus,
            expr,
        } => signed_number(expr),
        _ => Err(invalid("expected a finite number")),
    }
}
fn position_values(expr: &Expr, sign: i64, out: &mut Vec<i64>) -> GenerationResult<()> {
    match expr {
        Expr::Function { name, args } if name == "c" => {
            for arg in args {
                position_values(arg, sign, out)?;
            }
        }
        Expr::Unary {
            operator: UnaryOp::Minus,
            expr,
        } => position_values(expr, -sign, out)?,
        Expr::Unary {
            operator: UnaryOp::Plus,
            expr,
        } => position_values(expr, sign, out)?,
        Expr::Function { name, args } if name == "__select_range" && args.len() == 2 => {
            let start = signed_number(&args[0])?;
            let end = signed_number(&args[1])?;
            if start.fract() != 0.0 || end.fract() != 0.0 || (end - start).abs() > 100000.0 {
                return Err(invalid(
                    "slice range must contain at most 100001 integer positions",
                ));
            }
            let step = if end >= start { 1 } else { -1 };
            let mut value = start as i64;
            loop {
                out.push(
                    value
                        .checked_mul(sign)
                        .ok_or_else(|| invalid("slice position overflow"))?,
                );
                if value == end as i64 {
                    break;
                }
                value += step;
            }
        }
        _ => {
            let value = signed_number(expr)?;
            if value.fract() != 0.0 || value.abs() > 9007199254740991.0 {
                return Err(invalid("slice positions must be exact integers"));
            }
            out.push(
                (value as i64)
                    .checked_mul(sign)
                    .ok_or_else(|| invalid("slice position overflow"))?,
            );
        }
    }
    Ok(())
}

fn position_range(expr: &Expr, sign: i64) -> GenerationResult<Option<(i64, i64)>> {
    match expr {
        Expr::Unary {
            operator: UnaryOp::Minus,
            expr,
        } => position_range(expr, -sign),
        Expr::Unary {
            operator: UnaryOp::Plus,
            expr,
        } => position_range(expr, sign),
        Expr::Function { name, args } if name == "__select_range" && args.len() == 2 => {
            let start = signed_number(&args[0])?;
            let end = signed_number(&args[1])?;
            if [start, end]
                .iter()
                .any(|v| v.fract() != 0.0 || v.abs() > 9007199254740991.0)
            {
                return Err(invalid("slice range endpoints must be exact integers"));
            }
            Ok(Some((start as i64 * sign, end as i64 * sign)))
        }
        _ => Ok(None),
    }
}
