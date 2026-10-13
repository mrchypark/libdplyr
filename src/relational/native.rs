//! Native function calls and arithmetic are parsed before binding, never pasted into SQL.
use super::*;
use crate::{lexer::Lexer, parser::Parser};

pub(super) fn expand(expr: &Expr) -> GenerationResult<Expr> {
    expand_at(expr, 0)
}
fn expand_at(expr: &Expr, depth: usize) -> GenerationResult<Expr> {
    if depth > crate::parser::MAX_EXPRESSION_DEPTH {
        return Err(invalid(
            "native expression nesting exceeds the parser limit",
        ));
    }
    let expand = |expr: &Expr| expand_at(expr, depth + 1);
    Ok(match expr {
        Expr::Function { name, args } if name == "sql" => {
            let [Expr::Literal(LiteralValue::String(source))] = args.as_slice() else {
                return Err(invalid("sql() requires one constant expression string"));
            };
            let mut parser = Parser::new(Lexer::new(format!(
                "mutate(__native_expression = {source})"
            )))
            .map_err(|error| invalid(format!("invalid native expression: {error}")))?;
            let ast = parser.parse().map_err(|error| {
                invalid(format!(
                    "native expression must be parsed arithmetic or function syntax: {error}"
                ))
            })?;
            let DplyrNode::Pipeline {
                operations,
                target: None,
                source: None,
                ..
            } = ast
            else {
                return Err(invalid("sql() accepts one expression"));
            };
            let [DplyrOperation::Mutate { assignments, .. }] = operations.as_slice() else {
                return Err(invalid("sql() accepts one expression"));
            };
            let [assignment] = assignments.as_slice() else {
                return Err(invalid("sql() accepts one expression"));
            };
            if assignment.column != "__native_expression" {
                return Err(invalid("sql() cannot assign columns"));
            }
            if matches!(&assignment.expr,Expr::Function{name,..} if name=="sql") {
                return Err(invalid("sql() cannot contain another sql() call"));
            }
            expand(&assignment.expr)?
        }
        Expr::Function { name, args } => Expr::Function {
            name: name.clone(),
            args: args.iter().map(expand).collect::<GenerationResult<_>>()?,
        },
        Expr::Unary { operator, expr } => Expr::Unary {
            operator: operator.clone(),
            expr: Box::new(expand(expr)?),
        },
        Expr::Binary {
            left,
            operator,
            right,
        } => Expr::Binary {
            left: Box::new(expand(left)?),
            operator: operator.clone(),
            right: Box::new(expand(right)?),
        },
        Expr::NamedArg { name, value } => Expr::NamedArg {
            name: name.clone(),
            value: Box::new(expand(value)?),
        },
        Expr::In { expr, values } => Expr::In {
            expr: Box::new(expand(expr)?),
            values: values.clone(),
        },
        Expr::CaseWhen { branches, default } => Expr::CaseWhen {
            branches: branches
                .iter()
                .map(|(a, b)| Ok((expand(a)?, expand(b)?)))
                .collect::<GenerationResult<_>>()?,
            default: default
                .as_ref()
                .map(|e| expand(e).map(Box::new))
                .transpose()?,
        },
        expr => expr.clone(),
    })
}
