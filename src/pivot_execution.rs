//! Runtime discovery for pivot columns under the same stable read snapshot.
use crate::{
    execute, relational, ExecutionError, SnapshotExecutor, SourceSchema, TranspileError, Transpiler,
};
use thiserror::Error;

/// The adapter returns scalar cells from the first column of a discovery query.
pub trait PivotExecutor: SnapshotExecutor {
    fn discover_values(&mut self, sql: &str) -> Result<Vec<serde_json::Value>, Self::Error>;
}

#[derive(Debug, Error)]
pub enum PivotExecutionError<E: std::error::Error> {
    #[error("failed to begin snapshot: {0}")]
    Begin(E),
    #[error("pivot key discovery failed: {0}")]
    Discovery(E),
    #[error("pivot compilation failed: {0}")]
    Compile(TranspileError),
    #[error(transparent)]
    Execute(ExecutionError<E>),
    #[error("{}; rollback also failed: {rollback}", .original.as_ref())]
    Rollback {
        original: Box<PivotExecutionError<E>>,
        rollback: E,
    },
}

/// Discovers unresolved `pivot_wider()` keys, compiles the query, then fetches
/// buffered rows. Every discovery, assertion, and result sees the same snapshot.
pub fn execute_with_pivot_discovery<E: PivotExecutor>(
    transpiler: &Transpiler,
    code: &str,
    schemas: &[SourceSchema],
    executor: &mut E,
) -> Result<E::Rows, PivotExecutionError<E::Error>> {
    let mut ast = transpiler
        .parse_dplyr(code)
        .map_err(TranspileError::from)
        .map_err(PivotExecutionError::Compile)?;
    executor
        .begin_snapshot()
        .map_err(PivotExecutionError::Begin)?;
    let result = (|| {
        while let Some(sql) = relational::pivot_keys(&ast, schemas, &transpiler.generator)
            .map_err(TranspileError::from)
            .map_err(PivotExecutionError::Compile)?
        {
            let keys = executor
                .discover_values(&sql)
                .map_err(PivotExecutionError::Discovery)?;
            relational::supply_pivot_keys(&mut ast, &keys)
                .map_err(TranspileError::from)
                .map_err(PivotExecutionError::Compile)?;
        }
        relational::compile_for_execution(&ast, schemas, &transpiler.generator)
            .map_err(TranspileError::from)
            .map_err(PivotExecutionError::Compile)
    })();
    match result {
        Ok(plan) => execute(&plan, &mut Started(executor)).map_err(PivotExecutionError::Execute),
        Err(original) => match executor.rollback() {
            Ok(()) => Err(original),
            Err(rollback) => Err(PivotExecutionError::Rollback {
                original: Box::new(original),
                rollback,
            }),
        },
    }
}

// The transaction has already started before discovery; execute owns its end.
struct Started<'a, E>(&'a mut E);
impl<E: SnapshotExecutor> SnapshotExecutor for Started<'_, E> {
    type Rows = E::Rows;
    type Error = E::Error;
    fn begin_snapshot(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn has_rows(&mut self, sql: &str) -> Result<bool, Self::Error> {
        self.0.has_rows(sql)
    }
    fn fetch_all(&mut self, sql: &str) -> Result<Self::Rows, Self::Error> {
        self.0.fetch_all(sql)
    }
    fn commit(&mut self) -> Result<(), Self::Error> {
        self.0.commit()
    }
    fn rollback(&mut self) -> Result<(), Self::Error> {
        self.0.rollback()
    }
}
