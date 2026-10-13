//! End-to-end function behavior through the parser and schema compiler.
use libdplyr::{
    DuckDbDialect, MySqlDialect, PostgreSqlDialect, SourceSchema, SqlDialect, SqliteDialect,
    Transpiler,
};
use serde_json::json;

#[path = "../src/sql_generator/tests/parity_sqlite.rs"]
mod sqlite;

fn compile(dialect: Box<dyn SqlDialect>, code: &str) -> Result<String, libdplyr::TranspileError> {
    Transpiler::new(dialect)
        .transpile_with_schema(
            code,
            &SourceSchema::new("data", ["id", "g", "x", "y", "flag", "label", "date_value"]),
        )
        .map(|query| query.sql)
}

fn sql(code: &str) -> String {
    compile(Box::new(SqliteDialect::new()), code).expect("supported function query")
}

#[test]
fn parity_ranking_nulls_and_partition_sizes() {
    let query = sql("data %>% group_by(g) %>% mutate(r = min_rank(x), d = dense_rank(x), p = percent_rank(x), c = cume_dist(x), t = ntile(x, 3)) %>% ungroup() %>% select(id, r, d, p, c, t) %>% arrange(id)");
    let result = sqlite::execute(&query, false);
    assert_eq!(result[0][1], json!(3));
    assert_eq!(result[0][2], json!(2));
    assert!((result[0][3].as_f64().expect("percent rank") - 2.0 / 3.0).abs() < 1e-12);
    assert_eq!(result[0][4], json!(0.75));
    assert_eq!(result[0][5], json!(2));
    assert_eq!(result[3], json!([4, null, null, null, null, null]));
    assert_eq!(result[5], json!([6, null, null, null, null, null]));
    assert_eq!(result[6][3], json!(0.0));
    assert_eq!(result[6][4], json!(1.0));
}

#[test]
fn parity_descending_rank() {
    let query = sql("data %>% group_by(g) %>% mutate(r = min_rank(desc(x))) %>% ungroup() %>% select(id, r) %>% arrange(id)");
    assert_eq!(
        sqlite::execute(&query, false),
        json!([
            [1, 2],
            [2, 3],
            [3, 3],
            [4, null],
            [5, 1],
            [6, null],
            [7, 1],
            [8, 1]
        ])
    );
}

#[test]
fn parity_cumulative_and_whole_partition_aggregates() {
    let query = sql("data %>% group_by(g) %>% window_order(id) %>% mutate(s = cumsum(x), a = cummean(x), lo = cummin(x), hi = cummax(x), total = sum(x)) %>% ungroup() %>% select(id, s, a, lo, hi, total) %>% arrange(id)");
    let result = sqlite::execute(&query, false);
    assert_eq!(result[0], json!([1, 20, 20.0, 20, 20, 70]));
    assert_eq!(result[1], json!([2, 30, 15.0, 10, 20, 70]));
    assert_eq!(result[4], json!([5, 70, 17.5, 10, 30, 70]));
    assert_eq!(result[5], json!([6, null, null, null, null, 10]));
    assert_eq!(result[7], json!([8, 10, 5.0, 5, 5, 10]));
    assert!(query.contains("ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW"));
    assert!(query.contains("ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING"));
}

#[test]
fn parity_explicit_rows_frame() {
    let query = sql("data %>% group_by(g) %>% window_order(id) %>% window_frame(-1, 0) %>% mutate(s = sum(x)) %>% ungroup() %>% select(id, s) %>% arrange(id)");
    assert_eq!(
        sqlite::execute(&query, false),
        json!([
            [1, 20],
            [2, 30],
            [3, 20],
            [4, 10],
            [5, 30],
            [6, null],
            [7, 5],
            [8, 10]
        ])
    );
}

#[test]
fn parity_nested_lag_and_lead_use_pipeline_order() {
    let query = sql("data %>% group_by(g) %>% window_order(id) %>% mutate(previous = coalesce(lag(x), -1), next = lead(x)) %>% ungroup() %>% select(id, previous, next) %>% arrange(id)");
    assert_eq!(
        sqlite::execute(&query, false),
        json!([
            [1, -1, 10],
            [2, 20, 10],
            [3, 10, null],
            [4, 10, 30],
            [5, -1, null],
            [6, -1, 5],
            [7, -1, 5],
            [8, 5, null]
        ])
    );
}

#[test]
fn parity_conditionals_have_three_null_branches() {
    let query = sql("data %>% mutate(a = ifelse(flag, 1, 0), b = if_else(flag, 1, 0), c = if_else(flag, 1, 0, missing = -1)) %>% select(id, a, b, c) %>% arrange(id)");
    assert_eq!(
        sqlite::execute(&query, false),
        json!([
            [1, 1, 1, 1],
            [2, 0, 0, 0],
            [3, null, null, -1],
            [4, null, null, -1],
            [5, 1, 1, 1],
            [6, null, null, -1],
            [7, 1, 1, 1],
            [8, 0, 0, 0]
        ])
    );
}

