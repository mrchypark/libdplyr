//! Parser regressions for the schema-aware query extensions.
use libdplyr::{SourceSchema, SqliteDialect, Transpiler};
fn transpiler() -> Transpiler {
    Transpiler::new(Box::new(SqliteDialect::new()))
}
fn schema() -> SourceSchema {
    SourceSchema::new("data", ["id", "grp", "x", "y"])
}
#[test]
fn lambdas_and_vector_predicates_compile() {
    for code in [
        "data %>% mutate(across(c(x, y), function(v) v + 1))",
        "data %>% mutate(across(c(x, y), ~ .x + 1))",
        "data %>% filter(if_any(c(x, y), ~ .x > 1))",
        "data %>% filter(if_all(c(x, y), function(v) v > 1))",
        "data %>% filter(x > 0 && y > 0 || id == 1)",
        "data %>% arrange(desc(x + y))",
    ] {
        transpiler()
            .transpile_with_schema(code, &schema())
            .unwrap_or_else(|e| panic!("{code}: {e}"));
    }
}
#[test]
fn relation_pipelines_and_assertions_compile_as_plans() {
    let schemas = [
        schema(),
        SourceSchema::new("other", ["id", "x", "low", "high"]),
    ];
    for code in [
        "data %>% left_join(other %>% filter(x > 1), by = join_by(id), relationship = 'many-to-one')",
        "data %>% left_join(other, by = join_by(closest(x >= x)))",
        "data %>% left_join(other, by = join_by(between(x, low, high)))",
        "data %>% union(other %>% select(id, x))",
        "bind_queries(data, other %>% select(id, x))",
        "data %>% slice_sample(n = 2, weight_by = x, replace = TRUE)",
    ] {transpiler().plan_with_schemas(code,&schemas).unwrap_or_else(|e|panic!("{code}: {e}"));}
}
#[test]
fn invalid_options_and_trailing_input_are_rejected() {
    for code in [
        "data %>% mutate(z = across(x))",
        "data %>% left_join(other, relationship = 'wrong')",
        "data %>% left_join(other, unmatched = 'wrong')",
        "bind_queries(data) junk",
        "data %>% slice_sample(weight = x)",
    ] {
        assert!(
            transpiler().transpile_with_schema(code, &schema()).is_err(),
            "{code}"
        );
    }
}
