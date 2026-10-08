//! Schema-aware lowering for tidyr operations.
//!
//! Each operation lowers to structured SQL AST using the existing
//! SqlQuery/SqlSource/SelectItem/SelectExpression types.
//!
//! Every operation wraps the input in a subquery so that the input's
//! projection, limit, and computed columns are preserved.

use std::collections::HashMap;

use super::schema::SchemaColumn;
use super::sql::{SelectExpression, SelectItem, SqlQuery, SqlSource};
use super::{invalid, selection};
use crate::error::{GenerationError, GenerationResult};
use crate::parser::{BinaryOp, ColumnExpr, Expr, JoinType, LiteralValue, OrderExpr, SetOperation};

fn unsupported(operation: impl Into<String>) -> GenerationError {
    GenerationError::UnsupportedOperation {
        operation: operation.into(),
        dialect: "tidyr".to_string(),
    }
}

/// Lowers a tidyr operation to structured SQL AST.
pub(super) fn lower(
    input: SqlQuery,
    name: &str,
    args: &[Expr],
    columns: &[SchemaColumn],
    groups: &[String],
    order: &[OrderExpr],
) -> GenerationResult<(SqlQuery, Vec<SchemaColumn>)> {
    match name {
        "pivot_longer" => pivot_longer(input, args, columns, groups, order),
        "pivot_wider" => pivot_wider(input, args, columns, groups, order),
        "replace_na" => replace_na(input, args, columns, groups, order),
        "fill" => fill(input, args, columns, groups, order),
        "expand" | "complete" => expand_complete(input, name, args, columns, groups, order),
        _ => Err(unsupported(format!("{name}()"))),
    }
}

/// Splits args into positional and named arguments.
fn split_args<'a>(
    args: &'a [Expr],
    named: &[&str],
) -> GenerationResult<(Vec<&'a Expr>, HashMap<&'a str, &'a Expr>)> {
    let mut positional = Vec::new();
    let mut named_map = HashMap::new();
    for arg in args {
        if let Expr::NamedArg { name, value } = arg {
            if named.contains(&name.as_str()) {
                if named_map.insert(name.as_str(), value.as_ref()).is_some() {
                    return Err(invalid(format!("option {name} was given twice")));
                }
                continue;
            }
            if name.starts_with('.') {
                return Err(invalid(format!("unsupported option {name}")));
            }
        }
        positional.push(arg);
    }
    Ok((positional, named_map))
}

/// Converts a selector expression to a ColumnExpr list.
fn selector_list(expr: &Expr) -> Vec<ColumnExpr> {
    match expr {
        Expr::Function { name, args } if name == "c" || name == "list" => args
            .iter()
            .map(|arg| ColumnExpr {
                expr: arg.clone(),
                alias: None,
            })
            .collect(),
        other => vec![ColumnExpr {
            expr: other.clone(),
            alias: None,
        }],
    }
}

/// Resolves a selector expression to a list of column names.
fn resolve_columns(expr: &Expr, columns: &[SchemaColumn]) -> GenerationResult<Vec<String>> {
    let selectors = selector_list(expr);
    let resolved = selection::resolve(&selectors, columns)?;
    resolved
        .into_iter()
        .map(|column| match column.expr {
            Expr::Identifier(name) => Ok(name),
            _ => Err(invalid("tidyr column selectors must name plain columns")),
        })
        .collect()
}

/// Extracts a string value from an expression (Identifier or String literal).
fn string_value(expr: &Expr) -> GenerationResult<String> {
    match expr {
        Expr::Literal(LiteralValue::String(value)) => Ok(value.clone()),
        Expr::Identifier(name) => Ok(name.clone()),
        _ => Err(invalid("expected a constant string or identifier")),
    }
}

/// Builds a simple projection that selects all columns.
fn identity_projection(columns: &[SchemaColumn]) -> Vec<SelectItem> {
    columns
        .iter()
        .map(|column| SelectItem {
            alias: column.name.clone(),
            expression: SelectExpression::Scalar {
                expr: Expr::Identifier(column.name.clone()),
                partition_by: Vec::new(),
            },
        })
        .collect()
}

