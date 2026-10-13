use libdplyr::{
    execute_with_pivot_discovery, PivotExecutor, SnapshotExecutor, SourceSchema, SqliteDialect,
    Transpiler,
};
use std::io;

#[derive(Default)]
struct RecordingDatabase {
    discovery_sql: String,
}

impl SnapshotExecutor for RecordingDatabase {
    type Rows = String;
    type Error = io::Error;

    fn begin_snapshot(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn has_rows(&mut self, _: &str) -> Result<bool, Self::Error> {
        Ok(false)
    }

    fn fetch_all(&mut self, sql: &str) -> Result<Self::Rows, Self::Error> {
        Ok(sql.into())
    }

    fn commit(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl PivotExecutor for RecordingDatabase {
    fn discover_values(&mut self, sql: &str) -> Result<Vec<serde_json::Value>, Self::Error> {
        self.discovery_sql = sql.into();
        Ok(vec![serde_json::json!("small")])
    }
}

fn transpiler() -> Transpiler {
    Transpiler::new(Box::new(SqliteDialect::new()))
}

fn assert_explicit_sources_only(code: &str) {
    let schemas = [
        SourceSchema::new("decoy", ["id", "key", "value"]),
        SourceSchema::new("b", ["id", "key", "value"]),
        SourceSchema::new("a", ["id", "key", "value"]),
    ];
    let mut database = RecordingDatabase::default();

    execute_with_pivot_discovery(&transpiler(), code, &schemas, &mut database)
        .expect("standalone bind query discovery");

    assert!(database.discovery_sql.contains("FROM \"a\""));
    assert!(database.discovery_sql.contains("FROM \"b\""));
    assert!(!database.discovery_sql.contains("\"decoy\""));
    assert_eq!(database.discovery_sql.matches("FROM \"a\"").count(), 1);
    assert_eq!(database.discovery_sql.matches("FROM \"b\"").count(), 1);
}

#[test]
fn standalone_summary_discovery_uses_each_explicit_source_once_under_schema_reorder() {
    for code in [
        "bind_queries(a, b) %>% summarise(key = n(), value = sum(value)) %>% pivot_wider(key, value)",
        "bind_queries(all = TRUE, a %>% filter(id > 0), b) %>% summarise(key = n(), value = sum(value)) %>% pivot_wider(key, value)",
    ] {
        assert_explicit_sources_only(code);
    }
}
