//! Expansion of across() into one assignment per selected column.
//!
//! across() is not a SQL function. It is a shape in the AST that this module
//! rewrites into a flat list of Assignment values, one per selected column, so
//! the planner can project them atomically against a single input snapshot.

use super::schema::SchemaColumn;
use crate::error::{GenerationError, GenerationResult};
use crate::parser::{Assignment, ColumnExpr, Expr, LiteralValue};

/// Parser marker for a formula body captured inside across().
const LAMBDA: &str = "__across_lambda";

/// Upper bound on the depth of one substituted lambda body.
///
/// Substitution never deepens a tree, but this keeps a hostile input from
/// reaching the recursive walk or SQL generation without a checked bound.
const MAX_SUBSTITUTE_DEPTH: usize = 64;

/// A function to apply to every selected column, with the label naming it.
#[derive(Clone)]
struct Applied {
    label: String,
    kind: Kind,
}

#[derive(Clone)]
enum Kind {
    /// A formula body, already captured as an expression tree.
    Lambda(Expr),
    /// A bare function name such as sqrt.
    Named(String),
    Identity,
}

impl Applied {
    /// A single unnamed .fns entry, or the identity when .fns is absent.
    /// Its label is unused because a bare function keeps the column name.
    fn plain(kind: Kind) -> Self {
        Self {
            label: String::new(),
            kind,
        }
    }
}

fn invalid(reason: impl Into<String>) -> GenerationError {
    GenerationError::InvalidAst {
        reason: reason.into(),
    }
}

fn unsupported(operation: impl Into<String>) -> GenerationError {
    GenerationError::UnsupportedOperation {
        operation: operation.into(),
        dialect: "across".to_string(),
    }
}

/// Expands a top-level across() call into one Assignment per column.
///
/// An empty selection expands to no assignments, which makes a mutate() call
/// a no-op rather than an error.
pub(super) fn expand(expr: &Expr, schema: &[SchemaColumn]) -> GenerationResult<Vec<Assignment>> {
    let Expr::Function { name, args } = expr else {
        return Err(invalid("across() must be a function call"));
    };
    if name != "across" {
        return Err(invalid(format!("expected across(), found {name}()")));
    }

    let (cols, fns, names) = split_arguments(args)?;
    let selected = super::selection::resolve(&cols, schema)?;
    let columns = selected
        .into_iter()
        .map(|column| match column.expr {
            Expr::Identifier(name) => Ok(name),
            _ => Err(invalid("across() selectors must name plain columns")),
        })
        .collect::<GenerationResult<Vec<String>>>()?;
    if columns.is_empty() {
        return Ok(Vec::new());
    }

    let (applied, listed) = match fns {
        None => (vec![Applied::plain(Kind::Identity)], false),
        Some(fns) => parse_functions(&fns)?,
    };

    let mut assignments = Vec::with_capacity(columns.len() * applied.len());
    for column in &columns {
        for function in &applied {
            let name = output_name(column, function, names.as_deref(), listed)?;
            let value = match &function.kind {
                Kind::Identity => Expr::Identifier(column.clone()),
                Kind::Named(name) => Expr::Function {
                    name: name.clone(),
                    args: vec![Expr::Identifier(column.clone())],
                },
                Kind::Lambda(body) => substitute(body, column, 1)?,
            };
            assignments.push(Assignment {
                column: name,
                expr: value,
            });
        }
    }

    let mut seen = std::collections::HashSet::new();
    for assignment in &assignments {
        if !seen.insert(assignment.column.as_str()) {
            return Err(invalid(format!(
                "across() produced the duplicate output column '{}'",
                assignment.column
            )));
        }
    }
    Ok(assignments)
}

/// Splits across() arguments into .cols, .fns, and .names.
///
/// Positional arguments fill the first slot that no named argument claimed,
/// which is how R matches across(.cols = x, mean).
fn split_arguments(
    args: &[Expr],
) -> GenerationResult<(Vec<ColumnExpr>, Option<Expr>, Option<String>)> {
    let mut positional = Vec::new();
    let mut slots: [Option<Expr>; 3] = [None, None, None];
    let mut named = [false; 3];

    for arg in args {
        let Expr::NamedArg { name, value } = arg else {
            positional.push(arg.clone());
            continue;
        };
        let index = match name.as_str() {
            ".cols" => 0,
            ".fns" => 1,
            ".names" => 2,
            _ => {
                return Err(GenerationError::UnsupportedNamedArgument {
                    function: "across".to_string(),
                    argument: name.clone(),
                    dialect: "across".to_string(),
                })
            }
        };
        if named[index] {
            return Err(invalid(format!("across() {name} was given twice")));
        }
        named[index] = true;
        slots[index] = Some(value.as_ref().clone());
    }

    if positional.len() > 3 {
        return Err(invalid("across() takes at most three arguments"));
    }
    let mut remaining = positional.into_iter();
    for slot in &mut slots {
        if slot.is_none() {
            *slot = remaining.next();
        }
    }

    if remaining.next().is_some() {
        return Err(invalid("across() has extra positional arguments"));
    }
    let Some(cols) = slots[0].clone() else {
        return Err(invalid("across() requires .cols"));
    };
    let names = match slots[2].take() {
        None | Some(Expr::Literal(LiteralValue::Null)) => None,
        Some(Expr::Literal(LiteralValue::String(template))) => Some(template),
        Some(_) => return Err(invalid("across() .names must be a single string")),
    };
    Ok((selector_list(&cols), slots[1].take(), names))
}

/// Converts a selector expression into the ColumnExpr list that selection
/// understands: c(...) and list(...) spread, anything else stands alone.
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

