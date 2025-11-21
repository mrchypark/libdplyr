//! SQL generator module
//!
//! Provides functionality to convert AST to various SQL dialects.

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{
    Aggregation, BinaryOp, ColumnExpr, DplyrNode, DplyrOperation, Expr, LiteralValue,
    OrderDirection, OrderExpr,
};

/// SQL dialect trait for database-specific SQL generation
///
/// This trait defines the interface for handling specialized SQL syntax
/// for different database systems. Each database has its own quirks and
/// syntax variations, which are abstracted through this trait.
///
/// # Examples
///
/// ```rust
/// use libdplyr::{SqlDialect, PostgreSqlDialect, MySqlDialect};
///
/// let pg_dialect = PostgreSqlDialect::new();
/// let mysql_dialect = MySqlDialect::new();
///
/// // Different identifier quoting
/// assert_eq!(pg_dialect.quote_identifier("name"), "\"name\"");
/// assert_eq!(mysql_dialect.quote_identifier("name"), "`name`");
///
/// // Different string concatenation
/// let left = "\"first\"";
/// let right = "\"last\"";
/// assert_eq!(pg_dialect.string_concat(left, right), "\"first\" || \"last\"");
/// assert_eq!(mysql_dialect.string_concat(left, right), "CONCAT(\"first\", \"last\")");
/// ```
pub trait SqlDialect {
    /// Quotes identifiers according to the database's conventions.
    ///
    /// Different databases use different characters to quote identifiers
    /// (table names, column names, etc.) to handle reserved words and
    /// special characters.
    ///
    /// # Arguments
    ///
    /// * `name` - The identifier name to quote
    ///
    /// # Returns
    ///
    /// The properly quoted identifier string
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{PostgreSqlDialect, MySqlDialect, SqlDialect};
    ///
    /// let pg = PostgreSqlDialect::new();
    /// let mysql = MySqlDialect::new();
    ///
    /// assert_eq!(pg.quote_identifier("user"), "\"user\"");      // PostgreSQL
    /// assert_eq!(mysql.quote_identifier("user"), "`user`");     // MySQL
    /// ```
    fn quote_identifier(&self, name: &str) -> String;

    /// Quotes string literals according to the database's conventions.
    ///
    /// Handles proper escaping of quotes within string values.
    ///
    /// # Arguments
    ///
    /// * `value` - The string value to quote
    ///
    /// # Returns
    ///
    /// The properly quoted and escaped string literal
    fn quote_string(&self, value: &str) -> String;

    /// Generates a LIMIT clause for the database.
    ///
    /// Most databases use `LIMIT n` syntax, but some variations exist.
    ///
    /// # Arguments
    ///
    /// * `limit` - The maximum number of rows to return
    ///
    /// # Returns
    ///
    /// The LIMIT clause string
    fn limit_clause(&self, limit: usize) -> String;

    /// Generates string concatenation operation.
    ///
    /// Different databases have different ways to concatenate strings:
    /// - PostgreSQL/SQLite: `||` operator
    /// - MySQL: `CONCAT()` function
    ///
    /// # Arguments
    ///
    /// * `left` - Left operand (already quoted if needed)
    /// * `right` - Right operand (already quoted if needed)
    ///
    /// # Returns
    ///
    /// The concatenation expression
    fn string_concat(&self, left: &str, right: &str) -> String;

    /// Maps dplyr aggregate function names to SQL equivalents.
    ///
    /// Converts R/dplyr function names to their SQL counterparts,
    /// handling database-specific variations.
    ///
    /// # Arguments
    ///
    /// * `function` - The dplyr function name (e.g., "mean", "n")
    ///
    /// # Returns
    ///
    /// The corresponding SQL function name
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{PostgreSqlDialect, SqlDialect};
    ///
    /// let dialect = PostgreSqlDialect::new();
    /// assert_eq!(dialect.aggregate_function("mean"), "AVG");
    /// assert_eq!(dialect.aggregate_function("n"), "COUNT(*)");
    /// ```
    fn aggregate_function(&self, function: &str) -> String;

    /// Returns whether the database is case-sensitive for identifiers.
    ///
    /// This affects how identifiers are handled and compared.
    ///
    /// # Returns
    ///
    /// `true` if case-sensitive, `false` otherwise
    fn is_case_sensitive(&self) -> bool;

    /// Creates a boxed clone of this dialect.
    ///
    /// Used internally for performance benchmarking and testing.
    ///
    /// # Returns
    ///
    /// A boxed clone of the dialect
    fn clone_box(&self) -> Box<dyn SqlDialect>;
}

/// PostgreSQL dialect implementation
///
/// Implements SQL generation for PostgreSQL databases. PostgreSQL uses
/// double quotes for identifier quoting and the `||` operator for string
/// concatenation.
///
/// # Features
///
/// - Double-quoted identifiers: `"column_name"`
/// - String concatenation with `||` operator
/// - Standard SQL aggregate functions
/// - Case-insensitive identifier handling
///
/// # Examples
///
/// ```rust
/// use libdplyr::{Transpiler, PostgreSqlDialect};
///
/// let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
/// let sql = transpiler.transpile("select(name, age) %>% filter(age > 18)").unwrap();
///
/// // Generated SQL:
/// // SELECT "name", "age" FROM "data" WHERE "age" > 18
/// ```
#[derive(Debug, Clone)]
pub struct PostgreSqlDialect;

