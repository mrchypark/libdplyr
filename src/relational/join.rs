//! Schema-aware join lowering: natural keys, `keep`, and custom suffixes.

use super::{
    add_suffixes, column_by_id, invalid, visible_column, Column, JoinProjection, Planner, RelNode,
    Relation,
};
use crate::error::GenerationResult;
use crate::parser::{BinaryOp, Expr, JoinKey, JoinOptions, JoinSpec, JoinType, LiteralValue};

fn visible(columns: &[Column]) -> impl Iterator<Item = &Column> {
    columns.iter().filter(|column| !column.hidden)
}

/// Names shared by both inputs, in left order. A join without `by` or `on`
/// needs one; otherwise it would silently cross-join.
fn natural_keys(left: &[Column], right: &[Column]) -> GenerationResult<Vec<JoinKey>> {
    let keys = visible(left)
        .filter(|column| visible(right).any(|other| other.schema.name == column.schema.name))
        .map(|column| JoinKey {
            left: column.schema.name.clone(),
            right: column.schema.name.clone(),
        })
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return Err(invalid(
            "join without `by` requires a column name shared by both inputs",
        ));
    }
    Ok(keys)
}

/// Renames colliding names in input order. An empty suffix renames nothing:
/// dplyr keeps that side as it is, and any surviving output clash is caught by
/// the caller's output validation.
fn suffixed(
    names: Vec<String>,
    reserved: &[String],
    suffix: &str,
) -> GenerationResult<Vec<String>> {
    if suffix.is_empty() {
        return Ok(names);
    }
    Ok(add_suffixes(names, reserved, suffix))
}

/// Semi/anti joins drop the right side, so `keep` and `suffix` cannot change
/// their output and are rejected there.
fn check_filtering_options(options: &JoinOptions, filtering: bool) -> GenerationResult<()> {
    if filtering && (options.keep || options.suffix.0 != ".x" || options.suffix.1 != ".y") {
        return Err(invalid(
            "semi and anti joins ignore output shape, so `keep` and `suffix` must stay default",
        ));
    }
    Ok(())
}

