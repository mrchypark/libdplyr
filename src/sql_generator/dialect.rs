//! SQL dialects.

/// Translates a common R/tidyverse function to dialect-specific SQL.
fn translate_common_function<D: SqlDialect + ?Sized>(
    dialect: &D,
    function: &str,
    args: &[String],
) -> Option<String> {
    let fn_lower = function.to_lowercase();
    match fn_lower.as_str() {
        // Math functions
        "abs" => unary_sql_function("ABS", args),
        "round" => one_or_two_arg_sql_function("ROUND", args),
        "floor" => unary_sql_function("FLOOR", args),
        "ceiling" | "ceil" => unary_sql_function("CEILING", args),
        "sqrt" => unary_sql_function("SQRT", args),
        "sign" => unary_sql_function("SIGN", args),
        "exp" => unary_sql_function("EXP", args),
        "log" => {
            if args.len() == 1 {
                Some(format!("LN({})", args[0]))
            } else if args.len() == 2 {
                Some(format!("LOG({}, {})", args[1], args[0]))
            } else {
                None
            }
        }
        "log10" => {
            if args.len() == 1 {
                Some(dialect.log10(&args[0]))
            } else {
                None
            }
        }
        // Modulo
        "mod" | "%%" => {
            if args.len() == 2 {
                Some(format!("{} % {}", args[0], args[1]))
            } else {
                None
            }
        }
        // Trigonometric functions
        "sin" => unary_sql_function("SIN", args),
        "cos" => unary_sql_function("COS", args),
        "tan" => unary_sql_function("TAN", args),
        "asin" => unary_sql_function("ASIN", args),
        "acos" => unary_sql_function("ACOS", args),
        "atan" => unary_sql_function("ATAN", args),
        "atan2" => {
            if args.len() == 2 {
                Some(format!("ATAN2({}, {})", args[0], args[1]))
            } else {
                None
            }
        }
        "sinh" => unary_sql_function("SINH", args),
        "cosh" => unary_sql_function("COSH", args),
        "tanh" => unary_sql_function("TANH", args),
        // String functions
        "concat" | "paste0" => dialect.concat_no_separator(args),
        "paste" => dialect.concat_with_separator("' '", args),
        "tolower" | "lower" => unary_sql_function("LOWER", args),
        "toupper" | "touppercase" | "upper" => unary_sql_function("UPPER", args),
        "str_detect" => {
            if args.len() == 2 {
                dialect.regex_detect(&args[0], &args[1])
            } else {
                None
            }
        }
        "str_length" => {
            if args.len() == 1 {
                Some(dialect.char_length(&args[0]))
            } else {
                None
            }
        }
        "str_to_lower" => unary_sql_function("LOWER", args),
        "str_to_upper" => unary_sql_function("UPPER", args),
        "str_trim" => unary_sql_function("TRIM", args),
        "substr" => {
            if args.len() >= 3 {
                Some(format!(
                    "SUBSTR({}, {}, ({} - {} + 1))",
                    args[0], args[1], args[2], args[1]
                ))
            } else if args.len() == 2 {
                Some(format!("SUBSTR({}, {})", args[0], args[1]))
            } else {
                None
            }
        }
        "nchar" => {
            if args.len() == 1 {
                Some(dialect.char_length(&args[0]))
            } else {
                None
            }
        }
        "nzchar" => {
            if args.len() == 1 {
                Some(format!("({} > 0)", dialect.char_length(&args[0])))
            } else {
                None
            }
        }
        "trimws" => unary_sql_function("TRIM", args),
        // Conditional
        "as.numeric" | "as.double" | "as.integer" | "as.character" | "as.logical" => {
            if args.len() == 1 {
                dialect
                    .r_cast_type(&fn_lower)
                    .map(|sql_type| format!("CAST({} AS {sql_type})", args[0]))
            } else {
                None
            }
        }
        "ifelse" | "if_else" => {
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
        "is.na" => {
            if args.len() == 1 {
                Some(format!("{} IS NULL", args[0]))
            } else {
                None
            }
        }
        // Window functions
        "lead" => {
            if args.is_empty() {
                None
            } else {
                let n = args.get(1).map(String::as_str).unwrap_or("1");
                Some(format!("LEAD({}, {}) OVER ()", args[0], n))
            }
        }
        "lag" => {
            if args.is_empty() {
                None
            } else {
                let n = args.get(1).map(String::as_str).unwrap_or("1");
                Some(format!("LAG({}, {}) OVER ()", args[0], n))
            }
        }
        "rank" => Some("RANK() OVER ()".to_string()),
        "dense_rank" => Some("DENSE_RANK() OVER ()".to_string()),
        "ntile" => {
            if !args.is_empty() {
                Some(format!("NTILE({}) OVER ()", args[0]))
            } else {
                None
            }
        }
        "first" | "first_value" => unary_window_function("FIRST_VALUE", args),
        "last" | "last_value" => unary_window_function("LAST_VALUE", args),
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

fn unary_sql_function(sql_function: &str, args: &[String]) -> Option<String> {
    if args.len() == 1 {
        Some(format!("{sql_function}({})", args[0]))
    } else {
        None
    }
}

fn one_or_two_arg_sql_function(sql_function: &str, args: &[String]) -> Option<String> {
    if (1..=2).contains(&args.len()) {
        Some(format!("{sql_function}({})", args.join(", ")))
    } else {
        None
    }
}

fn unary_window_function(sql_function: &str, args: &[String]) -> Option<String> {
    if args.len() == 1 {
        Some(format!("{sql_function}({}) OVER ()", args[0]))
    } else {
        None
    }
}

/// Returns whether a common R function has an explicit SQL translation.
fn is_supported_common_function(function: &str) -> bool {
    matches!(
        function.to_lowercase().as_str(),
        "abs"
            | "round"
            | "floor"
            | "ceiling"
            | "ceil"
            | "sqrt"
            | "sign"
            | "exp"
            | "log"
            | "log10"
            | "mod"
            | "%%"
            | "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "atan2"
            | "sinh"
            | "cosh"
            | "tanh"
            | "concat"
            | "paste"
            | "paste0"
            | "tolower"
            | "lower"
            | "toupper"
            | "touppercase"
            | "upper"
            | "str_detect"
            | "str_length"
            | "str_to_lower"
            | "str_to_upper"
            | "str_trim"
            | "substr"
            | "nchar"
            | "nzchar"
            | "trimws"
            | "as.numeric"
            | "as.double"
            | "as.integer"
            | "as.character"
            | "as.logical"
            | "ifelse"
            | "if_else"
            | "is.na"
            | "lead"
            | "lag"
            | "rank"
            | "dense_rank"
            | "ntile"
            | "first"
            | "first_value"
            | "last"
            | "last_value"
            | "nth_value"
            | "row_number"
            | "coalesce"
            | "na.replace"
            | "replace_na"
    )
}

/// Translates common dplyr aggregate functions to SQL aggregate names.
fn translate_common_aggregate_function(function: &str) -> Option<String> {
    match function.to_lowercase().as_str() {
        "mean" | "avg" => Some("AVG".to_string()),
        "sum" => Some("SUM".to_string()),
        "count" => Some("COUNT".to_string()),
        "min" => Some("MIN".to_string()),
        "max" => Some("MAX".to_string()),
        "n" => Some("COUNT(*)".to_string()),
        _ => None,
    }
}

fn is_safe_sql_function_name(function: &str) -> bool {
    function.split('.').all(|part| {
        let mut chars = part.chars();
        matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
            && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

fn pass_through_function_call(function: &str, args: &[String]) -> Option<String> {
    if is_safe_sql_function_name(function) {
        Some(format!("{function}({})", args.join(", ")))
    } else {
        None
    }
}

fn concat_with_operator(args: &[String]) -> Option<String> {
    if args.is_empty() {
        None
    } else if args.len() == 1 {
        Some(args[0].clone())
    } else {
        Some(format!("({})", args.join(" || ")))
    }
}

fn concat_with_separator_operator(separator: &str, args: &[String]) -> Option<String> {
    if args.is_empty() {
        return None;
    }

    let mut parts = Vec::with_capacity(args.len() * 2 - 1);
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            parts.push(separator.to_string());
        }
        parts.push(arg.clone());
    }

    concat_with_operator(&parts)
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
        translate_common_function(self, function, args)
            .or_else(|| self.translate_unknown_function(function, args))
    }

    /// Returns whether this dialect allows the function to be called.
    fn is_supported_function(&self, function: &str) -> bool {
        is_supported_common_function(function)
    }

    /// Translates aggregate function names to SQL equivalents.
    fn translate_aggregate_function(&self, function: &str) -> Option<String> {
        translate_common_aggregate_function(function)
    }

    /// Late-bound translation hook for dialects that can resolve functions later.
    fn translate_unknown_function(&self, _function: &str, _args: &[String]) -> Option<String> {
        None
    }

    /// Dialect-specific regular expression predicate for stringr::str_detect().
    fn regex_detect(&self, _value: &str, _pattern: &str) -> Option<String> {
        None
    }

    /// Dialect-specific character-count function for R string helpers.
    fn char_length(&self, value: &str) -> String {
        format!("LENGTH({value})")
    }

    /// Dialect-specific SQL type for R cast helpers.
    fn r_cast_type(&self, function: &str) -> Option<&'static str> {
        match function {
            "as.numeric" | "as.double" => Some("DOUBLE"),
            "as.integer" => Some("INTEGER"),
            "as.character" => Some("VARCHAR"),
            "as.logical" => Some("BOOLEAN"),
            _ => None,
        }
    }

    /// Dialect-specific base-10 logarithm function.
    fn log10(&self, value: &str) -> String {
        format!("LOG10({value})")
    }

    /// Concatenates string expressions without a separator.
    fn concat_no_separator(&self, args: &[String]) -> Option<String> {
        if args.is_empty() {
            None
        } else {
            Some(format!("CONCAT({})", args.join(", ")))
        }
    }

    /// Concatenates string expressions with a separator.
    fn concat_with_separator(&self, separator: &str, args: &[String]) -> Option<String> {
        if args.is_empty() {
            None
        } else {
            Some(format!("CONCAT_WS({separator}, {})", args.join(", ")))
        }
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
    pub const fn new() -> Self {
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

    fn regex_detect(&self, value: &str, pattern: &str) -> Option<String> {
        Some(format!("({value} ~ {pattern})"))
    }

    fn r_cast_type(&self, function: &str) -> Option<&'static str> {
        match function {
            "as.numeric" | "as.double" => Some("DOUBLE PRECISION"),
            "as.integer" => Some("INTEGER"),
            "as.character" => Some("TEXT"),
            "as.logical" => Some("BOOLEAN"),
            _ => None,
        }
    }

    fn log10(&self, value: &str) -> String {
        format!("LOG({value})")
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
    pub const fn new() -> Self {
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

    fn regex_detect(&self, value: &str, pattern: &str) -> Option<String> {
        Some(format!("REGEXP_LIKE({value}, {pattern})"))
    }

    fn char_length(&self, value: &str) -> String {
        format!("CHAR_LENGTH({value})")
    }

    fn r_cast_type(&self, function: &str) -> Option<&'static str> {
        match function {
            "as.numeric" | "as.double" => Some("DOUBLE"),
            "as.integer" => Some("SIGNED"),
            "as.character" => Some("CHAR"),
            "as.logical" => Some("BOOLEAN"),
            _ => None,
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
    pub const fn new() -> Self {
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
    pub const fn new() -> Self {
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

    fn translate_aggregate_function(&self, function: &str) -> Option<String> {
        translate_common_aggregate_function(function).or_else(|| {
            match function.to_lowercase().as_str() {
                "median" => Some("MEDIAN".to_string()),
                "mode" => Some("MODE".to_string()),
                _ if is_safe_sql_function_name(function) => Some(function.to_string()),
                _ => None,
            }
        })
    }

    fn translate_unknown_function(&self, function: &str, args: &[String]) -> Option<String> {
        pass_through_function_call(function, args)
    }

    fn regex_detect(&self, value: &str, pattern: &str) -> Option<String> {
        Some(format!("regexp_matches({value}, {pattern})"))
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

    fn r_cast_type(&self, function: &str) -> Option<&'static str> {
        match function {
            "as.numeric" | "as.double" => Some("REAL"),
            "as.integer" => Some("INTEGER"),
            "as.character" => Some("TEXT"),
            "as.logical" => Some("INTEGER"),
            _ => None,
        }
    }

    fn concat_no_separator(&self, args: &[String]) -> Option<String> {
        concat_with_operator(args)
    }

    fn concat_with_separator(&self, separator: &str, args: &[String]) -> Option<String> {
        concat_with_separator_operator(separator, args)
    }

    fn is_case_sensitive(&self) -> bool {
        false
    }

    fn clone_box(&self) -> Box<dyn SqlDialect> {
        Box::new(self.clone())
    }
}
