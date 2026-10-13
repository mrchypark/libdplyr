//! Schema-aware lowering for slice_min(), slice_max(), and slice_sample().
//!
//! Every variant lowers to window functions over one derived table: a ranking
//! window picks the rows, a count window turns prop into an integer threshold,
//! and an outer stage drops the ranking columns so nothing leaks downstream.

use super::selection;
use super::sql::{SelectExpression, SelectItem, SqlOrderTerm, SqlQuery, SqlSource};
use super::{invalid, visible_column, BoundExpr, Planner, RelNode, Relation, SchemaColumn};
use crate::error::GenerationResult;
use crate::parser::{BinaryOp, Expr, LiteralValue, SliceKind, SliceSpec};

/// Aliases for the generated stages. Each slice introduces its own scope, so a
/// fixed name cannot collide with a nested slice.
const INPUT_ALIAS: &str = "__libdplyr_slice_input";
const RANK_ALIAS: &str = "__libdplyr_slice_rank";
const COUNT_ALIAS: &str = "__libdplyr_slice_count";
const OUTPUT_ALIAS: &str = "__libdplyr_slice_output";

fn verb(kind: &SliceKind) -> &'static str {
    match kind {
        SliceKind::Min => "slice_min()",
        SliceKind::Max => "slice_max()",
        SliceKind::Sample => "slice_sample()",
    }
}

/// TRUE for rows where expr is not NULL. The expression vocabulary has no
/// IS NOT NULL, and membership in an NA-only vector tests "expr IS NULL".
fn is_not_null(expr: &Expr) -> Expr {
    Expr::CaseWhen {
        branches: vec![(
            Expr::In {
                expr: Box::new(expr.clone()),
                values: vec![LiteralValue::Null],
            },
            Expr::Literal(LiteralValue::Boolean(false)),
        )],
        default: Some(Box::new(Expr::Literal(LiteralValue::Boolean(true)))),
    }
}

fn identity(columns: &[String]) -> Vec<SelectItem> {
    columns
        .iter()
        .map(|name| SelectItem {
            alias: name.clone(),
            expression: SelectExpression::Scalar {
                expr: Expr::Identifier(name.clone()),
                partition_by: Vec::new(),
            },
        })
        .collect()
}

/// Keeps a generated alias from shadowing a real column of the same name.
fn unique(name: &str, columns: &[String]) -> String {
    let mut candidate = name.to_string();
    while columns.contains(&candidate) {
        candidate.push('x');
    }
    candidate
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
    /// Records a slice as its own stage. Grouping carries through unchanged; a
    /// by argument is a temporary grouping used only for this window.
    pub(super) fn slice(
        &mut self,
        input: Relation,
        spec: &SliceSpec,
    ) -> GenerationResult<Relation> {
        let mut by = Vec::new();
        if spec.by.is_empty() {
            by.clone_from(&input.groups);
        } else {
            if !input.groups.is_empty() {
                return Err(invalid(
                    "slice by is not supported on an already grouped relation; use group_by() first",
                ));
            }
            let visible = input
                .columns
                .iter()
                .filter(|column| !column.hidden)
                .map(|column| column.schema.clone())
                .collect::<Vec<SchemaColumn>>();
            for column in selection::resolve(&spec.by, &visible)? {
                let Expr::Identifier(name) = column.expr else {
                    return Err(invalid("slice by requires column names"));
                };
                let id = visible_column(&input.columns, &name)?.id;
                if !by.contains(&id) {
                    by.push(id);
                }
            }
        }

        if let Some(order_by) = &spec.order_by {
            self.validate_expression(order_by)?;
            // An aggregate or a nested window needs its own GROUP BY or frame,
            // which a per-slice window cannot provide.
            if self.has_window(order_by) {
                return Err(
                    self.unsupported("aggregate or window function in a slice order expression")
                );
            }
            // Binds now, so an unknown column is a slice error rather than a
            // rendering failure deep in the lower pass.
            BoundExpr::bind(order_by, &input.columns)?;
        }
        if let Some(prop) = spec.prop {
            if !prop.is_finite() || prop < 0.0 {
                return Err(invalid(format!(
                    "{} prop must be a finite nonnegative number",
                    verb(&spec.kind)
                )));
            }
        }
        match spec.kind {
            SliceKind::Min | SliceKind::Max => {
                if spec.n.is_some() && spec.prop.is_some() {
                    return Err(invalid(format!(
                        "{} takes n or prop, not both",
                        verb(&spec.kind)
                    )));
                }
                if spec.order_by.is_none() {
                    return Err(invalid(format!(
                        "{} requires an order expression",
                        verb(&spec.kind)
                    )));
                }
            }
            SliceKind::Sample => {
                if spec.order_by.is_some() {
                    return Err(self.unsupported("slice_sample(order_by =)"));
                }
                if spec.na_rm {
                    return Err(invalid("slice_sample() has no na_rm argument"));
                }
                if spec.n.is_some() && spec.prop.is_some() {
                    return Err(invalid("slice_sample() takes n or prop, not both"));
                }
                if spec.n.is_none() && spec.prop.is_none() {
                    return Err(invalid("slice_sample() requires n or prop"));
                }
            }
        }

        // One ranking stage plus one keep stage.
        self.stage()?;
        self.stage()?;
        let columns = input.columns.clone();
        let groups = input.groups.clone();
        let order = input.order.clone();
        let frame = input.frame;
        Ok(Relation {
            node: RelNode::Slice {
                input: Box::new(input),
                spec: spec.clone(),
                by,
            },
            columns,
            groups,
            order,
            frame,
        })
    }
}

