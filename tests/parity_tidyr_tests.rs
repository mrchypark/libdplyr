//! Result regressions run in parity_execution.py against four real engines.
use libdplyr::{SourceSchema, SqliteDialect, Transpiler};
use serde_json::json;

#[path = "../src/sql_generator/tests/parity_sqlite.rs"]
mod sqlite;

fn compile(code: &str) -> libdplyr::CompiledQuery {
    Transpiler::new(Box::new(SqliteDialect::new()))
        .transpile_with_schema(code, &SourceSchema::new("data", ["id", "grp", "x", "y"]))
        .expect("tidyr query")
}

fn compile_execution(code: &str) -> libdplyr::CompiledQuery {
    Transpiler::new(Box::new(SqliteDialect::new()))
        .transpile_with_schema(
            code,
            &SourceSchema::new("data", ["id", "g", "x", "y", "flag", "label", "date_value"]),
        )
        .expect("tidyr query")
}
#[test]
fn long_pivot_projects_union_outputs_once() {
    let q = compile("data %>% pivot_longer(c(x,y), names_to = 'key', values_to = 'value')");
    assert_eq!(
        q.columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "grp", "key", "value"]
    );
    assert!(q.sql.contains("UNION ALL"));
    assert!(q.sql.starts_with(
        "SELECT \"id\" AS \"id\", \"grp\" AS \"grp\", \"key\" AS \"key\", \"value\" AS \"value\""
    ));
}
#[test]
fn fill_preserves_stage_and_order_requirements() {
    for direction in ["down", "up", "downup", "updown"] {
        compile(&format!(
            "data %>% filter(id > 0) %>% arrange(id) %>% fill(y, .direction = '{direction}')"
        ));
    }
    let t = Transpiler::new(Box::new(SqliteDialect::new()));
    let s = SourceSchema::new("data", ["id", "x"]);
    for code in [
        "data %>% fill(x)",
        "data %>% arrange(id) %>% fill()",
        "data %>% pivot_wider(names_from = id, values_from = x)",
    ] {
        assert!(t.transpile_with_schema(code, &s).is_err());
    }
}
#[test]
fn pivot_literal_keys_are_escaped_and_empty_discovery_is_valid() {
    let q =
        compile("data %>% pivot_wider(names_from = grp, values_from = x, keys = c(\"a'quoted\"))");
    assert!(q.sql.contains("a''quoted"));
    let q = compile("data %>% pivot_wider(names_from = grp, values_from = x, keys = c())");
    assert_eq!(q.columns.len(), 2);
    assert!(q.sql.contains("DISTINCT"));
}

#[test]
fn pivot_wider_values_fill_only_fills_absent_cells() {
    let query = compile_execution(
        "data %>% pivot_wider(names_from = label, values_from = x, keys = c('x', 'hi'), values_fill = 0, id_cols = g) %>% arrange(g)",
    );
    assert_eq!(
        sqlite::execute(&query.sql, false),
        json!([['a', null, 10], ['b', 0, 0]])
    );

    let query = compile_execution(
        "data %>% pivot_wider(names_from = label, values_from = x, keys = c('x', 'hi'), values_fill = 9, values_fn = n, id_cols = g) %>% arrange(g)",
    );
    assert_eq!(
        sqlite::execute(&query.sql, false),
        json!([['a', 1, 1], ['b', 9, 9]])
    );
}
