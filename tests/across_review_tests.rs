use libdplyr::relational::SourceSchema;
use libdplyr::{PostgreSqlDialect, Transpiler};

fn columns(code: &str) -> Vec<String> {
    Transpiler::new(Box::new(PostgreSqlDialect::new()))
        .transpile_with_schema(code, &SourceSchema::new("data", vec!["x"]))
        .expect("across must compile")
        .columns
        .into_iter()
        .map(|column| column.name)
        .collect()
}

#[test]
fn plain_fns_use_the_positional_name_in_templates() {
    assert_eq!(
        columns(r#"data %>% summarise(across(x, mean, .names = "{.col}_{.fn}"))"#),
        ["x_1"]
    );
    assert_eq!(
        columns(r#"data %>% summarise(across(x, ~ mean(.x), .names = "{.fn}"))"#),
        ["1"]
    );
    assert_eq!(
        columns(r#"data %>% mutate(across(x, .names = "{.col}_{.fn}"))"#),
        ["x", "x_1"]
    );
    assert_eq!(columns("data %>% summarise(across(x, mean))"), ["x"]);
}
