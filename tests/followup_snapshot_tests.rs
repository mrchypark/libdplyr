use libdplyr::{SourceSchema, SqliteDialect, Transpiler};

#[test]
fn across_overwrites_read_one_input_stage() {
    let compiler = Transpiler::new(Box::new(SqliteDialect::new()));
    let schema = SourceSchema::new("data", vec!["x", "y"]);
    let compiled = compiler
        .transpile_with_schema("data %>% mutate(across(c(x, y), ~ .x + x))", &schema)
        .expect("across must compile");
    assert_eq!(compiled.columns.len(), 2);
    assert_eq!(compiled.stages, 2, "one atomic across projection");
}

#[test]
fn schema_free_api_rejects_schema_dependent_syntax() {
    let compiler = Transpiler::new(Box::new(SqliteDialect::new()));
    for operation in [
        "select(-x)",
        "select(1:2)",
        "select(everything())",
        "mutate(across(x, round))",
        "summarise(across(x, sum))",
        "slice_min(x)",
        "left_join(other, by = \"x\", na_matches = \"na\")",
    ] {
        assert!(
            compiler
                .transpile(&format!("data %>% {operation}"))
                .is_err(),
            "{operation}"
        );
    }
}
