//! Optional execution protocol for compiled queries with pre-flight validation.
//!
//! This module defines a minimal, database-agnostic execution contract.  The
//! caller supplies a [`SnapshotExecutor`] implementation that guarantees a
//! single stable read snapshot; this module orchestrates the
//! begin → validate → fetch → commit / rollback lifecycle.
//!
//! # Volatile-input caveat
//!
//! The trait contract requires one stable read snapshot, but it **cannot**
//! freeze evaluation of volatile SQL functions (e.g. `RANDOM()`, `NOW()`) or
//! user-defined functions that the planner does not materialise or reject.
//! Adapters that need deterministic re-execution must ensure the planner
//! materialises or rejects such expressions before they reach this layer.

use crate::relational::CompiledQuery;
use thiserror::Error;

/// A compiled query together with optional validation queries that must
/// return zero rows before the main query is executed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionPlan {
    /// The main query to execute after all checks pass.
    pub query: CompiledQuery,
    /// Validation queries; each must return zero rows.  Empty = no checks.
    pub checks: Vec<ValidationQuery>,
}

impl ExecutionPlan {
    /// Create a plan with no validation checks.
    pub fn new(query: CompiledQuery) -> Self {
        Self {
            query,
            checks: Vec::new(),
        }
    }

    /// Add a validation query that must return zero rows.
    pub fn with_check(mut self, check: ValidationQuery) -> Self {
        self.checks.push(check);
        self
    }
}

/// A single validation query.  If `has_rows` returns `true` the
/// corresponding `message` is surfaced as a [`ExecutionError::Validation`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationQuery {
    /// SQL that should return zero rows when the plan is safe to execute.
    pub sql: String,
    /// Human-readable message returned when the check finds rows.
    pub message: String,
}

impl ValidationQuery {
    /// Create a validation query.
    pub fn new(sql: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            message: message.into(),
        }
    }
}

/// Errors produced by [`execute`].
#[derive(Debug, Error)]
pub enum ExecutionError<E: std::error::Error> {
    /// A validation query returned one or more rows.
    #[error("validation failed: {message}")]
    Validation { message: String },

    /// The executor failed while beginning the snapshot.
    #[error("failed to begin snapshot: {0}")]
    Begin(#[source] E),

    /// The executor failed while running a validation query.
    #[error("validation query failed: {0}")]
    Check(#[source] E),

    /// The executor failed while fetching result rows.
    #[error("fetch failed: {0}")]
    Fetch(#[source] E),

    /// The executor failed while committing the snapshot.
    #[error("commit failed: {0}")]
    Commit(#[source] E),

    /// The original operation failed and rollback also failed.
    /// Both errors are preserved.
    #[error("operation failed: {}; rollback also failed: {rollback}", .original.as_ref())]
    Rollback {
        /// The error that triggered the rollback.
        original: Box<ExecutionError<E>>,
        /// The error returned by the rollback attempt.
        rollback: E,
    },
}

/// A database adapter that can execute queries inside a single stable read
/// snapshot.
///
/// # Contract
///
/// Implementations **must** guarantee:
///
/// 1. **One stable read snapshot** — all queries issued between
///    `begin_snapshot` and `commit` / `rollback` see the same database
///    state.  A plain `BEGIN` on `READ COMMITTED` is **not** sufficient;
///    use `REPEATABLE READ`, `SERIALIZABLE`, or an equivalent mechanism.
/// 2. **Buffered results** — `fetch_all` must return the complete result set
///    before `commit` is called.  Streaming rows before validation or
///    commit is not permitted.
/// 3. **No side effects before commit** — the executor must not apply any
///    durable changes until `commit` is called.
pub trait SnapshotExecutor {
    /// The row type returned by [`SnapshotExecutor::fetch_all`].
    type Rows;
    /// The error type returned by all fallible methods.
    type Error: std::error::Error;

    /// Begin a stable read snapshot.
    fn begin_snapshot(&mut self) -> Result<(), Self::Error>;

    /// Return `true` if the given SQL returns at least one row.
    fn has_rows(&mut self, sql: &str) -> Result<bool, Self::Error>;

    /// Execute the given SQL and return all rows.
    fn fetch_all(&mut self, sql: &str) -> Result<Self::Rows, Self::Error>;

    /// Commit the current snapshot.
    fn commit(&mut self) -> Result<(), Self::Error>;

    /// Roll back the current snapshot.
    fn rollback(&mut self) -> Result<(), Self::Error>;
}

/// Execute an [`ExecutionPlan`] against the given executor.
///
/// Lifecycle:
/// 1. `begin_snapshot`
/// 2. Run each validation query via `has_rows`; abort on first violation
/// 3. `fetch_all` the main query
/// 4. `commit`
///
/// On any failure the snapshot is rolled back.  If rollback itself fails both
/// the original and rollback errors are preserved in
/// [`ExecutionError::Rollback`].
pub fn execute<E: SnapshotExecutor>(
    plan: &ExecutionPlan,
    executor: &mut E,
) -> Result<E::Rows, ExecutionError<E::Error>> {
    executor.begin_snapshot().map_err(ExecutionError::Begin)?;

    let outcome = run_plan(plan, executor).and_then(|rows| {
        executor
            .commit()
            .map_err(ExecutionError::Commit)
            .map(|_| rows)
    });
    match outcome {
        Ok(rows) => Ok(rows),
        Err(original) => match executor.rollback() {
            Ok(()) => Err(original),
            Err(rollback) => Err(ExecutionError::Rollback {
                original: Box::new(original),
                rollback,
            }),
        },
    }
}

fn run_plan<E: SnapshotExecutor>(
    plan: &ExecutionPlan,
    executor: &mut E,
) -> Result<E::Rows, ExecutionError<E::Error>> {
    for check in &plan.checks {
        let violated = executor
            .has_rows(&check.sql)
            .map_err(ExecutionError::Check)?;
        if violated {
            return Err(ExecutionError::Validation {
                message: check.message.clone(),
            });
        }
    }
    executor
        .fetch_all(&plan.query.sql)
        .map_err(ExecutionError::Fetch)
}
