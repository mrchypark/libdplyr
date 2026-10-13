//! Tidy-select resolution against a known schema.
//!
//! Every entry is evaluated to a set of schema positions. Positions and ranges
//! index the full input schema, never the running selection, matching dplyr.
//! Entries then fold left to right: a positive entry adds, a negative entry
//! removes. Output order is the order the names were selected.

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{BinaryOp, ColumnExpr, Expr, LiteralValue, UnaryOp};

use super::SchemaColumn;

/// Name of the synthetic helper carrying a positional range.
pub(super) const SELECT_RANGE: &str = "__select_range";

fn invalid(reason: impl Into<String>) -> GenerationError {
    GenerationError::InvalidAst {
        reason: reason.into(),
    }
}

/// A resolved entry: schema positions plus whether it subtracts.
struct Selection {
    items: Vec<usize>,
    negative: bool,
}

impl Selection {
    fn positive(items: Vec<usize>) -> Self {
        Self {
            items,
            negative: false,
        }
    }

    /// Positions this entry contributes once polarity is applied.
    fn resolved(&self, schema: &[SchemaColumn]) -> Vec<usize> {
        if self.negative {
            (0..schema.len())
                .filter(|index| !self.items.contains(index))
                .collect()
        } else {
            self.items.clone()
        }
    }
}

/// Resolves a tidy-select list into ordered column references.
///
/// A rename keeps the source name in `expr` and the new name in `alias`.
pub(super) fn resolve(
    columns: &[ColumnExpr],
    schema: &[SchemaColumn],
) -> GenerationResult<Vec<ColumnExpr>> {
    // Schema positions in output order, each with its optional output name.
    let mut selected: Vec<(usize, Option<String>)> = Vec::new();
    let mut started = false;

    let expanded = named_entries(columns, schema)?;
    for entry in &expanded {
        let selection = evaluate(&entry.expr, schema)?;

        let Some(alias) = &entry.alias else {
            if selection.negative && !started {
                // The first subtraction starts from the whole schema.
                selected = (0..schema.len()).map(|index| (index, None)).collect();
            }
            started = true;
            if selection.negative {
                selected.retain(|(index, _)| !selection.items.contains(index));
            } else {
                for index in selection.items {
                    if !selected.iter().any(|(kept, _)| *kept == index) {
                        selected.push((index, None));
                    }
                }
            }
            continue;
        };

        // A rename selects its source and names the output; it need not be
        // selected earlier, and the source may arrive in any position.
        if selection.negative {
            return Err(invalid(format!(
                "rename '{alias}' cannot be used to exclude a column"
            )));
        }
        if selection.items.len() != 1 {
            return Err(invalid(format!(
                "rename '{alias}' must select exactly one column, but it selected {}",
                selection.items.len()
            )));
        }
        let index = selection.items[0];
        started = true;
        match selected.iter_mut().find(|(kept, _)| *kept == index) {
            Some((_, name)) => *name = Some(alias.clone()),
            None => selected.push((index, Some(alias.clone()))),
        }
    }

    // Duplicate output names are only decidable once every alias is mapped.
    let mut columns = Vec::with_capacity(selected.len());
    for (index, alias) in selected {
        let source = &schema[index].name;
        let name = alias.as_deref().unwrap_or(source);
        if columns.iter().any(|column: &ColumnExpr| {
            column.alias.as_deref().unwrap_or(match &column.expr {
                Expr::Identifier(name) => name.as_str(),
                _ => "",
            }) == name
        }) {
            return Err(invalid(format!("name '{name}' is used twice")));
        }
        columns.push(ColumnExpr {
            expr: Expr::Identifier(source.clone()),
            alias,
        });
    }
    Ok(columns)
}

// Named vectors carry output names, including when passed to all_of()/any_of().
fn named_entries(
    columns: &[ColumnExpr],
    schema: &[SchemaColumn],
) -> GenerationResult<Vec<ColumnExpr>> {
    let mut out = Vec::new();
    for entry in columns {
        let (values, permissive) = match &entry.expr {
            Expr::Function { name, args }
                if matches!(name.as_str(), "all_of" | "any_of") && args.len() == 1 =>
            {
                match &args[0] {
                    Expr::Function { name: vector, args } if vector == "c" || vector == "list" => {
                        (Some(args), name == "any_of")
                    }
                    _ => (None, false),
                }
            }
            Expr::Function { name, args } if name == "c" || name == "list" => (Some(args), false),
            _ => (None, false),
        };
        if let Some(values) =
            values.filter(|values| values.iter().any(|v| matches!(v, Expr::NamedArg { .. })))
        {
            if entry.alias.is_some() {
                return Err(invalid("a named vector cannot have an outer alias"));
            }
            for value in values {
                let (expr, alias) = match value {
                    Expr::NamedArg { name, value } => (value.as_ref(), Some(name.clone())),
                    e => (e, None),
                };
                if permissive {
                    if let Expr::Literal(LiteralValue::String(name)) | Expr::Identifier(name) = expr
                    {
                        if !schema.iter().any(|c| &c.name == name) {
                            continue;
                        }
                    }
                }
                out.push(ColumnExpr {
                    expr: expr.clone(),
                    alias,
                });
            }
        } else {
            out.push(entry.clone());
        }
    }
    Ok(out)
}

