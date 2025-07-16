//! Integration tests
//!
//! Tests the complete functionality of libdplyr.

use libdplyr::{Transpiler, PostgreSqlDialect, MySqlDialect, SqliteDialect};

/// Normalizes SQL strings (removes whitespace and unifies case)
fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

#[test]
fn test_simple_select_postgresql() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "select(name, age)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Conversion should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("SELECT"));
    assert!(normalized.contains("\"NAME\""));
    assert!(normalized.contains("\"AGE\""));
    assert!(normalized.contains("FROM"));
}

#[test]
fn test_simple_select_mysql() {
    let transpiler = Transpiler::new(Box::new(MySqlDialect));
    let dplyr_code = "select(name, age)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Conversion should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("SELECT"));
    assert!(normalized.contains("`NAME`"));
    assert!(normalized.contains("`AGE`"));
}

#[test]
fn test_simple_select_sqlite() {
    let transpiler = Transpiler::new(Box::new(SqliteDialect));
    let dplyr_code = "select(name, age)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Conversion should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("SELECT"));
    assert!(normalized.contains("\"NAME\""));
    assert!(normalized.contains("\"AGE\""));
}

#[test]
fn test_filter_operation() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "filter(age > 18)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Conversion should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("WHERE"));
    assert!(normalized.contains("\"AGE\""));
    assert!(normalized.contains(">"));
    assert!(normalized.contains("18"));
}

#[test]
fn test_chained_operations() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "select(name, age) %>% filter(age > 18)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Conversion should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("SELECT"));
    assert!(normalized.contains("\"NAME\""));
    assert!(normalized.contains("\"AGE\""));
    assert!(normalized.contains("WHERE"));
    assert!(normalized.contains(">"));
    assert!(normalized.contains("18"));
}

#[test]
fn test_arrange_operation() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "arrange(desc(age))";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Conversion should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("ORDER BY"));
    assert!(normalized.contains("\"AGE\""));
    assert!(normalized.contains("DESC"));
}

#[test]
fn test_group_by_and_summarise() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "group_by(category) %>% summarise(avg_age = mean(age))";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Conversion should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("GROUP BY"));
    assert!(normalized.contains("\"CATEGORY\""));
    assert!(normalized.contains("AVG"));
    assert!(normalized.contains("\"AGE\""));
}

#[test]
fn test_invalid_syntax() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "invalid_function(test)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_err(), "Invalid syntax should return an error");
}

#[test]
fn test_empty_input() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_err(), "Empty input should return an error");
}

#[test]
fn test_complex_filter_expression() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "filter(age > 18 & name == \"John\")";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "Complex filter expression should succeed: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("WHERE"));
    assert!(normalized.contains("AND"));
    assert!(normalized.contains("\"AGE\""));
    assert!(normalized.contains("\"NAME\""));
    assert!(normalized.contains("="));
    assert!(normalized.contains("'JOHN'"));
}