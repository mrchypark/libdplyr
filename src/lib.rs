//! # libdplyr
//!
//! A high-performance Rust-based transpiler that converts R dplyr syntax to SQL queries.
//!
//! libdplyr enables R users to write database queries using familiar dplyr syntax
//! and converts them to efficient SQL for various database dialects including
//! PostgreSQL, MySQL, SQLite, and DuckDB. The library provides both a programmatic
//! API for integration into Rust applications and a command-line interface for
//! direct usage.
//!
//! ## Features
//!
//! - **Complete dplyr Function Support**: `select()`, `filter()`, `mutate()`, `arrange()`, `group_by()`, `summarise()`
//! - **Pipeline Operations**: Chain operations using the `%>%` pipe operator
//! - **Multiple SQL Dialects**: PostgreSQL, MySQL, SQLite, DuckDB support with dialect-specific optimizations
//! - **Performance Optimized**: Efficient parsing and SQL generation with minimal memory allocation
//! - **Comprehensive Error Handling**: Detailed error messages with position information and helpful hints
//! - **CLI Tool**: Full-featured command-line interface with pretty-printing and file I/O
//! - **Type Safety**: Leverages Rust's type system for safe AST manipulation
//! - **Extensible Architecture**: Plugin-ready design for adding new SQL dialects
//!
//! ## Quick Start
//!
//! Add libdplyr to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! libdplyr = "0.1.0"
//! ```
//!
//! Basic usage:
//!
//! ```rust
//! use libdplyr::{Transpiler, PostgreSqlDialect};
//!
//! // Create a transpiler using PostgreSQL dialect
//! let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
//!
//! // Convert simple dplyr code to SQL
//! let dplyr_code = "select(name, age) %>% filter(age > 18)";
//! let sql = transpiler.transpile(dplyr_code).unwrap();
//! println!("{}", sql);
//! // Output: SELECT "name", "age" FROM "data" WHERE "age" > 18
//! ```
//!
//! ## Advanced Usage Examples
//!
//! ### Complex Data Analysis Pipeline
//!
//! ```rust
//! use libdplyr::{Transpiler, MySqlDialect};
//!
//! let transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
//!
//! let dplyr_code = "select(employee_id, name, salary) %>% filter(salary >= 50000) %>% mutate(bonus = salary * 0.1) %>% arrange(desc(salary))";
//!
//! let sql = transpiler.transpile(dplyr_code).unwrap();
//! println!("{}", sql);
//! ```
//!
//! ### Working with Different SQL Dialects
//!
//! ```rust
//! use libdplyr::{Transpiler, PostgreSqlDialect, MySqlDialect, SqliteDialect, DuckDbDialect};
//!
//! let dplyr_code = "select(name, age) %>% filter(age > 18) %>% arrange(desc(age))";
//!
//! // PostgreSQL - uses double quotes and || for concatenation
//! let pg_transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
//! let pg_sql = pg_transpiler.transpile(dplyr_code).unwrap();
//!
//! // MySQL - uses backticks and CONCAT() function
//! let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
//! let mysql_sql = mysql_transpiler.transpile(dplyr_code).unwrap();
//!
//! // SQLite - lightweight database support
//! let sqlite_transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
//! let sqlite_sql = sqlite_transpiler.transpile(dplyr_code).unwrap();
//!
//! // DuckDB - analytical database with extended functions
//! let duckdb_transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));
//! let duckdb_sql = duckdb_transpiler.transpile(dplyr_code).unwrap();
//! ```
//!
//! ### AST Manipulation and Custom Processing
//!
//! ```rust
//! use libdplyr::{Transpiler, PostgreSqlDialect, DplyrNode, DplyrOperation};
//!
//! let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
//!
//! // Parse dplyr code to AST
//! let ast = transpiler.parse_dplyr("select(name, age) %>% filter(age > 18)").unwrap();
//!
//! // Inspect the AST structure
//! if let DplyrNode::Pipeline { operations, .. } = &ast {
//!     for operation in operations {
//!         match operation {
//!             DplyrOperation::Select { columns, .. } => {
//!                 println!("Found SELECT with {} columns", columns.len());
//!             }
//!             DplyrOperation::Filter { condition, .. } => {
//!                 println!("Found FILTER with condition");
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//!
//! // Generate SQL from AST
//! let sql = transpiler.generate_sql(&ast).unwrap();
//! println!("Generated SQL: {}", sql);
//! ```
//!
//! ## Supported SQL Dialects
//!
//! | Dialect | Identifier Quoting | String Concatenation | Special Features |
//! |---------|-------------------|---------------------|------------------|
//! | **PostgreSQL** | `"column"` | `\|\|` operator | Full SQL standard support |
//! | **MySQL** | `` `column` `` | `CONCAT()` function | MySQL-specific functions |
//! | **SQLite** | `"column"` | `\|\|` operator | Lightweight, embedded |
//! | **DuckDB** | `"column"` | `\|\|` operator | Analytical functions (MEDIAN, MODE) |
//!
//! ## Supported dplyr Functions
//!
//! ### Data Selection and Filtering
//! - `select(col1, col2, ...)` - Select specific columns
//! - `filter(condition)` - Filter rows based on conditions
//!
//! ### Data Transformation
//! - `mutate(new_col = expression, ...)` - Create or modify columns
//! - `arrange(col1, desc(col2), ...)` - Sort data
//!
//! ### Data Aggregation
//! - `group_by(col1, col2, ...)` - Group data by columns
//! - `summarise(stat = function(col), ...)` - Aggregate data
//!
//! ### Supported Aggregate Functions
//! - `mean(col)` / `avg(col)` - Average
//! - `sum(col)` - Sum
//! - `count(col)` - Count non-null values
//! - `n()` - Count all rows
//! - `min(col)` / `max(col)` - Minimum/Maximum
//! - `median(col)` - Median (DuckDB only)
//! - `mode(col)` - Mode (DuckDB only)
//!
//! ## Error Handling
//!
//! libdplyr provides comprehensive error handling with detailed error messages:
//!
//! ```rust
//! use libdplyr::{Transpiler, TranspileError, PostgreSqlDialect};
//!
//! let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
//! let result = transpiler.transpile("invalid_syntax_here");
//!
//! match result {
//!     Ok(sql) => println!("Generated SQL: {}", sql),
//!     Err(TranspileError::LexError(e)) => {
//!         eprintln!("Tokenization error: {}", e);
//!         eprintln!("Check for invalid characters or malformed strings");
//!     }
//!     Err(TranspileError::ParseError(e)) => {
//!         eprintln!("Parsing error: {}", e);
//!         eprintln!("Check dplyr function syntax and arguments");
//!     }
//!     Err(TranspileError::GenerationError(e)) => {
//!         eprintln!("SQL generation error: {}", e);
//!         eprintln!("The operation may not be supported in the selected dialect");
//!     }
//!     Err(TranspileError::IoError(e)) => {
//!         eprintln!("I/O error: {}", e);
//!         eprintln!("Check file permissions and paths");
//!     }
//!     Err(TranspileError::ValidationError(e)) => {
//!         eprintln!("Validation error: {}", e);
//!         eprintln!("Check dplyr syntax and function usage");
//!     }
//!     Err(TranspileError::ConfigurationError(e)) => {
//!         eprintln!("Configuration error: {}", e);
//!         eprintln!("Check configuration settings and options");
//!     }
//! }
//! ```
//!
//! ## Performance Considerations
//!
//! libdplyr is designed for high performance:
//!
//! - **Zero-copy parsing** where possible
//! - **Minimal memory allocations** during AST construction
//! - **Efficient string handling** with pre-allocated buffers
//! - **Optimized SQL generation** with dialect-specific optimizations
//!
//! Benchmark results (on typical queries):
//! - Simple queries: < 1ms
//! - Complex queries: < 10ms
//! - Memory usage: < 3x input size
//!
//! ## Command Line Interface
//!
//! libdplyr includes a full-featured CLI:
//!
//! ```bash
//! # Convert dplyr code directly
//! libdplyr -t "data %>% select(name, age) %>% filter(age > 18)"
//!
//! # Read from file and write to file
//! libdplyr -i input.R -o output.sql -d mysql -p
//!
//! # Use with pipes
//! echo "data %>% select(*)" | libdplyr -d sqlite
//! ```
//!
//! ## Integration Examples
//!
//! ### Web Service Integration
//!
//! ```rust
//! use libdplyr::{Transpiler, PostgreSqlDialect};
//! use std::sync::Arc;
//!
//! // Create a shared transpiler instance
//! let transpiler = Arc::new(Transpiler::new(Box::new(PostgreSqlDialect::new())));
//!
//! // Use in request handler
//! fn handle_query(dplyr_code: &str, transpiler: Arc<Transpiler>) -> Result<String, String> {
//!     transpiler.transpile(dplyr_code)
//!         .map_err(|e| format!("Transpilation failed: {}", e))
//! }
//! ```
//!
//! ### Batch Processing
//!
//! ```rust
//! use libdplyr::{Transpiler, MySqlDialect};
//!
//! let transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
//! let queries = vec![
//!     "select(name, age)",
//!     "filter(age > 18)",
//!     "group_by(department) %>% summarise(count = n())"
//! ];
//!
//! let results: Result<Vec<_>, _> = queries
//!     .iter()
//!     .map(|query| transpiler.transpile(query))
//!     .collect();
//!
//! match results {
//!     Ok(sql_queries) => {
//!         for sql in sql_queries {
//!             println!("Generated: {}", sql);
//!         }
//!     }
//!     Err(e) => eprintln!("Batch processing failed: {}", e),
//! }
//! ```
//!
//! ## Contributing
//!
//! We welcome contributions! Please see our [GitHub repository](https://github.com/libdplyr/libdplyr)
//! for contribution guidelines.
//!
//! ## License
//!
//! This project is licensed under the MIT License - see the LICENSE file for details.

