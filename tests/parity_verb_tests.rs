use libdplyr::{CompiledQuery, SourceSchema, SqliteDialect, Transpiler};
use serde_json::json;

#[path = "../src/sql_generator/tests/parity_sqlite.rs"]
mod sqlite;

fn compile(code: &str) -> CompiledQuery {
    Transpiler::new(Box::new(SqliteDialect::new()))
        .transpile_with_schema(code, &SourceSchema::new("data", ["id", "grp", "x", "y"]))
        .expect("supported parity query")
}

fn names(query: &CompiledQuery) -> Vec<&str> {
    query
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect()
}

#[test]
fn mutate_controls_are_not_output_columns() {
    assert_eq!(
        names(&compile("data %>% mutate(z = x + 1, .keep = \"used\")")),
        ["x", "z"]
    );
    assert_eq!(
        names(&compile("data %>% mutate(z = x + 1, .before = y)")),
        ["id", "grp", "x", "z", "y"]
    );
    assert_eq!(
        names(&compile("data %>% mutate(x = NULL)")),
        ["id", "grp", "y"]
    );
    assert_eq!(names(&compile("data %>% transmute(z = x + y)")), ["z"]);
}

#[test]
fn temporary_groups_do_not_leak() {
    let query = compile("data %>% mutate(z = mean(x), .by = grp) %>% summarise(total = sum(z))");
    assert_eq!(names(&query), ["total"]);
    assert!(query.sql.contains("PARTITION BY"));
}

#[test]
fn groups_keep_affects_the_next_summary() {
    let query = compile("data %>% group_by(grp, id) %>% summarise(z = sum(x), .groups = \"keep\") %>% summarise(total = sum(z))");
    assert_eq!(names(&query), ["grp", "id", "total"]);
}

#[test]
fn basic_query_extensions_preserve_shape() {
    assert_eq!(
        names(&compile("data %>% relocate(y, .before = x)")),
        ["id", "grp", "y", "x"]
    );
    assert_eq!(
        names(&compile(
            "data %>% count(grp, wt = x, sort = TRUE, name = \"total\")"
        )),
        ["grp", "total"]
    );
    assert!(
        compile("data %>% filter(x > 0, y > 0) %>% arrange(desc(x + y)) %>% head(2)")
            .sql
            .contains("LIMIT 2")
    );
}

#[test]
fn grouped_distinct_without_columns_keeps_all_visible_columns() {
    let schema = SourceSchema::new("data", ["id", "g", "x", "y", "flag", "label", "date_value"]);
    for args in ["", ".keep_all = FALSE", ".keep_all = TRUE"] {
        let query = Transpiler::new(Box::new(SqliteDialect::new()))
            .transpile_with_schema(
                &format!("data %>% group_by(g) %>% distinct({args})"),
                &schema,
            )
            .expect("grouped distinct query");
        assert_eq!(
            query
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["id", "g", "x", "y", "flag", "label", "date_value"]
        );
        assert_eq!(
            sqlite::execute(&query.sql, false),
            json!([
                [1, "a", 20, 20, 1, "", "2026-01-02 03:04:05"],
                [2, "a", 10, 99, 0, "hi", "2026-01-02"],
                [3, "a", 10, 10, null, null, null],
                [4, "a", null, null, null, "x", null],
                [5, "a", 30, 30, 1, null, "2026-02-03"],
                [6, "b", null, null, null, null, null],
                [7, "b", 5, 5, 1, "z", "2026-01-04"],
                [8, "b", 5, 5, 0, "", "2026-01-04"]
            ])
        );
        let summary = Transpiler::new(Box::new(SqliteDialect::new()))
            .transpile_with_schema(
                &format!("data %>% group_by(g) %>% distinct({args}) %>% summarise(n = n())"),
                &schema,
            )
            .expect("grouped distinct summary");
        assert_eq!(
            sqlite::execute(&summary.sql, false),
            json!([["a", 5], ["b", 3]])
        );
    }
}