impl Planner<'_> {
    pub(super) fn join(
        &mut self,
        left: Relation,
        join_type: &JoinType,
        spec: &JoinSpec,
    ) -> GenerationResult<Relation> {
        let mut right = self.scan(&spec.table)?;
        for operation in &spec.right_operations {
            right = self.apply(right, operation)?;
        }
        self.join_relations(left, right, join_type, spec, false)
    }

    pub(super) fn join_relations(
        &mut self,
        left: Relation,
        right: Relation,
        join_type: &JoinType,
        spec: &JoinSpec,
        cross: bool,
    ) -> GenerationResult<Relation> {
        let options = &spec.options;
        let filtering = matches!(join_type, JoinType::Semi | JoinType::Anti);
        check_filtering_options(options, filtering)?;
        let (left_suffix, right_suffix) = (&options.suffix.0, &options.suffix.1);
        let mut by = spec.by.clone();
        let mut comparisons = Vec::new();
        let mut rolling = None;
        if let Some(expr) = &spec.on_expr {
            conditions(expr, &mut by, &mut comparisons, &mut rolling)?;
        }
        if by.is_empty() && comparisons.is_empty() && !cross {
            by = natural_keys(&left.columns, &right.columns)?;
        }
        let inequality = !comparisons.is_empty();
        if inequality && options.keep_explicit && !options.keep && !filtering {
            return Err(invalid("inequality joins require keep = TRUE"));
        }
        let keep = options.keep || (inequality && !options.keep_explicit);
        let predicates = comparisons
            .iter()
            .map(|(a, op, b)| {
                Ok((
                    visible_column(&left.columns, a)?.id,
                    op.clone(),
                    visible_column(&right.columns, b)?.id,
                ))
            })
            .collect::<GenerationResult<Vec<_>>>()?;
        let closest = rolling
            .as_ref()
            .map(|(a, op, b)| {
                Ok((
                    visible_column(&left.columns, a)?.id,
                    op.clone(),
                    visible_column(&right.columns, b)?.id,
                ))
            })
            .transpose()?;
        if options.multiple.as_deref().is_some_and(|v| v != "all") {
            return Err(self.unsupported("multiple = first/last/any needs an explicit tie-breaking order; use closest() with all ties"));
        }

        let mut left_keys = std::collections::HashSet::new();
        let mut right_keys = std::collections::HashSet::new();
        let keys = by
            .iter()
            .map(|key| {
                let left_id = visible_column(&left.columns, &key.left)?.id;
                let right_id = visible_column(&right.columns, &key.right)?.id;
                if !left_keys.insert(left_id.0) || !right_keys.insert(right_id.0) {
                    return Err(invalid("join keys must be unique on each side"));
                }
                Ok((left_id, right_id))
            })
            .collect::<GenerationResult<Vec<_>>>()?;

        self.check_join(
            &left,
            &right,
            join_type,
            &keys,
            &predicates,
            &closest,
            options,
        )?;

        // `keep` retains the right key, so both copies of a same-named key
        // collide like any other pair and both take part in suffixing.
        let right_visible = visible(&right.columns)
            .filter(|column| keep || !right_keys.contains(&column.id.0))
            .collect::<Vec<_>>();
        let left_names = left
            .columns
            .iter()
            .map(|column| column.schema.name.clone())
            .collect::<Vec<_>>();
        let left_suffixed = visible(&left.columns)
            .filter(|column| keep || !left_keys.contains(&column.id.0))
            .collect::<Vec<_>>();
        let key_names = by.iter().map(|key| key.left.clone()).collect::<Vec<_>>();
        // Reserved names are the output names this side cannot take. The right
        // names are reserved on the left, so every shared name is renamed on
        // both sides. Without `keep` the right key is dropped, and its bare
        // left name is reserved so the left key stays unsuffixed.
        let mut left_reserved = Vec::new();
        if !keep {
            left_reserved.extend(key_names.iter().cloned());
        }
        left_reserved.extend(
            left.columns
                .iter()
                .filter(|column| column.hidden)
                .map(|column| column.schema.name.clone()),
        );
        left_reserved.extend(
            right_visible
                .iter()
                .map(|column| column.schema.name.clone()),
        );
        let left_suffixed_names = left_suffixed
            .iter()
            .map(|column| column.schema.name.clone())
            .collect::<Vec<_>>();
        let right_visible_names = right_visible
            .iter()
            .map(|column| column.schema.name.clone())
            .collect::<Vec<_>>();
        let left_output = suffixed(left_suffixed_names, &left_reserved, left_suffix)?;
        let right_output = suffixed(right_visible_names, &left_names, right_suffix)?;

        // Right/full joins coalesce the key columns; `keep` opts out.
        let coalesce_keys = !keep && matches!(join_type, JoinType::Right | JoinType::Full);
        let mut items = Vec::new();
        for source in &left.columns {
            let mut column = source.clone();
            if !filtering {
                if let Some(index) = left_suffixed.iter().position(|item| item.id == source.id) {
                    column.schema.name.clone_from(&left_output[index]);
                }
            }
            let coalesced = if coalesce_keys {
                keys.iter()
                    .find(|(id, _)| *id == source.id)
                    .map(|(_, id)| *id)
            } else {
                None
            };
            if let Some(id) = coalesced {
                let metadata = &column_by_id(&right.columns, id)?.schema;
                if column.schema.data_type != metadata.data_type {
                    column.schema.data_type = None;
                }
                column.schema.nullable = match (column.schema.nullable, metadata.nullable) {
                    (Some(false), Some(false)) => Some(false),
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    _ => None,
                };
            } else if matches!(join_type, JoinType::Right | JoinType::Full) {
                column.schema.nullable = Some(true);
            }
            items.push(JoinProjection {
                column,
                left: Some(source.id),
                right: coalesced,
            });
        }
        if !filtering {
            for (index, source) in right_visible.iter().enumerate() {
                let mut column = (*source).clone();
                column.schema.name.clone_from(&right_output[index]);
                if matches!(join_type, JoinType::Left | JoinType::Full) {
                    column.schema.nullable = Some(true);
                }
                items.push(JoinProjection {
                    column,
                    left: None,
                    right: Some(source.id),
                });
            }
        }
        let columns = items
            .iter()
            .map(|item| item.column.clone())
            .collect::<Vec<_>>();
        self.validate_output(&columns)?;
        self.stage()?;
        Ok(Relation {
            columns,
            groups: left.groups.clone(),
            order: left.order.clone(),
            frame: left.frame,
            node: RelNode::Join {
                left: Box::new(left),
                right: Box::new(right),
                join_type: join_type.clone(),
                keys,
                predicates,
                closest,
                items,
                na_matches: options.na_matches,
            },
        })
    }
}