/// Evaluates one selection expression.
fn evaluate(expr: &Expr, schema: &[SchemaColumn]) -> GenerationResult<Selection> {
    match expr {
        Expr::Identifier(name) => {
            if name == "*" {
                return Ok(Selection::positive((0..schema.len()).collect()));
            }
            Ok(Selection::positive(vec![index_of(name, schema)?]))
        }
        Expr::Literal(LiteralValue::String(name)) => {
            Ok(Selection::positive(vec![index_of(name, schema)?]))
        }
        Expr::Literal(LiteralValue::Number(_)) => {
            Ok(Selection::positive(vec![range_endpoint(expr, schema)?]))
        }
        Expr::Unary {
            operator: UnaryOp::Plus,
            expr,
        } => evaluate(expr, schema),
        Expr::Unary {
            operator: UnaryOp::Not | UnaryOp::Minus,
            expr,
        } => {
            // `select(-1)` drops a position; `-name` removes a column.
            let mut inner = evaluate(expr, schema)?;
            inner.negative = !inner.negative;
            Ok(inner)
        }
        Expr::Binary {
            left,
            operator: BinaryOp::And,
            right,
        } => {
            let left = evaluate(left, schema)?.resolved(schema);
            let right = evaluate(right, schema)?.resolved(schema);
            Ok(Selection::positive(combine(&left, &right, |a, b| a && b)))
        }
        Expr::Binary {
            left,
            operator: BinaryOp::Or,
            right,
        } => {
            let left = evaluate(left, schema)?.resolved(schema);
            let right = evaluate(right, schema)?.resolved(schema);
            Ok(Selection::positive(combine(&left, &right, |a, b| a || b)))
        }
        Expr::Function { name, args } => evaluate_function(name, args, schema),
        other => Err(GenerationError::UnsupportedOperation {
            operation: format!("computed select() entry: {other}"),
            dialect: "select".to_string(),
        }),
    }
}

/// Applies a set operator over two index lists, keeping left-to-right order.
fn combine(left: &[usize], right: &[usize], op: fn(bool, bool) -> bool) -> Vec<usize> {
    let mut out = Vec::new();
    for index in left.iter().chain(right) {
        if op(left.contains(index), right.contains(index)) && !out.contains(index) {
            out.push(*index);
        }
    }
    out
}

fn index_of(name: &str, schema: &[SchemaColumn]) -> GenerationResult<usize> {
    schema
        .iter()
        .position(|column| column.name == name)
        .ok_or_else(|| GenerationError::InvalidColumnReference {
            column: name.to_string(),
            table: None,
        })
}

