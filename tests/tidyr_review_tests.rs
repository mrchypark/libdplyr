use libdplyr::{SchemaColumn, SourceSchema, SqliteDialect, Transpiler};

fn transpile(
    code: &str,
    columns: Vec<SchemaColumn>,
) -> Result<libdplyr::CompiledQuery, libdplyr::TranspileError> {
    Transpiler::new(Box::new(SqliteDialect::new()))
        .transpile_with_schema(code, &SourceSchema::with_columns("data", columns))
}

fn typed(name: &str, data_type: &str, nullable: bool) -> SchemaColumn {
    SchemaColumn {
        name: name.to_string(),
        data_type: Some(data_type.to_string()),
        nullable: Some(nullable),
    }
}

#[test]
fn pivots_reject_unknown_options_and_surplus_positionals() {
    let columns = vec![
        SchemaColumn::new("id"),
        SchemaColumn::new("key"),
        SchemaColumn::new("value"),
    ];
    for code in [
        "data %>% pivot_longer(value, names_repair = 'minimal')",
        "data %>% pivot_longer(value, values_drop_na = TRUE)",
        "data %>% pivot_longer(value, key)",
        "data %>% pivot_wider(names_from = key, values_from = value, names_repair = 'minimal', keys = c('a'))",
        "data %>% pivot_wider(key, value, id, keys = c('a'))",
    ] {
        assert!(transpile(code, columns.clone()).is_err(), "{code}");
    }
}

#[test]
fn pivots_preserve_identity_metadata_and_exclude_pivot_inputs_from_id_selection() {
    let columns = vec![
        typed("id", "INTEGER", false),
        typed("key", "TEXT", true),
        typed("value", "DOUBLE", true),
    ];

    let longer = transpile(
        "data %>% pivot_longer(c(key, value), names_to = 'field', values_to = 'content')",
        columns.clone(),
    )
    .expect("pivot_longer compiles");
    assert_eq!(longer.columns[0], columns[0]);

    let wider = transpile(
        "data %>% pivot_wider(names_from = key, values_from = value, id_cols = everything(), keys = c('a'))",
        columns.clone(),
    )
    .expect("pivot_wider compiles");
    assert_eq!(wider.columns[0], columns[0]);
    assert_eq!(
        wider
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "a"]
    );
}

#[test]
fn complete_keeps_named_domain_assignments_and_pivot_wider_allows_mixed_arguments() {
    let columns = vec![
        SchemaColumn::new("id"),
        SchemaColumn::new("key"),
        SchemaColumn::new("value"),
    ];
    transpile(
        "data %>% complete(id, key = c('x', 'y'), fill = list(value = 0))",
        columns.clone(),
    )
    .expect("complete accepts named domains");
    transpile(
        "data %>% pivot_wider(names_from = key, value, keys = c('x'))",
        columns,
    )
    .expect("pivot_wider accepts mixed required arguments");
}