pub mod error;
pub mod lexer;
pub mod parser;
pub mod performance;
pub mod sql_generator;

// CLI module (included when building binary)
pub mod cli;

// Re-export public API
pub use crate::error::{GenerationError, LexError, ParseError, TranspileError};
pub use crate::lexer::{Lexer, Token};
pub use crate::parser::{DplyrNode, DplyrOperation, Parser};
pub use crate::performance::{
    BatchPerformanceStats, PerformanceMetrics, PerformanceProfiler, RegressionDetector,
};
pub use crate::sql_generator::{
    DialectConfig, DuckDbDialect, MySqlDialect, PostgreSqlDialect, SqlDialect, SqlGenerator,
    SqliteDialect,
};

/// Main transpiler struct for converting dplyr code to SQL
///
/// The `Transpiler` provides the primary interface for converting R dplyr syntax
/// to SQL queries. It encapsulates the entire transformation pipeline from
/// tokenization through parsing to SQL generation.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use libdplyr::{Transpiler, PostgreSqlDialect};
///
/// let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
/// let sql = transpiler.transpile("select(name, age) %>% filter(age > 18)").unwrap();
/// println!("{}", sql);
/// ```
///
/// ## Different SQL Dialects
///
/// ```rust
/// use libdplyr::{Transpiler, MySqlDialect, SqliteDialect, DuckDbDialect};
///
/// // MySQL dialect
/// let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
/// let mysql_sql = mysql_transpiler.transpile("select(name)").unwrap();
///
/// // SQLite dialect  
/// let sqlite_transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
/// let sqlite_sql = sqlite_transpiler.transpile("select(name)").unwrap();
///
/// // DuckDB dialect
/// let duckdb_transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));
/// let duckdb_sql = duckdb_transpiler.transpile("select(name)").unwrap();
/// ```
///
/// ## Error Handling
///
/// ```rust
/// use libdplyr::{Transpiler, TranspileError, PostgreSqlDialect};
///
/// let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
/// match transpiler.transpile("invalid_syntax") {
///     Ok(sql) => println!("Generated SQL: {}", sql),
///     Err(TranspileError::LexError(e)) => eprintln!("Tokenization failed: {}", e),
///     Err(TranspileError::ParseError(e)) => eprintln!("Parsing failed: {}", e),
///     Err(TranspileError::GenerationError(e)) => eprintln!("SQL generation failed: {}", e),
///     Err(TranspileError::IoError(e)) => eprintln!("I/O operation failed: {}", e),
///     Err(TranspileError::ValidationError(e)) => eprintln!("Validation failed: {}", e),
///     Err(TranspileError::ConfigurationError(e)) => eprintln!("Configuration error: {}", e),
/// }
/// ```
pub struct Transpiler {
    generator: SqlGenerator,
}