type Comparison = (String, BinaryOp, String);
fn conditions(
    expr: &Expr,
    keys: &mut Vec<JoinKey>,
    predicates: &mut Vec<Comparison>,
    closest: &mut Option<Comparison>,
) -> GenerationResult<()> {
    match expr {
        Expr::Identifier(name) => keys.push(JoinKey {
            left: name.clone(),
            right: name.clone(),
        }),
        Expr::Binary {
            left,
            operator: BinaryOp::And,
            right,
        } => {
            conditions(left, keys, predicates, closest)?;
            conditions(right, keys, predicates, closest)?;
        }
        Expr::Binary {
            left,
            operator,
            right,
        } => {
            let (Expr::Identifier(a), Expr::Identifier(b)) = (left.as_ref(), right.as_ref()) else {
                return Err(invalid("join comparisons require column identifiers"));
            };
            if matches!(operator, BinaryOp::Equal) {
                keys.push(JoinKey {
                    left: a.clone(),
                    right: b.clone(),
                });
            } else if matches!(
                operator,
                BinaryOp::GreaterThan
                    | BinaryOp::GreaterThanOrEqual
                    | BinaryOp::LessThan
                    | BinaryOp::LessThanOrEqual
            ) {
                predicates.push((a.clone(), operator.clone(), b.clone()));
            } else {
                return Err(invalid(
                    "join predicates require equality or an ordered comparison",
                ));
            }
        }
        Expr::Function { name, args } if name == "closest" && args.len() == 1 => {
            if closest.is_some() {
                return Err(invalid("join_by() accepts one closest() condition"));
            }
            let mut comparison = Vec::new();
            conditions(&args[0], keys, &mut comparison, &mut None)?;
            if comparison.len() != 1 {
                return Err(invalid("closest() requires one inequality"));
            }
            *closest = comparison.first().cloned();
            predicates.extend(comparison);
        }
        Expr::Function { name, args }
            if matches!(name.as_str(), "between" | "within" | "overlaps") =>
        {
            let allow_bounds = name != "within";
            let mut bounds = "[]";
            let mut seen_bounds = false;
            let mut values = Vec::new();
            for arg in args {
                match arg {
                    Expr::NamedArg { name, value } if name == "bounds" && allow_bounds => {
                        if seen_bounds {
                            return Err(invalid("duplicate range join bounds"));
                        }
                        seen_bounds = true;
                        let Expr::Literal(LiteralValue::String(v)) = value.as_ref() else {
                            return Err(invalid("range join bounds must be a constant string"));
                        };
                        bounds = v;
                    }
                    Expr::NamedArg { .. } => return Err(invalid("unknown range join option")),
                    arg => values.push(arg),
                }
            }
            if !matches!(bounds, "[]" | "[)" | "(]" | "()") {
                return Err(invalid("invalid overlap bounds"));
            }
            let names = values
                .iter()
                .map(|arg| {
                    if let Expr::Identifier(n) = arg {
                        Ok(n.clone())
                    } else {
                        Err(invalid("range joins require column identifiers"))
                    }
                })
                .collect::<GenerationResult<Vec<_>>>()?;
            let ge = if bounds.starts_with('[') {
                BinaryOp::GreaterThanOrEqual
            } else {
                BinaryOp::GreaterThan
            };
            let le = if bounds.ends_with(']') {
                BinaryOp::LessThanOrEqual
            } else {
                BinaryOp::LessThan
            };
            match (name.as_str(), names.as_slice()) {
                ("between", [x, lo, hi]) => {
                    predicates.push((x.clone(), ge, lo.clone()));
                    predicates.push((x.clone(), le, hi.clone()));
                }
                ("within", [lo, hi, rlo, rhi]) => {
                    predicates.push((lo.clone(), BinaryOp::GreaterThanOrEqual, rlo.clone()));
                    predicates.push((hi.clone(), BinaryOp::LessThanOrEqual, rhi.clone()));
                }
                ("overlaps", [lo, hi, rlo, rhi]) => {
                    predicates.push((lo.clone(), le, rhi.clone()));
                    predicates.push((hi.clone(), ge, rlo.clone()));
                }
                _ => return Err(invalid("invalid range join arguments")),
            }
        }
        _ => return Err(invalid("unsupported join_by() condition")),
    }
    Ok(())
}

