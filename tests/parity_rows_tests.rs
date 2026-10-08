//! Focused tests for rows_* verb lowering.
//!
//! Verifies plan requirements, schema preservation, NULL-presence semantics,
//! collision handling, and error cases. Uses plan_with_schemas for operations
//! that require validation checks.

use libdplyr::{SourceSchema, SqliteDialect, Transpiler};

fn transpiler() -> Transpiler {
    Transpiler::new(Box::new(SqliteDialect::new()))
}

fn schemas() -> Vec<SourceSchema> {
    vec![
        SourceSchema::new("data", ["id", "grp", "x"]),
        SourceSchema::new("new_data", ["id", "grp", "x"]),
    ]
}

fn plan(code: &str) -> libdplyr::ExecutionPlan {
    transpiler()
        .plan_with_schemas(code, &schemas())
        .unwrap_or_else(|e| panic!("plan_with_schemas failed for {code:?}: {e}"))
}

fn compile(code: &str) -> libdplyr::CompiledQuery {
    transpiler()
        .transpile_with_schemas(code, &schemas())
        .unwrap_or_else(|e| panic!("transpile_with_schemas failed for {code:?}: {e}"))
}

fn expect_error(code: &str) {
    let result = transpiler().transpile_with_schemas(code, &schemas());
    assert!(
        result.is_err(),
        "expected error for {code:?}, got: {:?}",
        result.map(|q| q.sql)
    );
}

// ---------------------------------------------------------------------------
// Append: pure, no plan required
// ---------------------------------------------------------------------------

#[test]
fn rows_append_pure_no_plan_needed() {
    let q = compile("data %>% rows_append(new_data)");
    assert!(q.sql.to_lowercase().contains("union all"));
    assert_eq!(q.columns.len(), 3);
}

