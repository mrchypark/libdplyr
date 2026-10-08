//! Behavior tests for across() expansion.
//!
//! These run through the public transpiler so they assert the observable
//! output schema. mutate() keeps every input column and adds or overwrites the
//! ones across() names; summarise() keeps grouping columns and adds one column
//! per summary.

use libdplyr::relational::SourceSchema;
use libdplyr::{PostgreSqlDialect, Transpiler};

fn schema() -> SourceSchema {
    SourceSchema::new("data", vec!["id", "grp", "x", "y"])
}

fn compile(code: &str) -> Result<libdplyr::CompiledQuery, libdplyr::TranspileError> {
    Transpiler::new(Box::new(PostgreSqlDialect::new())).transpile_with_schema(code, &schema())
}

fn columns(code: &str) -> Vec<String> {
    compile(code)
        .expect("across must compile")
        .columns
        .into_iter()
        .map(|column| column.name)
        .collect()
}

fn error(code: &str) -> String {
    compile(code)
        .expect_err("across must be rejected")
        .to_string()
}

#[test]
fn identity_across_keeps_every_input_column() {
    assert_eq!(
        columns("data %>% mutate(across(c(x, y)))"),
        ["id", "grp", "x", "y"]
    );
}

#[test]
fn null_fns_is_identity() {
    assert_eq!(
        columns("data %>% mutate(across(c(x, y), NULL))"),
        ["id", "grp", "x", "y"]
    );
}

#[test]
fn bare_function_overwrites_the_column() {
    assert_eq!(
        columns("data %>% mutate(across(c(x, y), round))"),
        ["id", "grp", "x", "y"]
    );
}

#[test]
fn named_cols_with_positional_fns_matches_r() {
    // across(.cols = x, mean) fills the first unmatched slot with `mean`.
    assert_eq!(
        columns("data %>% summarise(across(.cols = x, mean))"),
        ["x"]
    );
}

#[test]
fn unnamed_list_entries_are_numbered() {
    // A summarise() list of aggregates. Labels are positions, not function
    // names, so dplyr would produce x_1 and x_2.
    assert_eq!(
        columns("data %>% summarise(across(c(x), list(sum, mean)))"),
        ["x_1", "x_2"]
    );
}

#[test]
fn named_list_entry_keeps_its_label() {
    assert_eq!(
        columns("data %>% summarise(across(c(x, y), list(total = mean)))"),
        ["x_total", "y_total"]
    );
}

#[test]
fn single_named_list_entry_still_gets_a_label() {
    // list(total = sum) has one entry, but a list always labels its output.
    assert_eq!(
        columns("data %>% summarise(across(c(x), list(total = sum)))"),
        ["x_total"]
    );
}

#[test]
fn mutate_list_keeps_inputs_and_adds_labels() {
    assert_eq!(
        columns("data %>% mutate(across(c(x, y), list(round, floor)))"),
        ["id", "grp", "x", "y", "x_1", "x_2", "y_1", "y_2"]
    );
}

#[test]
fn lambda_substitutes_the_column_name() {
    assert_eq!(
        columns("data %>% mutate(across(c(x, y), ~ .x + id))"),
        ["id", "grp", "x", "y"]
    );
}

#[test]
fn cur_column_renders_a_string_literal() {
    let compiled = compile("data %>% mutate(across(c(x, y), ~ round(.x, nchar(cur_column()))))")
        .expect("cur_column must compile");
    assert_eq!(
        compiled
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "grp", "x", "y"]
    );
    // The column name reaches SQL as a quoted literal, never as raw text.
    assert!(
        compiled.sql.contains("'x'") && compiled.sql.contains("'y'"),
        "cur_column() must render as a literal: {}",
        compiled.sql
    );
}

#[test]
fn names_template_substitutes_col_and_fn() {
    assert_eq!(
        columns(r#"data %>% mutate(across(c(x), list(round), .names = "{.col}_r{.fn}"))"#),
        ["id", "grp", "x", "y", "x_r1"]
    );
}

#[test]
fn names_template_without_fn_placeholder_is_still_valid() {
    assert_eq!(
        columns(r#"data %>% summarise(across(c(x, y), ~ mean(.x), .names = "v{.col}"))"#),
        ["vx", "vy"]
    );
}

#[test]
fn everything_excludes_grouping_columns() {
    assert_eq!(
        columns("data %>% group_by(grp) %>% mutate(across(everything(), ~ .x))"),
        ["id", "grp", "x", "y"]
    );
}

#[test]
fn explicit_group_name_in_the_selection_is_rejected() {
    let message = error("data %>% group_by(grp) %>% mutate(across(c(x, y, grp), ~ .x))");
    assert!(message.contains("grp"), "{message}");
}

#[test]
fn all_expressions_read_one_input_stage() {
    let compiled =
        compile("data %>% mutate(across(c(x, y), ~ .x + id))").expect("across must compile");
    // One stage to read the scan plus one atomic projection for every
    // generated expression.
    assert_eq!(compiled.stages, 2, "one atomic across projection");
}

#[test]
fn summarise_keeps_grouping_columns() {
    assert_eq!(
        columns("data %>% group_by(grp) %>% summarise(across(c(x, y), mean))"),
        ["grp", "x", "y"]
    );
}

#[test]
fn lambda_aggregate_accepts_na_rm_true() {
    assert_eq!(
        columns("data %>% summarise(across(c(x, y), ~ mean(.x, na.rm = TRUE)))"),
        ["x", "y"]
    );
}

#[test]
fn n_distinct_lambda_accepts_na_rm_true() {
    assert_eq!(
        columns("data %>% summarise(across(c(x), ~ n_distinct(.x, na.rm = TRUE)))"),
        ["x"]
    );
}

#[test]
fn list_of_na_rm_lambdas_is_numbered() {
    assert_eq!(
        columns(
            "data %>% summarise(across(c(x), list(~ mean(.x, na.rm = TRUE), ~ n_distinct(.x, na.rm = TRUE))))"
        ),
        ["x_1", "x_2"]
    );
}

#[test]
fn empty_selection_is_a_no_op() {
    // everything() on a fully grouped relation selects nothing, so mutate()
    // keeps the input unchanged.
    assert_eq!(
        columns("data %>% group_by(grp, id, x, y) %>% mutate(across(everything(), ~ .x))"),
        ["id", "grp", "x", "y"]
    );
}

#[test]
fn unknown_names_placeholder_is_rejected() {
    let message = error(r#"data %>% summarise(across(c(x), ~ mean(.x), .names = "{.colid}"))"#);
    assert!(message.contains("not supported"), "{message}");
}

#[test]
fn colliding_names_are_rejected() {
    let message = error(r#"data %>% summarise(across(c(x, y), ~ mean(.x), .names = "same"))"#);
    assert!(message.contains("duplicate output column"), "{message}");
}

#[test]
fn unknown_lambda_variable_is_rejected() {
    let message = error("data %>% mutate(across(c(x, y), ~ .z + 1))");
    assert!(
        message.contains("is not an across() lambda variable"),
        "{message}"
    );
}

#[test]
fn unsupported_across_argument_is_rejected() {
    let message = error("data %>% mutate(across(c(x, y), ~ .x, .ifns = list(round)))");
    assert!(message.contains(".ifns"), "{message}");
}

#[test]
fn named_across_output_is_rejected() {
    let message = error("data %>% mutate(z = across(c(x, y)))");
    assert!(message.contains("unnamed call"), "{message}");
}
