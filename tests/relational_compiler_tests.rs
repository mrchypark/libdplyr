//! Behavior tests for the schema-aware relational compiler.
//!
//! Assertions cover structure (output columns and order, dependency stages,
//! dialect quoting) and error behavior, never exact pretty-printed SQL.

use libdplyr::relational::SourceSchema;
use libdplyr::{
    CompiledQuery, DuckDbDialect, MySqlDialect, PostgreSqlDialect, SqliteDialect, TranspileError,
    Transpiler,
};

fn schema() -> SourceSchema {
    SourceSchema::new("data", vec!["id", "grp", "x", "y", "z", "label"])
}

fn pg() -> Transpiler {
    Transpiler::new(Box::new(PostgreSqlDialect::new()))
}

fn compile(code: &str) -> Result<CompiledQuery, TranspileError> {
    pg().transpile_with_schema(code, &schema())
}

/// Compiles against an arbitrary set of source schemas, for pipelines whose
/// right-hand relations need metadata of their own.
fn compile_with(code: &str, schemas: &[SourceSchema]) -> Result<CompiledQuery, TranspileError> {
    pg().transpile_with_schemas(code, schemas)
}

fn other_schema() -> SourceSchema {
    SourceSchema::new("other", vec!["id", "tag"])
}

fn joined(code: &str) -> Result<CompiledQuery, TranspileError> {
    compile_with(code, &[schema(), other_schema()])
}

fn columns(q: &CompiledQuery) -> Vec<&str> {
    q.columns.iter().map(|c| c.name.as_str()).collect()
}

