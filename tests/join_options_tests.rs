//! Join options: natural keys, custom suffixes, `keep`, and `na_matches`.

use libdplyr::relational::SourceSchema;
use libdplyr::{CompiledQuery, PostgreSqlDialect, TranspileError, Transpiler};

fn pg() -> Transpiler {
    Transpiler::new(Box::new(PostgreSqlDialect::new()))
}

fn data() -> SourceSchema {
    SourceSchema::new("data", vec!["id", "grp", "x", "y", "z", "label"])
}

fn other() -> SourceSchema {
    SourceSchema::new("other", vec!["id", "tag"])
}

fn joined(code: &str) -> Result<CompiledQuery, TranspileError> {
    pg().transpile_with_schemas(code, &[data(), other()])
}

fn columns(q: &CompiledQuery) -> Vec<&str> {
    q.columns.iter().map(|c| c.name.as_str()).collect()
}

fn flat(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn expect_error(code: &str) {
    match joined(code) {
        Ok(q) => panic!("expected error for {code:?}, got: {}", flat(&q.sql)),
        Err(TranspileError::GenerationError(_)) => {}
        Err(other) => panic!("expected GenerationError for {code:?}, got {other:?}"),
    }
}

#[test]
fn natural_join_uses_names_shared_by_both_inputs() {
    let q = joined("data %>% inner_join(other)").expect("natural join");
    assert_eq!(
        columns(&q),
        vec!["id", "grp", "x", "y", "z", "label", "tag"]
    );
    let sql = flat(&q.sql);
    assert!(
        sql.contains("\"__libdplyr_left\".\"id\" = \"__libdplyr_right\".\"id\""),
        "{sql}"
    );
}

#[test]
fn natural_left_join_marks_right_columns_nullable() {
    let q = joined("data %>% left_join(other)").expect("natural left join");
    assert_eq!(
        columns(&q),
        vec!["id", "grp", "x", "y", "z", "label", "tag"]
    );
    assert_eq!(q.columns.last().map(|c| c.nullable), Some(Some(true)));
}

#[test]
fn join_without_shared_column_names_is_rejected() {
    let q = pg().transpile_with_schemas(
        "data %>% inner_join(alien)",
        &[data(), SourceSchema::new("alien", vec!["aid", "note"])],
    );
    assert!(matches!(q, Err(TranspileError::GenerationError(_))));
}

#[test]
fn custom_suffixes_name_both_colliding_sides() {
    let q = pg()
        .transpile_with_schemas(
            r#"data %>% inner_join(other, by = "id", suffix = c("_l", "_r"))"#,
            &[
                data(),
                SourceSchema::new("other", vec!["id", "grp", "x", "tag"]),
            ],
        )
        .expect("custom suffix");
    assert_eq!(
        columns(&q),
        vec!["id", "grp_l", "x_l", "y", "z", "label", "grp_r", "x_r", "tag"]
    );
    assert!(flat(&q.sql).contains("grp_l"), "{}", flat(&q.sql));
}

#[test]
fn keep_retains_the_right_key_column() {
    let q = joined(r#"data %>% inner_join(other, by = "id", keep = TRUE)"#).expect("keep");
    assert_eq!(
        columns(&q),
        vec!["id.x", "grp", "x", "y", "z", "label", "id.y", "tag"]
    );
}

#[test]
fn keep_suffixes_every_collision_including_keys() {
    let q = pg()
        .transpile_with_schemas(
            r#"data %>% inner_join(other, by = "id", keep = TRUE, suffix = c("_l", "_r"))"#,
            &[data(), SourceSchema::new("other", vec!["id", "grp", "tag"])],
        )
        .expect("keep with collisions");
    assert_eq!(
        columns(&q),
        vec!["id_l", "grp_l", "x", "y", "z", "label", "id_r", "grp_r", "tag"]
    );
}

#[test]
fn an_empty_suffix_leaves_that_side_unchanged() {
    // `keep` makes both sides carry the key, so the right one must move even
    // though the left suffix is empty.
    let q = joined(r#"data %>% inner_join(other, by = "id", keep = TRUE, suffix = c("", "_r"))"#)
        .expect("one-sided empty suffix");
    assert_eq!(
        columns(&q),
        vec!["id", "grp", "x", "y", "z", "label", "id_r", "tag"]
    );
}

#[test]
fn two_empty_suffixes_are_rejected_only_when_the_output_clashes() {
    // Without `keep` the right key disappears, so nothing collides.
    let q = joined(r#"data %>% inner_join(other, by = "id", suffix = c("", ""))"#)
        .expect("no output clash");
    assert_eq!(
        columns(&q),
        vec!["id", "grp", "x", "y", "z", "label", "tag"]
    );

    // A shared non-key column still collides, and validation rejects it.
    let clashing = pg().transpile_with_schemas(
        r#"data %>% inner_join(other, by = "id", suffix = c("", ""))"#,
        &[data(), SourceSchema::new("other", vec!["id", "grp", "tag"])],
    );
    assert!(
        matches!(clashing, Err(TranspileError::GenerationError(_))),
        "{clashing:?}"
    );
}

#[test]
fn right_join_coalesces_keys_unless_keep_is_set() {
    let coalesced = joined("data %>% right_join(other, by = \"id\")").expect("right join");
    assert_eq!(columns(&coalesced).first(), Some(&"id"));
    assert!(
        flat(&coalesced.sql).contains("coalesce"),
        "{}",
        flat(&coalesced.sql)
    );

    let kept =
        joined(r#"data %>% right_join(other, by = "id", keep = TRUE)"#).expect("right join keep");
    assert!(columns(&kept).contains(&"id.y"), "{:?}", columns(&kept));
}

#[test]
fn semi_and_anti_joins_reject_output_shaping_options() {
    expect_error(r#"data %>% semi_join(other, by = "id", keep = TRUE)"#);
    expect_error(r#"data %>% anti_join(other, by = "id", suffix = c("_l", "_r"))"#);
    let q = joined(r#"data %>% semi_join(other, by = "id")"#).expect("plain semi join");
    assert_eq!(columns(&q), vec!["id", "grp", "x", "y", "z", "label"]);
}

#[test]
fn na_matches_uses_a_null_safe_predicate() {
    let q =
        joined(r#"data %>% inner_join(other, by = "id", na_matches = "na")"#).expect("na_matches");
    let sql = flat(&q.sql);
    assert!(sql.contains("is null and"), "{sql}");
    assert!(sql.contains(r#""__libdplyr_right"."id" is null"#), "{sql}");
}

#[test]
fn groups_survive_a_join_into_summarise() {
    let q =
        joined(r#"data %>% group_by(grp) %>% inner_join(other, by = "id") %>% summarise(n = n())"#)
            .expect("grouped join");
    let sql = flat(&q.sql);
    assert!(sql.contains("group by \"grp\""), "{sql}");
    assert_eq!(columns(&q), vec!["grp", "n"]);
}