impl PostgreSqlDialect {
    /// Creates a new PostgreSQL dialect instance.
    ///
    /// # Returns
    ///
    /// A new `PostgreSqlDialect` configured for PostgreSQL databases.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{PostgreSqlDialect, SqlDialect};
    ///
    /// let dialect = PostgreSqlDialect::new();
    /// assert_eq!(dialect.quote_identifier("user"), "\"user\"");
    /// assert_eq!(dialect.string_concat("'a'", "'b'"), "'a' || 'b'");
    /// ```
    pub fn new() -> Self {
        Self
    }
}

impl Default for PostgreSqlDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlDialect for PostgreSqlDialect {
    fn quote_identifier(&self, name: &str) -> String {
        format!("\"{name}\"")
    }

    fn quote_string(&self, value: &str) -> String {
        let escaped = value.replace('\'', "''");
        format!("'{escaped}'")
    }

    fn limit_clause(&self, limit: usize) -> String {
        format!("LIMIT {limit}")
    }

    fn string_concat(&self, left: &str, right: &str) -> String {
        format!("{left} || {right}")
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

    fn clone_box(&self) -> Box<dyn SqlDialect> {
        Box::new(self.clone())
    }
}

/// MySQL dialect implementation
///
/// Implements SQL generation for MySQL databases. MySQL uses backticks
/// for identifier quoting and the `CONCAT()` function for string concatenation.
///
/// # Features
///
/// - Backtick-quoted identifiers: `` `column_name` ``
/// - String concatenation with `CONCAT()` function
/// - Standard SQL aggregate functions
/// - Case-insensitive identifier handling
///
/// # Examples
///
/// ```rust
/// use libdplyr::{Transpiler, MySqlDialect};
///
/// let transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
/// let sql = transpiler.transpile("select(name, age) %>% filter(age > 18)").unwrap();
///
/// // Generated SQL:
/// // SELECT `name`, `age` FROM `data` WHERE `age` > 18
/// ```
#[derive(Debug, Clone)]
pub struct MySqlDialect;

impl MySqlDialect {
    /// Creates a new MySQL dialect instance.
    ///
    /// # Returns
    ///
    /// A new `MySqlDialect` configured for MySQL databases.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{MySqlDialect, SqlDialect};
    ///
    /// let dialect = MySqlDialect::new();
    /// assert_eq!(dialect.quote_identifier("user"), "`user`");
    /// assert_eq!(dialect.string_concat("'a'", "'b'"), "CONCAT('a', 'b')");
    /// ```
    pub fn new() -> Self {
        Self
    }
}

impl Default for MySqlDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlDialect for MySqlDialect {
    fn quote_identifier(&self, name: &str) -> String {
        format!("`{name}`")
    }

    fn quote_string(&self, value: &str) -> String {
        let escaped = value.replace('\'', "''");
        format!("'{escaped}'")
    }

    fn limit_clause(&self, limit: usize) -> String {
        format!("LIMIT {limit}")
    }

    fn string_concat(&self, left: &str, right: &str) -> String {
        format!("CONCAT({left}, {right})")
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

    fn clone_box(&self) -> Box<dyn SqlDialect> {
        Box::new(self.clone())
    }
}

/// SQLite dialect implementation
///
/// Implements SQL generation for SQLite databases. SQLite uses double quotes
/// for identifier quoting and the `||` operator for string concatenation,
/// similar to PostgreSQL but with some limitations.
///
/// # Features
///
/// - Double-quoted identifiers: `"column_name"`
/// - String concatenation with `||` operator
/// - Standard SQL aggregate functions
/// - Case-insensitive identifier handling
/// - Lightweight database support
///
/// # Examples
///
/// ```rust
/// use libdplyr::{Transpiler, SqliteDialect};
///
/// let transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
/// let sql = transpiler.transpile("select(name, age) %>% filter(age > 18)").unwrap();
///
/// // Generated SQL:
/// // SELECT "name", "age" FROM "data" WHERE "age" > 18
/// ```
#[derive(Debug, Clone)]
pub struct SqliteDialect;

impl SqliteDialect {
    /// Creates a new SQLite dialect instance.
    ///
    /// # Returns
    ///
    /// A new `SqliteDialect` configured for SQLite databases.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{SqliteDialect, SqlDialect};
    ///
    /// let dialect = SqliteDialect::new();
    /// assert_eq!(dialect.quote_identifier("user"), "\"user\"");
    /// assert_eq!(dialect.string_concat("'a'", "'b'"), "'a' || 'b'");
    /// ```
    pub fn new() -> Self {
        Self
    }
}

impl Default for SqliteDialect {
    fn default() -> Self {
        Self::new()
    }
}

/// DuckDB dialect implementation
///
/// Implements SQL generation for DuckDB databases. DuckDB is PostgreSQL-compatible
/// with additional analytical functions and optimizations for data analysis workloads.
///
/// # Features
///
/// - Double-quoted identifiers: `"column_name"`
/// - String concatenation with `||` operator
/// - Extended aggregate functions (median, mode)
/// - PostgreSQL compatibility
/// - Analytical query optimizations
///
/// # Examples
///
/// ```rust
/// use libdplyr::{Transpiler, DuckDbDialect};
///
/// let transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));
/// let sql = transpiler.transpile("group_by(category) %>% summarise(median_price = median(price))").unwrap();
///
/// // Generated SQL:
/// // SELECT "category", MEDIAN("price") AS "median_price"
/// // FROM "data"
/// // GROUP BY "category"
/// ```
#[derive(Debug, Clone)]
pub struct DuckDbDialect;

impl DuckDbDialect {
    /// Creates a new DuckDB dialect instance.
    ///
    /// # Returns
    ///
    /// A new `DuckDbDialect` configured for DuckDB databases with analytical extensions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{DuckDbDialect, SqlDialect};
    ///
    /// let dialect = DuckDbDialect::new();
    /// assert_eq!(dialect.quote_identifier("user"), "\"user\"");
    /// assert_eq!(dialect.aggregate_function("median"), "MEDIAN");
    /// assert_eq!(dialect.string_concat("'a'", "'b'"), "'a' || 'b'");
    /// ```
    pub fn new() -> Self {
        Self
    }
}

impl Default for DuckDbDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlDialect for DuckDbDialect {
    fn quote_identifier(&self, name: &str) -> String {
        format!("\"{name}\"")
    }

