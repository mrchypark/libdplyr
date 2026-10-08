//! Result regressions run in parity_execution.py against four real engines.
use libdplyr::{SourceSchema, SqliteDialect, Transpiler};
fn compile(code: &str) -> libdplyr::CompiledQuery {
    Transpiler::new(Box::new(SqliteDialect::new()))
        .transpile_with_schema(code, &SourceSchema::new("data", ["id", "grp", "x", "y"]))
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