/// Ranks rows in one stage, then keeps the selected ones in a second stage.
/// columns carries every input column, including hidden sort carriers, so a
/// later arrange() still resolves; the rank columns never leave this scope.
pub(super) fn lower(
    input: SqlQuery,
    spec: &SliceSpec,
    groups: Vec<String>,
    columns: Vec<String>,
) -> GenerationResult<SqlQuery> {
    let rank_name = unique(RANK_ALIAS, &columns);
    let count_name = unique(COUNT_ALIAS, &columns);

    let order_values = match &spec.order_by {
        Some(Expr::Function { name, args }) if matches!(name.as_str(), "tibble" | "c") => args
            .iter()
            .map(|e| match e {
                Expr::NamedArg { value, .. } => value.as_ref().clone(),
                e => e.clone(),
            })
            .collect::<Vec<_>>(),
        Some(expr) => vec![expr.clone()],
        None => Vec::new(),
    };
    if spec.order_by.is_some() && order_values.is_empty() {
        return Err(invalid("slice order needs at least one column"));
    }
    let order_by = if spec.order_by.is_none() {
        vec![SqlOrderTerm::Random]
    } else {
        order_values
            .iter()
            .map(|expr| SqlOrderTerm::Value {
                expr: expr.clone(),
                descending: matches!(spec.kind, SliceKind::Max),
            })
            .collect()
    };
    // WITH TIES compares the cumulative share directly, so the integer count
    // threshold is unnecessary.
    let cume_dist = spec.prop.is_some() && spec.with_ties;
    // sample() has no with_ties argument, so the parser's shared default of true
    // must not apply: RANK() over a random key returns a random number of rows.
    let ties = spec.with_ties && matches!(spec.kind, SliceKind::Min | SliceKind::Max);
    let ranking = if cume_dist {
        "CUME_DIST()"
    } else if ties {
        "RANK()"
    } else {
        "ROW_NUMBER()"
    };

    let bound = match (spec.prop, cume_dist) {
        (Some(prop), true) => Expr::Literal(LiteralValue::Number(prop)),
        // Integer ranks compared with a nonnegative fraction already round down.
        (Some(prop), false) => Expr::Binary {
            left: Box::new(Expr::Literal(LiteralValue::Number(prop))),
            operator: BinaryOp::Multiply,
            right: Box::new(Expr::Identifier(count_name.clone())),
        },
        // n defaults to one row, matching head().
        (None, _) => Expr::Literal(LiteralValue::Number(spec.n.unwrap_or(1) as f64)),
    };
    let needs_count = spec.prop.is_some() && !cume_dist;

    let mut inner = query(
        SqlSource::Subquery(Box::new(input), INPUT_ALIAS.to_string()),
        identity(&columns),
    );
    inner.projection.push(SelectItem {
        alias: rank_name.clone(),
        expression: SelectExpression::WindowRank {
            function: ranking.to_string(),
            partition_by: groups.clone(),
            order_by,
        },
    });
    if needs_count {
        inner.projection.push(SelectItem {
            alias: count_name.clone(),
            expression: SelectExpression::WindowRank {
                function: "COUNT(*)".to_string(),
                partition_by: groups,
                order_by: Vec::new(),
            },
        });
    }
    // na_rm drops NULL keys before ranking, so they never take a slot.
    if spec.na_rm {
        inner.filter = order_values
            .iter()
            .map(is_not_null)
            .reduce(|left, right| Expr::Binary {
                left: Box::new(left),
                operator: BinaryOp::And,
                right: Box::new(right),
            });
    }

    let mut outer = query(
        SqlSource::Subquery(Box::new(inner), OUTPUT_ALIAS.to_string()),
        identity(&columns),
    );
    outer.filter = Some(Expr::Binary {
        left: Box::new(Expr::Identifier(rank_name)),
        operator: BinaryOp::LessThanOrEqual,
        right: Box::new(bound),
    });
    Ok(outer)
}

