//! External-boundary tests for the execution protocol.
//!
//! Uses a configurable fake executor to verify call order, validation
//! short-circuit, and rollback semantics without a real database.

use libdplyr::execution::{
    execute, ExecutionError, ExecutionPlan, SnapshotExecutor, ValidationQuery,
};
use libdplyr::relational::CompiledQuery;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
struct FakeError(&'static str);

impl fmt::Display for FakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fake error: {}", self.0)
    }
}

impl Error for FakeError {}

#[derive(Debug, Clone, PartialEq, Default)]
struct FakeRows(Vec<Vec<String>>);

#[derive(Debug, Default)]
struct FakeExecutor {
    calls: Vec<&'static str>,
    check_results: Vec<bool>,
    fail_begin: bool,
    fail_check: bool,
    fail_fetch: bool,
    fail_commit: bool,
    fail_rollback: bool,
    rows: FakeRows,
}

impl FakeExecutor {
    fn new() -> Self {
        Self::default()
    }

    fn with_check_results(mut self, results: Vec<bool>) -> Self {
        self.check_results = results;
        self
    }

    fn with_rows(mut self, rows: Vec<Vec<String>>) -> Self {
        self.rows = FakeRows(rows);
        self
    }

    fn fail_begin(mut self) -> Self {
        self.fail_begin = true;
        self
    }

    fn fail_check(mut self) -> Self {
        self.fail_check = true;
        self
    }

    fn fail_fetch(mut self) -> Self {
        self.fail_fetch = true;
        self
    }

    fn fail_commit(mut self) -> Self {
        self.fail_commit = true;
        self
    }

    fn fail_rollback(mut self) -> Self {
        self.fail_rollback = true;
        self
    }

    fn call_order(&self) -> &[&'static str] {
        &self.calls
    }
}

impl SnapshotExecutor for FakeExecutor {
    type Rows = FakeRows;
    type Error = FakeError;

    fn begin_snapshot(&mut self) -> Result<(), Self::Error> {
        self.calls.push("begin");
        if self.fail_begin {
            return Err(FakeError("begin"));
        }
        Ok(())
    }

    fn has_rows(&mut self, _sql: &str) -> Result<bool, Self::Error> {
        self.calls.push("has_rows");
        if self.fail_check {
            return Err(FakeError("check"));
        }
        Ok(self.check_results.remove(0))
    }

    fn fetch_all(&mut self, _sql: &str) -> Result<Self::Rows, Self::Error> {
        self.calls.push("fetch_all");
        if self.fail_fetch {
            return Err(FakeError("fetch"));
        }
        Ok(self.rows.clone())
    }

    fn commit(&mut self) -> Result<(), Self::Error> {
        self.calls.push("commit");
        if self.fail_commit {
            return Err(FakeError("commit"));
        }
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), Self::Error> {
        self.calls.push("rollback");
        if self.fail_rollback {
            return Err(FakeError("rollback"));
        }
        Ok(())
    }
}

fn compiled_query() -> CompiledQuery {
    CompiledQuery {
        sql: "SELECT id FROM data".to_string(),
        columns: vec![],
        stages: 1,
    }
}

fn plan_with_checks(checks: Vec<ValidationQuery>) -> ExecutionPlan {
    ExecutionPlan {
        query: compiled_query(),
        checks,
    }
}

fn check(sql: &str, msg: &str) -> ValidationQuery {
    ValidationQuery::new(sql, msg)
}

#[test]
fn happy_path_call_order() {
    let plan = plan_with_checks(vec![check("SELECT 1", "c1"), check("SELECT 2", "c2")]);
    let mut exec = FakeExecutor::new()
        .with_check_results(vec![false, false])
        .with_rows(vec![vec!["1".to_string()]]);

    let rows = execute(&plan, &mut exec).expect("execute should succeed");

    assert_eq!(rows.0, vec![vec!["1".to_string()]]);
    assert_eq!(
        exec.call_order(),
        &["begin", "has_rows", "has_rows", "fetch_all", "commit"]
    );
}

#[test]
fn no_checks_skips_has_rows() {
    let plan = ExecutionPlan::new(compiled_query());
    let mut exec = FakeExecutor::new().with_rows(vec![]);

    let rows = execute(&plan, &mut exec).expect("execute should succeed");
    assert!(rows.0.is_empty());
    assert_eq!(exec.call_order(), &["begin", "fetch_all", "commit"]);
}

