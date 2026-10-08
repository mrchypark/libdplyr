//! Structure tests for joins, set operations, and compound aggregates.
//!
//! SQL execution lives in `relational_execution.py`; these tests pin the
//! output schema and the planner decisions that SQL alone cannot show, such as
//! column alignment by name and suffix resolution.

use libdplyr::relational::{SchemaColumn, SourceSchema};
use libdplyr::{CompiledQuery, PostgreSqlDialect, TranspileError, Transpiler};

fn pg() -> Transpiler {
    Transpiler::new(Box::new(PostgreSqlDialect::new()))
}

fn compile(code: &str, schemas: &[SourceSchema]) -> Result<CompiledQuery, TranspileError> {
    pg().transpile_with_schemas(code, schemas)
}

fn columns(q: &CompiledQuery) -> Vec<&str> {
    q.columns.iter().map(|c| c.name.as_str()).collect()
}

fn lower(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn names(q: &CompiledQuery, name: &str) -> usize {
    columns(q).iter().filter(|c| **c == name).count()
}

fn set_schemas() -> Vec<SourceSchema> {
    vec![
        SourceSchema::new("set_a", vec!["key", "val"]),
        // Declared in the opposite order, with a compatible but different
        // integer type, so alignment has to be by name and not by position.
        SourceSchema::with_columns(
            "set_b",
            vec![
                SchemaColumn::new("val"),
                SchemaColumn {
                    data_type: Some("bigint".to_string()),
                    ..SchemaColumn::new("key")
                },
            ],
        ),
    ]
}

fn aggregate_schema() -> SourceSchema {
    SourceSchema::new("agg_data", vec!["g", "x", "y"])
}

#[test]
fn set_operation_output_follows_the_left_source_order() {
    for code in [
        "set_a %>% union(set_b)",
        "set_a %>% intersect(set_b)",
        "set_a %>% setdiff(set_b)",
    ] {
        let q = compile(code, &set_schemas()).expect(code);
        assert_eq!(columns(&q), vec!["key", "val"], "for {code}");
    }
}

#[test]
fn set_operation_aligns_a_reordered_right_source_by_name() {
    let q = compile("set_a %>% union(set_b)", &set_schemas()).expect("union");
    let sql = lower(&q.sql);
    assert!(sql.contains("union"), "sql: {}", q.sql);
    // `key` and `val` must be selected from set_b by name even though set_b's
    // schema lists them in the opposite order.
    let right = sql.rsplit("union").next().unwrap_or_default();
    let key_at = right.find("\"key\"").unwrap_or(usize::MAX);
    let val_at = right.find("\"val\"").unwrap_or(usize::MAX);
    assert!(
        key_at < val_at,
        "right side must read key before val: {right}"
    );
}

#[test]
fn set_operation_aligns_missing_column_names() {
    let schemas = vec![
        SourceSchema::new("set_a", vec!["key", "val"]),
        SourceSchema::new("set_b", vec!["key", "other"]),
    ];
    let q = compile("set_a %>% union(set_b)", &schemas).expect("NULL column alignment");
    assert!(q.sql.contains("NULL"));
}

#[test]
fn set_operation_aligns_missing_column_counts() {
    let schemas = vec![
        SourceSchema::new("set_a", vec!["key", "val"]),
        SourceSchema::new("set_b", vec!["key", "val", "extra"]),
    ];
    let q = compile("set_a %>% union(set_b)", &schemas).expect("NULL column alignment");
    assert!(q.sql.contains("NULL"));
}

#[test]
fn a_set_operation_rejects_an_undescribed_source() {
    assert!(compile("set_a %>% union(no_such_table)", &set_schemas()).is_err());
}

#[test]
fn set_operations_chain() {
    let q =
        compile("set_a %>% union(set_a) %>% union(set_b)", &set_schemas()).expect("chained unions");
    assert_eq!(columns(&q), vec!["key", "val"]);
}

#[test]
fn operations_after_a_set_operation_are_allowed() {
    let schemas = set_schemas();

    let filtered = compile("set_a %>% union(set_b) %>% filter(val != 'p')", &schemas)
        .expect("filter after a set operation");
    assert_eq!(columns(&filtered), vec!["key", "val"]);

    let grouped = compile(
        "set_a %>% union(set_b) %>% group_by(val) %>% summarise(n = n())",
        &schemas,
    )
    .expect("group after a set operation");
    assert_eq!(columns(&grouped), vec!["val", "n"]);
    assert!(
        lower(&grouped.sql).contains("group by"),
        "sql: {}",
        grouped.sql
    );

    let selected = compile("set_a %>% union(set_b) %>% select(val)", &schemas)
        .expect("select after a set operation");
    assert_eq!(columns(&selected), vec!["val"]);

    let mutated = compile("set_a %>% union(set_b) %>% mutate(z = key + 1)", &schemas)
        .expect("mutate after a set operation");
    assert_eq!(columns(&mutated), vec!["key", "val", "z"]);
}

#[test]
fn a_set_operation_clears_an_earlier_sort() {
    // Ordering has no meaning across an equality-based set operation, so the
    // pre-existing arrange must not survive as a stale sort key.
    let q = compile(
        "set_a %>% arrange(desc(key)) %>% union(set_b)",
        &set_schemas(),
    )
    .expect("arrange then union");
    assert_eq!(columns(&q), vec!["key", "val"]);
    assert!(
        !lower(&q.sql).contains("order by"),
        "stale sort survived: {}",
        q.sql
    );
}

#[test]
fn distinct_after_a_set_operation_applies_to_the_combined_rows() {
    let q = compile("set_a %>% union(set_b) %>% distinct(val)", &set_schemas())
        .expect("distinct after a set operation");
    assert_eq!(columns(&q), vec!["val"]);
    assert!(lower(&q.sql).contains("distinct"), "sql: {}", q.sql);
}

#[test]
fn sum_of_a_product_is_a_single_aggregate() {
    let q = compile(
        "agg_data %>% group_by(g) %>% summarise(p = sum(x * y))",
        &[aggregate_schema()],
    )
    .expect("sum of a product");
    assert_eq!(columns(&q), vec!["g", "p"]);
    assert!(lower(&q.sql).contains("group by"), "sql: {}", q.sql);
}

#[test]
fn compound_measures_coexist_in_one_summarise() {
    let q = compile(
        "agg_data %>% group_by(g) %>% summarise(m = sum(x) / n(), u = n_distinct(x) + 1)",
        &[aggregate_schema()],
    )
    .expect("compound summarise");
    assert_eq!(columns(&q), vec!["g", "m", "u"]);
    let sql = lower(&q.sql);
    assert!(sql.contains("count("), "sql: {}", q.sql);
    assert!(sql.contains("count(distinct"), "sql: {}", q.sql);
}

#[test]
fn a_division_by_a_count_is_forced_to_floating_point() {
    let schema = SourceSchema::new("ratio_data", vec!["v"]);
    for code in [
        "ratio_data %>% summarise(m = sum(v) / n())",
        "ratio_data %>% summarise(m = sum(v) / 4)",
        "ratio_data %>% mutate(half = v / 2)",
    ] {
        let q = compile(code, std::slice::from_ref(&schema)).expect(code);
        let sql = lower(&q.sql);
        // SQLite truncates INTEGER / INTEGER; the left operand must be made
        // numeric so 3/2 is 1.5 and 3/4 is 0.75 rather than 1 and 0.
        assert!(
            sql.contains("* 1.0"),
            "division must be float-promoted for {code}: {}",
            q.sql
        );
    }
}

#[test]
fn grouped_and_global_aggregates_differ_in_kept_columns() {
    let schema = aggregate_schema();

    let grouped = compile(
        "agg_data %>% group_by(g) %>% summarise(total = sum(x))",
        std::slice::from_ref(&schema),
    )
    .expect("grouped summarise");
    assert_eq!(columns(&grouped), vec!["g", "total"]);

    let global =
        compile("agg_data %>% summarise(total = sum(x))", &[schema]).expect("global summarise");
    assert_eq!(columns(&global), vec!["total"]);
}

#[test]
fn filter_after_a_compound_summarise_reads_a_derived_measure() {
    let q = compile(
        "agg_data %>% group_by(g) %>% summarise(m = sum(x) / n(), u = n_distinct(x) + 1) \
         %>% filter(m >= 5)",
        &[aggregate_schema()],
    )
    .expect("filter after a compound summarise");
    assert_eq!(columns(&q), vec!["g", "m", "u"]);
    // The measure may be pushed into HAVING or read from a derived query with
    // WHERE. Pin the observable behaviour, not the chosen SQL shape.
    let sql = lower(&q.sql);
    assert!(
        sql.contains("having") || sql.contains("where"),
        "the derived measure must be filtered: {}",
        q.sql
    );
}

#[test]
fn a_join_suffix_never_creates_a_duplicate_output_column() {
    let schemas = vec![
        SourceSchema::new("left_side", vec!["kid", "v"]),
        // The right side already owns the name a suffix would produce.
        SourceSchema::new("right_side", vec!["kid", "v.x", "v", "w"]),
    ];
    let q = compile(
        "left_side %>% inner_join(right_side, by = c(\"kid\" = \"kid\"))",
        &schemas,
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
}

#[test]
fn equality_key_keeps_its_name_when_the_right_auxiliary_has_that_name() {
    let schemas = [
        SourceSchema::new("left_side", ["id"]),
        SourceSchema::new("right_side", ["rid", "id"]),
    ];
    let q = compile(
        "left_side %>% full_join(right_side, by = c(\"id\" = \"rid\"))",
        &schemas,
    )
    .expect("key name remains available");
    assert_eq!(columns(&q), ["id", "id.y"]);
}

#[test]
fn suffix_collisions_follow_original_column_order() {
    let schemas = [
        SourceSchema::new("left_side", ["id", "v", "v.x"]),
        SourceSchema::new("right_side", ["id", "v"]),
    ];
    let q =
        compile("left_side %>% left_join(right_side, by = \"id\")", &schemas).expect("suffixes");
    assert_eq!(columns(&q), ["id", "v.x", "v.x.x", "v.y"]);
}

#[test]
fn all_six_join_types_bind_their_keys() {
    let schemas = vec![
        SourceSchema::new("data", vec!["id", "grp"]),
        SourceSchema::new("other", vec!["id", "tag"]),
    ];
    for verb in [
        "inner_join",
        "left_join",
        "right_join",
        "full_join",
        "semi_join",
        "anti_join",
    ] {
        let code = format!("data %>% {verb}(other, by = \"id\")");
        let q = compile(&code, &schemas).expect(&code);
        assert_eq!(names(&q, "id"), 1, "key must survive once in {code}");
    }
}