fn evaluate_function(
    name: &str,
    args: &[Expr],
    schema: &[SchemaColumn],
) -> GenerationResult<Selection> {
    match name {
        SELECT_RANGE => {
            if args.len() != 2 {
                return Err(invalid("__select_range() takes a start and an end"));
            }
            if args.iter().any(is_negated) {
                if !args.iter().all(is_negated) {
                    return Err(invalid("ranges cannot mix positive and negative positions"));
                }
                let endpoints = args
                    .iter()
                    .map(|arg| {
                        let Expr::Unary { expr, .. } = arg else {
                            unreachable!("checked above")
                        };
                        range_endpoint(expr, schema)
                    })
                    .collect::<GenerationResult<Vec<_>>>()?;
                let low = endpoints[0].min(endpoints[1]);
                let high = endpoints[0].max(endpoints[1]);
                return Ok(Selection {
                    items: (low..=high).collect(),
                    negative: true,
                });
            }
            // Ranges index the full input schema, like dplyr.
            let start = range_endpoint(&args[0], schema)?;
            let end = range_endpoint(&args[1], schema)?;
            // A reversed range such as `3:2` keeps the written order.
            let items = if start <= end {
                (start..=end).collect()
            } else {
                (end..=start).rev().collect()
            };
            Ok(Selection::positive(items))
        }
        "c" => {
            let columns = args
                .iter()
                .map(|expr| ColumnExpr {
                    expr: expr.clone(),
                    alias: None,
                })
                .collect::<Vec<_>>();
            let resolved = resolve(&columns, schema)?;
            let items = resolved
                .iter()
                .map(|column| {
                    let Expr::Identifier(name) = &column.expr else {
                        unreachable!("resolved selector")
                    };
                    index_of(name, schema)
                })
                .collect::<GenerationResult<_>>()?;
            Ok(Selection::positive(items))
        }
        "everything" => {
            if !args.is_empty() {
                return Err(invalid("everything() takes no arguments"));
            }
            Ok(Selection::positive((0..schema.len()).collect()))
        }
        "starts_with" | "ends_with" | "contains" => {
            let (pattern, ignore_case) = string_and_ignore_case(args)?;
            let matched = schema
                .iter()
                .enumerate()
                .filter(|(_, column)| {
                    let (column, pattern) = if ignore_case {
                        (column.name.to_lowercase(), pattern.to_lowercase())
                    } else {
                        (column.name.clone(), pattern.clone())
                    };
                    match name {
                        "starts_with" => column.starts_with(&pattern),
                        "ends_with" => column.ends_with(&pattern),
                        _ => column.contains(&pattern),
                    }
                })
                .map(|(index, _)| index)
                .collect();
            Ok(Selection::positive(matched))
        }
        "matches" => {
            let (pattern, ignore_case) = string_and_ignore_case(args)?;
            // Compile first so an invalid pattern fails loudly.
            let regex = build_regex(&pattern, ignore_case)?;
            let matched = schema
                .iter()
                .enumerate()
                .filter(|(_, column)| regex.is_match(&column.name))
                .map(|(index, _)| index)
                .collect();
            Ok(Selection::positive(matched))
        }
        "where" => {
            if args.len() != 1 {
                return Err(invalid("where() takes one predicate"));
            }
            let name = match &args[0] {
                Expr::Identifier(name) => name,
                Expr::Function { name, args } if args.is_empty() => name,
                _ => return Err(invalid("where() needs a supported type predicate")),
            };
            if !is_type_predicate(name) {
                return Err(invalid("where() needs a supported type predicate"));
            }
            let mut matched = Vec::new();
            for (index, column) in schema.iter().enumerate() {
                let data_type = column.data_type.as_deref().ok_or_else(|| {
                    invalid(format!(
                        "where({name}()) needs data types; column '{}' has none in the schema",
                        column.name
                    ))
                })?;
                if type_matches(name, data_type) {
                    matched.push(index);
                }
            }
            Ok(Selection::positive(matched))
        }
        "all_of" => {
            if args.len() != 1 {
                return Err(invalid("all_of() needs one literal vector"));
            }
            // Strict: every literal name must exist.
            let mut names = Vec::new();
            for arg in args {
                collect_names(arg, schema, &mut names, &mut Vec::new(), true)?;
            }
            Ok(Selection::positive(names))
        }
        "any_of" => {
            if args.len() != 1 {
                return Err(invalid("any_of() needs one literal vector"));
            }
            // Lenient: names that are not columns are skipped, like dplyr.
            let mut names = Vec::new();
            for arg in args {
                collect_names(arg, schema, &mut names, &mut Vec::new(), false)?;
            }
            Ok(Selection::positive(names))
        }
        "last_col" => {
            if args.len() > 1 {
                return Err(invalid("last_col() takes at most one offset argument"));
            }
            let offset = match args.first() {
                None => 0i64,
                Some(Expr::NamedArg { value, .. }) => number(value)?,
                Some(expr) => number(expr)?,
            };
            if offset < 0 {
                return Err(invalid("last_col() offset must not be negative"));
            }
            // `last_col()` counts back from the end of the full input schema.
            let index = schema.len() as i64 - 1 - offset;
            if index < 0 {
                return Err(invalid(format!(
                    "last_col({offset}) is out of range for {} columns",
                    schema.len()
                )));
            }
            Ok(Selection::positive(vec![index as usize]))
        }
        other => Err(invalid(format!("unknown selection helper '{other}()'"))),
    }
}