/// Positional operations require the explicit order from the relation metadata.
pub(super) enum Positions {
    Indices(Vec<i64>),
    Range { start: i64, end: i64 },
    Head { amount: f64, proportional: bool },
    Tail { amount: f64, proportional: bool },
}

fn number(value: f64) -> Expr {
    Expr::Literal(LiteralValue::Number(value))
}
fn identifier(name: &str) -> Expr {
    Expr::Identifier(name.into())
}
fn binary(left: Expr, operator: BinaryOp, right: Expr) -> Expr {
    Expr::Binary {
        left: Box::new(left),
        operator,
        right: Box::new(right),
    }
}
fn function(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Function {
        name: name.into(),
        args,
    }
}

pub(super) fn lower_positional(
    input: SqlQuery,
    positions: Positions,
    groups: Vec<String>,
    columns: Vec<String>,
    request_name: &str,
) -> GenerationResult<(SqlQuery, bool)> {
    if input.order_by.is_empty() {
        return Err(invalid(
            "positional slice requires arrange() or window_order()",
        ));
    }
    let order = input
        .order_by
        .iter()
        .map(|o| SqlOrderTerm::Value {
            expr: identifier(&o.column),
            descending: matches!(o.direction, crate::parser::OrderDirection::Desc),
        })
        .collect();
    let row_name = unique(RANK_ALIAS, &columns);
    let count_name = unique(COUNT_ALIAS, &columns);
    let mut ranked = query(
        SqlSource::Subquery(Box::new(input), INPUT_ALIAS.into()),
        identity(&columns),
    );
    ranked.projection.push(SelectItem {
        alias: row_name.clone(),
        expression: SelectExpression::WindowRank {
            function: "ROW_NUMBER()".into(),
            partition_by: groups.clone(),
            order_by: order,
        },
    });
    ranked.projection.push(SelectItem {
        alias: count_name.clone(),
        expression: SelectExpression::WindowRank {
            function: "COUNT(*)".into(),
            partition_by: groups,
            order_by: Vec::new(),
        },
    });
    if let Positions::Range { start, end } = positions {
        if start.min(end) < 0 && start.max(end) > 0 {
            return Err(invalid("slice cannot mix positive and negative positions"));
        }
        let negative = start.min(end) < 0;
        let mut result = query(
            SqlSource::Subquery(Box::new(ranked), OUTPUT_ALIAS.into()),
            identity(&columns),
        );
        let low = if negative {
            start.max(end).abs()
        } else {
            start.min(end)
        };
        let high = if negative {
            start.min(end).abs()
        } else {
            start.max(end)
        };
        let inside = binary(
            binary(
                identifier(&row_name),
                BinaryOp::GreaterThanOrEqual,
                number(low as f64),
            ),
            BinaryOp::And,
            binary(
                identifier(&row_name),
                BinaryOp::LessThanOrEqual,
                number(high as f64),
            ),
        );
        result.filter = Some(if negative {
            Expr::Unary {
                operator: crate::parser::UnaryOp::Not,
                expr: Box::new(inside),
            }
        } else {
            inside
        });
        if !negative {
            result.projection.push(SelectItem {
                alias: request_name.into(),
                expression: SelectExpression::Scalar {
                    expr: if start > end {
                        Expr::Unary {
                            operator: crate::parser::UnaryOp::Minus,
                            expr: Box::new(identifier(&row_name)),
                        }
                    } else {
                        identifier(&row_name)
                    },
                    partition_by: Vec::new(),
                },
            });
        }
        return Ok((result, !negative));
    }
    if let Positions::Indices(values) = &positions {
        let positive = values.iter().any(|n| *n > 0);
        let negative = values.iter().any(|n| *n < 0);
        if positive && negative {
            return Err(invalid("slice cannot mix positive and negative positions"));
        }
        if positive {
            let domain = query(
                SqlSource::Values {
                    columns: vec!["__position".into(), request_name.into()],
                    rows: values
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| **n > 0)
                        .map(|(index, n)| vec![number(*n as f64), number(index as f64)])
                        .collect(),
                },
                vec![
                    SelectItem {
                        alias: "__position".into(),
                        expression: SelectExpression::Scalar {
                            expr: identifier("__position"),
                            partition_by: Vec::new(),
                        },
                    },
                    SelectItem {
                        alias: request_name.into(),
                        expression: SelectExpression::Scalar {
                            expr: identifier(request_name),
                            partition_by: Vec::new(),
                        },
                    },
                ],
            );
            let mut projection = columns
                .iter()
                .map(|column| SelectItem {
                    alias: column.clone(),
                    expression: SelectExpression::Qualified {
                        relation: "__libdplyr_left".into(),
                        column: column.clone(),
                    },
                })
                .collect::<Vec<_>>();
            projection.push(SelectItem {
                alias: request_name.into(),
                expression: SelectExpression::Qualified {
                    relation: "__libdplyr_right".into(),
                    column: request_name.into(),
                },
            });
            return Ok((
                query(
                    SqlSource::Join {
                        left: Box::new(ranked),
                        right: Box::new(domain),
                        join_type: crate::parser::JoinType::Inner,
                        keys: vec![(row_name, "__position".into())],
                        predicates: Vec::new(),
                        closest: None,
                        na_matches: false,
                    },
                    projection,
                ),
                true,
            ));
        }
    }
    let mut result = query(
        SqlSource::Subquery(Box::new(ranked), OUTPUT_ALIAS.into()),
        identity(&columns),
    );
    result.filter = Some(match positions {
        Positions::Range { .. } => return Err(invalid("range was not lowered")),
        Positions::Indices(values) => {
            if values.iter().all(|n| *n == 0) {
                Expr::Literal(LiteralValue::Boolean(false))
            } else {
                Expr::Unary {
                    operator: crate::parser::UnaryOp::Not,
                    expr: Box::new(Expr::In {
                        expr: Box::new(identifier(&row_name)),
                        values: values
                            .into_iter()
                            .filter(|n| *n < 0)
                            .map(|n| {
                                n.checked_neg()
                                    .map(|n| LiteralValue::Number(n as f64))
                                    .ok_or_else(|| invalid("slice position overflow"))
                            })
                            .collect::<GenerationResult<_>>()?,
                    }),
                }
            }
        }
        Positions::Head {
            amount,
            proportional,
        }
        | Positions::Tail {
            amount,
            proportional,
        } => {
            let tail = matches!(positions, Positions::Tail { .. });
            let count = identifier(&count_name);
            let magnitude = if proportional {
                function(
                    "floor",
                    vec![binary(
                        count.clone(),
                        BinaryOp::Multiply,
                        number(amount.abs()),
                    )],
                )
            } else {
                number(amount.abs().floor())
            };
            let bound = if (tail && amount >= 0.0) || (!tail && amount < 0.0) {
                binary(count, BinaryOp::Minus, magnitude)
            } else {
                magnitude
            };
            binary(
                identifier(&row_name),
                if tail {
                    BinaryOp::GreaterThan
                } else {
                    BinaryOp::LessThanOrEqual
                },
                bound,
            )
        }
    });
    Ok((result, false))
}