impl Transpiler {
    /// Creates a new transpiler instance with the specified SQL dialect.
    ///
    /// # Arguments
    ///
    /// * `dialect` - A boxed SQL dialect implementation that defines how to generate
    ///   SQL for a specific database system (PostgreSQL, MySQL, SQLite, or DuckDB)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{Transpiler, PostgreSqlDialect, MySqlDialect};
    ///
    /// // Create PostgreSQL transpiler
    /// let pg_transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    ///
    /// // Create MySQL transpiler
    /// let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
    /// ```
    pub fn new(dialect: Box<dyn SqlDialect>) -> Self {
        Self {
            generator: SqlGenerator::new(dialect),
        }
    }

    /// Converts dplyr code to SQL in a single operation.
    ///
    /// This is the main entry point for transpilation. It performs the complete
    /// transformation pipeline: tokenization → parsing → SQL generation.
    ///
    /// # Arguments
    ///
    /// * `dplyr_code` - A string containing dplyr code to convert
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The generated SQL query string
    /// * `Err(TranspileError)` - An error that occurred during any stage of transpilation
    ///
    /// # Errors
    ///
    /// This function can return the following error types:
    /// - `TranspileError::LexError` - Invalid characters or malformed tokens
    /// - `TranspileError::ParseError` - Invalid dplyr syntax or unsupported operations
    /// - `TranspileError::GenerationError` - SQL generation failures or dialect limitations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{Transpiler, PostgreSqlDialect};
    ///
    /// let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    ///
    /// // Simple select
    /// let sql = transpiler.transpile("select(name, age)").unwrap();
    /// assert!(sql.contains("SELECT"));
    ///
    /// // Complex pipeline
    /// let complex_sql = transpiler.transpile(r#"
    ///     select(name, age, salary) %>%
    ///     filter(age >= 18 & salary > 50000) %>%
    ///     arrange(desc(salary))
    /// "#).unwrap();
    /// ```
    pub fn transpile(&self, dplyr_code: &str) -> Result<String, TranspileError> {
        let ast = self.parse_dplyr(dplyr_code)?;
        Ok(self.generate_sql(&ast)?)
    }

