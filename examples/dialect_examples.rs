//! SQL Dialect Examples
//!
//! This example demonstrates how to use libdplyr with different SQL dialects.
//! Each dialect has its own syntax quirks and optimizations.

use libdplyr::{
    DuckDbDialect, MySqlDialect, PostgreSqlDialect, SqliteDialect, TranspileError, Transpiler,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== libdplyr SQL Dialect Examples ===\n");

    // Sample dplyr queries to demonstrate dialect differences
    let queries = vec![
        ("Basic Selection", "select(name, age, salary)"),
        ("Filtering", "select(name, age) %>% filter(age > 18 & salary >= 50000)"),
        ("Mutation", "select(name, salary) %>% mutate(bonus = salary * 0.1, tax = salary * 0.2)"),
        ("Sorting", "select(name, age, salary) %>% arrange(desc(salary), name)"),
        ("Grouping and Aggregation", "group_by(department) %>% summarise(avg_salary = mean(salary), employee_count = n())"),
        ("Complex Pipeline", "select(employee_id, name, department, salary) %>% filter(salary >= 50000) %>% mutate(annual_bonus = salary * 0.15) %>% arrange(desc(salary)) %>% group_by(department) %>% summarise(total_employees = n(), avg_salary = mean(salary))"),
    ];

    // Test each dialect
    let dialects: Vec<(&str, Box<dyn libdplyr::SqlDialect>)> = vec![
        ("PostgreSQL", Box::new(PostgreSqlDialect::new())),
        ("MySQL", Box::new(MySqlDialect::new())),
        ("SQLite", Box::new(SqliteDialect::new())),
        ("DuckDB", Box::new(DuckDbDialect::new())),
    ];

    for (query_name, dplyr_code) in &queries {
        println!("## {}\n", query_name);
        println!("**dplyr code:**");
        println!("```r");
        println!("{}", dplyr_code.trim());
        println!("```\n");

        for (dialect_name, _) in &dialects {
            let transpiler = match *dialect_name {
                "PostgreSQL" => Transpiler::new(Box::new(PostgreSqlDialect::new())),
                "MySQL" => Transpiler::new(Box::new(MySqlDialect::new())),
                "SQLite" => Transpiler::new(Box::new(SqliteDialect::new())),
                "DuckDB" => Transpiler::new(Box::new(DuckDbDialect::new())),
                _ => continue,
            };

            match transpiler.transpile(dplyr_code) {
                Ok(sql) => {
                    println!("**{} SQL:**", dialect_name);
                    println!("```sql");
                    println!("{}", format_sql_for_display(&sql));
                    println!("```\n");
                }
                Err(e) => {
                    println!("**{} Error:**", dialect_name);
                    println!("```");
                    println!("Error: {}", e);
                    println!("```\n");
                }
            }
        }

        println!("---\n");
    }

    // Demonstrate dialect-specific features
    demonstrate_dialect_specific_features()?;

    // Show error handling examples
    demonstrate_error_handling()?;

    Ok(())
}

/// Demonstrates features specific to certain dialects
fn demonstrate_dialect_specific_features() -> Result<(), Box<dyn std::error::Error>> {
    println!("## Dialect-Specific Features\n");

    // DuckDB-specific analytical functions
    println!("### DuckDB Analytical Functions\n");
    let duckdb_transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));

    let analytical_queries = vec![
        (
            "Median calculation",
            "group_by(department) %>% summarise(median_salary = median(salary))",
        ),
        (
            "Mode calculation",
            "group_by(category) %>% summarise(most_common = mode(status))",
        ),
    ];

    for (desc, query) in analytical_queries {
        println!("**{}:**", desc);
        println!("```r");
        println!("{}", query);
        println!("```");

        match duckdb_transpiler.transpile(query) {
            Ok(sql) => {
                println!("```sql");
                println!("{}", format_sql_for_display(&sql));
                println!("```\n");
            }
            Err(e) => {
                println!("Error: {}\n", e);
            }
        }
    }

    // MySQL-specific string functions
    println!("### MySQL String Concatenation\n");
    let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
    let pg_transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

    let concat_query = r#"select(first_name, last_name) %>% mutate(full_name = concat(first_name, " ", last_name))"#;

    println!("**Query:**");
    println!("```r");
    println!("{}", concat_query);
    println!("```\n");

    println!("**MySQL (uses CONCAT function):**");
    if let Ok(sql) = mysql_transpiler.transpile(concat_query) {
        println!("```sql");
        println!("{}", format_sql_for_display(&sql));
        println!("```\n");
    }

    println!("**PostgreSQL (uses || operator):**");
    if let Ok(sql) = pg_transpiler.transpile(concat_query) {
        println!("```sql");
        println!("{}", format_sql_for_display(&sql));
        println!("```\n");
    }

    Ok(())
}

