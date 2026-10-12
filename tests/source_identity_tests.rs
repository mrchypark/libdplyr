use libdplyr::{PostgreSqlDialect, SourceRef, SourceSchema, Transpiler};

fn compiler() -> Transpiler {
    Transpiler::new(Box::new(PostgreSqlDialect::new()))
}

fn schema(source: impl Into<SourceRef>) -> SourceSchema {
    SourceSchema::new(source, ["id"])
}

#[test]
fn qualified_sources_compile_from_schema_and_catalog() {
    let schema_query = compiler()
        .transpile_with_schema(
            "in_schema(\"reporting\", \"orders\") %>% select(id)",
            &schema(SourceRef::Qualified(vec![
                "reporting".into(),
                "orders".into(),
            ])),
        )
        .expect("schema-qualified source");
    assert!(schema_query.sql.contains("FROM \"reporting\".\"orders\""));

    let catalog_query = compiler()
        .transpile_with_schema(
            "in_catalog(\"warehouse\", \"reporting\", \"orders\") %>% select(id)",
            &schema(SourceRef::Qualified(vec![
                "warehouse".into(),
                "reporting".into(),
                "orders".into(),
            ])),
        )
        .expect("catalog-qualified source");
    assert!(catalog_query
        .sql
        .contains("FROM \"warehouse\".\"reporting\".\"orders\""));
}

#[test]
fn literal_dots_remain_one_source_identifier() {
    let query = compiler()
        .transpile_with_schema(
            "reporting.orders %>% select(id)",
            &schema("reporting.orders"),
        )
        .expect("literal dotted source");

    assert!(query.sql.contains("FROM \"reporting.orders\""));
    assert!(!query.sql.contains("FROM \"reporting\".\"orders\""));
}

#[test]
fn source_ref_json_round_trips_string_and_array_forms() {
    let literal = SourceRef::Literal("reporting.orders".into());
    let qualified = SourceRef::Qualified(vec![
        "warehouse".into(),
        "reporting".into(),
        "orders".into(),
    ]);

    assert_eq!(
        serde_json::to_string(&literal).expect("literal JSON"),
        "\"reporting.orders\""
    );
    assert_eq!(
        serde_json::from_str::<SourceRef>("\"reporting.orders\"").expect("literal source"),
        literal
    );
    assert_eq!(
        serde_json::to_string(&qualified).expect("qualified JSON"),
        "[\"warehouse\",\"reporting\",\"orders\"]"
    );
    assert_eq!(
        serde_json::from_str::<SourceRef>("[\"warehouse\",\"reporting\",\"orders\"]")
            .expect("qualified source"),
        qualified
    );
}

#[test]
fn invalid_source_paths_are_rejected_at_compile_boundary() {
    for source in [
        SourceRef::Literal(String::new()),
        SourceRef::Literal("bad\0source".into()),
        SourceRef::Qualified(vec![]),
        SourceRef::Qualified(vec!["reporting".into()]),
        SourceRef::Qualified(vec![
            "warehouse".into(),
            "reporting".into(),
            "orders".into(),
            "archive".into(),
        ]),
        SourceRef::Qualified(vec!["reporting".into(), String::new()]),
        SourceRef::Qualified(vec!["reporting".into(), "bad\0orders".into()]),
    ] {
        assert!(compiler()
            .transpile_with_schema("data %>% select(id)", &schema(source))
            .is_err());
    }
}

#[test]
fn literal_and_qualified_sources_with_same_text_are_distinct_identities() {
    let schemas = [
        schema(SourceRef::Literal("reporting.orders".into())),
        schema(SourceRef::Qualified(vec![
            "reporting".into(),
            "orders".into(),
        ])),
    ];

    let literal = compiler()
        .transpile_with_schemas("reporting.orders %>% select(id)", &schemas)
        .expect("literal source must resolve");
    let qualified = compiler()
        .transpile_with_schemas(
            "in_schema(\"reporting\", \"orders\") %>% select(id)",
            &schemas,
        )
        .expect("qualified source must resolve");

    assert!(literal.sql.contains("FROM \"reporting.orders\""));
    assert!(qualified.sql.contains("FROM \"reporting\".\"orders\""));
}