    /// Parses dplyr code to generate an Abstract Syntax Tree (AST).
    ///
    /// This method performs only the parsing phase of transpilation, returning
    /// the AST representation without generating SQL. Useful for inspecting
    /// the parsed structure or performing custom transformations.
    ///
    /// # Arguments
    ///
    /// * `code` - The dplyr code string to parse
    ///
    /// # Returns
    ///
    /// * `Ok(DplyrNode)` - The root AST node representing the parsed dplyr code
    /// * `Err(ParseError)` - A parsing error with position information
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{Transpiler, PostgreSqlDialect, DplyrNode};
    ///
    /// let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    /// let ast = transpiler.parse_dplyr("select(name) %>% filter(age > 18)").unwrap();
    ///
    /// // Check if it's a pipeline
    /// assert!(ast.is_pipeline());
    /// ```
    pub fn parse_dplyr(&self, code: &str) -> Result<DplyrNode, ParseError> {
        let lexer = Lexer::new(code.to_string());
        let mut parser = Parser::new(lexer)?;
        parser.parse()
    }

    /// Converts an AST to SQL using the configured dialect.
    ///
    /// This method performs only the SQL generation phase, taking a pre-parsed
    /// AST and converting it to SQL. Useful when you need to generate SQL from
    /// a modified or programmatically constructed AST.
    ///
    /// # Arguments
    ///
    /// * `ast` - The AST node to convert to SQL
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The generated SQL query string
    /// * `Err(GenerationError)` - A SQL generation error
    ///
    /// # Examples
    ///
    /// ```rust
    /// use libdplyr::{Transpiler, PostgreSqlDialect};
    ///
    /// let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    ///
    /// // Parse first
    /// let ast = transpiler.parse_dplyr("select(name, age)").unwrap();
    ///
    /// // Then generate SQL
    /// let sql = transpiler.generate_sql(&ast).unwrap();
    /// assert!(sql.contains("SELECT"));
    /// ```
    pub fn generate_sql(&self, ast: &DplyrNode) -> Result<String, GenerationError> {
        self.generator.generate(ast)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpiler_creation() {
        let _transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        // Verify that transpiler creation succeeds
    }

    #[test]
    fn test_transpiler_with_different_dialects() {
        let dialects: Vec<Box<dyn SqlDialect>> = vec![
            Box::new(PostgreSqlDialect::new()),
            Box::new(MySqlDialect::new()),
            Box::new(SqliteDialect::new()),
            Box::new(DuckDbDialect::new()),
        ];

        for dialect in dialects {
            let _transpiler = Transpiler::new(dialect);
            // Verify that transpiler creation succeeds with all dialects
        }
    }

    #[test]
    fn test_transpile_simple_select() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let dplyr_code = "select(name, age)";

        let result = transpiler.transpile(dplyr_code);
        assert!(result.is_ok(), "변환이 성공해야 합니다: {:?}", result);

        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("\"name\""));
        assert!(sql.contains("\"age\""));
    }

    #[test]
    fn test_transpile_with_filter() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let dplyr_code = "select(name, age) %>% filter(age > 18)";

        let result = transpiler.transpile(dplyr_code);
        assert!(result.is_ok(), "변환이 성공해야 합니다: {:?}", result);

        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("\"age\" > 18"));
    }

    #[test]
    fn test_transpile_complex_pipeline() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let dplyr_code = r#"
            select(name, age, salary) %>%
            filter(age >= 18) %>%
            arrange(desc(salary))
        "#;

        let result = transpiler.transpile(dplyr_code);
        assert!(result.is_ok(), "변환이 성공해야 합니다: {:?}", result);

        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("\"salary\" DESC"));
    }

    #[test]
    fn test_transpile_with_mutate() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let dplyr_code = "select(name, salary) %>% mutate(bonus = salary * 0.1)";

        let result = transpiler.transpile(dplyr_code);
        assert!(result.is_ok(), "변환이 성공해야 합니다: {:?}", result);

        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("\"bonus\""));
    }

    #[test]
    fn test_transpile_with_group_by_and_summarise() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let dplyr_code = "group_by(department) %>% summarise(avg_salary = mean(salary))";

        let result = transpiler.transpile(dplyr_code);
        assert!(result.is_ok(), "변환이 성공해야 합니다: {:?}", result);

        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("GROUP BY"));
        assert!(sql.contains("AVG"));
    }

    #[test]
    fn test_parse_dplyr_individual_function() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let dplyr_code = "select(name, age)";

        let result = transpiler.parse_dplyr(dplyr_code);
        assert!(result.is_ok(), "파싱이 성공해야 합니다: {:?}", result);

        let ast = result.unwrap();
        assert!(ast.is_pipeline());
    }

    #[test]
    fn test_generate_sql_from_ast() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let dplyr_code = "select(name, age)";

        // First parse to get AST
        let ast = transpiler.parse_dplyr(dplyr_code).unwrap();

        // Then generate SQL from AST
        let result = transpiler.generate_sql(&ast);
        assert!(result.is_ok(), "SQL 생성이 성공해야 합니다: {:?}", result);

        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("\"name\""));
        assert!(sql.contains("\"age\""));
    }

    #[test]
    fn test_transpile_error_handling() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

        // Test invalid syntax
        let result = transpiler.transpile("invalid_function(test)");
        assert!(result.is_err(), "잘못된 문법은 오류를 반환해야 합니다");

        match result.unwrap_err() {
            TranspileError::ParseError(_) => {} // 예상된 에러 타입
            other => panic!("예상치 못한 에러 타입: {:?}", other),
        }
    }

    #[test]
    fn test_transpile_different_dialects() {
        let test_cases = vec![
            (
                "PostgreSQL",
                Box::new(PostgreSqlDialect::new()) as Box<dyn SqlDialect>,
            ),
            (
                "MySQL",
                Box::new(MySqlDialect::new()) as Box<dyn SqlDialect>,
            ),
            (
                "SQLite",
                Box::new(SqliteDialect::new()) as Box<dyn SqlDialect>,
            ),
            (
                "DuckDB",
                Box::new(DuckDbDialect::new()) as Box<dyn SqlDialect>,
            ),
        ];

        let dplyr_code = "select(name, age) %>% filter(age > 18)";

        for (dialect_name, dialect) in test_cases {
            let transpiler = Transpiler::new(dialect);
            let result = transpiler.transpile(dplyr_code);

            assert!(
                result.is_ok(),
                "{} 방언에서 변환이 성공해야 합니다: {:?}",
                dialect_name,
                result
            );

            let sql = result.unwrap();
            assert!(
                sql.contains("SELECT"),
                "{} 방언 결과에 SELECT가 포함되어야 합니다",
                dialect_name
            );
            assert!(
                sql.contains("WHERE"),
                "{} 방언 결과에 WHERE가 포함되어야 합니다",
                dialect_name
            );
        }
    }

    #[test]
    fn test_transpile_empty_input() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

        let result = transpiler.transpile("");
        assert!(result.is_err(), "빈 입력은 오류를 반환해야 합니다");
    }

    #[test]
    fn test_transpile_whitespace_only() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

        let result = transpiler.transpile("   \t  \n  ");
        assert!(result.is_err(), "공백만 있는 입력은 오류를 반환해야 합니다");
    }

    #[test]
    fn test_transpile_result_types() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

        // Test successful case returns String
        let success_result = transpiler.transpile("select(name)");
        assert!(success_result.is_ok());
        let _sql: String = success_result.unwrap();

        // Test error case returns TranspileError - use clearly invalid syntax
        let error_result = transpiler.transpile("@#$%invalid");
        assert!(error_result.is_err());
        let _error: TranspileError = error_result.unwrap_err();
    }
}
