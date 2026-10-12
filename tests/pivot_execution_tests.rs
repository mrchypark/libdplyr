use libdplyr::{
    execute_with_pivot_discovery, PivotExecutionError, PivotExecutor, SnapshotExecutor,
    SourceSchema, SqliteDialect, Transpiler,
};
use std::io;

#[derive(Default)]
struct Database {
    calls: Vec<&'static str>,
    sql: String,
    fail_discovery: bool,
    discovered_values: Vec<serde_json::Value>,
}
impl SnapshotExecutor for Database {
    type Rows = String;
    type Error = io::Error;
    fn begin_snapshot(&mut self) -> Result<(), Self::Error> {
        self.calls.push("begin");
        Ok(())
    }
    fn has_rows(&mut self, _: &str) -> Result<bool, Self::Error> {
        self.calls.push("check");
        Ok(false)
    }
    fn fetch_all(&mut self, sql: &str) -> Result<String, Self::Error> {
        self.calls.push("fetch");
        self.sql = sql.into();
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
        assert!(sql.contains("DISTINCT"));
        if self.fail_discovery {
            return Err(io::Error::other("discovery failed"));
        }
        if self.discovered_values.is_empty() {
            Ok(vec![serde_json::json!("x"), serde_json::json!("y'quoted")])
        } else {
            Ok(self.discovered_values.clone())
        }
    }
}
fn transpiler() -> Transpiler {
    Transpiler::new(Box::new(SqliteDialect::new()))
}
fn schemas() -> Vec<SourceSchema> {
    vec![SourceSchema::new("data", ["id", "key", "value"])]
}
#[test]
fn discovery_and_result_share_one_snapshot() {
    let mut database = Database::default();
    let sql = execute_with_pivot_discovery(
        &transpiler(),
        "data %>% pivot_wider(names_from = key, values_from = value)",
        &schemas(),
        &mut database,
    )
    .expect("dynamic pivot");
    assert_eq!(database.calls, ["begin", "discover", "fetch", "commit"]);
    assert!(sql.contains("y''quoted"));
}
#[test]
fn discovery_failure_rolls_back_without_fetch() {
    let mut database = Database {
        fail_discovery: true,
        ..Default::default()
    };
    let error = execute_with_pivot_discovery(
        &transpiler(),
        "data %>% pivot_wider(names_from = key, values_from = value)",
        &schemas(),
        &mut database,
    )
    .expect_err("discovery failure");
    assert!(matches!(error, PivotExecutionError::Discovery(_)));
    assert_eq!(database.calls, ["begin", "discover", "rollback"]);
}
#[test]
fn fixed_keys_do_not_trigger_discovery() {
    let mut database = Database::default();
    execute_with_pivot_discovery(
        &transpiler(),
        "data %>% pivot_wider(names_from = key, values_from = value, keys = c(\"x\"))",
        &schemas(),
        &mut database,
    )
    .expect("fixed pivot");
    assert_eq!(database.calls, ["begin", "fetch", "commit"]);
}

#[test]
fn discovery_rejects_lossy_integer_keys() {
    for value in [serde_json::json!(i64::MAX), serde_json::json!(u64::MAX)] {
        let mut database = Database {
            discovered_values: vec![value],
            ..Default::default()
        };
        let error = execute_with_pivot_discovery(
            &transpiler(),
            "data %>% pivot_wider(names_from = key, values_from = value)",
            &schemas(),
            &mut database,
        )
        .expect_err("lossy pivot integer");
        assert!(matches!(error, PivotExecutionError::Compile(_)));
        assert_eq!(database.calls, ["begin", "discover", "rollback"]);
    }
}

#[test]
fn discovery_keeps_exact_integer_keys() {
    let mut database = Database {
        discovered_values: vec![
            serde_json::json!(-9_007_199_254_740_992_i64),
            serde_json::json!(9_007_199_254_740_992_u64),
        ],
        ..Default::default()
    };
    let sql = execute_with_pivot_discovery(
        &transpiler(),
        "data %>% pivot_wider(names_from = key, values_from = value)",
        &schemas(),
        &mut database,
    )
    .expect("exact pivot integers");
    assert!(sql.contains("-9007199254740992"));
    assert!(sql.contains("9007199254740992"));
    assert_eq!(database.calls, ["begin", "discover", "fetch", "commit"]);
}