/// Collapse whitespace so assertions do not depend on pretty-printing.
fn flat(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lower(sql: &str) -> String {
    flat(sql).to_lowercase()
}

fn names(q: &CompiledQuery, name: &str) -> usize {
    columns(q).iter().filter(|c| **c == name).count()
}

fn expect_generation_error(code: &str) {
    match compile(code) {
        Ok(q) => panic!("expected error for {code:?}, got:\n{}", flat(&q.sql)),
        Err(TranspileError::GenerationError(_)) => {}
        Err(other) => panic!("expected GenerationError for {code:?}, got {other:?}"),
    }
}

#[test]
fn dependent_mutate_within_one_operation() {
    let q = compile("data %>% mutate(a = x * 2, b = a + 1)").expect("mutate");
    assert_eq!(
        columns(&q),
        vec!["id", "grp", "x", "y", "z", "label", "a", "b"]
    );
    assert!(q.stages >= 2, "stages: {}", q.stages);
    let sql = lower(&q.sql);
    assert!(sql.contains("select") && sql.contains("from"));
}

#[test]
fn dependent_mutate_across_operations() {
    let q = compile("data %>% mutate(a = x * 2) %>% mutate(b = a + 1)").expect("mutate");
    assert_eq!(columns(&q).last(), Some(&"b"));
    assert_eq!(names(&q, "a"), 1);
    assert!(q.stages >= 2, "stages: {}", q.stages);
}

#[test]
fn mutate_overwrite_does_not_duplicate_output_name() {
    let q = compile("data %>% mutate(a = x) %>% mutate(a = y)").expect("overwrite");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y", "z", "label", "a"]);
    assert_eq!(names(&q, "a"), 1);
}

#[test]
fn reference_to_projected_away_column_is_rejected() {
    expect_generation_error("data %>% select(id, x) %>% mutate(b = z + 1)");
}

#[test]
fn filter_after_summarise_uses_derived_column() {
    let q = compile("data %>% group_by(grp) %>% summarise(n = n()) %>% filter(n > 2)")
        .expect("filter after summarise");
    assert_eq!(columns(&q), vec!["grp", "n"]);
    // The aggregate and the following filter cannot share a single SELECT.
    assert!(q.stages >= 3, "stages: {}", q.stages);
    assert!(lower(&q.sql).contains("group by"), "sql: {}", flat(&q.sql));
}

#[test]
fn global_aggregate_drops_input_columns_from_the_output() {
    // A group-less summarise keeps only its measures, so grp is no longer bound.
    expect_generation_error("data %>% summarise(total = sum(x)) %>% group_by(grp)");
}

#[test]
fn late_group_by_on_a_derived_column_keeps_the_earlier_aggregate() {
    let q = compile(
        "data %>% summarise(total = sum(x)) %>% group_by(total) %>% summarise(grand = sum(total))",
    )
    .expect("late group_by on a derived column");
    assert_eq!(columns(&q), vec!["total", "grand"]);
    // At minimum: scan + first aggregate + second aggregate.
    assert!(q.stages >= 3, "stages: {}", q.stages);
    assert!(lower(&q.sql).contains("sum("), "sql: {}", flat(&q.sql));
}

#[test]
fn grouped_select_retains_group_keys() {
    let q = compile("data %>% group_by(grp) %>% select(grp, x)").expect("grouped select");
    assert_eq!(columns(&q), vec!["grp", "x"]);
    // group_by() is metadata until an aggregate consumes it, so a bare
    // grouped select must not emit GROUP BY.
    assert!(!lower(&q.sql).contains("group by"), "sql: {}", flat(&q.sql));
}

#[test]
fn grouped_select_keeps_grouping_for_a_later_aggregate() {
    let q = compile("data %>% group_by(grp) %>% select(grp, x) %>% summarise(n = n())")
        .expect("grouped select then summarise");
    assert_eq!(columns(&q), vec!["grp", "n"]);
    assert!(lower(&q.sql).contains("group by"), "sql: {}", flat(&q.sql));
}

#[test]
fn select_keeps_a_dropped_group_key_for_grouping() {
    let q = compile("data %>% group_by(grp) %>% select(x)").expect("grouped select");
    assert_eq!(columns(&q), vec!["grp", "x"]);
}

#[test]
fn rename_binds_new_name_and_drops_the_old_one() {
    let q = compile("data %>% rename(value = x) %>% filter(value > 1)").expect("rename");
    assert_eq!(names(&q, "value"), 1);
    assert_eq!(names(&q, "x"), 0, "old name must be gone");

    expect_generation_error("data %>% rename(value = x) %>% filter(x > 1)");
}

#[test]
fn distinct_then_filter() {
    let q = compile("data %>% distinct(grp) %>% filter(grp != 'a')").expect("distinct");
    assert_eq!(columns(&q), vec!["grp"]);
    assert!(lower(&q.sql).contains("distinct"), "sql: {}", flat(&q.sql));
}

#[test]
fn count_creates_derivable_column() {
    let q = compile("data %>% group_by(grp) %>% count() %>% filter(n > 1)").expect("count");
    assert_eq!(columns(&q), vec!["grp", "n"]);
    assert!(lower(&q.sql).contains("count("), "sql: {}", flat(&q.sql));
}

#[test]
fn count_name_avoids_clashing_with_an_existing_column() {
    let q = compile("data %>% mutate(n = x) %>% count(n)").expect("count with a key named n");
    assert_eq!(columns(&q), vec!["n", "nn"]);
}

#[test]
fn arrange_then_filter_uses_a_renamed_column() {
    let q = compile("data %>% rename(v = x) %>% arrange(desc(v))").expect("arrange");
    assert_eq!(names(&q, "v"), 1);
    assert!(lower(&q.sql).contains("order by"), "sql: {}", flat(&q.sql));
}

#[test]
fn sort_survives_a_select_that_drops_the_sort_column() {
    let q = compile("data %>% arrange(desc(x)) %>% select(id, grp)").expect("hidden sort key");
    assert_eq!(columns(&q), vec!["id", "grp"]);
    assert_eq!(names(&q, "x"), 0, "sort key must not stay visible");
    assert!(
        !columns(&q).iter().any(|c| c.starts_with("__libdplyr")),
        "internal column leaked: {:?}",
        columns(&q)
    );
    assert!(lower(&q.sql).contains("order by"), "sql: {}", flat(&q.sql));
}

#[test]
fn sort_survives_overwriting_the_sort_column() {
    let q = compile("data %>% arrange(desc(x)) %>% mutate(x = y)").expect("overwritten sort key");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y", "z", "label"]);
    assert!(
        !columns(&q).iter().any(|c| c.starts_with("__libdplyr")),
        "internal column leaked: {:?}",
        columns(&q)
    );
    assert!(lower(&q.sql).contains("order by"), "sql: {}", flat(&q.sql));
}

#[test]
fn mutate_of_a_grouping_column_is_rejected() {
    expect_generation_error("data %>% group_by(grp) %>% mutate(grp = paste(grp, 'x'))");
}

#[test]
fn n_distinct_summarise_is_null_inclusive() {
    let q = compile("data %>% group_by(grp) %>% summarise(u = n_distinct(x))").expect("n_distinct");
    assert_eq!(columns(&q), vec!["grp", "u"]);
    let sql = lower(&q.sql);
    assert!(sql.contains("count(distinct"), "sql: {}", flat(&q.sql));
    assert!(sql.contains("case when"), "sql: {}", flat(&q.sql));
}

#[test]
fn n_distinct_as_a_window_expression_is_rejected() {
    expect_generation_error("data %>% filter(n_distinct(x) > 1)");
}

#[test]
fn computed_select_is_rejected() {
    expect_generation_error("data %>% select(z = x + 1)");
}

#[test]
fn target_assignment_is_rejected() {
    expect_generation_error("data %>% mutate(a = x) -> out");
}

#[test]
fn joins_and_setops_need_a_schema_for_their_right_source() {
    // Only `data` is described, so the right relation has no columns to bind.
    expect_generation_error("data %>% inner_join(other, by = \"id\")");
    expect_generation_error("data %>% union(other)");
}

#[test]
fn inner_join_concatenates_the_two_column_sets() {
    let q = joined("data %>% inner_join(other, by = \"id\")").expect("inner join");
    assert_eq!(
        columns(&q),
        vec!["id", "grp", "x", "y", "z", "label", "tag"]
    );
    assert!(lower(&q.sql).contains("join"), "sql: {}", flat(&q.sql));
}

#[test]
fn semi_join_keeps_only_the_left_columns() {
    let q = joined("data %>% semi_join(other, by = \"id\")").expect("semi join");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y", "z", "label"]);
}

#[test]
fn anti_join_keeps_only_the_left_columns() {
    let q = joined("data %>% anti_join(other, by = \"id\")").expect("anti join");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y", "z", "label"]);
}

#[test]
fn overlapping_non_key_columns_get_a_suffix() {
    // `grp` exists on both sides, so dplyr renames it to .x and .y.
    let q = compile_with(
        "data %>% inner_join(other, by = \"id\")",
        &[
            schema(),
            SourceSchema::new("other", vec!["id", "grp", "tag"]),
        ],
    )
    .expect("overlapping join");
    assert_eq!(
        columns(&q),
        vec!["id", "grp.x", "x", "y", "z", "label", "grp.y", "tag"]
    );
}

#[test]
fn a_suffix_collision_is_resolved_rather_than_repeated() {
    // The right side already owns `grp.x`, so a second `.x` suffix cannot
    // produce a duplicate output name.
    let q = compile_with(
        "data %>% inner_join(other, by = \"id\")",
        &[
            schema(),
            SourceSchema::new("other", vec!["id", "grp.x", "grp", "tag"]),
        ],
    )
    .expect("suffix collision");
    let produced = columns(&q);
    let mut seen = std::collections::HashSet::new();
    for name in &produced {
        assert!(
            seen.insert(*name),
            "duplicate output column {name} in {produced:?}"
        );
    }
    assert!(
        produced.iter().any(|c| c.starts_with("grp")),
        "{produced:?}"
    );
}

#[test]
fn right_and_full_joins_emit_one_coalesced_key_column() {
    for code in [
        "data %>% right_join(other, by = \"id\")",
        "data %>% full_join(other, by = \"id\")",
    ] {
        let q = joined(code).expect(code);
        assert_eq!(names(&q, "id"), 1, "key must appear once for {code}");
        assert_eq!(names(&q, "id.x"), 0, "no suffix for {code}");
        assert_eq!(names(&q, "id.y"), 0, "no suffix for {code}");
        assert!(lower(&q.sql).contains("coalesce"), "sql: {}", flat(&q.sql));
    }
}

#[test]
fn differently_named_keys_bind_by_their_own_names() {
    let q = compile_with(
        "left %>% inner_join(right, by = c(\"lid\" = \"rid\"))",
        &[
            SourceSchema::new("left", vec!["lid", "lval"]),
            SourceSchema::new("right", vec!["rid", "rval"]),
        ],
    )
    .expect("differently named keys");
    assert_eq!(columns(&q), vec!["lid", "lval", "rval"]);
}

#[test]
fn a_join_key_absent_from_either_side_is_rejected() {
    assert!(compile_with(
        "data %>% inner_join(other, by = \"nope\")",
        &[schema(), other_schema()],
    )
    .is_err());
}

#[test]
fn a_non_equality_join_predicate_is_rejected() {
    assert!(compile_with(
        "data %>% inner_join(other, by = x > 1)",
        &[schema(), other_schema()],
    )
    .is_err());
}

#[test]
fn operations_after_a_join_use_both_sides() {
    let q = joined(
        "data %>% inner_join(other, by = \"id\") %>% mutate(t = x + y) %>% \
         group_by(tag) %>% summarise(n = n())",
    )
    .expect("operations after a join");
    assert_eq!(columns(&q), vec!["tag", "n"]);
    assert!(lower(&q.sql).contains("group by"), "sql: {}", flat(&q.sql));
}

#[test]
fn filter_after_a_join_can_read_a_right_column() {
    let q = joined("data %>% left_join(other, by = \"id\") %>% filter(tag == 't1')")
        .expect("filter on a right column");
    assert!(columns(&q).contains(&"tag"), "{:?}", columns(&q));
    assert!(lower(&q.sql).contains("where"), "sql: {}", flat(&q.sql));
}

#[test]
fn all_four_dialects_quote_identifiers() {
    let code = "data %>% mutate(total = x + y) %>% filter(total > 0)";

    let cases: Vec<(Transpiler, &str)> = vec![
        (
            Transpiler::new(Box::new(PostgreSqlDialect::new())),
            "\"total\"",
        ),
        (Transpiler::new(Box::new(MySqlDialect::new())), "`total`"),
        (Transpiler::new(Box::new(SqliteDialect::new())), "\"total\""),
        (Transpiler::new(Box::new(DuckDbDialect::new())), "\"total\""),
    ];

    for (transpiler, quoted) in cases {
        let q = transpiler
            .transpile_with_schema(code, &schema())
            .expect("dialect compile");
        assert!(
            q.sql.contains(quoted),
            "missing {quoted} in {}",
            flat(&q.sql)
        );
        assert_eq!(columns(&q).last(), Some(&"total"));
    }
}

#[test]
fn source_name_must_match_the_schema_source() {
    match pg().transpile_with_schema("other %>% select(id)", &schema()) {
        Ok(q) => panic!("expected error, got:\n{}", flat(&q.sql)),
        Err(TranspileError::GenerationError(_)) => {}
        Err(other) => panic!("expected GenerationError, got {other:?}"),
    }
}

#[test]
fn invalid_schema_is_rejected() {
    let dup = SourceSchema::new("data", vec!["id", "id"]);
    assert!(pg()
        .transpile_with_schema("data %>% select(id)", &dup)
        .is_err());
}

#[test]
fn schema_with_a_nul_byte_is_rejected() {
    let schema = SourceSchema::new("data", vec!["id", "na\0me"]);
    assert!(pg()
        .transpile_with_schema("data %>% select(id)", &schema)
        .is_err());

    let schema = SourceSchema::new("da\0ta", vec!["id"]);
    assert!(pg()
        .transpile_with_schema("data %>% select(id)", &schema)
        .is_err());
}

#[test]
fn empty_pipeline_selects_the_schema_columns() {
    let q = compile("data").expect("bare source");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y", "z", "label"]);
}
