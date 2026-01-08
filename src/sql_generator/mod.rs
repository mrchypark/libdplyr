//! SQL generator module
//!
//! Provides functionality to convert AST to various SQL dialects.

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{
    Aggregation, BinaryOp, ColumnExpr, DplyrNode, DplyrOperation, Expr, JoinSpec, JoinType,
    LiteralValue, OrderDirection, OrderExpr, RenameSpec,
};

// Decomposition scaffolding (“Tidy First”): these modules are placeholders to
// enable incremental extraction from this large module without behavior changes.
pub mod assemble;
pub mod dialect;
pub mod mutate_support;

use assemble::QueryParts;

pub use dialect::{
    DialectConfig, DuckDbDialect, MySqlDialect, PostgreSqlDialect, SqlDialect, SqliteDialect,
};

/// SQL generator struct
pub struct SqlGenerator {
    dialect: Box<dyn SqlDialect>,
}

impl SqlGenerator {
    /// Creates a new SQL generator instance.
    ///
    /// # Arguments
    ///
    /// * `dialect` - The SQL dialect to use
    pub fn new(dialect: Box<dyn SqlDialect>) -> Self {
        Self { dialect }
    }

    /// Converts AST to SQL query.
    ///
    /// # Arguments
    ///
    /// * `ast` - The AST node to convert
    ///
    /// # Returns
    ///
    /// Returns SQL query string on success, GenerationError on failure.
    pub fn generate(&self, ast: &DplyrNode) -> GenerationResult<String> {
        match ast {
            DplyrNode::Pipeline {
                source, operations, ..
            } => self.generate_pipeline(source, operations),
            DplyrNode::DataSource { name, .. } => Ok(format!(
                "SELECT * FROM {}",
                self.dialect.quote_identifier(name)
            )),
        }
    }

    /// Converts pipeline to SQL.
    fn generate_pipeline(
        &self,
        source: &Option<String>,
        operations: &[DplyrOperation],
    ) -> GenerationResult<String> {
        if operations.is_empty() {
            return Err(GenerationError::InvalidAst {
                reason: "Empty pipeline: at least one operation is required".to_string(),
            });
        }

        let mut query_parts = QueryParts::new();

        // Process each operation in order
        for operation in operations {
            self.process_operation(operation, &mut query_parts)?;
        }

        // Assemble final SQL query
        self.assemble_query(source, &query_parts)
    }

