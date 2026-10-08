//! External constants remain typed AST values; visible columns take precedence.
use super::{invalid, SchemaColumn};
use crate::{
    error::GenerationResult,
    parser::{Expr, LiteralValue, MAX_EXPRESSION_DEPTH},
};
use std::collections::HashMap;

fn literal(value: &serde_json::Value, depth: usize) -> GenerationResult<Expr> {
    if depth > MAX_EXPRESSION_DEPTH {
        return Err(invalid(
            "binding nesting exceeds the expression depth limit",
        ));
    }
    Ok(match value {
        serde_json::Value::Null => Expr::Literal(LiteralValue::Null),
        serde_json::Value::Bool(value) => Expr::Literal(LiteralValue::Boolean(*value)),
        serde_json::Value::Number(value) => {
            let number = value
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| invalid("binding number must be finite"))?;
            if value.as_i64().is_some_and(|v| number as i128 != v as i128)
                || value.as_u64().is_some_and(|v| number as u128 != v as u128)
            {
                return Err(invalid("binding integer is not exactly representable"));
            }
            Expr::Literal(LiteralValue::Number(number))
        }
        serde_json::Value::String(value) => Expr::Literal(LiteralValue::String(value.clone())),
        serde_json::Value::Array(values) => Expr::Function {
            name: "c".into(),
            args: values
                .iter()
                .map(|v| literal(v, depth + 1))
                .collect::<GenerationResult<_>>()?,
        },
        serde_json::Value::Object(values) => Expr::Function {
            name: "c".into(),
            args: values
                .iter()
                .map(|(name, value)| {
                    Ok(Expr::NamedArg {
                        name: name.clone(),
                        value: Box::new(literal(value, depth + 1)?),
                    })
                })
                .collect::<GenerationResult<_>>()?,
        },
    })
}
pub(super) fn resolve(
    expr: &Expr,
    columns: &[SchemaColumn],
    bindings: &HashMap<String, serde_json::Value>,
) -> GenerationResult<Expr> {
    resolve_at(expr, columns, bindings, 0)
}
fn resolve_at(
    expr: &Expr,
    columns: &[SchemaColumn],
    bindings: &HashMap<String, serde_json::Value>,
    depth: usize,
) -> GenerationResult<Expr> {
    if depth > MAX_EXPRESSION_DEPTH {
        return Err(invalid("resolved expression exceeds the depth limit"));
    }
    let next = |expr: &Expr| resolve_at(expr, columns, bindings, depth + 1);
    Ok(match expr {
        Expr::Function { name, args }
            if matches!(name.as_str(), "__data_column" | "__environment_value") =>
        {
            let [Expr::Literal(LiteralValue::String(key))] = args.as_slice() else {
                return Err(invalid("invalid pronoun member"));
            };
            if name == "__data_column" {
                if !columns.iter().any(|c| &c.name == key) {
                    return Err(invalid(format!("unknown .data column {key}")));
                }
                Expr::Identifier(key.clone())
            } else {
                literal(
                    bindings
                        .get(key)
                        .ok_or_else(|| invalid(format!("missing .env binding {key}")))?,
                    depth,
                )?
            }
        }
        Expr::Identifier(name) if !columns.iter().any(|c| &c.name == name) => {
            match bindings.get(name) {
                Some(value) => literal(value, depth)?,
                None => expr.clone(),
            }
        }
        Expr::Function { name, args } => Expr::Function {
            name: name.clone(),
            args: args.iter().map(next).collect::<GenerationResult<_>>()?,
        },
        Expr::NamedArg { name, value } => Expr::NamedArg {
            name: name.clone(),
            value: Box::new(next(value)?),
        },
        Expr::Unary { operator, expr } => Expr::Unary {
            operator: operator.clone(),
            expr: Box::new(next(expr)?),
        },
        Expr::Binary {
            left,
            operator,
            right,
        } => Expr::Binary {
            left: Box::new(next(left)?),
            operator: operator.clone(),
            right: Box::new(next(right)?),
        },
        Expr::In { expr, values } => Expr::In {
            expr: Box::new(next(expr)?),
            values: values.clone(),
        },
        Expr::CaseWhen { branches, default } => Expr::CaseWhen {
            branches: branches
                .iter()
                .map(|(a, b)| Ok((next(a)?, next(b)?)))
                .collect::<GenerationResult<_>>()?,
            default: default
                .as_ref()
                .map(|e| next(e).map(Box::new))
                .transpose()?,
        },
        _ => expr.clone(),
    })
}
