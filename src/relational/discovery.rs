//! Data-dependent pivot columns are discovered within the execution snapshot.
use super::*;
use crate::parser::{DplyrOperation, Expr, LiteralValue, OrderExpr};

pub(crate) fn pivot_keys(
    ast: &DplyrNode,
    schemas: &[SourceSchema],
    generator: &SqlGenerator,
) -> GenerationResult<Option<String>> {
    schema::validate_schemas(schemas)?;
    let (source, operations) = match ast {
        DplyrNode::Pipeline {
            source,
            operations,
            target,
            ..
        } => {
            if target.is_some() {
                return Err(invalid("pivot discovery cannot create a target table"));
            }
            (
                source.as_deref().unwrap_or(&schemas[0].source),
                operations.as_slice(),
            )
        }
        DplyrNode::DataSource { .. } => return Ok(None),
    };
    let constants = std::collections::HashMap::new();
    let mut planner = Planner {
        generator,
        schemas,
        next_id: 0,
        stages: 1,
        checks: Some(Vec::new()),
        bindings: &constants,
    };
    let mut input = planner.scan(source)?;
    for operation in operations {
        if let DplyrOperation::Extended { name, args, .. } = operation {
            if name == "pivot_wider"
                && !args
                    .iter()
                    .any(|arg| matches!(arg,Expr::NamedArg{name,..} if name=="keys"))
            {
                let names = args
                    .iter()
                    .find_map(|arg| match arg {
                        Expr::NamedArg { name, value } if name == "names_from" => {
                            Some(value.as_ref())
                        }
                        _ => None,
                    })
                    .or_else(|| args.first())
                    .ok_or_else(|| invalid("pivot_wider requires names_from"))?;
                let name = match names {
                    Expr::Identifier(name) | Expr::Literal(LiteralValue::String(name)) => name,
                    _ => {
                        return Err(invalid(
                            "dynamic pivot discovery requires one names_from column",
                        ))
                    }
                };
                visible_column(&input.columns, name)?;
                if super::join::volatile_relation(&input) {
                    return Err(invalid("dynamic pivot requires stable input; materialize volatile expressions first"));
                }
                let query = SqlQuery {
                    source: SqlSource::Subquery(
                        Box::new(lower(&input, &mut 1)?),
                        "__pivot_keys".into(),
                    ),
                    projection: vec![SelectItem {
                        alias: name.clone(),
                        expression: SelectExpression::Scalar {
                            expr: Expr::Identifier(name.clone()),
                            partition_by: Vec::new(),
                        },
                    }],
                    filter: None,
                    group_by: Vec::new(),
                    order_by: vec![OrderExpr {
                        column: name.clone(),
                        direction: OrderDirection::Asc,
                    }],
                    distinct: true,
                    limit: None,
                };
                return Ok(Some(query.render(generator)?));
            }
        }
        input = planner.apply(input, operation)?;
    }
    Ok(None)
}

pub(crate) fn supply_pivot_keys(
    ast: &mut DplyrNode,
    keys: &[serde_json::Value],
) -> GenerationResult<()> {
    let DplyrNode::Pipeline { operations, .. } = ast else {
        return Err(invalid("pivot discovery requires a pipeline"));
    };
    for operation in operations {
        if let DplyrOperation::Extended { name, args, .. } = operation {
            if name == "pivot_wider"
                && !args
                    .iter()
                    .any(|arg| matches!(arg,Expr::NamedArg{name,..} if name=="keys"))
            {
                let values = keys
                    .iter()
                    .map(|key| {
                        Ok(Expr::Literal(match key {
                            serde_json::Value::Null => LiteralValue::Null,
                            serde_json::Value::String(value) => LiteralValue::String(value.clone()),
                            serde_json::Value::Bool(value) => LiteralValue::Boolean(*value),
                            serde_json::Value::Number(value) => LiteralValue::Number(
                                value
                                    .as_f64()
                                    .filter(|n| n.is_finite())
                                    .ok_or_else(|| invalid("pivot key is not finite"))?,
                            ),
                            _ => return Err(invalid("pivot keys must be scalar values")),
                        }))
                    })
                    .collect::<GenerationResult<Vec<_>>>()?;
                args.push(Expr::NamedArg {
                    name: "keys".into(),
                    value: Box::new(Expr::Function {
                        name: "c".into(),
                        args: values,
                    }),
                });
                return Ok(());
            }
        }
    }
    Err(invalid("no unresolved pivot_wider operation"))
}
