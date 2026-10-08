use libdplyr::{SourceSchema, SqliteDialect, Transpiler};

fn transpiler() -> Transpiler {
    Transpiler::new(Box::new(SqliteDialect::new()))
}
fn schemas() -> Vec<SourceSchema> {
    vec![
        SourceSchema::new("data", ["id", "grp", "x", "lo", "hi"]),
        SourceSchema::new("other", ["other_id", "grp", "y", "rlo", "rhi"]),
    ]
}

#[test]
fn inequality_and_range_joins_bind_both_sides() {
    for code in [
        "data %>% left_join(other, by = join_by(x >= y))",
        "data %>% inner_join(other, by = join_by(between(x, rlo, rhi)))",
        "data %>% semi_join(other, by = join_by(overlaps(lo, hi, rlo, rhi)))",
        "data %>% left_join(other, by = join_by(closest(x >= y)))",
    ] {
        let query = transpiler()
            .transpile_with_schemas(code, &schemas())
            .expect("bound range join");
        assert!(query.sql.contains("__libdplyr_left"));
        assert!(query.sql.contains("__libdplyr_right"));
    }
}

#[test]
fn nearest_join_keeps_all_ties() {
    let query = transpiler()
        .transpile_with_schemas(
            "data %>% left_join(other, by = join_by(grp, closest(x >= y)))",
            &schemas(),
        )
        .expect("rolling join");
    assert!(query.sql.contains("MAX("));
    assert!(!query.sql.contains("LIMIT 1"));
}

#[test]
fn right_pipeline_and_cross_join_have_visible_output_schemas() {
    let query=transpiler().transpile_with_schemas("data %>% left_join(other %>% filter(y > 0) %>% select(other_id, y), by = c(id = \"other_id\"))",&schemas()).expect("right pipeline");
    assert_eq!(query.columns.len(), 6);
    assert!(query.sql.contains("WHERE"));
    let query = transpiler()
        .transpile_with_schemas("data %>% cross_join(other)", &schemas())
        .expect("cross join");
    assert_eq!(query.columns.len(), 10);
    assert!(query.sql.contains("CROSS JOIN"));
}

#[test]
fn relationship_assertions_require_an_execution_plan() {
    let code="data %>% left_join(other, by = c(id = \"other_id\"), relationship = \"one-to-one\", unmatched = \"error\")";
    assert!(transpiler()
        .transpile_with_schemas(code, &schemas())
        .is_err());
    let plan = transpiler()
        .plan_with_schemas(code, &schemas())
        .expect("checked execution plan");
    assert_eq!(plan.checks.len(), 3);
    assert!(plan.checks[0].sql.contains("COUNT(*)"));
    assert!(plan.checks[0].sql.contains("> 1"));
    assert!(plan.checks[2].sql.contains("= 0"));
}

#[test]
fn source_discovery_includes_nested_right_sources() {
    assert_eq!(
        transpiler()
            .required_sources("data %>% union_all(other %>% filter(y > 0))")
            .expect("source discovery"),
        ["data", "other"]
    );
}
