//! Behavior tests for tidy-select resolution.
//!
//! Assertions cover resolved column order, rename output names, and error
//! behavior. Every case is written in quoted R source so the test states
//! exactly which grammar it exercises.

use libdplyr::pipe_syntax::PipeSyntax;
use libdplyr::relational::{SchemaColumn, SourceSchema};
use libdplyr::{PostgreSqlDialect, TranspileError, Transpiler};

fn schema() -> SourceSchema {
    SourceSchema::with_columns(
        "data",
        vec![
            typed("id", "integer"),
            typed("grp", "character"),
            typed("x", "double"),
            typed("y", "double"),
            typed("flag", "boolean"),
        ],
    )
}

fn typed(name: &str, data_type: &str) -> SchemaColumn {
    SchemaColumn {
        name: name.to_string(),
        data_type: Some(data_type.to_string()),
        nullable: None,
    }
}

/// Compiles `data %>% select(<selection>)` and returns the output names in order.
fn names(selection: &str) -> Result<Vec<String>, TranspileError> {
    let code = format!("data %>% select({selection})");
    let query = transpiler().transpile_with_schema(&code, &schema())?;
    Ok(query
        .columns
        .into_iter()
        .map(|column| column.name)
        .collect())
}

fn transpiler() -> Transpiler {
    Transpiler::with_pipe_syntax(Box::new(PostgreSqlDialect::new()), PipeSyntax::Magrittr)
}

#[test]
fn output_follows_selection_order_not_schema_order() {
    assert_eq!(names("y, x").expect("select() succeeds"), vec!["y", "x"]);
}

#[test]
fn literal_positions_and_names_are_selectors() {
    assert_eq!(names("3, 'id'").expect("selection"), ["x", "id"]);
    assert_eq!(names("-1:-2").expect("negative range"), ["x", "y", "flag"]);
    assert!(names("-1:3").is_err());
    assert!(names("-1.5").is_err());
}

#[test]
fn nested_selection_helpers_fold_in_order() {
    assert_eq!(
        names("c(everything(), -x, -grp)").expect("nested"),
        ["id", "y", "flag"]
    );
    assert_eq!(
        names("c(-grp, x)").expect("initial complement"),
        ["id", "x", "y", "flag"]
    );
}

#[test]
fn regex_case_default_and_where_function_references_work() {
    assert_eq!(names("matches('^X$')").expect("case insensitive"), ["x"]);
    assert_eq!(
        names("id, matches('^X$', ignore.case = FALSE)").expect("case sensitive"),
        ["id"]
    );
    assert_eq!(
        names("where(is.numeric)").expect("type predicate"),
        ["id", "x", "y"]
    );
}

#[test]
fn parameterized_numeric_types_exclude_arrays() {
    let schema = libdplyr::SourceSchema::with_columns(
        "data",
        vec![
            typed("amount", "DECIMAL(12,2)"),
            typed("amounts", "DECIMAL(12,2)[]"),
            typed("items", "DOUBLE[]"),
        ],
    );
    let query = libdplyr::Transpiler::new(Box::new(libdplyr::DuckDbDialect::new()))
        .transpile_with_schema("data %>% select(where(is.numeric))", &schema)
        .expect("type predicate");
    assert_eq!(
        query
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["amount"]
    );
}

#[test]
fn duplicate_names_appear_once() {
    assert_eq!(names("x, y, x").expect("select() succeeds"), vec!["x", "y"]);
}

#[test]
fn rename_exposes_the_new_output_name() {
    assert_eq!(
        names("height = y").expect("select() succeeds"),
        vec!["height"]
    );
}

#[test]
fn negative_first_selection_excludes_from_full_schema() {
    assert_eq!(
        names("-x, -y").expect("select() succeeds"),
        vec!["id", "grp", "flag"]
    );
}

#[test]
fn negative_after_positive_removes_only_from_selection() {
    assert_eq!(
        names("everything(), -x").expect("select() succeeds"),
        vec!["id", "grp", "y", "flag"]
    );
}

