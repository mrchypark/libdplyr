//! CLI (Command Line Interface) module
//!
//! Provides an interface for using libdplyr from the command line.

use clap::{Arg, Command, ArgMatches, value_parser};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::{Transpiler, SqlDialect, PostgreSqlDialect, MySqlDialect, SqliteDialect};
use crate::error::TranspileError;

/// CLI arguments structure
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub input_file: Option<String>,
    pub output_file: Option<String>,
    pub dialect: SqlDialectType,
    pub pretty_print: bool,
    pub input_text: Option<String>,
}

/// Supported SQL dialect types
#[derive(Debug, Clone, PartialEq)]
pub enum SqlDialectType {
    PostgreSql,
    MySql,
    Sqlite,
}

impl std::fmt::Display for SqlDialectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlDialectType::PostgreSql => write!(f, "postgresql"),
            SqlDialectType::MySql => write!(f, "mysql"),
            SqlDialectType::Sqlite => write!(f, "sqlite"),
        }
    }
}

impl std::str::FromStr for SqlDialectType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgresql" | "postgres" | "pg" => Ok(SqlDialectType::PostgreSql),
            "mysql" => Ok(SqlDialectType::MySql),
            "sqlite" => Ok(SqlDialectType::Sqlite),
            _ => Err(format!("Unsupported SQL dialect: {}", s)),
        }
    }
}

/// Parses CLI arguments.
pub fn parse_args() -> CliArgs {
    let matches = Command::new("libdplyr")
        .version("0.1.0")
        .author("libdplyr contributors")
        .about("A transpiler that converts R dplyr syntax to SQL")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("FILE")
                .help("Input dplyr file path")
                .conflicts_with("text")
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output SQL file path (stdout if not specified)")
        )
        .arg(
            Arg::new("dialect")
                .short('d')
                .long("dialect")
                .value_name("DIALECT")
                .help("Target SQL dialect")
                .value_parser(value_parser!(SqlDialectType))
                .default_value("postgresql")
        )
        .arg(
            Arg::new("pretty")
                .short('p')
                .long("pretty")
                .help("Pretty-format SQL output")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("text")
                .short('t')
                .long("text")
                .value_name("DPLYR_CODE")
                .help("Direct dplyr code input")
                .conflicts_with("input")
        )
        .get_matches();

    parse_matches(&matches)
}

/// Creates CliArgs from ArgMatches.
fn parse_matches(matches: &ArgMatches) -> CliArgs {
    CliArgs {
        input_file: matches.get_one::<String>("input").cloned(),
        output_file: matches.get_one::<String>("output").cloned(),
        dialect: matches.get_one::<SqlDialectType>("dialect").cloned()
            .unwrap_or(SqlDialectType::PostgreSql),
        pretty_print: matches.get_flag("pretty"),
        input_text: matches.get_one::<String>("text").cloned(),
    }
}

/// Runs the CLI.
pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    
    // Read input
    let dplyr_code = read_input(&args)?;
    
    // Create transpiler
    let dialect: Box<dyn SqlDialect> = match args.dialect {
        SqlDialectType::PostgreSql => Box::new(PostgreSqlDialect),
        SqlDialectType::MySql => Box::new(MySqlDialect),
        SqlDialectType::Sqlite => Box::new(SqliteDialect),
    };
    
    let transpiler = Transpiler::new(dialect);
    
    // Perform conversion
    let sql = transpiler.transpile(&dplyr_code)
        .map_err(|e| format!("Conversion error: {}", e))?;
    
    // Apply formatting
    let formatted_sql = if args.pretty_print {
        format_sql(&sql)
    } else {
        sql
    };
    
    // Write output
    write_output(&args, &formatted_sql)?;
    
    Ok(())
}

/// Reads input.
fn read_input(args: &CliArgs) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(text) = &args.input_text {
        Ok(text.clone())
    } else if let Some(input_file) = &args.input_file {
        if !Path::new(input_file).exists() {
            return Err(format!("Input file not found: {}", input_file).into());
        }
        
        fs::read_to_string(input_file)
            .map_err(|e| format!("File read error ({}): {}", input_file, e).into())
    } else {
        // Read from standard input
        let mut input = String::new();
        io::stdin().read_line(&mut input)
            .map_err(|e| format!("Standard input read error: {}", e))?;
        Ok(input)
    }
}

/// Writes output.
fn write_output(args: &CliArgs, sql: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(output_file) = &args.output_file {
        fs::write(output_file, sql)
            .map_err(|e| format!("File write error ({}): {}", output_file, e))?;
        eprintln!("SQL saved to {}", output_file);
    } else {
        print!("{}", sql);
        io::stdout().flush()?;
    }
    Ok(())
}

/// Pretty-formats SQL.
fn format_sql(sql: &str) -> String {
    // Simple SQL formatting implementation
    sql.replace(" FROM ", "\nFROM ")
       .replace(" WHERE ", "\nWHERE ")
       .replace(" GROUP BY ", "\nGROUP BY ")
       .replace(" ORDER BY ", "\nORDER BY ")
       .replace(" AND ", "\n  AND ")
       .replace(" OR ", "\n  OR ")
}

/// Prints errors in a user-friendly way.
pub fn print_error(error: &TranspileError) {
    eprintln!("Error: {}", error);
    
    match error {
        TranspileError::LexError(_) => {
            eprintln!("Hint: Please check the syntax of your input code.");
            eprintln!("      Pay special attention to string quotes and special characters.");
        }
        TranspileError::ParseError(_) => {
            eprintln!("Hint: Please check the usage of dplyr functions.");
            eprintln!("      Example: data %>% select(col1, col2) %>% filter(col1 > 10)");
        }
        TranspileError::GenerationError(_) => {
            eprintln!("Hint: The selected SQL dialect may not support this feature.");
            eprintln!("      Try a different dialect or use simpler expressions.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_dialect_parsing() {
        assert_eq!("postgresql".parse::<SqlDialectType>().unwrap(), SqlDialectType::PostgreSql);
        assert_eq!("mysql".parse::<SqlDialectType>().unwrap(), SqlDialectType::MySql);
        assert_eq!("sqlite".parse::<SqlDialectType>().unwrap(), SqlDialectType::Sqlite);
        
        assert!("invalid".parse::<SqlDialectType>().is_err());
    }

    #[test]
    fn test_format_sql() {
        let sql = "SELECT name FROM users WHERE age > 18 ORDER BY name";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("\nFROM"));
        assert!(formatted.contains("\nWHERE"));
        assert!(formatted.contains("\nORDER BY"));
    }
}