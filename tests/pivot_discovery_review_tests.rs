use libdplyr::{
    execute_with_pivot_discovery, PivotExecutor, SnapshotExecutor, SourceSchema, SqliteDialect,
    Transpiler,
};
use std::io;

#[derive(Default)]
struct Database {
    calls: Vec<&'static str>,
    discovery_sql: String,
    result_sql: String,
}

impl SnapshotExecutor for Database {
    type Rows = String;
    type Error = io::Error;

    fn begin_snapshot(&mut self) -> Result<(), Self::Error> {
        self.calls.push("begin");
        Ok(())
    }

    fn has_rows(&mut self, _: &str) -> Result<bool, Self::Error> {
        Ok(false)
    }

    fn fetch_all(&mut self, sql: &str) -> Result<Self::Rows, Self::Error> {
        self.calls.push("fetch");
        self.result_sql = sql.into();
        Ok(sql.into())
    }

    fn commit(&mut self) -> Result<(), Self::Error> {
        self.calls.push("commit");
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), Self::Error> {
        self.calls.push("rollback");
        Ok(())
    }
}

impl PivotExecutor for Database {
    fn discover_values(&mut self, sql: &str) -> Result<Vec<serde_json::Value>, Self::Error> {
        self.calls.push("discover");
        self.discovery_sql = sql.into();
        Ok(vec![serde_json::json!("x"), serde_json::json!("y")])
    }
}

fn transpiler() -> Transpiler {
    Transpiler::new(Box::new(SqliteDialect::new()))
}

fn execute(code: &str, schemas: &[SourceSchema]) -> Database {
    let mut database = Database::default();
    execute_with_pivot_discovery(&transpiler(), code, schemas, &mut database)
        .expect("dynamic pivot compiles and executes");
    assert_eq!(database.calls, ["begin", "discover", "fetch", "commit"]);
    database
}

#[test]
fn discovery_uses_first_positional_names_from_after_named_arguments() {
    let schemas = [SourceSchema::new("data", ["id", "key", "value"])];
    for code in [
        "data %>% pivot_wider(values_from = value, key)",
        "data %>% pivot_wider(id_cols = id, values_from = value, key)",
    ] {
        let database = execute(code, &schemas);
        assert!(database.discovery_sql.contains("\"key\""), "{code}");
        assert!(database.result_sql.contains("\"x\""), "{code}");
        assert!(database.result_sql.contains("\"y\""), "{code}");
    }
}

#[test]
fn standalone_bind_queries_discovers_from_its_first_pipeline_relation() {
    let schemas = [
        SourceSchema::new("other", ["id", "key", "value"]),
        SourceSchema::new("keys", ["id", "key", "value"]),
    ];
    let database = execute(
        "bind_queries(keys %>% filter(id > 0)) %>% pivot_wider(id_cols = id, values_from = value, key)",
        &schemas,
    );
    assert!(database.discovery_sql.contains("FROM \"keys\""));
    assert!(database.discovery_sql.contains("\"id\" > 0"));
    assert!(!database.discovery_sql.contains("\"other\""));
    assert!(database.result_sql.contains("FROM \"keys\""));
    assert!(!database.result_sql.contains("\"other\""));
}