// A window prevents a draw from being re-evaluated for every join candidate.
fn open_uniform(dialect: &str) -> &'static str {
    match dialect {
        "sqlite"=>"MIN(MAX(0.5 + (1.0 * RANDOM() / 18446744073709551616.0), 0.00000000000000011102230246251565), 0.9999999999999999)",
        "mysql"=>"LEAST(GREATEST(RAND(), 0.00000000000000011102230246251565), 0.9999999999999999)",
        _=>"LEAST(GREATEST(RANDOM(), 0.00000000000000011102230246251565), 0.9999999999999999)",
    }
}

pub(super) fn lower_sample(
    input: SqlQuery,
    spec: &SliceSpec,
    weight: Option<&Expr>,
    replace: bool,
    groups: Vec<String>,
    columns: Vec<String>,
    dialect: &str,
) -> GenerationResult<SqlQuery> {
    if weight.is_none() && !replace {
        return lower(input, spec, groups, columns);
    }
    let weight_name = unique("__sample_weight", &columns);
    let row_name = unique("__sample_row", &columns);
    let count_name = unique(COUNT_ALIAS, &columns);
    let uniform_name = unique("__sample_uniform", &columns);
    let rank_name = unique(RANK_ALIAS, &columns);
    let mut weighted = query(
        SqlSource::Subquery(Box::new(input), INPUT_ALIAS.into()),
        identity(&columns),
    );
    weighted.projection.push(SelectItem {
        alias: weight_name.clone(),
        expression: SelectExpression::Scalar {
            expr: weight.cloned().unwrap_or_else(|| number(1.0)),
            partition_by: Vec::new(),
        },
    });
    weighted.projection.push(SelectItem {
        alias: row_name.clone(),
        expression: SelectExpression::WindowRank {
            function: "ROW_NUMBER()".into(),
            partition_by: groups.clone(),
            order_by: Vec::new(),
        },
    });
    weighted.projection.push(SelectItem {
        alias: count_name.clone(),
        expression: SelectExpression::WindowRank {
            function: "COUNT(*)".into(),
            partition_by: groups.clone(),
            order_by: Vec::new(),
        },
    });
    let mut working = columns.clone();
    working.extend([weight_name.clone(), row_name.clone(), count_name.clone()]);
    let mut positive = query(
        SqlSource::Subquery(Box::new(weighted), "__sample_positive".into()),
        identity(&working),
    );
    positive.filter = Some(binary(
        identifier(&weight_name),
        BinaryOp::GreaterThan,
        number(0.0),
    ));
    let projection = working
        .iter()
        .map(|name| {
            if name == &weight_name {
                SelectItem {
                    alias: name.clone(),
                    expression: SelectExpression::WindowScalar {
                        expr: binary(
                            identifier(name),
                            BinaryOp::Divide,
                            function("max", vec![identifier(name)]),
                        ),
                        partition_by: groups.clone(),
                        order_by: Vec::new(),
                        frame: None,
                    },
                }
            } else {
                SelectItem {
                    alias: name.clone(),
                    expression: SelectExpression::Scalar {
                        expr: identifier(name),
                        partition_by: Vec::new(),
                    },
                }
            }
        })
        .collect();
    let mut positive = query(
        SqlSource::Subquery(Box::new(positive), "__sample_scaled".into()),
        projection,
    );
    if !replace {
        let mut partition = groups.clone();
        partition.push(row_name);
        positive.projection.push(SelectItem {
            alias: uniform_name.clone(),
            expression: SelectExpression::WindowRank {
                function: format!("MAX({})", open_uniform(dialect)),
                partition_by: partition,
                order_by: Vec::new(),
            },
        });
        let mut ranked = query(
            SqlSource::Subquery(Box::new(positive), "__sample_keys".into()),
            identity(&columns),
        );
        let key = binary(
            Expr::Unary {
                operator: crate::parser::UnaryOp::Minus,
                expr: Box::new(function("log", vec![identifier(&uniform_name)])),
            },
            BinaryOp::Divide,
            identifier(&weight_name),
        );
        ranked.projection.push(SelectItem {
            alias: rank_name.clone(),
            expression: SelectExpression::WindowRank {
                function: "ROW_NUMBER()".into(),
                partition_by: groups,
                order_by: vec![SqlOrderTerm::Value {
                    expr: key,
                    descending: false,
                }],
            },
        });
        ranked.projection.push(SelectItem {
            alias: count_name.clone(),
            expression: SelectExpression::Scalar {
                expr: identifier(&count_name),
                partition_by: Vec::new(),
            },
        });
        let bound = spec
            .prop
            .map(|p| binary(identifier(&count_name), BinaryOp::Multiply, number(p)))
            .unwrap_or_else(|| number(spec.n.unwrap_or(1) as f64));
        let mut result = query(
            SqlSource::Subquery(Box::new(ranked), OUTPUT_ALIAS.into()),
            identity(&columns),
        );
        result.filter = Some(binary(
            identifier(&rank_name),
            BinaryOp::LessThanOrEqual,
            bound,
        ));
        return Ok(result);
    }
    let total_name = unique("__sample_total", &columns);
    let cumulative_name = unique("__sample_cumulative", &columns);
    let lower_name = unique("__sample_lower", &columns);
    let upper_name = unique("__sample_upper", &columns);
    let order = vec![crate::parser::OrderExpr {
        column: row_name,
        direction: crate::parser::OrderDirection::Asc,
    }];
    let mut cumulative = query(
        SqlSource::Subquery(Box::new(positive), "__sample_cdf".into()),
        identity(&working),
    );
    for (alias, frame) in [
        (&total_name, (i64::MIN, i64::MAX)),
        (&cumulative_name, (i64::MIN, 0)),
    ] {
        cumulative.projection.push(SelectItem {
            alias: alias.clone(),
            expression: SelectExpression::WindowScalar {
                expr: function("sum", vec![identifier(&weight_name)]),
                partition_by: groups.clone(),
                order_by: order.clone(),
                frame: Some(frame),
            },
        });
    }
    let mut cdf_projection = identity(&columns);
    cdf_projection.push(SelectItem {
        alias: lower_name.clone(),
        expression: SelectExpression::Scalar {
            expr: binary(
                identifier(&cumulative_name),
                BinaryOp::Minus,
                identifier(&weight_name),
            ),
            partition_by: Vec::new(),
        },
    });
    cdf_projection.push(SelectItem {
        alias: upper_name.clone(),
        expression: SelectExpression::Scalar {
            expr: identifier(&cumulative_name),
            partition_by: Vec::new(),
        },
    });
    let cdf = query(
        SqlSource::Subquery(Box::new(cumulative.clone()), "__sample_intervals".into()),
        cdf_projection,
    );
    let mut draw_columns = groups.clone();
    draw_columns.extend([total_name.clone(), count_name.clone()]);
    let mut domains = query(
        SqlSource::Subquery(Box::new(cumulative), "__sample_domains".into()),
        identity(&draw_columns),
    );
    domains.distinct = true;
    let instance = unique("__sample_instance", &columns);
    let target = unique("__sample_target", &columns);
    let draws = query(
        SqlSource::Repeat {
            input: Box::new(domains),
            columns: draw_columns.clone(),
            weights: spec
                .prop
                .map(|p| binary(identifier(&count_name), BinaryOp::Multiply, number(p)))
                .unwrap_or_else(|| number(spec.n.unwrap_or(1) as f64)),
            instance_column: instance.clone(),
        },
        {
            let mut projection = identity(&draw_columns);
            projection.push(SelectItem {
                alias: instance.clone(),
                expression: SelectExpression::Scalar {
                    expr: identifier(&instance),
                    partition_by: Vec::new(),
                },
            });
            let mut partition = groups.clone();
            partition.push(instance);
            projection.push(SelectItem {
                alias: uniform_name.clone(),
                expression: SelectExpression::WindowRank {
                    function: format!("MAX({})", open_uniform(dialect)),
                    partition_by: partition,
                    order_by: Vec::new(),
                },
            });
            projection
        },
    );
    let mut projection = identity(&groups);
    projection.push(SelectItem {
        alias: target.clone(),
        expression: SelectExpression::Scalar {
            expr: binary(
                identifier(&uniform_name),
                BinaryOp::Multiply,
                identifier(&total_name),
            ),
            partition_by: Vec::new(),
        },
    });
    let draws = query(
        SqlSource::Subquery(Box::new(draws), "__sample_draws".into()),
        projection,
    );
    Ok(query(
        SqlSource::Join {
            left: Box::new(draws),
            right: Box::new(cdf),
            join_type: crate::parser::JoinType::Inner,
            keys: groups
                .into_iter()
                .map(|name| (name.clone(), name))
                .collect(),
            predicates: vec![
                (target.clone(), BinaryOp::GreaterThanOrEqual, lower_name),
                (target, BinaryOp::LessThan, upper_name),
            ],
            closest: None,
            na_matches: true,
        },
        columns
            .iter()
            .map(|column| SelectItem {
                alias: column.clone(),
                expression: SelectExpression::Qualified {
                    relation: "__libdplyr_right".into(),
                    column: column.clone(),
                },
            })
            .collect(),
    ))
}