#[test]
fn sequential_inclusion_and_exclusion_fold_left_to_right() {
    assert_eq!(
        names(r#"c("x", "y"), -y"#).expect("select() succeeds"),
        vec!["x"]
    );
}

#[test]
fn logical_and_is_intersection() {
    assert_eq!(
        names(r#"c("x", "y") & x"#).expect("select() succeeds"),
        vec!["x"]
    );
    // An intersection with no overlap resolves to nothing; the SQL layer owns
    // the empty-projection error, so pair it with a real column to see the
    // empty set itself.
    assert_eq!(names("id, x & y").expect("select() succeeds"), vec!["id"]);
    assert!(names("x & y").is_err());
}

#[test]
fn logical_or_is_union_in_operand_order() {
    assert_eq!(
        names(r#"starts_with("g") | id"#).expect("select() succeeds"),
        vec!["grp", "id"]
    );
}

#[test]
fn bang_is_complement_against_full_schema() {
    assert_eq!(
        names(r#"!c("id", "grp")"#).expect("select() succeeds"),
        vec!["x", "y", "flag"]
    );
}

#[test]
fn numeric_range_indexes_the_full_input_schema() {
    // Positions 2:3 mean grp and x, not the running selection.
    assert_eq!(
        names(r#"c("grp", "flag"), 2:3"#).expect("select() succeeds"),
        vec!["grp", "flag", "x"]
    );
}

#[test]
fn name_range_selects_by_name() {
    assert_eq!(names("x:grp").expect("select() succeeds"), vec!["x", "grp"]);
}

#[test]
fn reverse_range_keeps_the_written_order() {
    assert_eq!(names("3:2").expect("select() succeeds"), vec!["x", "grp"]);
}

#[test]
fn negative_positions_exclude_rather_than_count_back() {
    assert_eq!(
        names("-1").expect("select() succeeds"),
        vec!["grp", "x", "y", "flag"]
    );
}

#[test]
fn out_of_range_positions_are_rejected() {
    assert!(names("1:9").is_err());
}

#[test]
fn c_with_negative_name_excludes_from_full_schema() {
    assert_eq!(
        names(r#"c(-x)"#).expect("select() succeeds"),
        vec!["id", "grp", "y", "flag"]
    );
}

#[test]
fn helper_prefixes_honour_ignore_case_default() {
    assert_eq!(
        names(r#"starts_with("F")"#).expect("select() succeeds"),
        vec!["flag"]
    );
}

#[test]
fn ignore_case_false_is_case_sensitive() {
    assert_eq!(
        names(r#"id, starts_with("F", ignore.case = FALSE)"#).expect("select() succeeds"),
        vec!["id"]
    );
    assert!(names(r#"starts_with("F", ignore.case = FALSE)"#).is_err());
}

#[test]
fn ends_with_and_contains_match_substrings() {
    assert_eq!(
        names(r#"ends_with("p")"#).expect("select() succeeds"),
        vec!["grp"]
    );
    assert_eq!(
        names(r#"contains("la")"#).expect("select() succeeds"),
        vec!["flag"]
    );
}

#[test]
fn all_of_is_strict() {
    let message = names(r#"all_of(c("x", "nope"))"#).expect_err("unknown name must fail");
    assert!(
        message.to_string().contains("nope"),
        "unexpected message: {message}"
    );
}

#[test]
fn any_of_skips_unknown_names() {
    assert_eq!(
        names(r#"any_of(c("x", "nope"))"#).expect("select() succeeds"),
        vec!["x"]
    );
}

#[test]
fn last_col_on_a_fresh_selection_is_the_final_column() {
    assert_eq!(
        names("last_col()").expect("select() succeeds"),
        vec!["flag"]
    );
}

#[test]
fn last_col_offset_moves_backwards() {
    // Positions count back from the last column: 5 columns, offset 2 is `x`.
    assert_eq!(names("last_col(2)").expect("select() succeeds"), vec!["x"]);
}

#[test]
fn last_col_indexes_the_full_schema_after_a_positive_entry() {
    assert_eq!(
        names("x, last_col()").expect("select() succeeds"),
        vec!["x", "flag"]
    );
}

#[test]
fn last_col_accepts_a_named_offset_and_rejects_extra_arguments() {
    assert_eq!(
        names("last_col(offset = 1)").expect("select() succeeds"),
        vec!["y"]
    );
    assert!(names(r#"last_col(1, "x")"#).is_err());
}

#[test]
fn last_col_out_of_range_is_rejected() {
    assert!(names("last_col(9)").is_err());
}

#[test]
fn where_filters_by_known_data_type() {
    assert_eq!(
        names("where(is.numeric())").expect("select() succeeds"),
        vec!["id", "x", "y"]
    );
    assert_eq!(
        names("where(is.character())").expect("select() succeeds"),
        vec!["grp"]
    );
    assert_eq!(
        names("where(is.logical())").expect("select() succeeds"),
        vec!["flag"]
    );
}

#[test]
fn where_is_integer_and_double_are_distinct() {
    assert_eq!(
        names("where(is.integer())").expect("select() succeeds"),
        vec!["id"]
    );
    assert_eq!(
        names("where(is.double())").expect("select() succeeds"),
        vec!["x", "y"]
    );
}

#[test]
fn where_reports_unknown_metadata_clearly() {
    let bare = SourceSchema::new("data", vec!["x"]);
    let result = transpiler().transpile_with_schema("data %>% select(where(is.numeric()))", &bare);
    let message = result
        .expect_err("missing data types are rejected")
        .to_string();
    assert!(
        message.contains("needs data types") && message.contains("'x'"),
        "unexpected message: {message}"
    );
}

#[test]
fn matches_uses_a_real_regex() {
    assert_eq!(
        names(r#"matches("^x|y$")"#).expect("select() succeeds"),
        vec!["x", "y"]
    );
}

#[test]
fn invalid_regex_is_reported_not_ignored() {
    let message = names(r#"matches("[")"#)
        .expect_err("an invalid pattern must fail")
        .to_string();
    assert!(
        message.contains("matches() pattern"),
        "unexpected message: {message}"
    );
}

#[test]
fn unknown_helpers_and_computed_entries_fail() {
    assert!(names(r#"nope("x")"#).is_err());
    assert!(names("x + 1").is_err());
}

#[test]
fn unknown_column_is_rejected() {
    let message = names("nope")
        .expect_err("unknown column must fail")
        .to_string();
    assert!(message.contains("nope"), "unexpected message: {message}");
}

#[test]
fn rename_first_entry_adds_its_source() {
    assert_eq!(names("new = x").expect("select() succeeds"), vec!["new"]);
}

#[test]
fn rename_in_the_middle_inserts_its_source_there() {
    assert_eq!(
        names("grp, new = x, y").expect("select() succeeds"),
        vec!["grp", "new", "y"]
    );
}

#[test]
fn renaming_an_already_selected_column_updates_that_output() {
    assert_eq!(names("x, new = x").expect("select() succeeds"), vec!["new"]);
}

#[test]
fn negative_rename_is_rejected() {
    assert!(names("new = -x").is_err());
}

#[test]
fn duplicate_output_names_are_rejected_after_mapping() {
    assert!(names("new = x, new = y").is_err());
    assert!(names("x, y, x").is_ok());
}