/// Renders only bound comparisons. Identifiers and input queries use the SQL AST renderer.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_match_predicate(
    generator: &crate::sql_generator::SqlGenerator,
    keys: &[(String, String)],
    predicates: &[(String, BinaryOp, String)],
    closest: &Option<(String, BinaryOp, String)>,
    na_matches: bool,
    left_alias: &str,
    right_alias: &str,
    right: &super::sql::SqlQuery,
) -> GenerationResult<String> {
    let dialect = generator.dialect();
    let base = |alias: &str| -> GenerationResult<String> {
        let mut terms = keys
            .iter()
            .map(|(a, b)| {
                let a = dialect.quote_identifier_path(&[left_alias, a]);
                let b = dialect.quote_identifier_path(&[alias, b]);
                if na_matches {
                    format!("({a} = {b} OR ({a} IS NULL AND {b} IS NULL))")
                } else {
                    format!("{a} = {b}")
                }
            })
            .collect::<Vec<_>>();
        for (a, op, b) in predicates {
            let operator = match op {
                BinaryOp::LessThan => "<",
                BinaryOp::LessThanOrEqual => "<=",
                BinaryOp::GreaterThan => ">",
                BinaryOp::GreaterThanOrEqual => ">=",
                _ => return Err(invalid("invalid bound join comparison")),
            };
            terms.push(format!(
                "{} {operator} {}",
                dialect.quote_identifier_path(&[left_alias, a]),
                dialect.quote_identifier_path(&[alias, b])
            ));
        }
        Ok(if terms.is_empty() {
            "1 = 1".into()
        } else {
            terms.join(" AND ")
        })
    };
    let mut predicate = base(right_alias)?;
    if let Some((_, op, column)) = closest {
        let candidate = "__libdplyr_candidate";
        let aggregate = if matches!(op, BinaryOp::GreaterThan | BinaryOp::GreaterThanOrEqual) {
            "MAX"
        } else {
            "MIN"
        };
        predicate.push_str(&format!(
            " AND {} = (SELECT {aggregate}({}) FROM ({}) AS {} WHERE {})",
            dialect.quote_identifier_path(&[right_alias, column]),
            dialect.quote_identifier_path(&[candidate, column]),
            right.render(generator)?,
            dialect.quote_identifier(candidate),
            base(candidate)?
        ));
    }
    Ok(predicate)
}

