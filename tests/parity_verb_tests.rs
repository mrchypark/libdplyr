use libdplyr::{CompiledQuery, SourceSchema, SqliteDialect, Transpiler};

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