/// Demonstrates comprehensive error handling
fn demonstrate_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("## Error Handling Examples\n");

    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

    let error_cases = vec![
        ("Invalid function", "invalid_function(test)"),
        ("Malformed syntax", "select(name %>% filter"),
        ("Unterminated string", r#"select("unterminated_string)"#),
        ("Invalid pipe operator", "select(name) %> filter(age > 18)"),
        ("Missing parentheses", "select name, age"),
        ("Invalid operator", "filter(age >> 18)"),
    ];

    for (error_type, invalid_code) in error_cases {
        println!("### {}\n", error_type);
        println!("**Invalid dplyr code:**");
        println!("```r");
        println!("{}", invalid_code);
        println!("```\n");

        match transpiler.transpile(invalid_code) {
            Ok(_) => {
                println!("Unexpected success!\n");
            }
            Err(e) => {
                println!("**Error type and message:**");
                println!("```");
                match e {
                    TranspileError::LexError(lex_err) => {
                        println!("Lexing Error: {}", lex_err);
                        println!("Hint: Check for invalid characters, unterminated strings, or malformed operators");
                    }
                    TranspileError::ParseError(parse_err) => {
                        println!("Parsing Error: {}", parse_err);
                        println!("Hint: Check dplyr function syntax, parentheses matching, and argument structure");
                    }
                    TranspileError::GenerationError(gen_err) => {
                        println!("Generation Error: {}", gen_err);
                        println!(
                            "Hint: The operation may not be supported in the selected SQL dialect"
                        );
                    }
                    TranspileError::IoError(io_err) => {
                        println!("I/O Error: {}", io_err);
                        println!("Hint: Check file permissions and paths");
                    }
                    TranspileError::ValidationError(val_err) => {
                        println!("Validation Error: {}", val_err);
                        println!("Hint: Check dplyr syntax and function usage");
                    }
                    TranspileError::ConfigurationError(config_err) => {
                        println!("Configuration Error: {}", config_err);
                        println!("Hint: Check configuration settings and options");
                    }
                    TranspileError::SystemError(sys_err) => {
                        println!("System Error: {}", sys_err);
                        println!("Hint: Check system resources and permissions");
                    }
                }
                println!("```\n");
            }
        }
    }

    Ok(())
}

/// Formats SQL for better display in examples
fn format_sql_for_display(sql: &str) -> String {
    sql.replace(" FROM ", "\nFROM ")
        .replace(" WHERE ", "\nWHERE ")
        .replace(" GROUP BY ", "\nGROUP BY ")
        .replace(" ORDER BY ", "\nORDER BY ")
        .replace(" AND ", "\n  AND ")
        .replace(" OR ", "\n  OR ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_dialects_basic_query() {
        let dialects: Vec<Box<dyn libdplyr::SqlDialect>> = vec![
            Box::new(PostgreSqlDialect::new()),
            Box::new(MySqlDialect::new()),
            Box::new(SqliteDialect::new()),
            Box::new(DuckDbDialect::new()),
        ];

        let query = "select(name, age) %>% filter(age > 18)";

        for dialect in dialects {
            let transpiler = Transpiler::new(dialect);
            let result = transpiler.transpile(query);
            assert!(result.is_ok(), "Basic query should work for all dialects");
        }
    }

    #[test]
    fn test_dialect_specific_identifier_quoting() {
        let pg_transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect::new()));

        let query = "select(user_name)";

        let pg_sql = pg_transpiler.transpile(query).unwrap();
        let mysql_sql = mysql_transpiler.transpile(query).unwrap();

        assert!(
            pg_sql.contains("\"user_name\""),
            "PostgreSQL should use double quotes"
        );
        assert!(
            mysql_sql.contains("`user_name`"),
            "MySQL should use backticks"
        );
    }

    #[test]
    fn test_complex_pipeline_all_dialects() {
        let dialects: Vec<Box<dyn libdplyr::SqlDialect>> = vec![
            Box::new(PostgreSqlDialect::new()),
            Box::new(MySqlDialect::new()),
            Box::new(SqliteDialect::new()),
            Box::new(DuckDbDialect::new()),
        ];

        let complex_query = r#"
            select(name, age, salary) %>%
            filter(age >= 18 & salary > 50000) %>%
            mutate(bonus = salary * 0.1) %>%
            arrange(desc(salary)) %>%
            group_by(department) %>%
            summarise(avg_salary = mean(salary), count = n())
        "#;

        for dialect in dialects {
            let transpiler = Transpiler::new(dialect);
            let result = transpiler.transpile(complex_query);
            assert!(result.is_ok(), "Complex query should work for all dialects");

            let sql = result.unwrap();
            assert!(sql.contains("SELECT"), "Should contain SELECT");
            assert!(sql.contains("WHERE"), "Should contain WHERE");
            assert!(sql.contains("GROUP BY"), "Should contain GROUP BY");
        }
    }

    #[test]
    fn test_error_handling_consistency() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

        let invalid_queries = vec![
            "invalid_function(test)",
            "select(name %>% filter",
            "select name, age",
        ];

        for query in invalid_queries {
            let result = transpiler.transpile(query);
            assert!(
                result.is_err(),
                "Invalid query '{}' should return error",
                query
            );
        }
    }

    #[test]
    fn test_duckdb_specific_functions() {
        let duckdb_transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));

        // Test median function (DuckDB specific)
        let median_query = "group_by(category) %>% summarise(median_value = median(price))";
        let result = duckdb_transpiler.transpile(median_query);

        assert!(result.is_ok(), "DuckDB should support median function");
        let sql = result.unwrap();
        assert!(sql.contains("MEDIAN"), "Should contain MEDIAN function");
    }

    #[test]
    fn test_format_sql_for_display() {
        let input = "SELECT name FROM users WHERE age > 18 GROUP BY department ORDER BY name";
        let formatted = format_sql_for_display(input);

        assert!(
            formatted.contains("\nFROM"),
            "Should format FROM on new line"
        );
        assert!(
            formatted.contains("\nWHERE"),
            "Should format WHERE on new line"
        );
        assert!(
            formatted.contains("\nGROUP BY"),
            "Should format GROUP BY on new line"
        );
        assert!(
            formatted.contains("\nORDER BY"),
            "Should format ORDER BY on new line"
        );
    }
}