impl Planner<'_> {
    #[allow(clippy::too_many_arguments)]
    fn check_join(
        &mut self,
        left: &Relation,
        right: &Relation,
        join_type: &JoinType,
        keys: &[(super::ColumnId, super::ColumnId)],
        predicates: &[(super::ColumnId, BinaryOp, super::ColumnId)],
        closest: &Option<(super::ColumnId, BinaryOp, super::ColumnId)>,
        options: &JoinOptions,
    ) -> GenerationResult<()> {
        let relationship = options.relationship.as_deref().unwrap_or("many-to-many");
        let unmatched = options.unmatched.as_deref().unwrap_or("drop");
        if relationship == "many-to-many" && unmatched == "drop" {
            return Ok(());
        }
        if self.checks.is_none() {
            return Err(invalid("relationship/unmatched assertions require plan_with_schemas() and execute() in a stable snapshot"));
        }
        if volatile_relation(left) || volatile_relation(right) {
            return Err(invalid("relationship checks require stable inputs; materialize volatile expressions before compiling the plan"));
        }
        let left_query = super::lower(left, &mut 1)?;
        let right_query = super::lower(right, &mut 1)?;
        let keys = keys
            .iter()
            .map(|(a, b)| {
                Ok((
                    column_by_id(&left.columns, *a)?.schema.name.clone(),
                    column_by_id(&right.columns, *b)?.schema.name.clone(),
                ))
            })
            .collect::<GenerationResult<Vec<_>>>()?;
        let predicates = predicates
            .iter()
            .map(|(a, op, b)| {
                Ok((
                    column_by_id(&left.columns, *a)?.schema.name.clone(),
                    op.clone(),
                    column_by_id(&right.columns, *b)?.schema.name.clone(),
                ))
            })
            .collect::<GenerationResult<Vec<_>>>()?;
        let closest = closest
            .as_ref()
            .map(|(a, op, b)| {
                Ok((
                    column_by_id(&left.columns, *a)?.schema.name.clone(),
                    op.clone(),
                    column_by_id(&right.columns, *b)?.schema.name.clone(),
                ))
            })
            .transpose()?;
        let left_alias = "__libdplyr_left";
        let right_alias = "__libdplyr_right";
        let predicate = render_match_predicate(
            self.generator,
            &keys,
            &predicates,
            &closest,
            options.na_matches,
            left_alias,
            right_alias,
            &right_query,
        )?;
        let left_sql = left_query.render(self.generator)?;
        let right_sql = right_query.render(self.generator)?;
        let quote = |name: &str| self.generator.dialect().quote_identifier(name);
        let mut checks = Vec::new();
        let mut check = |left_side: bool, comparison: &str, message: String| {
            let (outer, outer_alias, inner, inner_alias) = if left_side {
                (&left_sql, left_alias, &right_sql, right_alias)
            } else {
                (&right_sql, right_alias, &left_sql, left_alias)
            };
            checks.push(crate::execution::ValidationQuery::new(format!("SELECT 1 FROM ({outer}) AS {} WHERE (SELECT COUNT(*) FROM ({inner}) AS {} WHERE {predicate}) {comparison} LIMIT 1",quote(outer_alias),quote(inner_alias)),message));
        };
        if matches!(relationship, "many-to-one" | "one-to-one") {
            check(
                true,
                "> 1",
                format!("relationship = {relationship}: one left row matches multiple right rows"),
            );
        }
        if matches!(relationship, "one-to-many" | "one-to-one") {
            check(
                false,
                "> 1",
                format!("relationship = {relationship}: one right row matches multiple left rows"),
            );
        }
        if unmatched == "error" {
            if matches!(join_type, JoinType::Inner | JoinType::Right) {
                check(true, "= 0", "unmatched left rows would be dropped".into());
            }
            if matches!(join_type, JoinType::Inner | JoinType::Left) {
                check(false, "= 0", "unmatched right rows would be dropped".into());
            }
        }
        if let Some(plan_checks) = &mut self.checks {
            plan_checks.extend(checks);
        }
        Ok(())
    }
}

