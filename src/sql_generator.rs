//! SQL generator module
//!
//! Provides functionality to convert AST to various SQL dialects.

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{
    DplyrNode, DplyrOperation, Expr, LiteralValue, BinaryOp,
    ColumnExpr, OrderExpr, OrderDirection, Aggregation
};

/// SQL dialect trait
///
/// Interface for handling specialized SQL syntax for each database.
pub trait SqlDialect {
    /// Quotes identifiers (e.g., "column_name" or `column_name`)
    fn quote_identifier(&self, name: &str) -> String;
    
    /// Quotes string literals
    fn quote_string(&self, value: &str) -> String;
    
    /// Generates LIMIT clause
    fn limit_clause(&self, limit: usize) -> String;
    
    /// Generates string concatenation operation
    fn string_concat(&self, left: &str, right: &str) -> String;
    
    /// Returns dialect-specific aggregate function names
    fn aggregate_function(&self, function: &str) -> String;
    
    /// Returns whether case sensitivity is enabled
    fn is_case_sensitive(&self) -> bool;
}

/// PostgreSQL dialect implementation
#[derive(Debug, Clone)]
pub struct PostgreSqlDialect;

impl SqlDialect for PostgreSqlDialect {
    fn quote_identifier(&self, name: &str) -> String {
        format!("\"{}\"", name)
    }
    
    fn quote_string(&self, value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }
    
    fn limit_clause(&self, limit: usize) -> String {
        format!("LIMIT {}", limit)
    }
    
    fn string_concat(&self, left: &str, right: &str) -> String {
        format!("{} || {}", left, right)
    }
    
    fn aggregate_function(&self, function: &str) -> String {
        match function.to_lowercase().as_str() {
            "mean" | "avg" => "AVG".to_string(),
            "sum" => "SUM".to_string(),
            "count" => "COUNT".to_string(),
            "min" => "MIN".to_string(),
            "max" => "MAX".to_string(),
            "n" => "COUNT(*)".to_string(),
            _ => function.to_uppercase(),
        }
    }
    
    fn is_case_sensitive(&self) -> bool {
        false
    }
}

/// MySQL dialect implementation
#[derive(Debug, Clone)]
pub struct MySqlDialect;

impl SqlDialect for MySqlDialect {
    fn quote_identifier(&self, name: &str) -> String {
        format!("`{}`", name)
    }
    
    fn quote_string(&self, value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }
    
    fn limit_clause(&self, limit: usize) -> String {
        format!("LIMIT {}", limit)
    }
    
    fn string_concat(&self, left: &str, right: &str) -> String {
        format!("CONCAT({}, {})", left, right)
    }
    
    fn aggregate_function(&self, function: &str) -> String {
        match function.to_lowercase().as_str() {
            "mean" | "avg" => "AVG".to_string(),
            "sum" => "SUM".to_string(),
            "count" => "COUNT".to_string(),
            "min" => "MIN".to_string(),
            "max" => "MAX".to_string(),
            "n" => "COUNT(*)".to_string(),
            _ => function.to_uppercase(),
        }
    }
    
    fn is_case_sensitive(&self) -> bool {
        false
    }
}

/// SQLite dialect implementation
#[derive(Debug, Clone)]
pub struct SqliteDialect;

impl SqlDialect for SqliteDialect {
    fn quote_identifier(&self, name: &str) -> String {
        format!("\"{}\"", name)
    }
    
    fn quote_string(&self, value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }
    
    fn limit_clause(&self, limit: usize) -> String {
        format!("LIMIT {}", limit)
    }
    
    fn string_concat(&self, left: &str, right: &str) -> String {
        format!("{} || {}", left, right)
    }
    
    fn aggregate_function(&self, function: &str) -> String {
        match function.to_lowercase().as_str() {
            "mean" | "avg" => "AVG".to_string(),
            "sum" => "SUM".to_string(),
            "count" => "COUNT".to_string(),
            "min" => "MIN".to_string(),
            "max" => "MAX".to_string(),
            "n" => "COUNT(*)".to_string(),
            _ => function.to_uppercase(),
        }
    }
    
