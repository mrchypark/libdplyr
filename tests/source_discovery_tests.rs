use libdplyr::{DuckDbDialect, SchemaInput, SourceRef, SourceSchema, Transpiler};

fn transpiler() -> Transpiler {
    Transpiler::new(Box::new(DuckDbDialect::new()))
}

#[test]
fn discovers_every_input_once_in_pipeline_order() {
    let sources = transpiler().required_sources(
        "left %>% inner_join(right, by = \"id\") %>% union(extra) %>% semi_join(right, by = \"id\")",
    ).expect("source discovery");
    assert_eq!(sources, ["left", "right", "extra"].map(SourceRef::from));
}

#[test]
fn source_discovery_uses_structural_identity_and_validates_constructors() {
    let sources = transpiler().required_sources(
        r#"sales.orders %>% inner_join(in_schema("sales", "orders"), by = "id") %>% union_all(in_schema("sales", "orders") %>% filter(id > 0)) %>% filter(id == "sales.orders")"#,
    ).expect("mixed sources");
    assert_eq!(
        sources,
        [
            SourceRef::from("sales.orders"),
            SourceRef::Qualified(vec!["sales".into(), "orders".into()]),
        ]
    );
    for source in ["a.b.c.d", "a..b", "in_schema"] {
        assert_eq!(
            transpiler()
                .required_sources(source)
                .expect("literal source"),
            [SourceRef::from(source)]
        );
    }
    for source in [
        r#"in_schema("sales")"#,
        r#"in_schema("", "orders")"#,
        r#"in_catalog("sales", "orders")"#,
        r#"in_schema(schema, "orders")"#,
        r#"in_schema(sql("sales"), "orders")"#,
    ] {
        assert!(transpiler().required_sources(source).is_err(), "{source}");
        assert!(
            transpiler()
                .required_sources(&format!("data %>% union_all({source})"))
                .is_err(),
            "{source}"
        );
    }
}

#[test]
fn discovery_rejects_missing_source_and_table_assignment() {
    assert!(transpiler().required_sources("select(id)").is_err());
    assert!(transpiler()
        .required_sources("output <- data %>% select(id)")
        .is_err());
}

#[test]
fn metadata_json_preserves_the_existing_single_source_format() {
    let single: SchemaInput =
        serde_json::from_str(r#"{"source":"data","columns":[{"name":"id"}]}"#)
            .expect("single metadata object");
    let multiple: SchemaInput =
        serde_json::from_str(r#"[{"source":"data","columns":[{"name":"id"}]}]"#)
            .expect("metadata array");
    assert_eq!(single.as_slice(), multiple.as_slice());
    assert_eq!(
        transpiler()
            .transpile_with_schemas("data %>% select(id)", multiple.as_slice())
            .expect("array"),
        transpiler()
            .transpile_with_schema("data %>% select(id)", &single.as_slice()[0])
            .expect("single"),
    );
}

#[test]
fn rejects_empty_duplicate_and_missing_input_metadata() {
    let compiler = transpiler();
    let schema = SourceSchema::new("data", ["id"]);
    assert!(compiler
        .transpile_with_schemas("data %>% select(id)", &[])
        .is_err());
    assert!(compiler
        .transpile_with_schemas("data %>% select(id)", &[schema.clone(), schema.clone()])
        .is_err());
    let error = compiler
        .transpile_with_schemas("data %>% inner_join(other, by = \"id\")", &[schema])
        .expect_err("missing right metadata");
    assert!(error.to_string().contains("other"));
}
