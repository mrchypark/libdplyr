//! # libdplyr
//!
//! A Rust-based transpiler that converts R dplyr syntax to SQL queries.
//!
//! ## Usage Example
//!
//! ```rust
//! use libdplyr::{Transpiler, PostgreSqlDialect};
//!
//! // Create a transpiler using PostgreSQL dialect
//! let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
//!
//! // Convert dplyr code to SQL
//! let dplyr_code = "select(name, age) %>% filter(age > 18)";
//! let sql = transpiler.transpile(dplyr_code).unwrap();
//! println!("{}", sql);
//! ```

pub mod lexer;
pub mod parser;
pub mod sql_generator;
pub mod error;

// CLI module (included when building binary)
pub mod cli;

// Re-export public API
pub use crate::error::{TranspileError, LexError, ParseError, GenerationError};
pub use crate::lexer::{Lexer, Token};
pub use crate::parser::{Parser, DplyrNode, DplyrOperation};
pub use crate::sql_generator::{SqlGenerator, SqlDialect, PostgreSqlDialect, MySqlDialect, SqliteDialect};

/// Main transpiler struct
///
/// Provides the primary interface for converting dplyr code to SQL.
pub struct Transpiler {
    generator: SqlGenerator,
}

impl Transpiler {
    /// Creates a new transpiler instance.
    ///
    /// # Arguments
    ///
    /// * `dialect` - The SQL dialect to use
    pub fn new(dialect: Box<dyn SqlDialect>) -> Self {
        Self {
            generator: SqlGenerator::new(dialect),
        }
    }

    /// Converts dplyr code to SQL.
    ///
    /// # Arguments
    ///
    /// * `dplyr_code` - The dplyr code string to convert
    ///
    /// # Returns
    ///
    /// Returns SQL query string on success, TranspileError on failure.
    pub fn transpile(&self, dplyr_code: &str) -> Result<String, TranspileError> {
        let ast = self.parse_dplyr(dplyr_code)?;
        Ok(self.generate_sql(&ast)?)
    }

    /// Parses dplyr code to generate an AST.
    ///
    /// # Arguments
    ///
    /// * `code` - The dplyr code to parse
    pub fn parse_dplyr(&self, code: &str) -> Result<DplyrNode, ParseError> {
        let lexer = Lexer::new(code.to_string());
        let mut parser = Parser::new(lexer)?;
        parser.parse()
    }

    /// Converts AST to SQL.
    ///
    /// # Arguments
    ///
    /// * `ast` - The AST node to convert
    pub fn generate_sql(&self, ast: &DplyrNode) -> Result<String, GenerationError> {
        self.generator.generate(ast)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpiler_creation() {
        let _transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
        // Verify that transpiler creation succeeds
    }
}