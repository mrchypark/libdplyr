//! SQL dialects.

/// Translates a common R function to SQL
fn translate_common_function(function: &str, args: &[String]) -> Option<String> {
    let fn_lower = function.to_lowercase();
    match fn_lower.as_str() {
        // Math functions
        "abs" => Some(format!("ABS({})", args.join(", "))),
        "round" => Some(format!("ROUND({})", args.join(", "))),
        "floor" => Some(format!("FLOOR({})", args.join(", "))),
        "ceiling" | "ceil" => Some(format!("CEILING({})", args.join(", "))),
        "sqrt" => Some(format!("SQRT({})", args.join(", "))),
        "sign" => Some(format!("SIGN({})", args.join(", "))),
        "exp" => Some(format!("EXP({})", args.join(", "))),
        "log" => {
            if args.len() == 1 {
                Some(format!("LN({})", args[0]))
            } else if args.len() == 2 {
                Some(format!("LOG({}, {})", args[1], args[0]))
            } else {
                None
            }
        }
        "log10" => Some(format!("LOG({})", args.join(", "))),
        // Modulo
        "mod" | "%%" => Some(format!("{} % {}", args[0], args[1])),
        // Trigonometric functions
        "sin" => Some(format!("SIN({})", args.join(", "))),
        "cos" => Some(format!("COS({})", args.join(", "))),
        "tan" => Some(format!("TAN({})", args.join(", "))),
        "asin" => Some(format!("ASIN({})", args.join(", "))),
        "acos" => Some(format!("ACOS({})", args.join(", "))),
        "atan" => Some(format!("ATAN({})", args.join(", "))),
        "atan2" => Some(format!("ATAN2({}, {})", args[0], args[1])),
        "sinh" => Some(format!("SINH({})", args.join(", "))),
        "cosh" => Some(format!("COSH({})", args.join(", "))),
        "tanh" => Some(format!("TANH({})", args.join(", "))),
        // String functions
        "tolower" => Some(format!("LOWER({})", args.join(", "))),
        "toupper" | "touppercase" => Some(format!("UPPER({})", args.join(", "))),
        "substr" => {
            if args.len() >= 3 {
                Some(format!("SUBSTR({}, {}, {})", args[0], args[1], args[2]))
            } else if args.len() == 2 {
                Some(format!("SUBSTR({}, {})", args[0], args[1]))
            } else {
                None
            }
        }
        "nchar" | "nzchar" => Some(format!("LENGTH({})", args.join(", "))),
        "trimws" => Some(format!("TRIM({})", args.join(", "))),
        // Conditional
        "ifelse" => {
            if args.len() == 3 {
                Some(format!(
                    "CASE WHEN {} THEN {} ELSE {} END",
                    args[0], args[1], args[2]
                ))
            } else {
                None
            }
        }
        // NULL checks
        "is.na" => Some(format!("{} IS NULL", args[0])),
        // Window functions
        "lead" => {
            let n = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
            Some(format!("LEAD({}, {}) OVER ()", args[0], n))
        }
        "lag" => {
            let n = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
            Some(format!("LAG({}, {}) OVER ()", args[0], n))
        }
        "rank" => Some("RANK() OVER ()".to_string()),
        "ntile" => {
            if !args.is_empty() {
                Some(format!("NTILE({}) OVER ()", args[0]))
            } else {
                None
            }
        }
        "first" | "first_value" => Some("FIRST_VALUE() OVER ()".to_string()),
        "last" | "last_value" => Some("LAST_VALUE() OVER ()".to_string()),
        "nth_value" => {
            if args.len() >= 2 {
                Some(format!("NTH_VALUE({}, {}) OVER ()", args[0], args[1]))
            } else {
                None
            }
        }
        "row_number" => Some("ROW_NUMBER() OVER ()".to_string()),
        // NULL handling
        "coalesce" => Some(format!("COALESCE({})", args.join(", "))),
        "na.replace" | "replace_na" => {
            if args.len() >= 2 {
                Some(format!("COALESCE({}, {})", args[0], args[1]))
            } else {
                None
            }
        }
        _ => None,
    }
}

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

    /// Dialect name for error messages.
    fn dialect_name(&self) -> &'static str {
        "unknown"
    }

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

    /// Returns `* EXCLUDE (...)`-style projection if supported by the dialect.
    fn select_star_exclude(&self, _excluded_identifiers: &[String]) -> Option<String> {
        None
    }

    /// Translates R/dplyr function names to SQL equivalents.
    ///
    /// Maps common R functions to their SQL counterparts. Override this
    /// method in dialect implementations for database-specific translations.
    fn translate_function(&self, function: &str, args: &[String]) -> Option<String> {
        translate_common_function(function, args)
    }

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

    fn dialect_name(&self) -> &'static str {
        "postgresql"
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

    fn dialect_name(&self) -> &'static str {
        "mysql"
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

    fn dialect_name(&self) -> &'static str {
        "duckdb"
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

    fn select_star_exclude(&self, excluded_identifiers: &[String]) -> Option<String> {
        if excluded_identifiers.is_empty() {
            return Some("*".to_string());
        }
        let list = excluded_identifiers
            .iter()
            .map(|ident| self.quote_identifier(ident))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!("* EXCLUDE ({list})"))
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

    fn dialect_name(&self) -> &'static str {
        "sqlite"
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
