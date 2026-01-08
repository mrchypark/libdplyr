// SQL assembly helpers.

use std::collections::HashMap;

use super::{DplyrOperation, GenerationResult, SqlGenerator};

/// Struct to store SQL query components
#[derive(Debug, Default)]
pub(super) struct QueryParts {
    pub(super) select_columns: Vec<String>,
    pub(super) where_clauses: Vec<String>,
    pub(super) group_by: String,
    pub(super) order_by: String,
    pub(super) joins: Vec<String>,
    pub(super) mutated_columns: HashMap<String, String>,
}

impl QueryParts {
    pub(super) fn new() -> Self {
        Self::default()
    }
}

impl SqlGenerator {
    /// Handles nested pipeline processing for complex transformations.
    ///
    /// # Arguments
    ///
    /// * `operations` - Vector of operations in the nested pipeline
    ///
    /// # Returns
    ///
    /// Returns SQL for the nested pipeline
    pub fn generate_nested_pipeline(
        &self,
        operations: &[DplyrOperation],
    ) -> GenerationResult<String> {
        // Process nested operations recursively
        let mut nested_parts = QueryParts::new();

        for operation in operations {
            self.process_operation(operation, &mut nested_parts)?;
        }

        self.assemble_query(&None, &nested_parts)
    }

    /// Assembles the final SQL query.
    pub(super) fn assemble_query(
        &self,
        source: &Option<String>,
        parts: &QueryParts,
    ) -> GenerationResult<String> {
        let mut query = String::new();

        // SELECT clause
        query.push_str("SELECT ");
        if parts.select_columns.is_empty() {
            query.push('*');
        } else {
            query.push_str(&parts.select_columns.join(", "));
        }

        // FROM clause (using default table name)
        query.push_str("\nFROM ");
        let table_name = source.as_deref().unwrap_or("data");
        query.push_str(&self.dialect.quote_identifier(table_name));

        // JOIN clauses
        for join in &parts.joins {
            query.push('\n');
            query.push_str(join);
        }

        // WHERE clause
        if !parts.where_clauses.is_empty() {
            query.push_str("\nWHERE ");
            query.push_str(&parts.where_clauses.join(" "));
        }

        // GROUP BY clause
        if !parts.group_by.is_empty() {
            query.push_str("\nGROUP BY ");
            query.push_str(&parts.group_by);
        }

        // ORDER BY clause
        if !parts.order_by.is_empty() {
            query.push_str("\nORDER BY ");
            query.push_str(&parts.order_by);
        }

        Ok(query)
    }
}
