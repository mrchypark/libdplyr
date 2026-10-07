//! Depth bounds for parser expressions.
//!
//! Two shapes must be rejected rather than overflowing the stack: explicit
//! parenthesis nesting, which recurses in the parser, and left-deep binary
//! chains, which parse in a loop but still build a deep AST that the generator
//! and the relational binder walk recursively. Breadth is never limited, so a
//! long flat pipeline or a wide function call stays valid.

use libdplyr::error::ParseError;
use libdplyr::lexer::Lexer;
use libdplyr::parser::{Parser, MAX_EXPRESSION_DEPTH};
use libdplyr::relational::SourceSchema;
use libdplyr::{PostgreSqlDialect, TranspileError, Transpiler};

fn parse(code: &str) -> Result<(), ParseError> {
    let lexer = Lexer::new(code.to_string());
    Parser::new(lexer)?.parse().map(|_| ())
}

fn expect_depth_error(code: &str) {
    match parse(code) {
        Err(ParseError::InvalidExpression { .. }) => {}
        Err(other) => panic!("expected InvalidExpression, got {other:?}"),
        Ok(()) => panic!("expected InvalidExpression, but parsing succeeded"),
    }
}

/// `n` nested parentheses around `x`.
fn nested_parens(n: usize) -> String {
    format!("data %>% mutate(d = {}x{})", "(".repeat(n), ")".repeat(n))
}

/// A left-deep chain of `n` additions, which the parser builds iteratively.
fn long_addition_chain(n: usize) -> String {
    let terms = (0..=n).map(|i| format!("c{i}")).collect::<Vec<_>>();
    format!("data %>% mutate(s = {})", terms.join(" + "))
}

#[test]
fn parentheses_within_the_limit_parse() {
    parse(&nested_parens(MAX_EXPRESSION_DEPTH - 8))
        .unwrap_or_else(|e| panic!("shallow nesting must parse: {e:?}"));
}

#[test]
fn parentheses_above_the_limit_are_rejected() {
    expect_depth_error(&nested_parens(MAX_EXPRESSION_DEPTH * 4));
}

#[test]
fn nested_function_calls_above_the_limit_are_rejected() {
    let calls = "abs(".repeat(MAX_EXPRESSION_DEPTH * 3);
    let closes = ")".repeat(MAX_EXPRESSION_DEPTH * 3);
    expect_depth_error(&format!("data %>% mutate(d = {calls}x{closes})"));
}

#[test]
fn nested_case_when_above_the_limit_is_rejected() {
    let mut expr = String::from("1");
    for i in 0..(MAX_EXPRESSION_DEPTH * 2) {
        expr = format!("case_when(c{i} ~ {expr}, .default = 0)");
    }
    expect_depth_error(&format!("data %>% mutate(d = {expr})"));
}

#[test]
fn long_addition_chain_above_the_limit_is_rejected() {
    // The parser loop never recurses here, so only the AST depth check catches it.
    expect_depth_error(&long_addition_chain(MAX_EXPRESSION_DEPTH * 4));
}

#[test]
fn long_flat_pipeline_is_not_a_depth_error() {
    let mut code = String::from("data");
    for i in 0..200 {
        code.push_str(&format!(" %>% mutate(c{i} = x + {i})"));
    }
    parse(&code).unwrap_or_else(|e| panic!("long flat pipeline must parse: {e:?}"));
}

#[test]
fn wide_function_call_is_not_a_depth_error() {
    let args = (0..200)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    parse(&format!("data %>% mutate(d = coalesce({args}))"))
        .unwrap_or_else(|e| panic!("wide call must parse: {e:?}"));
}

#[test]
fn parens_inside_string_literals_do_not_count() {
    let deep_string = "(".repeat(MAX_EXPRESSION_DEPTH * 3);
    parse(&format!("data %>% mutate(d = '{deep_string}')"))
        .unwrap_or_else(|e| panic!("parentheses in a string are literal text: {e:?}"));
}

#[test]
fn legacy_transpile_reports_the_depth_error() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    let result = transpiler.transpile(&nested_parens(MAX_EXPRESSION_DEPTH * 4));
    assert!(matches!(result, Err(TranspileError::ParseError(_))));
}

#[test]
fn schema_aware_transpile_reports_the_depth_error() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    let schema = SourceSchema::new("data", vec!["x"]);
    let code = long_addition_chain(MAX_EXPRESSION_DEPTH * 4);
    let result = transpiler.transpile_with_schema(&code, &schema);
    assert!(matches!(result, Err(TranspileError::ParseError(_))));
}

/// The pre-fix behavior was a stack overflow, which aborts the process rather
/// than unwinding. Run the real CLI in a child process so a regression shows up
/// as a signal instead of taking the test harness down with it.
#[test]
fn cli_exits_with_an_error_instead_of_crashing() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_libdplyr"))
        .arg("--text")
        .arg(nested_parens(MAX_EXPRESSION_DEPTH * 4))
        .output()
        .expect("failed to run the CLI");

    assert!(
        !output.status.success(),
        "deep input must fail, but the CLI succeeded"
    );
    assert!(
        output.status.code().is_some(),
        "deep input must exit with a code, not a crash signal: {:?}",
        output.status
    );
}

#[test]
fn schema_cli_exits_with_an_error_instead_of_crashing() {
    let schema = tempfile::NamedTempFile::new().expect("temp schema file");
    std::fs::write(
        schema.path(),
        r#"{"source":"data","columns":[{"name":"x"}]}"#,
    )
    .expect("write schema");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_libdplyr"))
        .arg("--text")
        .arg(nested_parens(MAX_EXPRESSION_DEPTH * 4))
        .arg("--schema")
        .arg(schema.path())
        .output()
        .expect("failed to run the CLI");

    assert!(
        !output.status.success(),
        "deep input must fail with a schema too, but the CLI succeeded"
    );
    assert!(
        output.status.code().is_some(),
        "deep input must exit with a code, not a crash signal: {:?}",
        output.status
    );
}