/// Parses .fns into the functions to apply and whether it was written as a
/// list. A list always labels its outputs, even when it holds one function.
fn parse_functions(expr: &Expr) -> GenerationResult<(Vec<Applied>, bool)> {
    if matches!(expr, Expr::Literal(LiteralValue::Null)) {
        return Ok((vec![Applied::plain(Kind::Identity)], false));
    }
    let args = match expr {
        Expr::Function { name, args } if name == "c" || name == "list" => args,
        _ => return Ok((vec![Applied::plain(parse_kind(expr)?)], false)),
    };
    if args.is_empty() {
        return Err(invalid("across() was given an empty list of functions"));
    }
    let mut applied = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let (key, value) = match arg {
            Expr::NamedArg { name, value } => (Some(name.as_str()), value.as_ref()),
            other => (None, other),
        };
        // dplyr labels an unnamed list entry by position, never by the
        // function name, so list(mean, sd) becomes x_1 and x_2.
        let label = match key {
            Some(key) if !key.is_empty() => key.to_string(),
            _ => (index + 1).to_string(),
        };
        applied.push(Applied {
            label,
            kind: parse_kind(value)?,
        });
    }
    Ok((applied, true))
}

fn parse_kind(expr: &Expr) -> GenerationResult<Kind> {
    match expr {
        Expr::Identifier(name) => Ok(Kind::Named(name.clone())),
        Expr::Function { name, args } if name == LAMBDA => match args.as_slice() {
            [body] => Ok(Kind::Lambda(body.clone())),
            _ => Err(invalid("across() lambda must carry exactly one body")),
        },
        _ => Err(unsupported(
            "across() .fns must be a function name or a lambda",
        )),
    }
}

/// Builds one output column name, applying the default when .names is absent.
///
/// A bare function overwrites its input column, so it keeps the plain name. A
/// list always adds its label, so list(total = sum) is x_total even though it
/// holds one function.
fn output_name(
    column: &str,
    function: &Applied,
    names: Option<&str>,
    listed: bool,
) -> GenerationResult<String> {
    let Some(template) = names else {
        return Ok(if listed {
            format!("{column}_{}", function.label)
        } else {
            column.to_string()
        });
    };
    let rendered = render_template(template, column, &function.label)?;
    if rendered.is_empty() {
        return Err(invalid("across() .names produced an empty column name"));
    }
    Ok(rendered)
}

/// Substitutes only {.col} and {.fn}; any other brace group is an error rather
/// than being copied through into a column name.
fn render_template(template: &str, column: &str, function: &str) -> GenerationResult<String> {
    let remaining = template.replace("{.col}", "").replace("{.fn}", "");
    if remaining.contains(['{', '}']) {
        return Err(invalid(
            "across() .names placeholder is not supported; use {.col} and {.fn}",
        ));
    }
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        output.push_str(&rest[..open]);
        let tail = &rest[open..];
        let Some(end) = tail.find('}') else {
            return Err(invalid("across() .names has an unclosed '{'"));
        };
        match &tail[1..end] {
            ".col" => output.push_str(column),
            ".fn" => output.push_str(function),
            other => {
                return Err(invalid(format!(
                    "across() .names placeholder '{{{other}}}' is not supported; use .col or .fn"
                )))
            }
        }
        rest = &tail[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

/// Replaces the lambda variable with the current column and cur_column() with
/// its name. Other dot-prefixed identifiers are rejected here so they never
/// reach SQL generation as a bare column.
fn substitute(body: &Expr, column: &str, depth: usize) -> GenerationResult<Expr> {
    if depth > MAX_SUBSTITUTE_DEPTH {
        return Err(invalid(format!(
            "across() lambda body exceeds {MAX_SUBSTITUTE_DEPTH} levels of nesting"
        )));
    }
    let step = |child: &Expr| substitute(child, column, depth + 1);
    Ok(match body {
        Expr::Identifier(name) if name == "." || name == ".x" => {
            Expr::Identifier(column.to_string())
        }
        Expr::Identifier(name) if name.starts_with('.') => {
            return Err(invalid(format!(
                "'{name}' is not an across() lambda variable"
            )))
        }
        Expr::Function { name, args } if name == "cur_column" => match args.as_slice() {
            [] => Expr::Literal(LiteralValue::String(column.to_string())),
            _ => return Err(invalid("cur_column() takes no arguments")),
        },
        Expr::Unary { operator, expr } => Expr::Unary {
            operator: operator.clone(),
            expr: Box::new(step(expr)?),
        },
        Expr::In { expr, values } => Expr::In {
            expr: Box::new(step(expr)?),
            values: values.clone(),
        },
        Expr::Binary {
            left,
            operator,
            right,
        } => Expr::Binary {
            left: Box::new(step(left)?),
            operator: operator.clone(),
            right: Box::new(step(right)?),
        },
        Expr::Function { name, args } => Expr::Function {
            name: name.clone(),
            args: args
                .iter()
                .map(step)
                .collect::<GenerationResult<Vec<_>>>()?,
        },
        Expr::CaseWhen { branches, default } => Expr::CaseWhen {
            branches: branches
                .iter()
                .map(|(condition, value)| Ok((step(condition)?, step(value)?)))
                .collect::<GenerationResult<Vec<_>>>()?,
            default: default
                .as_ref()
                .map(|expr| step(expr).map(Box::new))
                .transpose()?,
        },
        Expr::NamedArg { name, value } => Expr::NamedArg {
            name: name.clone(),
            value: Box::new(step(value)?),
        },
        other => other.clone(),
    })
}