/// Collects literal column names, splitting `-x` arguments by polarity.
///
/// With `strict`, a name missing from the schema is an error. `any_of()` passes
/// `false` so unknown names are skipped instead.
fn collect_names(
    expr: &Expr,
    schema: &[SchemaColumn],
    positive: &mut Vec<usize>,
    _negative: &mut Vec<usize>,
    strict: bool,
) -> GenerationResult<()> {
    match expr {
        Expr::Literal(LiteralValue::String(name)) => {
            match schema.iter().position(|column| column.name == *name) {
                Some(index) => positive.push(index),
                None if strict => {
                    return Err(GenerationError::InvalidColumnReference {
                        column: name.clone(),
                        table: None,
                    })
                }
                None => {}
            }
        }
        Expr::Function { name, args } if name == "c" => {
            for arg in args {
                collect_names(arg, schema, positive, &mut Vec::new(), strict)?;
            }
        }
        _ => {
            return Err(invalid(
                "all_of()/any_of() require a literal character vector",
            ))
        }
    }
    Ok(())
}

/// Maps one range endpoint, either a position or a column name, to an index.
fn range_endpoint(expr: &Expr, schema: &[SchemaColumn]) -> GenerationResult<usize> {
    match expr {
        Expr::Literal(LiteralValue::Number(value)) => {
            if value.fract() != 0.0 {
                return Err(invalid("selection positions must be whole numbers"));
            }
            let raw = *value as i64;
            if raw < 1 || raw as usize > schema.len() {
                return Err(invalid(format!(
                    "selection position {raw} is out of range for {} columns",
                    schema.len()
                )));
            }
            Ok((raw - 1) as usize)
        }
        Expr::Identifier(name) => index_of(name, schema),
        other => Err(invalid(format!(
            "expected a position or column name in a range, got {other}"
        ))),
    }
}

fn is_negated(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Unary {
            operator: UnaryOp::Minus,
            ..
        }
    )
}

fn number(expr: &Expr) -> GenerationResult<i64> {
    let Expr::Literal(LiteralValue::Number(value)) = expr else {
        return Err(invalid(format!("expected a number, got {expr}")));
    };
    if value.fract() != 0.0 {
        return Err(invalid("selection positions must be whole numbers"));
    }
    Ok(*value as i64)
}

/// Reads `(pattern, ignore.case)`, defaulting `ignore.case` to true.
fn string_and_ignore_case(args: &[Expr]) -> GenerationResult<(String, bool)> {
    let mut pattern = None;
    let mut ignore_case = true;
    for arg in args {
        match arg {
            Expr::NamedArg { name, value } if name == "ignore.case" => {
                let Expr::Literal(LiteralValue::Boolean(value)) = value.as_ref() else {
                    return Err(invalid("ignore.case must be TRUE or FALSE"));
                };
                ignore_case = *value;
            }
            Expr::Literal(LiteralValue::String(value)) if pattern.is_none() => {
                pattern = Some(value.clone());
            }
            other => return Err(invalid(format!("expected a string pattern, got {other}"))),
        }
    }
    let pattern = pattern.ok_or_else(|| invalid("a string pattern is required"))?;
    Ok((pattern, ignore_case))
}

fn build_regex(pattern: &str, ignore_case: bool) -> GenerationResult<regex::Regex> {
    regex::RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|error| invalid(format!("matches() pattern '{pattern}' is invalid: {error}")))
}

fn is_type_predicate(name: &str) -> bool {
    matches!(
        name,
        "is.numeric" | "is.character" | "is.logical" | "is.integer" | "is.double"
    )
}

/// R type families for `where()`.
fn type_matches(predicate: &str, data_type: &str) -> bool {
    let lower = data_type.to_lowercase();
    if lower.contains('[') {
        return false;
    }
    let normalised = lower
        .split('(')
        .next()
        .unwrap_or(&lower)
        .trim()
        .trim_end_matches(" unsigned");
    let matches_any = |types: &[&str]| types.contains(&normalised);
    match predicate {
        "is.numeric" => matches_any(&[
            "integer",
            "int",
            "int2",
            "int4",
            "int8",
            "tinyint",
            "smallint",
            "bigint",
            "hugeint",
            "utinyint",
            "usmallint",
            "uinteger",
            "ubigint",
            "uhugeint",
            "double",
            "float",
            "float4",
            "float8",
            "real",
            "numeric",
            "decimal",
            "number",
        ]),
        "is.character" => matches_any(&[
            "character",
            "varchar",
            "varchar2",
            "char",
            "text",
            "string",
            "bpchar",
        ]),
        "is.logical" => matches_any(&["logical", "boolean", "bool"]),
        "is.integer" => matches_any(&[
            "integer",
            "int",
            "int2",
            "int4",
            "int8",
            "tinyint",
            "smallint",
            "bigint",
            "hugeint",
            "utinyint",
            "usmallint",
            "uinteger",
            "ubigint",
            "uhugeint",
        ]),
        "is.double" => matches_any(&[
            "double",
            "double precision",
            "float",
            "float4",
            "float8",
            "real",
        ]),
        _ => false,
    }
}