#[test]
fn parity_nzchar_default_and_keep_na() {
    let query = sql("data %>% mutate(a = nzchar(label), b = nzchar(label, keepNA = TRUE)) %>% select(id, a, b) %>% arrange(id)");
    assert_eq!(
        sqlite::execute(&query, false),
        json!([
            [1, 0, 0],
            [2, 1, 1],
            [3, 1, null],
            [4, 1, 1],
            [5, 1, null],
            [6, 1, null],
            [7, 1, 1],
            [8, 0, 0]
        ])
    );
}

#[test]
fn parity_null_helpers_date_and_native_function() {
    let query = sql("data %>% mutate(a = na_if(x, y), b = is.null(x), d = as.Date(date_value), n = length(label)) %>% select(id, a, b, d, n) %>% arrange(id)");
    let rows = sqlite::execute(&query, false);
    assert_eq!(rows[0], json!([1, null, 0, "2026-01-02", 0]));
    assert_eq!(rows[1], json!([2, 10, 0, "2026-01-02", 2]));
    assert_eq!(rows[3], json!([4, null, 1, null, 1]));
}

#[test]
fn parity_statistics_match_backend_capabilities() {
    for dialect in [
        Box::new(PostgreSqlDialect::new()) as Box<dyn SqlDialect>,
        Box::new(MySqlDialect::new()),
        Box::new(DuckDbDialect::new()),
    ] {
        let query = compile(
            dialect,
            "data %>% summarise(s = sd(x, na.rm = TRUE), v = var(x, na.rm = TRUE))",
        )
        .expect("supported statistics");
        assert!(query.contains("STDDEV_SAMP("), "{query}");
        assert!(query.contains("VAR_SAMP("), "{query}");
    }
    let q = sql("data %>% summarise(s = sd(x), v = var(x))");
    let rows = sqlite::execute(&q, false);
    let values = [20.0, 10.0, 10.0, 30.0, 5.0, 5.0];
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance =
        values.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (values.len() - 1) as f64;
    assert!((rows[0][0].as_f64().expect("sd") - variance.sqrt()).abs() < 1e-10);
    assert!((rows[0][1].as_f64().expect("variance") - variance).abs() < 1e-10);
    sql("data %>% mutate(v = var(x))");
}

#[test]
fn parity_invalid_argument_and_window_errors() {
    for code in [
        "data %>% mutate(x = ifelse(flag))",
        "data %>% mutate(x = if_else(flag, 1))",
        "data %>% mutate(x = if_else(flag, 1, 0, unexpected = 7))",
        "data %>% mutate(x = na_if(x))",
        "data %>% mutate(x = cumsum(x))",
        "data %>% mutate(x = cummean(x))",
        "data %>% mutate(x = cummin(x))",
        "data %>% mutate(x = cummax(x))",
    ] {
        assert!(
            compile(Box::new(SqliteDialect::new()), code).is_err(),
            "{code}"
        );
    }
}

#[test]
fn parity_retains_the_three_r_contracts() {
    let distinct = sql("data %>% summarise(n = n_distinct(x))");
    assert_eq!(sqlite::execute(&distinct, false), json!([[5]]));
    assert_eq!(sqlite::execute(&distinct, true), json!([[0]]));
    let division = sql("data %>% summarise(r = sum(x) / n())");
    assert_eq!(sqlite::execute(&division, false), json!([[10.0]]));
    let division = sql("data %>% filter(id <= 3) %>% summarise(r = sum(x) / n())");
    let fractional = sqlite::execute(&division, false);
    assert!((fractional[0][0].as_f64().expect("floating division") - 40.0 / 3.0).abs() < 1e-12);
    let constant = sql("data %>% summarise(z = 7)");
    assert_eq!(sqlite::execute(&constant, false), json!([[7]]));
    assert_eq!(sqlite::execute(&constant, true), json!([[7]]));
    let membership = sql("data %>% summarise(z = sum(x) %in% c())");
    assert_eq!(sqlite::execute(&membership, true), json!([[0]]));
}

#[test]
fn parity_modulo_stays_an_expression_call() {
    let query = sql("data %>% mutate(r = x %% 3) %>% select(id, r) %>% arrange(id)");
    assert_eq!(
        sqlite::execute(&query, false),
        json!([
            [1, 2.0],
            [2, 1.0],
            [3, 1.0],
            [4, null],
            [5, 0.0],
            [6, null],
            [7, 2.0],
            [8, 2.0]
        ])
    );
}