/// Builds a query with the given source and projection.
fn simple_query(source: SqlSource, projection: Vec<SelectItem>) -> SqlQuery {
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

/// Wraps input in a subquery so its projection and limit are preserved.
fn wrap_input(input: SqlQuery, alias: &str) -> SqlSource {
    SqlSource::Subquery(Box::new(input), alias.to_string())
}

/// Extracts literal domain values from a keys expression.
/// Keys are literal values: strings, numbers, or NULL.
fn extract_literal_values(expr: &Expr) -> GenerationResult<Vec<LiteralValue>> {
    selector_list(expr)
        .into_iter()
        .map(|column| match column.expr {
            Expr::Literal(value) => Ok(value),
            Expr::Function { name, args } if name == "__missing_value" && args.is_empty() => {
                Ok(LiteralValue::Null)
            }
            Expr::Unary {
                operator: crate::parser::UnaryOp::Minus,
                expr,
            } => match *expr {
                Expr::Literal(LiteralValue::Number(n)) => Ok(LiteralValue::Number(-n)),
                _ => Err(invalid("pivot keys must be literals")),
            },
            _ => Err(invalid("pivot keys must be literal values")),
        })
        .collect()
}
fn literal_name(value: &LiteralValue) -> String {
    match value {
        LiteralValue::String(s) => s.clone(),
        LiteralValue::Number(n) => n.to_string(),
        LiteralValue::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        LiteralValue::Null => "NA".into(),
    }
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
fn reference(name: &str) -> Expr {
    Expr::Identifier(name.into())
}
fn metadata(columns: &[SchemaColumn], name: &str) -> GenerationResult<SchemaColumn> {
    columns
        .iter()
        .find(|c| c.name == name)
        .cloned()
        .ok_or_else(|| invalid(format!("unknown tidyr column {name}")))
}

/// Validates and extracts the values_fn option.
fn parse_values_fn(expr: &Expr) -> GenerationResult<String> {
    let name = match expr {
        Expr::Identifier(name) => name.clone(),
        Expr::Literal(LiteralValue::String(s)) => s.clone(),
        _ => return Err(invalid("values_fn must be a function name")),
    };
    match name.as_str() {
        "max" | "sum" | "mean" | "min" | "n" => Ok(name),
        _ => Err(unsupported(format!("pivot_wider values_fn = {name}"))),
    }
}

// === pivot_longer ===

fn pivot_longer(
    input: SqlQuery,
    args: &[Expr],
    columns: &[SchemaColumn],
    _groups: &[String],
    _order: &[OrderExpr],
) -> GenerationResult<(SqlQuery, Vec<SchemaColumn>)> {
    let (positional, named) = split_args(args, &["names_to", "values_to", "cols"])?;

    let selector = if let Some(cols) = named.get("cols") {
        *cols
    } else if let Some(first) = positional.first() {
        *first
    } else {
        return Err(invalid("pivot_longer() requires a column selector"));
    };

    let names_to = named
        .get("names_to")
        .map(|e| string_value(e))
        .transpose()?
        .unwrap_or_else(|| "name".to_string());
    let values_to = named
        .get("values_to")
        .map(|e| string_value(e))
        .transpose()?
        .unwrap_or_else(|| "value".to_string());

    let pivot_cols = resolve_columns(selector, columns)?;
    if pivot_cols.is_empty() {
        return Err(invalid(
            "pivot_longer() requires at least one column to pivot",
        ));
    }

    let id_cols: Vec<String> = columns
        .iter()
        .filter(|c| !pivot_cols.contains(&c.name))
        .map(|c| c.name.clone())
        .collect();

    let input_source = wrap_input(input, "__libdplyr_pl_input");

    let mut branches = Vec::new();
    for col in &pivot_cols {
        let mut projection = Vec::new();
        for id_col in &id_cols {
            projection.push(SelectItem {
                alias: id_col.clone(),
                expression: SelectExpression::Scalar {
                    expr: Expr::Identifier(id_col.clone()),
                    partition_by: Vec::new(),
                },
            });
        }
        projection.push(SelectItem {
            alias: names_to.clone(),
            expression: SelectExpression::Scalar {
                expr: Expr::Literal(LiteralValue::String(col.clone())),
                partition_by: Vec::new(),
            },
        });
        projection.push(SelectItem {
            alias: values_to.clone(),
            expression: SelectExpression::Scalar {
                expr: Expr::Identifier(col.clone()),
                partition_by: Vec::new(),
            },
        });

        branches.push(SqlQuery {
            source: input_source.clone(),
            projection,
            filter: None,
            group_by: Vec::new(),
            order_by: Vec::new(),
            distinct: false,
            limit: None,
        });
    }

    let mut iter = branches.into_iter();
    let first = iter
        .next()
        .ok_or_else(|| invalid("pivot_longer() requires at least one column to pivot"))?;
    let mut combined = first;
    for branch in iter {
        let projection = combined
            .projection
            .iter()
            .map(|item| scalar(reference(&item.alias), &item.alias))
            .collect();
        combined = SqlQuery {
            source: SqlSource::Set {
                left: Box::new(combined),
                right: Box::new(branch),
                operation: SetOperation::UnionAll,
            },
            projection,
            filter: None,
            group_by: Vec::new(),
            order_by: Vec::new(),
            distinct: false,
            limit: None,
        };
    }

    let mut output_cols = id_cols
        .iter()
        .map(|n| SchemaColumn::new(n.clone()))
        .collect::<Vec<_>>();
    output_cols.push(SchemaColumn::new(names_to));
    output_cols.push(SchemaColumn::new(values_to));

    Ok((combined, output_cols))
}

// === pivot_wider ===

fn pivot_wider(
    input: SqlQuery,
    args: &[Expr],
    columns: &[SchemaColumn],
    _groups: &[String],
    _order: &[OrderExpr],
) -> GenerationResult<(SqlQuery, Vec<SchemaColumn>)> {
    let (positional, named) = split_args(
        args,
        &[
            "names_from",
            "values_from",
            "keys",
            "names_prefix",
            "values_fill",
            "values_fn",
            "id_cols",
        ],
    )?;

    let names_from = if let Some(nf) = named.get("names_from") {
        string_value(nf)?
    } else if let Some(first) = positional.first() {
        string_value(first)?
    } else {
        return Err(invalid("pivot_wider() requires names_from"));
    };

    let values_from = if let Some(vf) = named.get("values_from") {
        string_value(vf)?
    } else if let Some(second) = positional.get(1) {
        string_value(second)?
    } else {
        return Err(invalid("pivot_wider() requires values_from"));
    };

    let names_prefix = named
        .get("names_prefix")
        .map(|e| string_value(e))
        .transpose()?
        .unwrap_or_default();

    let values_fn = named
        .get("values_fn")
        .map(|e| parse_values_fn(e))
        .transpose()?
        .unwrap_or_else(|| "max".to_string());

    let values_fill = named.get("values_fill").copied();

    // id_cols: selector for identity columns (excludes names_from and values_from)
    let id_cols: Vec<String> = if let Some(id_expr) = named.get("id_cols") {
        resolve_columns(id_expr, columns)?
    } else {
        columns
            .iter()
            .filter(|c| c.name != names_from && c.name != values_from)
            .map(|c| c.name.clone())
            .collect()
    };

    // Keys: literal domain values, not schema column references
    let keys = if let Some(keys_expr) = named.get("keys") {
        extract_literal_values(keys_expr)?
    } else {
        // Empty keys: use DISTINCT on id_cols to discover the domain
        Vec::new()
    };

    metadata(columns, &names_from)?;
    metadata(columns, &values_from)?;
    let input_source = wrap_input(input, "__libdplyr_pw_input");

    if !named.contains_key("keys") {
        return Err(invalid(
            "pivot_wider needs explicit keys or execute_with_pivot_discovery()",
        ));
    }
    if keys.is_empty() {
        let schema = id_cols
            .iter()
            .map(|name| metadata(columns, name))
            .collect::<GenerationResult<Vec<_>>>()?;
        let mut q = simple_query(input_source, identity_projection(&schema));
        q.distinct = true;
        return Ok((q, schema));
    }
    let mut projection = Vec::new();
    for id_col in &id_cols {
        projection.push(SelectItem {
            alias: id_col.clone(),
            expression: SelectExpression::Scalar {
                expr: Expr::Identifier(id_col.clone()),
                partition_by: Vec::new(),
            },
        });
    }

    for key in &keys {
        let condition = if matches!(key, LiteralValue::Null) {
            Expr::Function {
                name: "is.na".into(),
                args: vec![reference(&names_from)],
            }
        } else {
            Expr::Binary {
                left: Box::new(reference(&names_from)),
                operator: BinaryOp::Equal,
                right: Box::new(Expr::Literal(key.clone())),
            }
        };
        let case_expr = Expr::CaseWhen {
            branches: vec![(condition, Expr::Identifier(values_from.clone()))],
            default: None,
        };

        let aggregate = if values_fn == "n" {
            Expr::Function {
                name: "sum".into(),
                args: vec![Expr::CaseWhen {
                    branches: vec![(
                        if matches!(key, LiteralValue::Null) {
                            Expr::Function {
                                name: "is.na".into(),
                                args: vec![reference(&names_from)],
                            }
                        } else {
                            Expr::Binary {
                                left: Box::new(reference(&names_from)),
                                operator: BinaryOp::Equal,
                                right: Box::new(Expr::Literal(key.clone())),
                            }
                        },
                        Expr::Literal(LiteralValue::Number(1.0)),
                    )],
                    default: Some(Box::new(Expr::Literal(LiteralValue::Number(0.0)))),
                }],
            }
        } else {
            Expr::Function {
                name: values_fn.clone(),
                args: vec![case_expr],
            }
        };
        let final_expr = if let Some(fill) = values_fill {
            Expr::Function {
                name: "coalesce".into(),
                args: vec![aggregate, fill.clone()],
            }
        } else {
            aggregate
        };
        let alias = format!("{names_prefix}{}", literal_name(key));
        projection.push(SelectItem {
            alias,
            expression: SelectExpression::AggregateExpression(final_expr),
        });
    }

    let group_by = id_cols.clone();
    let query = SqlQuery {
        source: input_source,
        projection,
        filter: None,
        group_by,
        order_by: Vec::new(),
        distinct: false,
        limit: None,
    };

    let mut output_cols = id_cols
        .iter()
        .map(|n| SchemaColumn::new(n.clone()))
        .collect::<Vec<_>>();
    for key in &keys {
        output_cols.push(SchemaColumn::new(format!(
            "{names_prefix}{}",
            literal_name(key)
        )));
    }

    Ok((query, output_cols))
}

// === replace_na ===

fn replace_na(
    input: SqlQuery,
    args: &[Expr],
    columns: &[SchemaColumn],
    _groups: &[String],
    _order: &[OrderExpr],
) -> GenerationResult<(SqlQuery, Vec<SchemaColumn>)> {
    let (positional, named) = split_args(args, &["data"])?;

    let data_expr = if let Some(d) = named.get("data") {
        *d
    } else if let Some(first) = positional.first() {
        *first
    } else {
        return Err(invalid("replace_na() requires a replacement list"));
    };

    let replacements = parse_replacement_list(data_expr)?;

    let input_source = wrap_input(input, "__libdplyr_rn_input");

    let mut projection = Vec::new();
    for col in columns {
        if let Some((_, value)) = replacements.iter().find(|(name, _)| name == &col.name) {
            projection.push(SelectItem {
                alias: col.name.clone(),
                expression: SelectExpression::Scalar {
                    expr: Expr::Function {
                        name: "coalesce".to_string(),
                        args: vec![Expr::Identifier(col.name.clone()), value.clone()],
                    },
                    partition_by: Vec::new(),
                },
            });
        } else {
            projection.push(SelectItem {
                alias: col.name.clone(),
                expression: SelectExpression::Scalar {
                    expr: Expr::Identifier(col.name.clone()),
                    partition_by: Vec::new(),
                },
            });
        }
    }

    let query = SqlQuery {
        source: input_source,
        projection,
        filter: None,
        group_by: Vec::new(),
        order_by: Vec::new(),
        distinct: false,
        limit: None,
    };

    Ok((query, columns.to_vec()))
}

fn parse_replacement_list(expr: &Expr) -> GenerationResult<Vec<(String, Expr)>> {
    let args = match expr {
        Expr::Function { name, args } if name == "list" => args,
        _ => return Err(invalid("replace_na() requires a list of replacements")),
    };
    let mut result = Vec::new();
    for arg in args {
        if let Expr::NamedArg { name, value } = arg {
            result.push((name.clone(), value.as_ref().clone()));
        } else {
            return Err(invalid("replace_na() list entries must be named"));
        }
    }
    Ok(result)
}

// === fill ===

fn fill(
    mut input: SqlQuery,
    args: &[Expr],
    columns: &[SchemaColumn],
    groups: &[String],
    order: &[OrderExpr],
) -> GenerationResult<(SqlQuery, Vec<SchemaColumn>)> {
    let (values, opts) = split_args(args, &[".direction", ".by"])?;
    if order.is_empty() {
        return Err(invalid(
            "fill requires an explicit window_order() or arrange()",
        ));
    }
    let direction = opts
        .get(".direction")
        .map(|e| string_value(e))
        .transpose()?
        .unwrap_or_else(|| "down".into());
    let steps = match direction.as_str() {
        "down" => vec![false],
        "up" => vec![true],
        "downup" => vec![false, true],
        "updown" => vec![true, false],
        _ => return Err(invalid("invalid fill direction")),
    };
    let mut partition = groups.to_vec();
    if let Some(by) = opts.get(".by") {
        if !groups.is_empty() {
            return Err(invalid("fill .by cannot be used with grouped input"));
        }
        partition = resolve_columns(by, columns)?;
    }
    let selected = values
        .into_iter()
        .map(|e| resolve_columns(e, columns))
        .collect::<GenerationResult<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(invalid("fill needs at least one column"));
    }
    for reverse in steps {
        let order = order
            .iter()
            .map(|o| OrderExpr {
                column: o.column.clone(),
                direction: if reverse {
                    match o.direction {
                        crate::parser::OrderDirection::Asc => crate::parser::OrderDirection::Desc,
                        crate::parser::OrderDirection::Desc => crate::parser::OrderDirection::Asc,
                    }
                } else {
                    o.direction.clone()
                },
            })
            .collect::<Vec<_>>();
        let mut aliases = Vec::new();
        let mut projection = identity_projection(columns);
        let mut reserved = columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        for column in &selected {
            let mut alias = "__fill_segment".to_owned();
            while reserved.contains(&alias) {
                alias.push('x');
            }
            reserved.push(alias.clone());
            aliases.push((column.clone(), alias.clone()));
            let present = Expr::CaseWhen {
                branches: vec![(
                    Expr::Function {
                        name: "is.na".into(),
                        args: vec![reference(column)],
                    },
                    Expr::Literal(LiteralValue::Number(0.0)),
                )],
                default: Some(Box::new(Expr::Literal(LiteralValue::Number(1.0)))),
            };
            projection.push(SelectItem {
                alias,
                expression: SelectExpression::WindowScalar {
                    expr: Expr::Function {
                        name: "sum".into(),
                        args: vec![present],
                    },
                    partition_by: partition.clone(),
                    order_by: order.clone(),
                    frame: Some((i64::MIN, 0)),
                },
            });
        }
        let segments = simple_query(wrap_input(input, "__fill_input"), projection);
        let projection = columns
            .iter()
            .map(|column| {
                if let Some((_, alias)) = aliases.iter().find(|(name, _)| name == &column.name) {
                    let mut groups = partition.clone();
                    groups.push(alias.clone());
                    SelectItem {
                        alias: column.name.clone(),
                        expression: SelectExpression::WindowScalar {
                            expr: Expr::Function {
                                name: "max".into(),
                                args: vec![reference(&column.name)],
                            },
                            partition_by: groups,
                            order_by: Vec::new(),
                            frame: None,
                        },
                    }
                } else {
                    scalar(reference(&column.name), &column.name)
                }
            })
            .collect();
        input = simple_query(wrap_input(segments, "__fill_segments"), projection);
    }
    Ok((input, columns.to_vec()))
}

fn expand_complete(
    input: SqlQuery,
    name: &str,
    args: &[Expr],
    columns: &[SchemaColumn],
    groups: &[String],
    _order: &[OrderExpr],
) -> GenerationResult<(SqlQuery, Vec<SchemaColumn>)> {
    let (values, opts) = split_args(args, &["fill", "explicit"])?;
    let base = wrap_input(input.clone(), "__expand_input");
    let mut domains = Vec::new();
    if !groups.is_empty() {
        let schema = groups
            .iter()
            .map(|n| metadata(columns, n))
            .collect::<GenerationResult<Vec<_>>>()?;
        let mut domain = simple_query(base.clone(), identity_projection(&schema));
        domain.distinct = true;
        domains.push((domain, schema));
    }
    for value in values {
        if let Expr::NamedArg { name, value } = value {
            let values = extract_literal_values(value)?;
            let schema = vec![metadata(columns, name)?];
            let domain = simple_query(
                SqlSource::Values {
                    columns: vec![name.clone()],
                    rows: values.into_iter().map(|v| vec![Expr::Literal(v)]).collect(),
                },
                identity_projection(&schema),
            );
            domains.push((domain, schema));
            continue;
        }
        let selections = match value {
            Expr::Function { name, args } if name == "nesting" => vec![args
                .iter()
                .map(|a| resolve_columns(a, columns))
                .collect::<GenerationResult<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()],
            _ => resolve_columns(value, columns)?
                .into_iter()
                .map(|n| vec![n])
                .collect(),
        };
        for selection in selections {
            if selection.iter().any(|n| groups.contains(n)) {
                return Err(invalid("expand cannot select a grouping column"));
            }
            let mut names = groups.to_vec();
            names.extend(selection);
            let schema = names
                .iter()
                .map(|n| metadata(columns, n))
                .collect::<GenerationResult<Vec<_>>>()?;
            let mut domain = simple_query(base.clone(), identity_projection(&schema));
            domain.distinct = true;
            domains.push((domain, schema));
        }
    }
    let mut domains = domains.into_iter();
    let (mut combined, mut schema) = domains
        .next()
        .ok_or_else(|| invalid("expand requires at least one column"))?;
    for (domain, right_schema) in domains {
        let keys = groups
            .iter()
            .filter(|n| right_schema.iter().any(|c| &c.name == *n))
            .map(|n| (n.clone(), n.clone()))
            .collect();
        let mut projection = schema
            .iter()
            .map(|c| SelectItem {
                alias: c.name.clone(),
                expression: SelectExpression::Qualified {
                    relation: "__libdplyr_left".into(),
                    column: c.name.clone(),
                },
            })
            .collect::<Vec<_>>();
        for c in right_schema {
            if !schema.iter().any(|old| old.name == c.name) {
                projection.push(SelectItem {
                    alias: c.name.clone(),
                    expression: SelectExpression::Qualified {
                        relation: "__libdplyr_right".into(),
                        column: c.name.clone(),
                    },
                });
                schema.push(c);
            }
        }
        combined = simple_query(
            SqlSource::Join {
                left: Box::new(combined),
                right: Box::new(domain),
                join_type: JoinType::Inner,
                keys,
                predicates: Vec::new(),
                closest: None,
                na_matches: true,
            },
            projection,
        );
    }
    if name == "expand" {
        return Ok((combined, schema));
    }
    let fills = opts
        .get("fill")
        .map(|e| parse_replacement_list(e))
        .transpose()?
        .unwrap_or_default();
    let explicit = opts
        .get("explicit")
        .map(|e| match e {
            Expr::Literal(LiteralValue::Boolean(b)) => Ok(*b),
            _ => Err(invalid("complete explicit must be TRUE or FALSE")),
        })
        .transpose()?
        .unwrap_or(true);
    let keys = schema
        .iter()
        .map(|c| (c.name.clone(), c.name.clone()))
        .collect();
    let mut output = schema.clone();
    output.extend(
        columns
            .iter()
            .filter(|c| !schema.iter().any(|key| key.name == c.name))
            .cloned(),
    );
    for c in &mut output {
        c.nullable = Some(true);
    }
    let columns = output.as_slice();
    let original = simple_query(base, identity_projection(columns));
    let projection = columns
        .iter()
        .map(|c| {
            if schema.iter().any(|key| key.name == c.name) {
                SelectItem {
                    alias: c.name.clone(),
                    expression: SelectExpression::Qualified {
                        relation: "__libdplyr_left".into(),
                        column: c.name.clone(),
                    },
                }
            } else {
                scalar(
                    fills
                        .iter()
                        .find(|(n, _)| n == &c.name)
                        .map(|(_, v)| v.clone())
                        .unwrap_or(Expr::Literal(LiteralValue::Null)),
                    &c.name,
                )
            }
        })
        .collect();
    let missing = simple_query(
        SqlSource::Join {
            left: Box::new(combined),
            right: Box::new(original.clone()),
            join_type: JoinType::Anti,
            keys,
            predicates: Vec::new(),
            closest: None,
            na_matches: true,
        },
        projection,
    );
    let union = simple_query(
        SqlSource::Set {
            left: Box::new(original),
            right: Box::new(missing),
            operation: SetOperation::UnionAll,
        },
        identity_projection(columns),
    );
    let result = if explicit && !fills.is_empty() {
        replace_na(
            union,
            &[Expr::Function {
                name: "list".into(),
                args: fills
                    .into_iter()
                    .map(|(name, value)| Expr::NamedArg {
                        name,
                        value: Box::new(value),
                    })
                    .collect(),
            }],
            columns,
            groups,
            &[],
        )?
        .0
    } else {
        union
    };
    Ok((result, columns.to_vec()))
}