#[test]
fn validation_violation_rolls_back_without_fetch_or_commit() {
    let plan = plan_with_checks(vec![
        check("SELECT 1", "first"),
        check("SELECT 2", "second"),
    ]);
    let mut exec = FakeExecutor::new().with_check_results(vec![false, true]);

    let err = execute(&plan, &mut exec).expect_err("should fail");

    match err {
        ExecutionError::Validation { message } => assert_eq!(message, "second"),
        other => panic!("expected Validation, got {other:?}"),
    }
    assert_eq!(
        exec.call_order(),
        &["begin", "has_rows", "has_rows", "rollback"]
    );
}

#[test]
fn first_violation_short_circuits_remaining_checks() {
    let plan = plan_with_checks(vec![
        check("SELECT 1", "first"),
        check("SELECT 2", "second"),
    ]);
    let mut exec = FakeExecutor::new().with_check_results(vec![true, false]);

    let err = execute(&plan, &mut exec).expect_err("should fail");

    match err {
        ExecutionError::Validation { message } => assert_eq!(message, "first"),
        other => panic!("expected Validation, got {other:?}"),
    }
    assert_eq!(exec.call_order(), &["begin", "has_rows", "rollback"]);
}

#[test]
fn fetch_failure_rolls_back() {
    let plan = ExecutionPlan::new(compiled_query());
    let mut exec = FakeExecutor::new().fail_fetch();

    let err = execute(&plan, &mut exec).expect_err("should fail");

    assert!(matches!(err, ExecutionError::Fetch(_)));
    assert_eq!(exec.call_order(), &["begin", "fetch_all", "rollback"]);
}

#[test]
fn commit_failure_rolls_back() {
    let plan = ExecutionPlan::new(compiled_query());
    let mut exec = FakeExecutor::new().fail_commit();

    let err = execute(&plan, &mut exec).expect_err("should fail");

    assert!(matches!(err, ExecutionError::Commit(_)));
    assert_eq!(
        exec.call_order(),
        &["begin", "fetch_all", "commit", "rollback"]
    );
}

#[test]
fn begin_failure_does_not_roll_back() {
    let plan = ExecutionPlan::new(compiled_query());
    let mut exec = FakeExecutor::new().fail_begin();

    let err = execute(&plan, &mut exec).expect_err("should fail");

    assert!(matches!(err, ExecutionError::Begin(_)));
    assert_eq!(exec.call_order(), &["begin"]);
}

#[test]
fn check_failure_rolls_back() {
    let plan = plan_with_checks(vec![check("SELECT 1", "c1")]);
    let mut exec = FakeExecutor::new().fail_check();

    let err = execute(&plan, &mut exec).expect_err("should fail");

    assert!(matches!(err, ExecutionError::Check(_)));
    assert_eq!(exec.call_order(), &["begin", "has_rows", "rollback"]);
}

#[test]
fn rollback_failure_preserves_both_errors() {
    let plan = ExecutionPlan::new(compiled_query());
    let mut exec = FakeExecutor::new().fail_fetch().fail_rollback();

    let err = execute(&plan, &mut exec).expect_err("should fail");

    match err {
        ExecutionError::Rollback { original, rollback } => {
            assert!(matches!(*original, ExecutionError::Fetch(_)));
            assert_eq!(rollback, FakeError("rollback"));
        }
        other => panic!("expected Rollback, got {other:?}"),
    }
    assert_eq!(exec.call_order(), &["begin", "fetch_all", "rollback"]);
}

#[test]
fn rollback_failure_after_commit_failure_preserves_both() {
    let plan = ExecutionPlan::new(compiled_query());
    let mut exec = FakeExecutor::new().fail_commit().fail_rollback();

    let err = execute(&plan, &mut exec).expect_err("should fail");

    match err {
        ExecutionError::Rollback { original, rollback } => {
            assert!(matches!(*original, ExecutionError::Commit(_)));
            assert_eq!(rollback, FakeError("rollback"));
        }
        other => panic!("expected Rollback, got {other:?}"),
    }
    assert_eq!(
        exec.call_order(),
        &["begin", "fetch_all", "commit", "rollback"]
    );
}

#[test]
fn validation_error_display_includes_message() {
    let err = ExecutionError::<FakeError>::Validation {
        message: "row exists".to_string(),
    };
    assert_eq!(err.to_string(), "validation failed: row exists");
}