    fn quote_string(&self, value: &str) -> String {
        let escaped = value.replace('\'', "''");
        format!("'{escaped}'")
    }

    fn limit_clause(&self, limit: usize) -> String {
        format!("LIMIT {limit}")
    }

    fn string_concat(&self, left: &str, right: &str) -> String {
        format!("{left} || {right}")
    }

    fn aggregate_function(&self, function: &str) -> String {
        match function.to_lowercase().as_str() {
            "mean" | "avg" => "AVG".to_string(),
            "sum" => "SUM".to_string(),
            "count" => "COUNT".to_string(),
            "min" => "MIN".to_string(),
            "max" => "MAX".to_string(),
            "n" => "COUNT(*)".to_string(),
            "median" => "MEDIAN".to_string(), // DuckDB specific
            "mode" => "MODE".to_string(),     // DuckDB specific
            _ => function.to_uppercase(),
        }
    }

    fn is_case_sensitive(&self) -> bool {
        false
    }

    fn clone_box(&self) -> Box<dyn SqlDialect> {
        Box::new(self.clone())
    }
}

/// Configuration for SQL dialect behavior
#[derive(Debug, Clone)]
pub struct DialectConfig {
    pub identifier_quote: char,
    pub string_quote: char,
    pub supports_limit: bool,
    pub supports_offset: bool,
    pub case_sensitive: bool,
}

impl SqlDialect for SqliteDialect {
    fn quote_identifier(&self, name: &str) -> String {
        format!("\"{name}\"")
    }

    fn quote_string(&self, value: &str) -> String {
        let escaped = value.replace('\'', "''");
        format!("'{escaped}'")
    }

    fn limit_clause(&self, limit: usize) -> String {
        format!("LIMIT {limit}")
    }

    fn string_concat(&self, left: &str, right: &str) -> String {
        format!("{left} || {right}")
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

    fn clone_box(&self) -> Box<dyn SqlDialect> {
        Box::new(self.clone())
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
            DplyrNode::Pipeline { operations, .. } => self.generate_pipeline(operations),
            DplyrNode::DataSource { name, .. } => Ok(format!(
                "SELECT * FROM {}",
                self.dialect.quote_identifier(name)
            )),
        }
    }

    /// Converts pipeline to SQL.
    fn generate_pipeline(&self, operations: &[DplyrOperation]) -> GenerationResult<String> {
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
        self.assemble_query(&query_parts)
    }

