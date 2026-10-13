use libdplyr::{
    execute_with_pivot_discovery, ExecutionError, PivotExecutionError, PivotExecutor,
    SnapshotExecutor, SourceSchema, SqliteDialect, Transpiler,
};
use std::io;

#[derive(Default)]
struct Database {
    calls: Vec<&'static str>,
    violation: bool,
    fail_check: bool,
    fail_rollback: bool,
}
impl SnapshotExecutor for Database {
    type Rows = ();
    type Error = io::Error;
    fn begin_snapshot(&mut self) -> Result<(), Self::Error> {
        self.calls.push("begin");
        Ok(())
    }
    fn has_rows(&mut self, sql: &str) -> Result<bool, Self::Error> {
        self.calls.push("check");
        assert!(sql.contains("FLOOR"), "{sql}");
        if self.fail_check {
            Err(io::Error::other("weight check failed"))
        } else {
            Ok(self.violation)
        }
    }
    fn fetch_all(&mut self, _: &str) -> Result<(), Self::Error> {
        self.calls.push("fetch");
        Ok(())
    }
    fn commit(&mut self) -> Result<(), Self::Error> {
        self.calls.push("commit");
        Ok(())
    }
    fn rollback(&mut self) -> Result<(), Self::Error> {
        self.calls.push("rollback");
        if self.fail_rollback {
            Err(io::Error::other("rollback failed"))
        } else {
            Ok(())
        }
    }
}
impl PivotExecutor for Database {
    fn discover_values(&mut self, _: &str) -> Result<Vec<serde_json::Value>, Self::Error> {
        self.calls.push("discover");
        Ok(vec![serde_json::json!("x")])
    }
}
fn execute(database: &mut Database) -> Result<(), PivotExecutionError<io::Error>> {
    let transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
    execute_with_pivot_discovery(
        &transpiler,
        "bind_queries(first %>% dbplyr_uncount(w), second) %>% pivot_wider(key, value)",
        &[
            SourceSchema::new("second", ["id", "key", "value"]),
            SourceSchema::new("first", ["id", "key", "value", "w"]),
        ],
        database,
    )
}
#[test]
fn invalid_weights_prevent_pivot_discovery() {
    let mut database = Database {
        violation: true,
        ..Default::default()
    };
    assert!(matches!(
        execute(&mut database),
        Err(PivotExecutionError::Execute(
            ExecutionError::Validation { .. }
        ))
    ));
    assert_eq!(database.calls, ["begin", "check", "rollback"]);
}
#[test]
fn prerequisite_check_error_prevents_discovery() {
    let mut database = Database {
        fail_check: true,
        ..Default::default()
    };
    let error = execute(&mut database).expect_err("check failure");
    assert!(matches!(
        error,
        PivotExecutionError::Execute(ExecutionError::Check(_))
    ));
    assert_eq!(database.calls, ["begin", "check", "rollback"]);
}
#[test]
fn prerequisites_run_before_discovery_and_result() {
    let mut database = Database::default();
    execute(&mut database).expect("valid weighted pivot");
    assert_eq!(
        database.calls,
        ["begin", "check", "discover", "check", "fetch", "commit"]
    );
}
#[test]
fn prerequisite_violation_survives_rollback_failure() {
    let mut database = Database {
        violation: true,
        fail_rollback: true,
        ..Default::default()
    };
    let error = execute(&mut database).expect_err("invalid weights and rollback failure");
    match error {
        PivotExecutionError::Rollback { original, rollback } => {
            assert!(matches!(
                *original,
                PivotExecutionError::Execute(ExecutionError::Validation { .. })
            ));
            assert_eq!(rollback.to_string(), "rollback failed");
        }
        other => panic!("unexpected error: {other}"),
    }
    assert_eq!(database.calls, ["begin", "check", "rollback"]);
}

#[test]
fn prerequisites_run_before_each_dynamic_pivot() {
    let mut database = Database::default();
    let transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
    execute_with_pivot_discovery(
        &transpiler,
        "first %>% dbplyr_uncount(w) %>% pivot_wider(key, value) %>% pivot_longer(x, names_to = \"key\", values_to = \"value\") %>% pivot_wider(key, value)",
        &[SourceSchema::new("first", ["id", "key", "value", "w"])],
        &mut database,
    ).expect("two pivots in one snapshot");
    assert_eq!(
        database.calls,
        ["begin", "check", "discover", "check", "discover", "check", "fetch", "commit"]
    );
}

#[test]
fn later_pivot_argument_error_rolls_back_without_result() {
    let mut database = Database::default();
    let transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
    let error = execute_with_pivot_discovery(
        &transpiler,
        "first %>% dbplyr_uncount(w) %>% pivot_wider(key, value, unknown = TRUE)",
        &[SourceSchema::new("first", ["id", "key", "value", "w"])],
        &mut database,
    )
    .expect_err("unknown pivot option");
    assert!(matches!(error, PivotExecutionError::Compile(_)));
    assert_eq!(database.calls, ["begin", "check", "discover", "rollback"]);
}