    /// Processes individual operations.
    fn process_operation(
        &self,
        operation: &DplyrOperation,
        query_parts: &mut QueryParts,
    ) -> GenerationResult<()> {
        match operation {
            DplyrOperation::Select { columns, .. } => {
                query_parts.select_columns =
                    self.generate_select_columns_with_mutations(columns, query_parts)?;
            }
            DplyrOperation::Filter { condition, .. } => {
                let where_clause = self.generate_expression(condition)?;
                if query_parts.where_clauses.is_empty() {
                    query_parts.where_clauses.push(where_clause);
                } else {
                    query_parts
                        .where_clauses
                        .push(format!("AND ({where_clause})"));
                }
            }
            DplyrOperation::Mutate { assignments, .. } => {
                // Handle mutate operations - may need subqueries for complex cases
                self.process_mutate_operation(assignments, query_parts)?;
            }
            DplyrOperation::Rename { renames, .. } => {
                self.process_rename_operation(renames, query_parts)?;
            }
            DplyrOperation::Arrange { columns, .. } => {
                query_parts.order_by = self.generate_order_by(columns)?;
            }
            DplyrOperation::GroupBy { columns, .. } => {
                query_parts.group_by = columns
                    .iter()
                    .map(|col| self.dialect.quote_identifier(col))
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            DplyrOperation::Summarise { aggregations, .. } => {
                query_parts.select_columns = self.generate_aggregations(aggregations)?;
            }
            DplyrOperation::Join {
                join_type, spec, ..
            } => {
                self.process_join_operation(join_type, spec, query_parts)?;
            }
        }
        Ok(())
    }

    fn process_rename_operation(
        &self,
        renames: &[RenameSpec],
        query_parts: &mut QueryParts,
    ) -> GenerationResult<()> {
        if renames.is_empty() {
            return Err(GenerationError::InvalidAst {
                reason: "rename() requires at least one mapping".to_string(),
            });
        }

        let excluded = renames
            .iter()
            .map(|spec| spec.old_name.clone())
            .collect::<Vec<_>>();

        let star_exclude = self.dialect.select_star_exclude(&excluded).ok_or_else(|| {
            GenerationError::UnsupportedOperation {
                operation: "rename".to_string(),
                dialect: self.dialect.dialect_name().to_string(),
            }
        })?;

        if query_parts.select_columns.is_empty() {
            query_parts.select_columns.push(star_exclude);
        } else {
            let mut replaced_star = false;
            for col in &mut query_parts.select_columns {
                if col == "*" {
                    *col = star_exclude.clone();
                    replaced_star = true;
                }
            }
            if !replaced_star {
                return Err(GenerationError::InvalidAst {
                    reason:
                        "rename() currently requires an implicit '*' projection (no prior select())"
                            .to_string(),
                });
            }
        }

        for spec in renames {
            query_parts.select_columns.push(format!(
                "{} AS {}",
                self.dialect.quote_identifier(&spec.old_name),
                self.dialect.quote_identifier(&spec.new_name)
            ));
        }

        Ok(())
    }

    fn process_join_operation(
        &self,
        join_type: &JoinType,
        spec: &JoinSpec,
        query_parts: &mut QueryParts,
    ) -> GenerationResult<()> {
        use crate::parser::JoinType;

        let join_sql = match join_type {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
            JoinType::Full => "FULL JOIN",
            JoinType::Semi => "SEMI JOIN",
            JoinType::Anti => "ANTI JOIN",
        };

        let on_clause = self.generate_expression(&spec.on)?;

        if query_parts.joins.is_empty() {
            query_parts.joins.push(format!(
                "{} {} ON {}",
                join_sql,
                self.dialect.quote_identifier(&spec.table),
                on_clause
            ));
        } else {
            query_parts.joins.push(format!(
                "{} {} ON {}",
                join_sql,
                self.dialect.quote_identifier(&spec.table),
                on_clause
            ));
        }

        Ok(())
    }

    /// Generates ORDER BY clause.
    fn generate_order_by(&self, columns: &[OrderExpr]) -> GenerationResult<String> {
        let order_items: Result<Vec<_>, _> = columns
            .iter()
            .map(|col| {
                let direction = match col.direction {
                    OrderDirection::Asc => "ASC",
                    OrderDirection::Desc => "DESC",
                };
                Ok(format!(
                    "{} {}",
                    self.dialect.quote_identifier(&col.column),
                    direction
                ))
            })
            .collect();

        Ok(order_items?.join(", "))
    }

    /// Generates aggregate functions.
    fn generate_aggregations(&self, aggregations: &[Aggregation]) -> GenerationResult<Vec<String>> {
        aggregations
            .iter()
            .map(|agg| {
                let func_name = self.dialect.aggregate_function(&agg.function);
                let column_ref = if agg.function.to_lowercase() == "n" {
                    String::new() // COUNT(*) is already included in function name
                } else {
                    self.dialect.quote_identifier(&agg.column)
                };

                let expr = if column_ref.is_empty() {
                    func_name
                } else {
                    format!("{func_name}({column_ref})")
                };

                if let Some(alias) = &agg.alias {
                    Ok(format!(
                        "{} AS {}",
                        expr,
                        self.dialect.quote_identifier(alias)
                    ))
                } else {
                    Ok(expr)
                }
            })
            .collect()
    }

    /// Converts expressions to SQL.
    fn generate_expression(&self, expr: &Expr) -> GenerationResult<String> {
        match expr {
            Expr::Identifier(name) => Ok(self.dialect.quote_identifier(name)),
            Expr::Literal(literal) => self.generate_literal(literal),
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_sql = self.generate_expression(left)?;
                let right_sql = self.generate_expression(right)?;
                let op_sql = self.generate_binary_operator(operator);
                Ok(format!("({left_sql} {op_sql} {right_sql})"))
            }
            Expr::Function { name, args } => {
                let args_sql: Result<Vec<_>, _> = args
                    .iter()
                    .map(|arg| self.generate_expression(arg))
                    .collect();
                let args_str = args_sql?.join(", ");
                let func_name = name.to_uppercase();
                Ok(format!("{func_name}({args_str})"))
            }
        }
    }

    /// Converts literal values to SQL.
    fn generate_literal(&self, literal: &LiteralValue) -> GenerationResult<String> {
        match literal {
            LiteralValue::String(s) => Ok(self.dialect.quote_string(s)),
            LiteralValue::Number(n) => Ok(n.to_string()),
            LiteralValue::Boolean(b) => Ok(if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }),
            LiteralValue::Null => Ok("NULL".to_string()),
        }
    }

    /// Converts binary operators to SQL.
    fn generate_binary_operator(&self, operator: &BinaryOp) -> &'static str {
        match operator {
            BinaryOp::Equal => "=",
            BinaryOp::NotEqual => "!=",
            BinaryOp::LessThan => "<",
            BinaryOp::LessThanOrEqual => "<=",
            BinaryOp::GreaterThan => ">",
            BinaryOp::GreaterThanOrEqual => ">=",
            BinaryOp::And => "AND",
            BinaryOp::Or => "OR",
            BinaryOp::Plus => "+",
            BinaryOp::Minus => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
        }
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
