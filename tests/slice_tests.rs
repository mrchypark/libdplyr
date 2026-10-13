//! Behavior tests for slice_min/slice_max/slice_sample lowering.
//!
//! Assertions cover observable structure (output columns, window shape,
//! ordering, leakage, errors), never exact pretty-printed SQL.

use libdplyr::relational::SourceSchema;
use libdplyr::{
    CompiledQuery, DuckDbDialect, MySqlDialect, PostgreSqlDialect, TranspileError, Transpiler,
};

fn schema() -> SourceSchema {
    SourceSchema::new("data", vec!["id", "grp", "x", "y"])
}

fn pg() -> Transpiler {
    Transpiler::new(Box::new(PostgreSqlDialect::new()))
}

fn compile(code: &str) -> Result<CompiledQuery, TranspileError> {
    pg().transpile_with_schema(code, &schema())
}

fn columns(q: &CompiledQuery) -> Vec<&str> {
    q.columns.iter().map(|c| c.name.as_str()).collect()
}

fn names(q: &CompiledQuery, name: &str) -> usize {
    columns(q).iter().filter(|c| **c == name).count()
}

fn flat(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn where_count(sql: &str) -> usize {
    sql.lines()
        .filter(|line| line.starts_with("WHERE "))
        .count()
}

fn lower(sql: &str) -> String {
    flat(sql).to_lowercase()
}

/// Errors raised by the relational planner. Parse-level rejections are the
/// parser's own contract and are covered there.
fn expect_generation_error(code: &str) {
    match compile(code) {
        Ok(q) => panic!("expected error for {code:?}, got:\n{}", flat(&q.sql)),
        Err(TranspileError::GenerationError(_)) => {}
        Err(other) => panic!("expected GenerationError for {code:?}, got {other:?}"),
    }
}

/// The rank column belongs to the slice stage only.
fn assert_no_internal_columns(q: &CompiledQuery) {
    assert!(
        !columns(q).iter().any(|c| c.starts_with("__libdplyr")),
        "internal column leaked: {:?}",
        columns(q)
    );
}

#[test]
fn slice_min_keeps_n_rows_per_group_with_row_number() {
    let q = compile("data %>% group_by(grp) %>% slice_min(x, n = 2, with_ties = FALSE)")
        .expect("slice_min");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y"]);
    assert_no_internal_columns(&q);
    let sql = lower(&q.sql);
    assert!(sql.contains("row_number() over"), "sql: {}", flat(&q.sql));
    assert!(
        sql.contains("partition by \"grp\""),
        "sql: {}",
        flat(&q.sql)
    );
    assert!(
        sql.contains("order by case when \"x\" is null"),
        "sql: {}",
        flat(&q.sql)
    );
    assert!(sql.contains("<= 2"), "sql: {}", flat(&q.sql));
}

#[test]
fn slice_min_defaults_to_tied_ranking() {
    let q = compile("data %>% slice_min(x, n = 2)").expect("default ties");
    assert!(
        lower(&q.sql).contains("rank() over"),
        "sql: {}",
        flat(&q.sql)
    );
}

#[test]
fn slice_max_reverses_the_order_and_keeps_n() {
    let q = compile("data %>% slice_max(x, n = 3, with_ties = FALSE)").expect("slice_max");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y"]);
    let sql = flat(&q.sql);
    assert!(sql.contains("\"x\" DESC"), "sql: {}", flat(&q.sql));
    assert!(sql.contains("<= 3"), "sql: {}", flat(&q.sql));
}

#[test]
fn slice_max_keeps_nulls_last_when_reversed() {
    let q = compile("data %>% slice_max(x, n = 2, na_rm = FALSE, with_ties = FALSE)")
        .expect("nulls last");
    assert!(
        lower(&q.sql).contains("case when \"x\" is null then 1 else 0 end"),
        "sql: {}",
        flat(&q.sql)
    );
}

#[test]
fn slice_max_with_ties_and_prop_uses_cume_dist() {
    let q = compile("data %>% slice_max(x, prop = 0.5, with_ties = TRUE)").expect("cume_dist");
    let sql = lower(&q.sql);
    assert!(sql.contains("cume_dist() over"), "sql: {}", flat(&q.sql));
    // WITH TIES compares the fraction directly, so no count window is needed.
    assert!(!sql.contains("count(*) over"), "sql: {}", flat(&q.sql));
    assert!(sql.contains("<= 0.5"), "sql: {}", flat(&q.sql));
}

#[test]
fn slice_min_accepts_prop() {
    let q = compile("data %>% group_by(grp) %>% slice_min(x, prop = 0.5, with_ties = FALSE)")
        .expect("prop");
    let sql = lower(&q.sql);
    assert!(sql.contains("count(*) over"), "sql: {}", flat(&q.sql));
    assert!(sql.contains("* "), "sql: {}", flat(&q.sql));
    assert!(sql.contains("0.5"), "sql: {}", flat(&q.sql));
}

#[test]
fn slice_by_partitions_without_persisting_grouping() {
    let q =
        compile("data %>% slice_min(x, n = 1, by = grp) %>% summarise(n = n())").expect("slice by");
    // by is temporary: the later global summarise still sees every group.
    assert_eq!(columns(&q), vec!["n"]);
    assert!(
        lower(&q.sql).contains("partition by \"grp\""),
        "sql: {}",
        flat(&q.sql)
    );
}

#[test]
fn slice_by_accepts_a_column_vector() {
    let q = compile("data %>% slice_min(x, n = 1, by = c(grp, id))").expect("slice by vector");
    let sql = lower(&q.sql);
    assert!(
        sql.contains("partition by \"grp\", \"id\""),
        "sql: {}",
        flat(&q.sql)
    );
}

#[test]
fn slice_by_on_a_grouped_relation_is_rejected() {
    expect_generation_error("data %>% group_by(grp) %>% slice_min(x, n = 1, by = y)");
}

#[test]
fn na_rm_filters_null_keys_before_ranking() {
    let q =
        compile("data %>% slice_min(x, n = 2, na_rm = TRUE, with_ties = FALSE)").expect("na_rm");
    assert!(
        lower(&q.sql).contains(r#"case when ("x" is null) then false else true end"#),
        "sql: {}",
        flat(&q.sql)
    );
    // The NULL filter belongs to the ranking stage, not the outer keep filter.
    assert_eq!(where_count(&q.sql), 2, "sql: {}", flat(&q.sql));
}

#[test]
fn without_na_rm_nulls_sort_last_and_still_take_a_slot() {
    let q = compile("data %>% slice_min(x, n = 2, na_rm = FALSE, with_ties = FALSE)")
        .expect("nulls last");
    assert!(
        !lower(&q.sql).contains("then false else true end"),
        "na_rm = FALSE must not filter NULL keys: {}",
        flat(&q.sql)
    );
    assert_eq!(where_count(&q.sql), 1, "sql: {}", flat(&q.sql));
}

#[test]
fn sample_orders_randomly_and_keeps_n() {
    let q = compile("data %>% slice_sample(n = 5)").expect("sample n");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y"]);
    assert!(
        lower(&q.sql).contains("row_number() over (order by random() asc)"),
        "sql: {}",
        flat(&q.sql)
    );
    assert!(lower(&q.sql).contains("<= 5"), "sql: {}", flat(&q.sql));
}

#[test]
fn sample_supports_grouped_slices() {
    let q = compile("data %>% group_by(grp) %>% slice_sample(n = 2)").expect("grouped sample");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y"]);
    let sql = lower(&q.sql);
    assert!(
        sql.contains("row_number() over (partition by \"grp\" order by random() asc)"),
        "sql: {}",
        flat(&q.sql)
    );
}

#[test]
fn sample_uses_rand_on_mysql() {
    let mysql = Transpiler::new(Box::new(MySqlDialect::new()));
    let q = mysql
        .transpile_with_schema("data %>% slice_sample(n = 2)", &schema())
        .expect("mysql sample");
    assert!(flat(&q.sql).contains("RAND()"), "sql: {}", flat(&q.sql));
}

#[test]
fn sample_prop_uses_floor_of_group_count() {
    let q = compile("data %>% slice_sample(prop = 0.25)").expect("sample prop");
    let sql = lower(&q.sql);
    // The count window must stay a window: a bare COUNT(*) would collapse the stage.
    assert!(
        sql.contains(r#"count(*) over () as "__libdplyr_slice_count""#),
        "sql: {}",
        flat(&q.sql)
    );
    assert!(
        sql.contains("(0.25 * \"__libdplyr_slice_count\")"),
        "sql: {}",
        flat(&q.sql)
    );
}

#[test]
fn prop_of_zero_keeps_no_rows() {
    let q = compile("data %>% slice_min(x, prop = 0, with_ties = FALSE)").expect("zero prop");
    assert!(
        lower(&q.sql).contains("<= (0 * \"__libdplyr_slice_count\")"),
        "sql: {}",
        flat(&q.sql)
    );
}

#[test]
fn prop_above_one_is_accepted_and_floors_to_the_group_size() {
    let q = compile("data %>% slice_min(x, prop = 2, with_ties = FALSE)").expect("prop > 1");
    assert!(lower(&q.sql).contains("2"), "sql: {}", flat(&q.sql));
}

#[test]
fn slice_rejects_an_unknown_order_column() {
    expect_generation_error("data %>% slice_min(nope, n = 1)");
}

#[test]
fn slice_rejects_an_aggregate_order_expression() {
    expect_generation_error("data %>% slice_min(n_distinct(x), n = 1)");
    expect_generation_error("data %>% slice_min(row_number(), n = 1)");
}

#[test]
fn slice_rejects_by_that_resolves_to_nothing() {
    expect_generation_error("data %>% slice_min(x, n = 1, by = nope)");
}

#[test]
fn slice_continues_downstream_and_preserves_a_later_sort() {
    let q = compile(
        "data %>% slice_min(x, n = 2, with_ties = FALSE) %>% filter(y > 0) %>% arrange(desc(y))",
    )
    .expect("pipeline after slice");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y"]);
    assert!(lower(&q.sql).contains("order by"), "sql: {}", flat(&q.sql));
}

#[test]
fn slice_survives_a_rename_of_the_order_column() {
    let q = compile("data %>% slice_min(x, n = 2, with_ties = FALSE) %>% rename(v = x)")
        .expect("rename after slice");
    assert_eq!(columns(&q), vec!["id", "grp", "v", "y"]);
    assert_eq!(names(&q, "x"), 0, "old name must be gone");
}

#[test]
fn slice_keeps_grouping_for_a_later_summarise() {
    let q = compile(
        "data %>% group_by(grp) %>% slice_min(x, n = 2, with_ties = FALSE) %>% summarise(n = n())",
    )
    .expect("grouped slice then summarise");
    assert_eq!(columns(&q), vec!["grp", "n"]);
    assert_no_internal_columns(&q);
}

#[test]
fn nested_slices_do_not_collide() {
    let q = compile("data %>% slice_min(x, n = 3, with_ties = FALSE) %>% slice_min(x, n = 2, with_ties = FALSE)")
        .expect("nested");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y"]);
    assert_no_internal_columns(&q);
}

#[test]
fn slice_output_is_identical_on_duckdb() {
    let duck = Transpiler::new(Box::new(DuckDbDialect::new()));
    let q = duck
        .transpile_with_schema(
            "data %>% group_by(grp) %>% slice_min(x, n = 2, with_ties = FALSE)",
            &schema(),
        )
        .expect("duckdb slice_min");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y"]);
    assert!(
        lower(&q.sql).contains("row_number() over"),
        "sql: {}",
        flat(&q.sql)
    );
}