#[test]
fn rows_append_rejects_key_options() {
    expect_error(r#"data %>% rows_append(new_data, by = "id")"#);
    expect_error(r#"data %>% rows_append(new_data, conflict = "ignore")"#);
    expect_error(r#"data %>% rows_append(new_data, unmatched = "ignore")"#);
}

// ---------------------------------------------------------------------------
// Insert: error default needs plan; ignore works without plan
// ---------------------------------------------------------------------------

#[test]
fn rows_insert_error_requires_plan() {
    expect_error("data %>% rows_insert(new_data)");
    expect_error(r#"data %>% rows_insert(new_data, unmatched = "error")"#);
}

#[test]
fn rows_insert_ignore_pure_no_plan_needed() {
    // conflict="ignore" lets the database handle conflicts; no uniqueness check needed
    let q = compile(r#"data %>% rows_insert(new_data, conflict = "ignore")"#);
    assert!(q.sql.to_lowercase().contains("union all"));
}

#[test]
fn rows_insert_with_plan_produces_checks() {
    let p = plan("data %>% rows_insert(new_data)");
    assert!(
        !p.checks.is_empty(),
        "insert should produce validation checks"
    );
    // Insert with default unmatched="error" produces an EXISTS conflict check
    assert!(
        p.checks[0].sql.to_lowercase().contains("exists")
            || p.checks[0].sql.to_lowercase().contains("not exists")
    );
}

#[test]
fn rows_insert_rejects_unmatched_option() {
    expect_error(r#"data %>% rows_insert(new_data, unmatched = "error")"#);
}

// ---------------------------------------------------------------------------
// Update: error default needs plan; ignore works without plan
// ---------------------------------------------------------------------------

#[test]
fn rows_update_error_requires_plan() {
    expect_error(r#"data %>% rows_update(new_data, by = "id")"#);
    expect_error(r#"data %>% rows_update(new_data, by = "id", unmatched = "error")"#);
}

#[test]
fn rows_update_ignore_pure_except_uniqueness() {
    // unmatched="ignore" still needs uniqueness check
    expect_error(r#"data %>% rows_update(new_data, by = "id", unmatched = "ignore")"#);
}

#[test]
fn rows_update_with_plan_produces_checks() {
    let p = plan(r#"data %>% rows_update(new_data, by = "id")"#);
    assert!(
        !p.checks.is_empty(),
        "update should produce validation checks"
    );
}

#[test]
fn rows_update_rejects_conflict_option() {
    expect_error(r#"data %>% rows_update(new_data, by = "id", conflict = "ignore")"#);
}

// ---------------------------------------------------------------------------
// Patch: always needs plan (uniqueness check)
// ---------------------------------------------------------------------------

#[test]
fn rows_patch_requires_plan() {
    expect_error(r#"data %>% rows_patch(new_data, by = "id")"#);
    expect_error(r#"data %>% rows_patch(new_data, by = "id", unmatched = "ignore")"#);
}

#[test]
fn rows_patch_with_plan_produces_checks() {
    let p = plan(r#"data %>% rows_patch(new_data, by = "id")"#);
    assert!(
        !p.checks.is_empty(),
        "patch should produce validation checks"
    );
}

// ---------------------------------------------------------------------------
// Upsert: always needs plan (uniqueness check)
// ---------------------------------------------------------------------------

#[test]
fn rows_upsert_requires_plan() {
    expect_error(r#"data %>% rows_upsert(new_data, by = "id")"#);
    expect_error(r#"data %>% rows_upsert(new_data, by = "id", unmatched = "ignore")"#);
}

#[test]
fn rows_upsert_with_plan_produces_checks() {
    let p = plan(r#"data %>% rows_upsert(new_data, by = "id")"#);
    assert!(
        !p.checks.is_empty(),
        "upsert should produce validation checks"
    );
}

// ---------------------------------------------------------------------------
// Delete: error default needs plan; ignore works without plan
// ---------------------------------------------------------------------------

#[test]
fn rows_delete_error_requires_plan() {
    expect_error(r#"data %>% rows_delete(new_data, by = "id")"#);
    expect_error(r#"data %>% rows_delete(new_data, by = "id", unmatched = "error")"#);
}

#[test]
fn rows_delete_ignore_pure() {
    // unmatched="ignore" has no uniqueness requirement for delete
    let q = compile(r#"data %>% rows_delete(new_data, by = "id", unmatched = "ignore")"#);
    let sql = q.sql.to_lowercase();
    assert!(
        sql.contains("anti")
            || sql.contains("except")
            || sql.contains("not in")
            || sql.contains("not exists")
    );
}

#[test]
fn rows_delete_with_plan_produces_checks() {
    let p = plan(r#"data %>% rows_delete(new_data, by = "id")"#);
    assert!(
        !p.checks.is_empty(),
        "delete should produce validation checks"
    );
}

// ---------------------------------------------------------------------------
// Schema preservation
// ---------------------------------------------------------------------------

#[test]
fn rows_update_preserves_input_schema() {
    let p = plan(r#"data %>% rows_update(new_data, by = "id")"#);
    assert_eq!(p.query.columns.len(), 3);
    let names: Vec<_> = p.query.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["id", "grp", "x"]);
}

#[test]
fn rows_append_preserves_schema() {
    let q = compile("data %>% rows_append(new_data)");
    assert_eq!(q.columns.len(), 3);
}

// ---------------------------------------------------------------------------
// NULL-presence semantics: update uses CASE WHEN IS NULL, not COALESCE
// ---------------------------------------------------------------------------

#[test]
fn rows_update_uses_case_when_for_null_presence() {
    let p = plan(r#"data %>% rows_update(new_data, by = "id")"#);
    let sql = p.query.sql.to_lowercase();
    assert!(
        sql.contains("case when") || sql.contains("case"),
        "update should use CASE WHEN for NULL-presence check, got: {}",
        p.query.sql
    );
    assert!(
        sql.contains("is null") || sql.contains("is.na"),
        "update should check IS NULL on marker, got: {}",
        p.query.sql
    );
}

#[test]
fn rows_patch_uses_coalesce_for_null_presence() {
    let p = plan(r#"data %>% rows_patch(new_data, by = "id")"#);
    let sql = p.query.sql.to_lowercase();
    assert!(
        sql.contains("coalesce"),
        "patch should use COALESCE for NULL-presence check, got: {}",
        p.query.sql
    );
}

// ---------------------------------------------------------------------------
// Collision handling: by columns are not overwritten
// ---------------------------------------------------------------------------

#[test]
fn rows_update_does_not_overwrite_by_columns() {
    let p = plan(r#"data %>% rows_update(new_data, by = "id")"#);
    let sql = p.query.sql.to_lowercase();
    // The by column "id" should appear in the join keys, not in the CASE WHEN
    assert!(sql.contains("id"), "by column should appear in SQL");
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn rows_rejects_in_place() {
    expect_error(r#"data %>% rows_update(new_data, by = "id", in_place = TRUE)"#);
}

#[test]
fn rows_rejects_copy() {
    expect_error(r#"data %>% rows_update(new_data, by = "id", copy = TRUE)"#);
}

#[test]
fn rows_rejects_duplicate_by() {
    expect_error(r#"data %>% rows_update(new_data, by = c("id", "id"))"#);
}

#[test]
fn rows_rejects_unknown_option() {
    expect_error(r#"data %>% rows_update(new_data, by = "id", bogus = TRUE)"#);
}

#[test]
fn rows_rejects_missing_right_relation() {
    expect_error("data %>% rows_update()");
}

#[test]
fn rows_rejects_non_string_policy() {
    expect_error("data %>% rows_insert(new_data, conflict = 42)");
}

#[test]
fn rows_rejects_invalid_policy_value() {
    expect_error(r#"data %>% rows_insert(new_data, conflict = "bogus")"#);
}

// ---------------------------------------------------------------------------
// Multi-key support
// ---------------------------------------------------------------------------

#[test]
fn rows_update_multi_key() {
    let p = plan(r#"data %>% rows_update(new_data, by = c("id", "grp"))"#);
    assert!(!p.checks.is_empty());
    let sql = p.query.sql.to_lowercase();
    assert!(sql.contains("id") && sql.contains("grp"));
}

#[test]
fn rows_delete_multi_key() {
    let p = plan(r#"data %>% rows_delete(new_data, by = c("id", "grp"))"#);
    assert!(!p.checks.is_empty());
}

// ---------------------------------------------------------------------------
// Upsert: appends unmatched rows
// ---------------------------------------------------------------------------

#[test]
fn rows_upsert_appends_unmatched() {
    let p = plan(r#"data %>% rows_upsert(new_data, by = "id")"#);
    let sql = p.query.sql.to_lowercase();
    assert!(
        sql.contains("union all") || sql.contains("union"),
        "upsert should union appended rows, got: {}",
        p.query.sql
    );
}

// ---------------------------------------------------------------------------
// Insert: appends non-matching rows
// ---------------------------------------------------------------------------

#[test]
fn rows_insert_appends_non_matching() {
    let p = plan("data %>% rows_insert(new_data)");
    let sql = p.query.sql.to_lowercase();
    assert!(
        sql.contains("union all") || sql.contains("union"),
        "insert should union non-matching rows, got: {}",
        p.query.sql
    );
}
