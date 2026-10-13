//! Portable summary statistics use an extra window stage instead of SQLite UDFs.
use super::sql::SqlOrderTerm;
use super::*;
fn f(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Function {
        name: name.into(),
        args,
    }
}
fn n(value: f64) -> Expr {
    Expr::Literal(LiteralValue::Number(value))
}
fn col(name: &str) -> Expr {
    Expr::Identifier(name.into())
}
fn b(left: Expr, operator: BinaryOp, right: Expr) -> Expr {
    Expr::Binary {
        left: Box::new(left),
        operator,
        right: Box::new(right),
    }
}
impl Planner<'_> {
    pub(super) fn summary_statistics(
        &mut self,
        mut input: Relation,
        expr: &Expr,
    ) -> GenerationResult<(Relation, Expr)> {
        let output = match expr {
            Expr::Function { name, args }
                if name == "median"
                    || (self.generator.dialect().dialect_name() == "sqlite"
                        && matches!(
                            name.as_str(),
                            "sd" | "var"
                                | "stddev"
                                | "stddev_samp"
                                | "stddev_pop"
                                | "variance"
                                | "var_samp"
                                | "var_pop"
                        )) =>
            {
                let mut value = None;
                let mut seen = false;
                for arg in args {
                    match arg {
                        Expr::NamedArg { name, value } if name == "na.rm" => {
                            if seen
                                || !matches!(
                                    value.as_ref(),
                                    Expr::Literal(LiteralValue::Boolean(_))
                                )
                            {
                                return Err(invalid("invalid statistic na.rm option"));
                            }
                            seen = true;
                        }
                        Expr::NamedArg { name, value: arg } if name == "x" => {
                            if value.replace(arg.as_ref().clone()).is_some() {
                                return Err(invalid("statistic needs one value"));
                            }
                        }
                        Expr::NamedArg { .. } => return Err(invalid("unknown statistic option")),
                        arg => {
                            if value.replace(arg.clone()).is_some() {
                                return Err(invalid("statistic needs one value"));
                            }
                        }
                    }
                }
                let value = value.ok_or_else(|| invalid("statistic needs one value"))?;
                if self.has_window(&value) {
                    return Err(invalid(
                        "statistic cannot contain another aggregate or window",
                    ));
                }
                BoundExpr::bind(&value, &input.columns)?;
                self.validate_expression(&value)?;
                let groups = names_for_ids(&input.columns, &input.groups)?;
                let mut projection = identity_select(&input.columns);
                let mut schema = input
                    .columns
                    .iter()
                    .map(|c| c.schema.clone())
                    .collect::<Vec<_>>();
                let carrier = self.hidden_name(&input.columns, "statistic");
                let result = if name == "median" {
                    let count = self.hidden_name(&input.columns, "stat_count");
                    projection.push(SelectItem {
                        alias: carrier.clone(),
                        expression: SelectExpression::WindowRank {
                            function: "ROW_NUMBER()".into(),
                            partition_by: groups.clone(),
                            order_by: vec![SqlOrderTerm::Value {
                                expr: value.clone(),
                                descending: false,
                            }],
                        },
                    });
                    let present = Expr::CaseWhen {
                        branches: vec![(f("is.na", vec![value.clone()]), n(0.0))],
                        default: Some(Box::new(n(1.0))),
                    };
                    projection.push(SelectItem {
                        alias: count.clone(),
                        expression: SelectExpression::WindowScalar {
                            expr: f("sum", vec![present]),
                            partition_by: groups,
                            order_by: Vec::new(),
                            frame: None,
                        },
                    });
                    schema.push(SchemaColumn::new(&carrier));
                    schema.push(SchemaColumn::new(&count));
                    let low = f(
                        "floor",
                        vec![b(
                            b(col(&count), BinaryOp::Plus, n(1.0)),
                            BinaryOp::Divide,
                            n(2.0),
                        )],
                    );
                    let high = f(
                        "floor",
                        vec![b(
                            b(col(&count), BinaryOp::Plus, n(2.0)),
                            BinaryOp::Divide,
                            n(2.0),
                        )],
                    );
                    let middle = b(
                        b(col(&carrier), BinaryOp::GreaterThanOrEqual, low),
                        BinaryOp::And,
                        b(col(&carrier), BinaryOp::LessThanOrEqual, high),
                    );
                    f(
                        "mean",
                        vec![Expr::CaseWhen {
                            branches: vec![(middle, value)],
                            default: None,
                        }],
                    )
                } else {
                    projection.push(SelectItem {
                        alias: carrier.clone(),
                        expression: SelectExpression::WindowScalar {
                            expr: f("mean", vec![value.clone()]),
                            partition_by: groups,
                            order_by: Vec::new(),
                            frame: None,
                        },
                    });
                    schema.push(SchemaColumn::new(&carrier));
                    let delta = b(value.clone(), BinaryOp::Minus, col(&carrier));
                    let numerator = f("sum", vec![b(delta.clone(), BinaryOp::Multiply, delta)]);
                    let count = f(
                        "sum",
                        vec![Expr::CaseWhen {
                            branches: vec![(f("is.na", vec![value]), n(0.0))],
                            default: Some(Box::new(n(1.0))),
                        }],
                    );
                    let population = matches!(name.as_str(), "stddev_pop" | "var_pop");
                    let denominator = f(
                        "nullif",
                        vec![
                            b(
                                count.clone(),
                                BinaryOp::Minus,
                                n(if population { 0.0 } else { 1.0 }),
                            ),
                            n(0.0),
                        ],
                    );
                    let variance = Expr::CaseWhen {
                        branches: vec![(
                            b(
                                count,
                                BinaryOp::LessThanOrEqual,
                                n(if population { 0.0 } else { 1.0 }),
                            ),
                            Expr::Literal(LiteralValue::Null),
                        )],
                        default: Some(Box::new(b(numerator, BinaryOp::Divide, denominator))),
                    };
                    if matches!(
                        name.as_str(),
                        "sd" | "stddev" | "stddev_samp" | "stddev_pop"
                    ) {
                        f("sqrt", vec![variance])
                    } else {
                        variance
                    }
                };
                let query = SqlQuery {
                    source: SqlSource::Subquery(
                        Box::new(lower(&input, &mut 1)?),
                        "__statistic_input".into(),
                    ),
                    projection,
                    filter: None,
                    group_by: Vec::new(),
                    order_by: Vec::new(),
                    distinct: false,
                    limit: None,
                };
                input = self.query_relation(input, query, schema)?;
                result
            }
            Expr::Function { name, args }
                if name == "n_distinct"
                    && args
                        .iter()
                        .filter(|e| !matches!(e, Expr::NamedArg { .. }))
                        .count()
                        > 1 =>
            {
                let (next, value, _) = self.distinct_windows(input, expr)?;
                input = next;
                let carrier = self.hidden_name(&input.columns, "distinct_count");
                let c = self.new_column(&carrier);
                let mut items = Self::identities(&input.columns);
                items.push(Projection {
                    column: c,
                    expression: BoundExpr::bind(&value, &input.columns)?,
                });
                input = self.project(input, items)?;
                f("coalesce", vec![f("max", vec![col(&carrier)]), n(0.0)])
            }
            Expr::Function { name, args } => {
                let mut values = Vec::new();
                for arg in args {
                    let (next, arg) = self.summary_statistics(input, arg)?;
                    input = next;
                    values.push(arg);
                }
                f(name, values)
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let (next, left) = self.summary_statistics(input, left)?;
                let (next, right) = self.summary_statistics(next, right)?;
                input = next;
                b(left, operator.clone(), right)
            }
            Expr::Unary { operator, expr } => {
                let (next, expr) = self.summary_statistics(input, expr)?;
                input = next;
                Expr::Unary {
                    operator: operator.clone(),
                    expr: Box::new(expr),
                }
            }
            Expr::NamedArg { name, value } => {
                let (next, value) = self.summary_statistics(input, value)?;
                input = next;
                Expr::NamedArg {
                    name: name.clone(),
                    value: Box::new(value),
                }
            }
            Expr::CaseWhen { branches, default } => {
                let mut values = Vec::new();
                for (a, b) in branches {
                    let (next, a) = self.summary_statistics(input, a)?;
                    let (next, b) = self.summary_statistics(next, b)?;
                    input = next;
                    values.push((a, b));
                }
                let default = if let Some(value) = default {
                    let (next, value) = self.summary_statistics(input, value)?;
                    input = next;
                    Some(Box::new(value))
                } else {
                    None
                };
                Expr::CaseWhen {
                    branches: values,
                    default,
                }
            }
            Expr::In { expr, values } => {
                let (next, expr) = self.summary_statistics(input, expr)?;
                input = next;
                Expr::In {
                    expr: Box::new(expr),
                    values: values.clone(),
                }
            }
            other => other.clone(),
        };
        Ok((input, output))
    }
}

pub(super) fn needs_stage(expr: &Expr, dialect: &str) -> bool {
    match expr {
        Expr::Function { name, args } => {
            name == "median"
                || (dialect == "sqlite"
                    && matches!(
                        name.as_str(),
                        "sd" | "var"
                            | "stddev"
                            | "stddev_samp"
                            | "stddev_pop"
                            | "variance"
                            | "var_samp"
                            | "var_pop"
                    ))
                || args.iter().any(|e| needs_stage(e, dialect))
        }
        Expr::Binary { left, right, .. } => {
            needs_stage(left, dialect) || needs_stage(right, dialect)
        }
        Expr::Unary { expr, .. } | Expr::In { expr, .. } => needs_stage(expr, dialect),
        Expr::NamedArg { value, .. } => needs_stage(value, dialect),
        Expr::CaseWhen { branches, default } => {
            branches
                .iter()
                .any(|(a, b)| needs_stage(a, dialect) || needs_stage(b, dialect))
                || default.as_deref().is_some_and(|e| needs_stage(e, dialect))
        }
        _ => false,
    }
}