    fn is_case_sensitive(&self) -> bool {
        false
    }
}

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
            DplyrNode::Pipeline { operations, .. } => {
                self.generate_pipeline(operations)
            }
            DplyrNode::DataSource { name, .. } => {
                Ok(format!("SELECT * FROM {}", self.dialect.quote_identifier(name)))
            }
        }
    }

    /// Converts pipeline to SQL.
    fn generate_pipeline(&self, operations: &[DplyrOperation]) -> GenerationResult<String> {
        if operations.is_empty() {
            return Err(GenerationError::InvalidAst { 
                reason: "Empty pipeline: at least one operation is required".to_string() 
            });
        }

        let mut query_parts = QueryParts::new();
        
        // Process each operation in order
        for operation in operations {
            self.process_operation(operation, &mut query_parts)?;
        }

        // Assemble final SQL query
        self.assemble_query(&query_parts)
    }

    /// Processes individual operations.
    fn process_operation(&self, operation: &DplyrOperation, query_parts: &mut QueryParts) -> GenerationResult<()> {
        match operation {
            DplyrOperation::Select { columns } => {
                query_parts.select_columns = self.generate_select_columns(columns)?;
            }
            DplyrOperation::Filter { condition } => {
                let where_clause = self.generate_expression(condition)?;
                if query_parts.where_clauses.is_empty() {
                    query_parts.where_clauses.push(where_clause);
                } else {
                    query_parts.where_clauses.push(format!("AND ({})", where_clause));
                }
            }
            DplyrOperation::Mutate { assignments } => {
                // mutate may need to be handled with subqueries or CTEs
                for assignment in assignments {
                    let column_expr = format!(
                        "{} AS {}",
                        self.generate_expression(&assignment.expr)?,
                        self.dialect.quote_identifier(&assignment.column)
                    );
                    query_parts.select_columns.push(column_expr);
                }
            }
            DplyrOperation::Arrange { columns } => {
                query_parts.order_by = self.generate_order_by(columns)?;
            }
            DplyrOperation::GroupBy { columns } => {
                query_parts.group_by = columns
                    .iter()
                    .map(|col| self.dialect.quote_identifier(col))
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            DplyrOperation::Summarise { aggregations } => {
                query_parts.select_columns = self.generate_aggregations(aggregations)?;
            }
        }
        Ok(())
    }

    /// Generates SELECT columns.
    fn generate_select_columns(&self, columns: &[ColumnExpr]) -> GenerationResult<Vec<String>> {
        columns
            .iter()
            .map(|col| {
                let expr_sql = self.generate_expression(&col.expr)?;
                if let Some(alias) = &col.alias {
                    Ok(format!("{} AS {}", expr_sql, self.dialect.quote_identifier(alias)))
                } else {
                    Ok(expr_sql)
                }
            })
            .collect()
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
                Ok(format!("{} {}", self.dialect.quote_identifier(&col.column), direction))
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
                    format!("{}({})", func_name, column_ref)
                };
                
                if let Some(alias) = &agg.alias {
                    Ok(format!("{} AS {}", expr, self.dialect.quote_identifier(alias)))
                } else {
                    Ok(expr)
                }
            })
            .collect()
    }

    /// Converts expressions to SQL.
    fn generate_expression(&self, expr: &Expr) -> GenerationResult<String> {
        match expr {
            Expr::Identifier(name) => {
                Ok(self.dialect.quote_identifier(name))
            }
            Expr::Literal(literal) => {
                self.generate_literal(literal)
            }
            Expr::Binary { left, operator, right } => {
                let left_sql = self.generate_expression(left)?;
                let right_sql = self.generate_expression(right)?;
                let op_sql = self.generate_binary_operator(operator);
                Ok(format!("({} {} {})", left_sql, op_sql, right_sql))
            }
            Expr::Function { name, args } => {
                let args_sql: Result<Vec<_>, _> = args
                    .iter()
                    .map(|arg| self.generate_expression(arg))
                    .collect();
                let args_str = args_sql?.join(", ");
                Ok(format!("{}({})", name.to_uppercase(), args_str))
            }
        }
    }

    /// Converts literal values to SQL.
    fn generate_literal(&self, literal: &LiteralValue) -> GenerationResult<String> {
        match literal {
            LiteralValue::String(s) => Ok(self.dialect.quote_string(s)),
            LiteralValue::Number(n) => Ok(n.to_string()),
            LiteralValue::Boolean(b) => Ok(if *b { "TRUE".to_string() } else { "FALSE".to_string() }),
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

    /// Assembles the final SQL query.
    fn assemble_query(&self, parts: &QueryParts) -> GenerationResult<String> {
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
        query.push_str(&self.dialect.quote_identifier("data"));
        
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

/// Struct to store SQL query components
#[derive(Debug, Default)]
struct QueryParts {
    select_columns: Vec<String>,
    where_clauses: Vec<String>,
    group_by: String,
    order_by: String,
}

impl QueryParts {
    fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{DplyrNode, DplyrOperation, Expr, ColumnExpr};

    #[test]
    fn test_postgresql_dialect() {
        let dialect = PostgreSqlDialect;
        assert_eq!(dialect.quote_identifier("test"), "\"test\"");
        assert_eq!(dialect.quote_string("hello"), "'hello'");
        assert_eq!(dialect.aggregate_function("mean"), "AVG");
    }

    #[test]
    fn test_mysql_dialect() {
        let dialect = MySqlDialect;
        assert_eq!(dialect.quote_identifier("test"), "`test`");
        assert_eq!(dialect.string_concat("a", "b"), "CONCAT(a, b)");
    }

    #[test]
    fn test_simple_select_generation() {
        let generator = SqlGenerator::new(Box::new(PostgreSqlDialect));
        
        let ast = DplyrNode::Pipeline {
            operations: vec![
                DplyrOperation::Select {
                    columns: vec![
                        ColumnExpr {
                            expr: Expr::Identifier("name".to_string()),
                            alias: None,
                        },
                        ColumnExpr {
                            expr: Expr::Identifier("age".to_string()),
                            alias: None,
                        },
                    ],
                },
            ],
            location: crate::parser::SourceLocation::unknown(),
        };
        
        let sql = generator.generate(&ast).unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("\"name\""));
        assert!(sql.contains("\"age\""));
    }
}