    /// Processes individual operations.
    fn process_operation(
        &self,
        operation: &DplyrOperation,
        query_parts: &mut QueryParts,
    ) -> GenerationResult<()> {
        match operation {
            DplyrOperation::Select { columns, .. } => {
                query_parts.select_columns = self.generate_select_columns(columns)?;
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
    fn process_mutate_operation(
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
    fn mutate_needs_subquery(
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
        for assignment in assignments {
            let column_expr = format!(
                "{} AS {}",
                self.generate_expression(&assignment.expr)?,
                self.dialect.quote_identifier(&assignment.column)
            );
            query_parts.select_columns.push(column_expr);
        }
        Ok(())
    }

    /// Checks if expression references any of the given columns.
    #[allow(clippy::only_used_in_recursion)]
    fn expression_references_columns(
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
    fn expression_is_complex(&self, expr: &Expr) -> bool {
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

        self.assemble_query(&nested_parts)
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
    use crate::parser::{
        Aggregation, Assignment, ColumnExpr, DplyrNode, DplyrOperation, Expr, OrderDirection,
        OrderExpr, SourceLocation,
    };

    // Helper function to normalize SQL for comparison
    fn normalize_sql(sql: &str) -> String {
        sql.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_uppercase()
    }

    // Helper function to create test AST nodes
    fn create_test_select_operation(columns: Vec<&str>) -> DplyrOperation {
        DplyrOperation::Select {
            columns: columns
                .into_iter()
                .map(|col| ColumnExpr {
                    expr: Expr::Identifier(col.to_string()),
                    alias: None,
                })
                .collect(),
            location: SourceLocation::unknown(),
        }
    }

    fn create_test_filter_operation(column: &str, value: f64) -> DplyrOperation {
        DplyrOperation::Filter {
            condition: Expr::Binary {
                left: Box::new(Expr::Identifier(column.to_string())),
                operator: BinaryOp::GreaterThan,
                right: Box::new(Expr::Literal(LiteralValue::Number(value))),
            },
            location: SourceLocation::unknown(),
        }
    }

    // ===== SQL Dialect Tests =====

    mod dialect_tests {
        use super::*;

        #[test]
        fn test_postgresql_dialect_identifier_quoting() {
            let dialect = PostgreSqlDialect::new();
            assert_eq!(dialect.quote_identifier("test"), "\"test\"");
            assert_eq!(dialect.quote_identifier("column_name"), "\"column_name\"");
            assert_eq!(dialect.quote_identifier("CamelCase"), "\"CamelCase\"");
        }

        #[test]
        fn test_postgresql_dialect_string_quoting() {
            let dialect = PostgreSqlDialect::new();
            assert_eq!(dialect.quote_string("hello"), "'hello'");
            assert_eq!(dialect.quote_string("it's"), "'it''s'");
            assert_eq!(dialect.quote_string(""), "''");
        }

        #[test]
        fn test_postgresql_dialect_aggregate_functions() {
            let dialect = PostgreSqlDialect::new();
            assert_eq!(dialect.aggregate_function("mean"), "AVG");
            assert_eq!(dialect.aggregate_function("avg"), "AVG");
            assert_eq!(dialect.aggregate_function("sum"), "SUM");
            assert_eq!(dialect.aggregate_function("count"), "COUNT");
            assert_eq!(dialect.aggregate_function("min"), "MIN");
            assert_eq!(dialect.aggregate_function("max"), "MAX");
            assert_eq!(dialect.aggregate_function("n"), "COUNT(*)");
            assert_eq!(dialect.aggregate_function("custom"), "CUSTOM");
        }

        #[test]
        fn test_postgresql_dialect_string_concat() {
            let dialect = PostgreSqlDialect::new();
            assert_eq!(dialect.string_concat("a", "b"), "a || b");
            assert_eq!(
                dialect.string_concat("'hello'", "'world'"),
                "'hello' || 'world'"
            );
        }

        #[test]
        fn test_mysql_dialect_identifier_quoting() {
            let dialect = MySqlDialect::new();
            assert_eq!(dialect.quote_identifier("test"), "`test`");
            assert_eq!(dialect.quote_identifier("column_name"), "`column_name`");
        }

        #[test]
        fn test_mysql_dialect_string_concat() {
            let dialect = MySqlDialect::new();
            assert_eq!(dialect.string_concat("a", "b"), "CONCAT(a, b)");
            assert_eq!(
                dialect.string_concat("'hello'", "'world'"),
                "CONCAT('hello', 'world')"
            );
        }

        #[test]
        fn test_sqlite_dialect() {
            let dialect = SqliteDialect::new();
            assert_eq!(dialect.quote_identifier("test"), "\"test\"");
            assert_eq!(dialect.string_concat("a", "b"), "a || b");
            assert_eq!(dialect.aggregate_function("mean"), "AVG");
        }

        #[test]
        fn test_duckdb_dialect_special_functions() {
            let dialect = DuckDbDialect::new();
            assert_eq!(dialect.aggregate_function("median"), "MEDIAN");
            assert_eq!(dialect.aggregate_function("mode"), "MODE");
            assert_eq!(dialect.aggregate_function("mean"), "AVG");
        }

        #[test]
        fn test_dialect_limit_clause() {
            let pg_dialect = PostgreSqlDialect::new();
            let mysql_dialect = MySqlDialect::new();
            let sqlite_dialect = SqliteDialect::new();

            assert_eq!(pg_dialect.limit_clause(10), "LIMIT 10");
            assert_eq!(mysql_dialect.limit_clause(5), "LIMIT 5");
            assert_eq!(sqlite_dialect.limit_clause(100), "LIMIT 100");
        }

        #[test]
        fn test_dialect_case_sensitivity() {
            let pg_dialect = PostgreSqlDialect::new();
            let mysql_dialect = MySqlDialect::new();
            let sqlite_dialect = SqliteDialect::new();
            let duckdb_dialect = DuckDbDialect::new();

            assert!(!pg_dialect.is_case_sensitive());
            assert!(!mysql_dialect.is_case_sensitive());
            assert!(!sqlite_dialect.is_case_sensitive());
            assert!(!duckdb_dialect.is_case_sensitive());
        }
    }

    // ===== SQL Clause Generation Tests =====

    mod clause_generation_tests {
        use super::*;

        #[test]
        fn test_select_clause_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let columns = vec![
                ColumnExpr {
                    expr: Expr::Identifier("name".to_string()),
                    alias: None,
                },
                ColumnExpr {
                    expr: Expr::Identifier("age".to_string()),
                    alias: Some("user_age".to_string()),
                },
            ];

            let result = generator.generate_select_columns(&columns).unwrap();
            assert_eq!(result.len(), 2);
            assert_eq!(result[0], "\"name\"");
            assert_eq!(result[1], "\"age\" AS \"user_age\"");
        }

        #[test]
        fn test_where_clause_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let condition = Expr::Binary {
                left: Box::new(Expr::Identifier("age".to_string())),
                operator: BinaryOp::GreaterThanOrEqual,
                right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
            };

            let result = generator.generate_expression(&condition).unwrap();
            assert_eq!(result, "(\"age\" >= 18)");
        }

        #[test]
        fn test_order_by_clause_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let columns = vec![
                OrderExpr {
                    column: "name".to_string(),
                    direction: OrderDirection::Asc,
                },
                OrderExpr {
                    column: "age".to_string(),
                    direction: OrderDirection::Desc,
                },
            ];

            let result = generator.generate_order_by(&columns).unwrap();
            assert_eq!(result, "\"name\" ASC, \"age\" DESC");
        }

        #[test]
        fn test_aggregation_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let aggregations = vec![
                Aggregation {
                    function: "mean".to_string(),
                    column: "salary".to_string(),
                    alias: Some("avg_salary".to_string()),
                },
                Aggregation {
                    function: "n".to_string(),
                    column: "".to_string(),
                    alias: Some("count".to_string()),
                },
            ];

            let result = generator.generate_aggregations(&aggregations).unwrap();
            assert_eq!(result.len(), 2);
            assert_eq!(result[0], "AVG(\"salary\") AS \"avg_salary\"");
            assert_eq!(result[1], "COUNT(*) AS \"count\"");
        }

        #[test]
        fn test_complex_expression_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            // Test nested binary expressions: (age > 18) AND (status = 'active')
            let condition = Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Identifier("age".to_string())),
                    operator: BinaryOp::GreaterThan,
                    right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
                }),
                operator: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Identifier("status".to_string())),
                    operator: BinaryOp::Equal,
                    right: Box::new(Expr::Literal(LiteralValue::String("active".to_string()))),
                }),
            };

            let result = generator.generate_expression(&condition).unwrap();
            assert_eq!(result, "((\"age\" > 18) AND (\"status\" = 'active'))");
        }

        #[test]
        fn test_function_expression_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let function_expr = Expr::Function {
                name: "upper".to_string(),
                args: vec![Expr::Identifier("name".to_string())],
            };

            let result = generator.generate_expression(&function_expr).unwrap();
            assert_eq!(result, "UPPER(\"name\")");
        }

        #[test]
        fn test_literal_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            assert_eq!(
                generator
                    .generate_literal(&LiteralValue::String("test".to_string()))
                    .unwrap(),
                "'test'"
            );
            assert_eq!(
                generator
                    .generate_literal(&LiteralValue::Number(42.5))
                    .unwrap(),
                "42.5"
            );
            assert_eq!(
                generator
                    .generate_literal(&LiteralValue::Boolean(true))
                    .unwrap(),
                "TRUE"
            );
            assert_eq!(
                generator
                    .generate_literal(&LiteralValue::Boolean(false))
                    .unwrap(),
                "FALSE"
            );
            assert_eq!(
                generator.generate_literal(&LiteralValue::Null).unwrap(),
                "NULL"
            );
        }
    }

    // ===== Dialect-Specific SQL Generation Tests =====

    mod dialect_specific_tests {
        use super::*;

        #[test]
        fn test_postgresql_vs_mysql_identifier_quoting() {
            let pg_generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
            let mysql_generator = SqlGenerator::new(Box::new(MySqlDialect::new()));

            let ast = DplyrNode::Pipeline {
                operations: vec![create_test_select_operation(vec!["name", "age"])],
                location: SourceLocation::unknown(),
            };

            let pg_sql = pg_generator.generate(&ast).unwrap();
            let mysql_sql = mysql_generator.generate(&ast).unwrap();

            assert!(pg_sql.contains("\"name\""));
            assert!(pg_sql.contains("\"age\""));
            assert!(mysql_sql.contains("`name`"));
            assert!(mysql_sql.contains("`age`"));
        }

        #[test]
        fn test_string_concatenation_differences() {
            let pg_generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
            let mysql_generator = SqlGenerator::new(Box::new(MySqlDialect::new()));

            let concat_expr = Expr::Function {
                name: "concat".to_string(),
                args: vec![
                    Expr::Identifier("first_name".to_string()),
                    Expr::Literal(LiteralValue::String(" ".to_string())),
                    Expr::Identifier("last_name".to_string()),
                ],
            };

            let pg_result = pg_generator.generate_expression(&concat_expr).unwrap();
            let mysql_result = mysql_generator.generate_expression(&concat_expr).unwrap();

            assert_eq!(pg_result, "CONCAT(\"first_name\", ' ', \"last_name\")");
            assert_eq!(mysql_result, "CONCAT(`first_name`, ' ', `last_name`)");
        }

        #[test]
        fn test_aggregate_function_mapping_consistency() {
            let dialects: Vec<Box<dyn SqlDialect>> = vec![
                Box::new(PostgreSqlDialect::new()),
                Box::new(MySqlDialect::new()),
                Box::new(SqliteDialect::new()),
                Box::new(DuckDbDialect::new()),
            ];

            let common_functions = vec!["mean", "sum", "count", "min", "max", "n"];

            for dialect in dialects {
                for func in &common_functions {
                    let result = dialect.aggregate_function(func);
                    assert!(
                        !result.is_empty(),
                        "Function {func} should map to something"
                    );

                    // Common mappings should be consistent
                    match *func {
                        "mean" => assert_eq!(result, "AVG"),
                        "sum" => assert_eq!(result, "SUM"),
                        "count" => assert_eq!(result, "COUNT"),
                        "min" => assert_eq!(result, "MIN"),
                        "max" => assert_eq!(result, "MAX"),
                        "n" => assert_eq!(result, "COUNT(*)"),
                        _ => {}
                    }
                }
            }
        }

        #[test]
        fn test_duckdb_specific_functions() {
            let duckdb_generator = SqlGenerator::new(Box::new(DuckDbDialect::new()));

            let aggregations = vec![
                Aggregation {
                    function: "median".to_string(),
                    column: "salary".to_string(),
                    alias: None,
                },
                Aggregation {
                    function: "mode".to_string(),
                    column: "category".to_string(),
                    alias: None,
                },
            ];

            let result = duckdb_generator
                .generate_aggregations(&aggregations)
                .unwrap();
            assert_eq!(result[0], "MEDIAN(\"salary\")");
            assert_eq!(result[1], "MODE(\"category\")");
        }
    }

    // ===== Complex Query Generation Tests =====

    mod complex_query_tests {
        use super::*;

        #[test]
        fn test_complete_pipeline_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let ast = DplyrNode::Pipeline {
                operations: vec![
                    create_test_select_operation(vec!["name", "age", "salary"]),
                    create_test_filter_operation("age", 25.0),
                    DplyrOperation::Arrange {
                        columns: vec![OrderExpr {
                            column: "salary".to_string(),
                            direction: OrderDirection::Desc,
                        }],
                        location: SourceLocation::unknown(),
                    },
                ],
                location: SourceLocation::unknown(),
            };

            let sql = generator.generate(&ast).unwrap();
            let normalized = normalize_sql(&sql);

            assert!(normalized.contains("SELECT"));
            assert!(normalized.contains("\"NAME\""));
            assert!(normalized.contains("\"AGE\""));
            assert!(normalized.contains("\"SALARY\""));
            assert!(normalized.contains("WHERE"));
            assert!(normalized.contains("\"AGE\" > 25"));
            assert!(normalized.contains("ORDER BY"));
            assert!(normalized.contains("\"SALARY\" DESC"));
        }

        #[test]
        fn test_group_by_with_aggregation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let ast = DplyrNode::Pipeline {
                operations: vec![
                    DplyrOperation::GroupBy {
                        columns: vec!["department".to_string()],
                        location: SourceLocation::unknown(),
                    },
                    DplyrOperation::Summarise {
                        aggregations: vec![
                            Aggregation {
                                function: "mean".to_string(),
                                column: "salary".to_string(),
                                alias: Some("avg_salary".to_string()),
                            },
                            Aggregation {
                                function: "n".to_string(),
                                column: "".to_string(),
                                alias: Some("count".to_string()),
                            },
                        ],
                        location: SourceLocation::unknown(),
                    },
                ],
                location: SourceLocation::unknown(),
            };

            let sql = generator.generate(&ast).unwrap();
            let normalized = normalize_sql(&sql);

            assert!(normalized.contains("SELECT"));
            assert!(normalized.contains("AVG(\"SALARY\") AS \"AVG_SALARY\""));
            assert!(normalized.contains("COUNT(*) AS \"COUNT\""));
            assert!(normalized.contains("GROUP BY"));
            assert!(normalized.contains("\"DEPARTMENT\""));
        }

        #[test]
        fn test_multiple_filter_conditions() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let ast = DplyrNode::Pipeline {
                operations: vec![
                    create_test_select_operation(vec!["name"]),
                    DplyrOperation::Filter {
                        condition: Expr::Binary {
                            left: Box::new(Expr::Identifier("age".to_string())),
                            operator: BinaryOp::GreaterThan,
                            right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
                        },
                        location: SourceLocation::unknown(),
                    },
                    DplyrOperation::Filter {
                        condition: Expr::Binary {
                            left: Box::new(Expr::Identifier("status".to_string())),
                            operator: BinaryOp::Equal,
                            right: Box::new(Expr::Literal(LiteralValue::String(
                                "active".to_string(),
                            ))),
                        },
                        location: SourceLocation::unknown(),
                    },
                ],
                location: SourceLocation::unknown(),
            };

            let sql = generator.generate(&ast).unwrap();
            let normalized = normalize_sql(&sql);

            assert!(normalized.contains("WHERE"));
            assert!(normalized.contains("\"AGE\" > 18"));
            assert!(normalized.contains("AND"));
            assert!(normalized.contains("\"STATUS\" = 'ACTIVE'"));
        }

        #[test]
        fn test_mutate_operation_integration() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let ast = DplyrNode::Pipeline {
                operations: vec![DplyrOperation::Mutate {
                    assignments: vec![
                        Assignment {
                            column: "adult".to_string(),
                            expr: Expr::Binary {
                                left: Box::new(Expr::Identifier("age".to_string())),
                                operator: BinaryOp::GreaterThanOrEqual,
                                right: Box::new(Expr::Literal(LiteralValue::Number(18.0))),
                            },
                        },
                        Assignment {
                            column: "salary_bonus".to_string(),
                            expr: Expr::Binary {
                                left: Box::new(Expr::Identifier("salary".to_string())),
                                operator: BinaryOp::Multiply,
                                right: Box::new(Expr::Literal(LiteralValue::Number(1.1))),
                            },
                        },
                    ],
                    location: SourceLocation::unknown(),
                }],
                location: SourceLocation::unknown(),
            };

            let sql = generator.generate(&ast).unwrap();
            let normalized = normalize_sql(&sql);

            assert!(normalized.contains("SELECT"));
            assert!(normalized.contains("\"AGE\" >= 18"));
            assert!(normalized.contains("AS \"ADULT\""));
            assert!(normalized.contains("\"SALARY\" * 1.1"));
            assert!(normalized.contains("AS \"SALARY_BONUS\""));
        }
    }

    // ===== Error Case Tests =====

    mod error_case_tests {
        use super::*;

        #[test]
        fn test_empty_pipeline_error() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let ast = DplyrNode::Pipeline {
                operations: vec![],
                location: SourceLocation::unknown(),
            };

            let result = generator.generate(&ast);
            assert!(result.is_err());

            match result.unwrap_err() {
                GenerationError::InvalidAst { reason } => {
                    assert!(reason.contains("Empty pipeline"));
                }
                _ => panic!("Expected InvalidAst error"),
            }
        }

        #[test]
        fn test_invalid_expression_handling() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            // Test with deeply nested expressions that might cause issues
            let mut nested_expr = Expr::Identifier("base".to_string());
            for i in 0..100 {
                nested_expr = Expr::Binary {
                    left: Box::new(nested_expr),
                    operator: BinaryOp::Plus,
                    right: Box::new(Expr::Literal(LiteralValue::Number(i as f64))),
                };
            }

            // This should not panic or cause stack overflow
            let result = generator.generate_expression(&nested_expr);
            assert!(result.is_ok(), "Should handle deeply nested expressions");
        }

        #[test]
        fn test_data_source_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let ast = DplyrNode::DataSource {
                name: "users".to_string(),
                location: SourceLocation::unknown(),
            };

            let sql = generator.generate(&ast).unwrap();
            assert_eq!(normalize_sql(&sql), "SELECT * FROM \"USERS\"");
        }

        #[test]
        fn test_binary_operator_coverage() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let operators = vec![
                (BinaryOp::Equal, "="),
                (BinaryOp::NotEqual, "!="),
                (BinaryOp::LessThan, "<"),
                (BinaryOp::LessThanOrEqual, "<="),
                (BinaryOp::GreaterThan, ">"),
                (BinaryOp::GreaterThanOrEqual, ">="),
                (BinaryOp::And, "AND"),
                (BinaryOp::Or, "OR"),
                (BinaryOp::Plus, "+"),
                (BinaryOp::Minus, "-"),
                (BinaryOp::Multiply, "*"),
                (BinaryOp::Divide, "/"),
            ];

            for (op, expected) in operators {
                let result = generator.generate_binary_operator(&op);
                assert_eq!(result, expected, "Operator {op:?} should map to {expected}");
            }
        }

        #[test]
        fn test_special_characters_in_strings() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let test_strings = vec![
                ("simple", "'simple'"),
                ("it's", "'it''s'"),
                ("", "''"),
                ("line\nbreak", "'line\nbreak'"),
                ("tab\there", "'tab\there'"),
            ];

            for (input, expected) in test_strings {
                let literal = LiteralValue::String(input.to_string());
                let result = generator.generate_literal(&literal).unwrap();
                assert_eq!(
                    result, expected,
                    "String '{input}' should be quoted as {expected}"
                );
            }
        }

        #[test]
        fn test_edge_case_numbers() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            let test_numbers = vec![
                (0.0, "0"),
                (-1.0, "-1"),
                (std::f64::consts::PI, "3.141592653589793"),
                (1e6, "1000000"),
                (1e-6, "0.000001"),
            ];

            for (input, expected) in test_numbers {
                let literal = LiteralValue::Number(input);
                let result = generator.generate_literal(&literal).unwrap();
                assert_eq!(
                    result, expected,
                    "Number {input} should be formatted as {expected}"
                );
            }
        }
    }

    // ===== Mutate Operation Advanced Tests =====

    mod mutate_advanced_tests {
        use super::*;

        #[test]
        fn test_mutate_column_dependency_detection() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
            let assignments = vec![
                Assignment {
                    column: "doubled".to_string(),
                    expr: Expr::Binary {
                        left: Box::new(Expr::Identifier("value".to_string())),
                        operator: BinaryOp::Multiply,
                        right: Box::new(Expr::Literal(LiteralValue::Number(2.0))),
                    },
                },
                Assignment {
                    column: "quadrupled".to_string(),
                    expr: Expr::Binary {
                        left: Box::new(Expr::Identifier("doubled".to_string())),
                        operator: BinaryOp::Multiply,
                        right: Box::new(Expr::Literal(LiteralValue::Number(2.0))),
                    },
                },
            ];

            let query_parts = QueryParts::new();
            let needs_subquery = generator.mutate_needs_subquery(&assignments, &query_parts);
            assert!(needs_subquery, "Should detect column dependencies");
        }

        #[test]
        fn test_mutate_with_window_functions() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
            let assignments = vec![Assignment {
                column: "row_num".to_string(),
                expr: Expr::Function {
                    name: "row_number".to_string(),
                    args: vec![],
                },
            }];

            let query_parts = QueryParts::new();
            let is_complex = generator.expression_is_complex(&assignments[0].expr);
            assert!(is_complex, "Should detect window function as complex");

            let needs_subquery = generator.mutate_needs_subquery(&assignments, &query_parts);
            assert!(needs_subquery, "Should need subquery for window functions");
        }

        #[test]
        fn test_mutate_subquery_generation() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
            let base_query = "SELECT * FROM employees";
            let assignments = vec![Assignment {
                column: "bonus".to_string(),
                expr: Expr::Binary {
                    left: Box::new(Expr::Identifier("salary".to_string())),
                    operator: BinaryOp::Multiply,
                    right: Box::new(Expr::Literal(LiteralValue::Number(0.1))),
                },
            }];

            let result = generator.generate_mutate_subquery(base_query, &assignments);
            assert!(
                result.is_ok(),
                "Subquery generation should succeed: {result:?}"
            );

            let sql = result.unwrap();
            assert!(sql.contains("SELECT *, (\"salary\" * 0.1) AS \"bonus\""));
            assert!(sql.contains("FROM ("));
            assert!(sql.contains("SELECT * FROM employees"));
            assert!(sql.contains(") AS subquery"));
        }

        #[test]
        fn test_nested_pipeline_processing() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
            let operations = vec![
                DplyrOperation::Filter {
                    condition: Expr::Binary {
                        left: Box::new(Expr::Identifier("active".to_string())),
                        operator: BinaryOp::Equal,
                        right: Box::new(Expr::Literal(LiteralValue::Boolean(true))),
                    },
                    location: SourceLocation::unknown(),
                },
                DplyrOperation::Mutate {
                    assignments: vec![Assignment {
                        column: "category".to_string(),
                        expr: Expr::Function {
                            name: "case".to_string(),
                            args: vec![
                                Expr::Identifier("score".to_string()),
                                Expr::Literal(LiteralValue::String("high".to_string())),
                            ],
                        },
                    }],
                    location: SourceLocation::unknown(),
                },
            ];

            let result = generator.generate_nested_pipeline(&operations);
            assert!(result.is_ok(), "Nested pipeline should succeed: {result:?}");

            let sql = result.unwrap();
            assert!(sql.contains("WHERE"));
            assert!(sql.contains("\"active\" = TRUE"));
            assert!(sql.contains("CASE"));
            assert!(sql.contains("AS \"category\""));
        }

        #[test]
        fn test_expression_reference_detection() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));
            let mut columns = std::collections::HashSet::new();
            columns.insert("existing_col".to_string());

            // Expression that references existing column
            let expr1 = Expr::Identifier("existing_col".to_string());
            assert!(generator.expression_references_columns(&expr1, &columns));

            // Expression that doesn't reference existing column
            let expr2 = Expr::Identifier("other_col".to_string());
            assert!(!generator.expression_references_columns(&expr2, &columns));

            // Binary expression with reference
            let expr3 = Expr::Binary {
                left: Box::new(Expr::Identifier("existing_col".to_string())),
                operator: BinaryOp::Plus,
                right: Box::new(Expr::Literal(LiteralValue::Number(1.0))),
            };
            assert!(generator.expression_references_columns(&expr3, &columns));
        }

        #[test]
        fn test_complex_expression_detection() {
            let generator = SqlGenerator::new(Box::new(PostgreSqlDialect::new()));

            // Window functions should be detected as complex
            let window_functions = vec![
                "row_number",
                "rank",
                "dense_rank",
                "lag",
                "lead",
                "first_value",
                "last_value",
                "nth_value",
            ];
            for func_name in window_functions {
                let expr = Expr::Function {
                    name: func_name.to_string(),
                    args: vec![],
                };
                assert!(
                    generator.expression_is_complex(&expr),
                    "Function {} should be detected as complex",
                    func_name
                );
            }

            // Regular functions should not be complex
            let regular_expr = Expr::Function {
                name: "upper".to_string(),
                args: vec![Expr::Identifier("name".to_string())],
            };
            assert!(!generator.expression_is_complex(&regular_expr));

            // Literals should not be complex
            let literal_expr = Expr::Literal(LiteralValue::Number(42.0));
            assert!(!generator.expression_is_complex(&literal_expr));
        }
    }
}
