// Mutate-related helpers.

use super::QueryParts;
use super::{ColumnExpr, Expr, GenerationResult, SqlGenerator};

impl SqlGenerator {
    /// Generates SELECT columns, inlining any columns created by previous mutate() calls.
    ///
    /// This allows pipelines like `mutate(x = a + b) %>% select(x)` to work by
    /// selecting the mutated expression with a stable alias.
    pub(super) fn generate_select_columns_with_mutations(
        &self,
        columns: &[ColumnExpr],
        parts: &QueryParts,
    ) -> GenerationResult<Vec<String>> {
        columns
            .iter()
            .map(|col| {
                let (expr_sql, implicit_alias) = match &col.expr {
                    Expr::Identifier(name) => {
                        if let Some(mutated_expr) = parts.mutated_columns.get(name) {
                            (mutated_expr.clone(), Some(name.as_str()))
                        } else {
                            (self.generate_expression(&col.expr)?, None)
                        }
                    }
                    _ => (self.generate_expression(&col.expr)?, None),
                };

                let alias = col.alias.as_deref().or(implicit_alias);
                if let Some(alias) = alias {
                    Ok(format!(
                        "{} AS {}",
                        expr_sql,
                        self.dialect.quote_identifier(alias)
                    ))
                } else {
                    Ok(expr_sql)
                }
            })
            .collect()
    }

    /// Processes mutate operations with support for complex expressions and subqueries.
    ///
    /// # Arguments
    ///
    /// * `assignments` - Vector of column assignments from mutate operation
    /// * `query_parts` - Mutable reference to query parts being built
    ///
    /// # Returns
    ///
    /// Returns Ok(()) on success, GenerationError on failure
    pub(super) fn process_mutate_operation(
        &self,
        assignments: &[crate::parser::Assignment],
        query_parts: &mut QueryParts,
    ) -> GenerationResult<()> {
        // Check if we need subqueries for complex expressions
        let needs_subquery = self.mutate_needs_subquery(assignments, query_parts);

        if needs_subquery {
            // For complex cases, we'll use a simpler approach for now
            // TODO: Implement full subquery/CTE support in future iterations
            self.process_simple_mutate(assignments, query_parts)
        } else {
            // Simple mutate - add columns to SELECT clause
            self.process_simple_mutate(assignments, query_parts)
        }
    }

    /// Determines if mutate operation needs subquery or CTE.
    pub(super) fn mutate_needs_subquery(
        &self,
        assignments: &[crate::parser::Assignment],
        query_parts: &QueryParts,
    ) -> bool {
        // Need subquery if:
        // 1. There are existing aggregations (GROUP BY + HAVING)
        // 2. Mutate expressions reference other mutated columns
        // 3. Complex window functions are used

        if !query_parts.group_by.is_empty() {
            return true;
        }

        // Check for column dependencies within mutate
        let mut defined_columns = std::collections::HashSet::new();
        for assignment in assignments {
            if self.expression_references_columns(&assignment.expr, &defined_columns) {
                return true;
            }
            defined_columns.insert(assignment.column.clone());
        }

        // Check for window functions or complex expressions
        for assignment in assignments {
            if self.expression_is_complex(&assignment.expr) {
                return true;
            }
        }

        false
    }

    /// Processes simple mutate operations by adding columns to SELECT clause.
    fn process_simple_mutate(
        &self,
        assignments: &[crate::parser::Assignment],
        query_parts: &mut QueryParts,
    ) -> GenerationResult<()> {
        // If no columns selected yet, implies all columns (*) are included
        if query_parts.select_columns.is_empty() {
            query_parts.select_columns.push("*".to_string());
        }

        for assignment in assignments {
            let expr_sql = self.generate_expression(&assignment.expr)?;
            query_parts
                .mutated_columns
                .insert(assignment.column.clone(), expr_sql.clone());
            let column_expr = format!(
                "{} AS {}",
                expr_sql,
                self.dialect.quote_identifier(&assignment.column)
            );
            query_parts.select_columns.push(column_expr);
        }
        Ok(())
    }

    /// Checks if expression references any of the given columns.
    #[allow(clippy::only_used_in_recursion)]
    pub(super) fn expression_references_columns(
        &self,
        expr: &Expr,
        columns: &std::collections::HashSet<String>,
    ) -> bool {
        match expr {
            Expr::Identifier(name) => columns.contains(name),
            Expr::Binary { left, right, .. } => {
                self.expression_references_columns(left, columns)
                    || self.expression_references_columns(right, columns)
            }
            Expr::Function { args, .. } => args
                .iter()
                .any(|arg| self.expression_references_columns(arg, columns)),
            Expr::Literal(_) => false,
        }
    }

    /// Checks if expression is complex and might need special handling.
    #[allow(clippy::only_used_in_recursion)]
    pub(super) fn expression_is_complex(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Function { name, .. } => {
                // Window functions or complex aggregations
                matches!(
                    name.to_lowercase().as_str(),
                    "row_number"
                        | "rank"
                        | "dense_rank"
                        | "lag"
                        | "lead"
                        | "first_value"
                        | "last_value"
                        | "nth_value"
                )
            }
            Expr::Binary { left, right, .. } => {
                self.expression_is_complex(left) || self.expression_is_complex(right)
            }
            _ => false,
        }
    }

    /// Generates a subquery for complex mutate operations.
    ///
    /// # Arguments
    ///
    /// * `base_query` - The base query to wrap in a subquery
    /// * `assignments` - Vector of column assignments from mutate operation
    ///
    /// # Returns
    ///
    /// Returns a SQL query with subquery structure
    pub fn generate_mutate_subquery(
        &self,
        base_query: &str,
        assignments: &[crate::parser::Assignment],
    ) -> GenerationResult<String> {
        let mut outer_select = Vec::new();

        // Add all existing columns (SELECT *)
        outer_select.push("*".to_string());

        // Add mutated columns
        for assignment in assignments {
            let column_expr = format!(
                "{} AS {}",
                self.generate_expression(&assignment.expr)?,
                self.dialect.quote_identifier(&assignment.column)
            );
            outer_select.push(column_expr);
        }

        let query = format!(
            "SELECT {}\nFROM (\n{}\n) AS subquery",
            outer_select.join(", "),
            base_query
        );

        Ok(query)
    }
}
