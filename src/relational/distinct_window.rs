//! COUNT(DISTINCT ...) OVER is not portable; a dense rank followed by MAX is.
use super::sql::SqlOrderTerm;
use super::*;

impl Planner<'_> {
    pub(super) fn distinct_windows(
        &mut self,
        mut input: Relation,
        expr: &Expr,
    ) -> GenerationResult<(Relation, Expr, Vec<ColumnId>)> {
        let mut hidden = Vec::new();
        let result = match expr {
            Expr::Function { name, args } if name.eq_ignore_ascii_case("n_distinct") => {
                if input.frame.is_some() {
                    return Err(self.unsupported("n_distinct() with a moving window frame"));
                }
                let mut values = Vec::new();
                let mut remove_na = false;
                let mut seen_na = false;
                for arg in args {
                    match arg {
                        Expr::NamedArg { name, value } if name == "na.rm" => {
                            if seen_na {
                                return Err(invalid("duplicate n_distinct na.rm option"));
                            }
                            seen_na = true;
                            let Expr::Literal(LiteralValue::Boolean(value)) = value.as_ref() else {
                                return Err(invalid("n_distinct na.rm must be TRUE or FALSE"));
                            };
                            remove_na = *value;
                        }
                        Expr::NamedArg { .. } => return Err(invalid("unknown n_distinct option")),
                        value => {
                            if self.has_window(value) {
                                return Err(invalid("n_distinct cannot contain another window"));
                            }
                            self.validate_expression(value)?;
                            values.push(
                                BoundExpr::bind(value, &input.columns)?.to_expr(&input.columns)?,
                            );
                        }
                    }
                }
                if values.is_empty() {
                    return Err(invalid("n_distinct requires at least one expression"));
                }
                let missing = values
                    .iter()
                    .map(|value| Expr::Function {
                        name: "is.na".into(),
                        args: vec![value.clone()],
                    })
                    .reduce(|left, right| Expr::Binary {
                        left: Box::new(left),
                        operator: BinaryOp::Or,
                        right: Box::new(right),
                    })
                    .ok_or_else(|| invalid("missing distinct values"))?;
                let mut order = values
                    .iter()
                    .map(|expr| SqlOrderTerm::Value {
                        expr: expr.clone(),
                        descending: false,
                    })
                    .collect::<Vec<_>>();
                if remove_na {
                    order.insert(
                        0,
                        SqlOrderTerm::Value {
                            expr: missing.clone(),
                            descending: false,
                        },
                    );
                }
                let alias = self.hidden_name(&input.columns, "unique_rank");
                let column = self.new_column(&alias);
                let id = column.id;
                let mut projection = identity_select(&input.columns);
                projection.push(SelectItem {
                    alias: alias.clone(),
                    expression: SelectExpression::WindowRank {
                        function: "DENSE_RANK()".into(),
                        partition_by: names_for_ids(&input.columns, &input.groups)?,
                        order_by: order,
                    },
                });
                let query = SqlQuery {
                    source: SqlSource::Subquery(
                        Box::new(lower(&input, &mut 1)?),
                        "__unique_input".into(),
                    ),
                    projection,
                    filter: None,
                    group_by: Vec::new(),
                    order_by: Vec::new(),
                    distinct: false,
                    limit: None,
                };
                let mut columns = input.columns.clone();
                columns.push(column);
                input = Relation {
                    node: RelNode::Query(Box::new(query)),
                    columns,
                    groups: input.groups,
                    order: input.order,
                    frame: input.frame,
                };
                self.stage()?;
                hidden.push(id);
                let rank = if remove_na {
                    Expr::CaseWhen {
                        branches: vec![(missing, Expr::Literal(LiteralValue::Null))],
                        default: Some(Box::new(Expr::Identifier(alias))),
                    }
                } else {
                    Expr::Identifier(alias)
                };
                Expr::Function {
                    name: "coalesce".into(),
                    args: vec![
                        Expr::Function {
                            name: "max".into(),
                            args: vec![rank],
                        },
                        Expr::Literal(LiteralValue::Number(0.0)),
                    ],
                }
            }
            Expr::Function { name, args } => {
                let mut converted = Vec::new();
                for arg in args {
                    let (next, value, ids) = self.distinct_windows(input, arg)?;
                    input = next;
                    converted.push(value);
                    hidden.extend(ids);
                }
                Expr::Function {
                    name: name.clone(),
                    args: converted,
                }
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let (next, left, ids) = self.distinct_windows(input, left)?;
                hidden.extend(ids);
                let (next, right, ids) = self.distinct_windows(next, right)?;
                hidden.extend(ids);
                input = next;
                Expr::Binary {
                    left: Box::new(left),
                    operator: operator.clone(),
                    right: Box::new(right),
                }
            }
            Expr::Unary { operator, expr } => {
                let (next, value, ids) = self.distinct_windows(input, expr)?;
                input = next;
                hidden.extend(ids);
                Expr::Unary {
                    operator: operator.clone(),
                    expr: Box::new(value),
                }
            }
            Expr::NamedArg { name, value } => {
                let (next, value, ids) = self.distinct_windows(input, value)?;
                input = next;
                hidden.extend(ids);
                Expr::NamedArg {
                    name: name.clone(),
                    value: Box::new(value),
                }
            }
            Expr::In { expr, values } => {
                let (next, value, ids) = self.distinct_windows(input, expr)?;
                input = next;
                hidden.extend(ids);
                Expr::In {
                    expr: Box::new(value),
                    values: values.clone(),
                }
            }
            Expr::CaseWhen { branches, default } => {
                let mut converted = Vec::new();
                for (a, b) in branches {
                    let (next, a, ids) = self.distinct_windows(input, a)?;
                    hidden.extend(ids);
                    let (next, b, ids) = self.distinct_windows(next, b)?;
                    input = next;
                    hidden.extend(ids);
                    converted.push((a, b));
                }
                let default = if let Some(value) = default {
                    let (next, value, ids) = self.distinct_windows(input, value)?;
                    input = next;
                    hidden.extend(ids);
                    Some(Box::new(value))
                } else {
                    None
                };
                Expr::CaseWhen {
                    branches: converted,
                    default,
                }
            }
            value => value.clone(),
        };
        Ok((input, result, hidden))
    }
}
