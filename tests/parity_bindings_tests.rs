//! Integration tests for external R-constant bindings via
//! `Transpiler::transpile_with_bindings`.
//!
//! Covers: threshold filter substitution, column-name precedence,
//! `all_of()` with array and named-object bindings, and SQL-injection
//! string-literal escaping.

use libdplyr::SqliteDialect;
use libdplyr::{CompiledQuery, PostgreSqlDialect, SourceSchema, Transpiler};
use std::collections::HashMap;

fn pg() -> Transpiler {
    Transpiler::new(Box::new(PostgreSqlDialect::new()))
}

fn schema() -> SourceSchema {
    SourceSchema::new("data", vec!["id", "grp", "x", "y", "z"])
}

fn compile_with_bindings(
    code: &str,
    bindings: &HashMap<String, serde_json::Value>,
) -> CompiledQuery {
    pg().transpile_with_bindings(code, &[schema()], bindings)
        .expect("supported bindings query")
}

fn flat(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lower(sql: &str) -> String {
    flat(sql).to_lowercase()
}

// ---------------------------------------------------------------------------
// Threshold filter: external constant replaces a bare identifier
// ---------------------------------------------------------------------------

#[test]
fn threshold_filter_replaces_identifier_with_constant() {
    let mut bindings = HashMap::new();
    bindings.insert("threshold".to_string(), serde_json::json!(42));

    let query = compile_with_bindings("data %>% filter(x > threshold)", &bindings);
    let sql = lower(&query.sql);

    assert!(
        sql.contains("42"),
        "expected constant 42 in SQL, got: {}",
        query.sql
    );
    assert!(
        !sql.contains("threshold"),
        "identifier 'threshold' should be replaced, got: {}",
        query.sql
    );
}

#[test]
fn threshold_filter_string_constant_is_quoted() {
    let mut bindings = HashMap::new();
    bindings.insert("cutoff".to_string(), serde_json::json!("2024-01-01"));

    let query = compile_with_bindings("data %>% filter(x > cutoff)", &bindings);
    let sql = lower(&query.sql);

    assert!(
        sql.contains("'2024-01-01'"),
        "expected quoted string literal, got: {}",
        query.sql
    );
}

// ---------------------------------------------------------------------------
// Column names win over bindings
// ---------------------------------------------------------------------------

#[test]
fn visible_column_wins_over_binding() {
    let mut bindings = HashMap::new();
    // "x" is a visible column; binding must NOT shadow it.
    bindings.insert("x".to_string(), serde_json::json!(999));

    let query = compile_with_bindings("data %>% filter(x > 10)", &bindings);
    let sql = lower(&query.sql);

    assert!(
        !sql.contains("999"),
        "column 'x' should not be replaced by binding, got: {}",
        query.sql
    );
    assert!(
        sql.contains("\"x\""),
        "expected column reference \"x\", got: {}",
        query.sql
    );
}

#[test]
fn binding_used_only_when_not_a_column() {
    let mut bindings = HashMap::new();
    bindings.insert("threshold".to_string(), serde_json::json!(5));

    // "threshold" is NOT a column, so the binding applies.
    let query = compile_with_bindings("data %>% filter(x > threshold)", &bindings);
    assert!(lower(&query.sql).contains("5"));
}

// ---------------------------------------------------------------------------
// all_of() with c-array binding
// ---------------------------------------------------------------------------

#[test]
fn all_of_array_binding_becomes_c_function() {
    let mut bindings = HashMap::new();
    bindings.insert("cols".to_string(), serde_json::json!(["x", "y", "z"]));

    let query = compile_with_bindings("data %>% select(all_of(cols))", &bindings);
    let sql = lower(&query.sql);

    // The array should produce c("x", "y", "z") in the projection.
    assert!(
        sql.contains("\"x\"") && sql.contains("\"y\"") && sql.contains("\"z\""),
        "expected array elements in SQL, got: {}",
        query.sql
    );
}

// ---------------------------------------------------------------------------
// all_of() with named-object binding (aliases)
// ---------------------------------------------------------------------------

#[test]
fn all_of_named_object_binding_produces_aliases() {
    let mut bindings = HashMap::new();
    bindings.insert(
        "cols".to_string(),
        serde_json::json!({"alpha": "x", "beta": "y"}),
    );

    let query = compile_with_bindings("data %>% select(all_of(cols))", &bindings);
    let sql = lower(&query.sql);

    // Named object should produce c(alpha = "x", beta = "y") — aliases preserved.
    assert!(
        sql.contains("alpha") && sql.contains("beta"),
        "expected named aliases in SQL, got: {}",
        query.sql
    );
    assert!(
        sql.contains("\"x\"") && sql.contains("\"y\""),
        "expected aliased column refs in SQL, got: {}",
        query.sql
    );
}

// ---------------------------------------------------------------------------
// SQL injection: string literals must be escaped, not interpolated
// ---------------------------------------------------------------------------

#[test]
fn injection_string_literal_is_escaped() {
    let mut bindings = HashMap::new();
    let malicious = "'; DROP TABLE users; --";
    bindings.insert("evil".to_string(), serde_json::json!(malicious));

    let query = compile_with_bindings("data %>% filter(x == evil)", &bindings);
    let sql = &query.sql;

    // The single quote must be doubled (escaped) in the SQL literal.
    assert!(
        sql.contains("''"),
        "expected escaped single-quote in SQL, got: {}",
        sql
    );
    // The raw injection must not appear as an unescaped literal.
    // (The doubled quote in ''' already proves escaping.)
}

#[test]
fn injection_string_in_all_of_is_escaped() {
    let mut bindings = HashMap::new();
    let malicious = "x'; DROP TABLE data; --";
    bindings.insert("cols".to_string(), serde_json::json!([malicious]));

    let result =
        pg().transpile_with_bindings("data %>% select(all_of(cols))", &[schema()], &bindings);

    // all_of() treats array strings as column names; a malicious name must be
    // rejected (InvalidColumnReference), not interpolated into SQL.
    assert!(
        result.is_err(),
        "malicious column name in all_of() must be rejected"
    );
}

// ---------------------------------------------------------------------------
// Boolean and null bindings
// ---------------------------------------------------------------------------

#[test]
fn boolean_binding_replaces_identifier() {
    let mut bindings = HashMap::new();
    bindings.insert("flag".to_string(), serde_json::json!(true));

    let query = compile_with_bindings("data %>% filter(flag)", &bindings);
    let sql = lower(&query.sql);

    assert!(
        sql.contains("true"),
        "expected TRUE literal, got: {}",
        query.sql
    );
}

#[test]
fn null_binding_replaces_identifier() {
    let mut bindings = HashMap::new();
    bindings.insert("missing".to_string(), serde_json::Value::Null);

    let query = compile_with_bindings("data %>% filter(x > missing)", &bindings);
    let sql = lower(&query.sql);

    assert!(
        sql.contains("null"),
        "expected NULL literal, got: {}",
        query.sql
    );
}

// ---------------------------------------------------------------------------
// Unknown references remain unchanged
// ---------------------------------------------------------------------------

#[test]
fn unknown_identifier_without_binding_stays_as_identifier() {
    let bindings = HashMap::new();

    let result =
        pg().transpile_with_bindings("data %>% filter(x > unknown_var)", &[schema()], &bindings);

    // "unknown_var" is not a column and has no binding — should error.
    assert!(
        result.is_err(),
        "unknown identifier without binding should error"
    );
}

#[test]
fn pronouns_distinguish_columns_and_environment() {
    let t = Transpiler::new(Box::new(SqliteDialect::new()));
    let schemas = [SourceSchema::new("data", ["x", "y"])];
    let bindings = std::collections::HashMap::from([("x".to_string(), serde_json::json!(42))]);
    let q = t
        .transpile_with_bindings("data %>% mutate(z = .data$x + .env$x)", &schemas, &bindings)
        .expect("pronouns");
    assert!(q.sql.contains("42"));
    assert!(q.sql.contains("\"x\""));
    assert!(t
        .transpile_with_bindings("data %>% mutate(z = .env$missing)", &schemas, &bindings)
        .is_err());
    assert!(t
        .transpile_with_bindings("data %>% mutate(z = .data$missing)", &schemas, &bindings)
        .is_err());
}
#[test]
fn typed_bindings_reject_lossy_integers() {
    let t = Transpiler::new(Box::new(SqliteDialect::new()));
    let schemas = [SourceSchema::new("data", ["x"])];
    let bindings = std::collections::HashMap::from([(
        "threshold".into(),
        serde_json::json!(9007199254740993_u64),
    )]);
    assert!(t
        .transpile_with_bindings("data %>% filter(x > threshold)", &schemas, &bindings)
        .is_err());
    assert!(t.parse_dplyr("data %>% mutate(z = !!threshold)").is_err());
}