fn volatile_expression(expr: &super::BoundExpr) -> bool {
    use super::BoundExpr;
    match expr {
        BoundExpr::Function(name, args) => {
            !crate::sql_generator::dialect::is_supported_common_function(name)
                || matches!(
                    name.as_str(),
                    "runif" | "rnorm" | "random" | "rand" | "now" | "Sys.time" | "today"
                )
                || args.iter().any(volatile_expression)
        }
        BoundExpr::Unary(_, e) | BoundExpr::In(e, _) | BoundExpr::NamedArg(_, e) => {
            volatile_expression(e)
        }
        BoundExpr::Binary(a, _, b) => volatile_expression(a) || volatile_expression(b),
        BoundExpr::CaseWhen(branches, default) => {
            branches
                .iter()
                .any(|(a, b)| volatile_expression(a) || volatile_expression(b))
                || default.as_deref().is_some_and(volatile_expression)
        }
        _ => false,
    }
}
pub(super) fn volatile_relation(relation: &Relation) -> bool {
    match &relation.node {
        RelNode::Scan(_) => false,
        RelNode::Query(query) => volatile_query(query),
        RelNode::Project { input, items, .. } => {
            volatile_relation(input) || items.iter().any(|i| volatile_expression(&i.expression))
        }
        RelNode::Filter { input, predicate } => {
            volatile_relation(input) || volatile_expression(predicate)
        }
        RelNode::Aggregate {
            input, measures, ..
        } => {
            volatile_relation(input) || measures.iter().any(|i| volatile_expression(&i.expression))
        }
        RelNode::Distinct(input) => volatile_relation(input),
        RelNode::Slice { input, spec, .. } => {
            matches!(spec.kind, crate::parser::SliceKind::Sample) || volatile_relation(input)
        }
        RelNode::Join { left, right, .. } | RelNode::Set { left, right, .. } => {
            volatile_relation(left) || volatile_relation(right)
        }
    }
}

pub(super) fn volatile_ast(expr: &Expr) -> bool {
    match expr {
        Expr::Function { name, args } => {
            !crate::sql_generator::dialect::is_supported_common_function(name)
                || matches!(
                    name.as_str(),
                    "runif" | "rnorm" | "random" | "rand" | "now" | "Sys.time" | "today"
                )
                || args.iter().any(volatile_ast)
        }
        Expr::Unary { expr, .. } | Expr::In { expr, .. } => volatile_ast(expr),
        Expr::NamedArg { value, .. } => volatile_ast(value),
        Expr::Binary { left, right, .. } => volatile_ast(left) || volatile_ast(right),
        Expr::CaseWhen { branches, default } => {
            branches
                .iter()
                .any(|(a, b)| volatile_ast(a) || volatile_ast(b))
                || default.as_deref().is_some_and(volatile_ast)
        }
        _ => false,
    }
}
fn volatile_query(query: &super::sql::SqlQuery) -> bool {
    use super::sql::{SelectExpression, SqlOrderTerm, SqlSource};
    let source = match &query.source {
        SqlSource::Table(_) => false,
        SqlSource::Subquery(input, _) => volatile_query(input),
        SqlSource::Repeat { input, weights, .. } => volatile_query(input) || volatile_ast(weights),
        SqlSource::Join { left, right, .. } | SqlSource::Set { left, right, .. } => {
            volatile_query(left) || volatile_query(right)
        }
        SqlSource::Values { rows, .. } => rows.iter().flatten().any(volatile_ast),
    };
    source
        || query.filter.as_ref().is_some_and(volatile_ast)
        || query.projection.iter().any(|item| match &item.expression {
            SelectExpression::Scalar { expr, .. }
            | SelectExpression::WindowScalar { expr, .. }
            | SelectExpression::AggregateExpression(expr) => volatile_ast(expr),
            SelectExpression::WindowRank {
                function, order_by, ..
            } => {
                function.to_uppercase().contains("RANDOM(")
                    || function.to_uppercase().contains("RAND(")
                    || order_by.iter().any(|order| match order {
                        SqlOrderTerm::Random => true,
                        SqlOrderTerm::Value { expr, .. } => volatile_ast(expr),
                    })
            }
            _ => false,
        })
}
