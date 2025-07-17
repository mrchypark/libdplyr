//! Transpiler usage examples
//!
//! This example demonstrates various ways to use the libdplyr transpiler,
//! including basic usage, error handling, and advanced features.

use libdplyr::{
    Transpiler, PostgreSqlDialect, MySqlDialect, SqliteDialect, DuckDbDialect,
    TranspileError, DplyrNode, SqlDialect
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== libdplyr Transpiler Usage Examples ===\n");

    // Basic usage examples
    basic_usage_examples()?;
    
    // Error handling examples
    error_handling_examples()?;
    
    // Advanced usage examples
    advanced_usage_examples()?;
    
    // Performance comparison
    performance_comparison_examples()?;

    println!("✅ All transpiler usage examples completed successfully!");
    Ok(())
}

/// Demonstrates basic transpiler usage with different dialects
fn basic_usage_examples() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Basic Usage Examples");
    println!("=======================");

    let dplyr_code = r#"
        select(name, age, salary) %>%
        filter(age >= 18 & salary > 50000) %>%
        arrange(desc(salary))
    "#;
    
    println!("Input dplyr code:");
    println!("{}", dplyr_code.trim());
    println!();

    // PostgreSQL example
    println!("🐘 PostgreSQL:");
    let pg_transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    let pg_sql = pg_transpiler.transpile(dplyr_code)?;
    println!("{}\n", pg_sql);
    
    // MySQL example
    println!("🐬 MySQL:");
    let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
    let mysql_sql = mysql_transpiler.transpile(dplyr_code)?;
    println!("{}\n", mysql_sql);
    
    // SQLite example
    println!("🪶 SQLite:");
    let sqlite_transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
    let sqlite_sql = sqlite_transpiler.transpile(dplyr_code)?;
    println!("{}\n", sqlite_sql);
    
    // DuckDB example
    println!("🦆 DuckDB:");
    let duckdb_transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));
    let duckdb_sql = duckdb_transpiler.transpile(dplyr_code)?;
    println!("{}\n", duckdb_sql);

    Ok(())
}

/// Demonstrates comprehensive error handling
fn error_handling_examples() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚠️  Error Handling Examples");
    println!("===========================");

    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

    // Test various error scenarios
    let error_cases = vec![
        ("Empty input", ""),
        ("Invalid function", "invalid_function(test)"),
        ("Malformed syntax", "select(name %>% filter"),
        ("Unclosed parentheses", "select(name, age"),
        ("Invalid operator", "select(name) %>>% filter(age > 18)"),
        ("Missing arguments", "select() %>% filter()"),
    ];

    for (description, code) in error_cases {
        println!("Testing: {}", description);
        println!("Input: '{}'", code);
        
        match transpiler.transpile(code) {
            Ok(sql) => {
                println!("✅ Unexpected success: {}", sql);
            }
            Err(TranspileError::LexError(e)) => {
                println!("❌ Lexing error: {}", e);
            }
            Err(TranspileError::ParseError(e)) => {
                println!("❌ Parsing error: {}", e);
            }
            Err(TranspileError::GenerationError(e)) => {
                println!("❌ Generation error: {}", e);
            }
        }
        println!();
    }

    // Demonstrate proper error handling in application code
    println!("📝 Proper Error Handling Pattern:");
    demonstrate_error_handling_pattern(&transpiler, "invalid_syntax_example")?;

    Ok(())
}

/// Shows how to properly handle errors in application code
fn demonstrate_error_handling_pattern(
    transpiler: &Transpiler, 
    code: &str
) -> Result<(), Box<dyn std::error::Error>> {
    match transpiler.transpile(code) {
        Ok(sql) => {
            println!("✅ Successfully generated SQL:");
            println!("   {}", sql);
        }
        Err(TranspileError::LexError(e)) => {
            eprintln!("🔤 Tokenization failed:");
            eprintln!("   Error: {}", e);
            eprintln!("   Hint: Check for invalid characters or malformed tokens");
        }
        Err(TranspileError::ParseError(e)) => {
            eprintln!("📝 Parsing failed:");
            eprintln!("   Error: {}", e);
            eprintln!("   Hint: Check dplyr syntax - ensure proper function calls and operators");
        }
        Err(TranspileError::GenerationError(e)) => {
            eprintln!("🏗️  SQL generation failed:");
            eprintln!("   Error: {}", e);
            eprintln!("   Hint: The operation might not be supported in the selected SQL dialect");
        }
    }
    
    Ok(())
}

/// Demonstrates advanced transpiler features
fn advanced_usage_examples() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Advanced Usage Examples");
    println!("==========================");

    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

    // Example 1: Separate parsing and generation
    println!("1. Separate Parsing and SQL Generation:");
    let dplyr_code = "select(name, age) %>% filter(age > 18)";
    println!("   Input: {}", dplyr_code);
    
    // Parse to AST
    let ast = transpiler.parse_dplyr(dplyr_code)?;
    println!("   ✅ Parsed to AST successfully");
    
    // Generate SQL from AST
    let sql = transpiler.generate_sql(&ast)?;
    println!("   ✅ Generated SQL: {}\n", sql);

    // Example 2: AST inspection
    println!("2. AST Inspection:");
    inspect_ast(&ast);
    println!();

    // Example 3: Complex aggregation with multiple operations
    println!("3. Complex Aggregation Pipeline:");
    let complex_code = "select(department, employee_id, salary, performance_score) %>% filter(performance_score >= 3.0) %>% group_by(department) %>% summarise(employee_count = n(), avg_salary = mean(salary), total_payroll = sum(salary), top_performer = max(performance_score)) %>% arrange(desc(total_payroll))";
    
    println!("   Input: {}", complex_code);
    let complex_sql = transpiler.transpile(complex_code)?;
    println!("   Output: {}\n", complex_sql);

    // Example 4: Window functions (if supported)
    println!("4. Window Functions (DuckDB):");
    let duckdb_transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));
    let window_code = "select(employee_id, department, salary) %>% mutate(salary_rank = row_number(), dept_avg_salary = avg(salary)) %>% filter(salary_rank <= 3)";
    
    println!("   Input: {}", window_code);
    let window_sql = duckdb_transpiler.transpile(window_code)?;
    println!("   Output: {}\n", window_sql);

    Ok(())
}

/// Inspects and displays AST structure
fn inspect_ast(ast: &DplyrNode) {
    match ast {
        DplyrNode::Pipeline { operations, location } => {
            println!("   AST Type: Pipeline");
            println!("   Location: line {}, column {}", location.line, location.column);
            println!("   Operations: {} total", operations.len());
            
            for (i, op) in operations.iter().enumerate() {
                match op {
                    libdplyr::DplyrOperation::Select { columns, .. } => {
                        println!("     {}. Select: {} columns", i + 1, columns.len());
                    }
                    libdplyr::DplyrOperation::Filter { .. } => {
                        println!("     {}. Filter: condition applied", i + 1);
                    }
                    libdplyr::DplyrOperation::Mutate { assignments, .. } => {
                        println!("     {}. Mutate: {} assignments", i + 1, assignments.len());
                    }
                    libdplyr::DplyrOperation::Arrange { columns, .. } => {
                        println!("     {}. Arrange: {} columns", i + 1, columns.len());
                    }
                    libdplyr::DplyrOperation::GroupBy { columns, .. } => {
                        println!("     {}. GroupBy: {} columns", i + 1, columns.len());
                    }
                    libdplyr::DplyrOperation::Summarise { aggregations, .. } => {
                        println!("     {}. Summarise: {} aggregations", i + 1, aggregations.len());
                    }
                }
            }
        }
        DplyrNode::DataSource { name, location } => {
            println!("   AST Type: DataSource");
            println!("   Name: {}", name);
            println!("   Location: line {}, column {}", location.line, location.column);
        }
    }
}

/// Compares performance across different dialects
fn performance_comparison_examples() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ Performance Comparison");
    println!("========================");

    let test_query = "select(customer_id, product_id, order_date, amount) %>% filter(order_date >= \"2024-01-01\" & amount > 100) %>% group_by(customer_id) %>% summarise(total_orders = n(), total_amount = sum(amount), avg_order_value = mean(amount)) %>% arrange(desc(total_amount))";

    let dialects: Vec<(&str, Box<dyn SqlDialect>)> = vec![
        ("PostgreSQL", Box::new(PostgreSqlDialect::new())),
        ("MySQL", Box::new(MySqlDialect::new())),
        ("SQLite", Box::new(SqliteDialect::new())),
        ("DuckDB", Box::new(DuckDbDialect::new())),
    ];

    println!("Testing query performance across dialects:");
    println!("Query: {}", test_query.trim());
    println!();

    for (name, dialect) in dialects {
        let transpiler = Transpiler::new(dialect);
        
        // Simple timing (for demonstration - use proper benchmarking tools for real measurements)
        let start = std::time::Instant::now();
        let result = transpiler.transpile(test_query);
        let duration = start.elapsed();
        
        match result {
            Ok(sql) => {
                println!("✅ {}: {:?} (SQL length: {} chars)", name, duration, sql.len());
            }
            Err(e) => {
                println!("❌ {}: {:?} - Error: {}", name, duration, e);
            }
        }
    }

    println!("\n💡 Note: Use `cargo bench` for accurate performance measurements");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_transpilation() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let result = transpiler.transpile("select(name, age) %>% filter(age > 18)");
        
        assert!(result.is_ok(), "Basic transpilation should succeed");
        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("WHERE"));
    }

    #[test]
    fn test_error_handling() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        
        // Test various error conditions
        let error_cases = vec![
            "",
            "invalid_function(test)",
            "select(name %>% filter",
            "select(name, age",
        ];

        for case in error_cases {
            let result = transpiler.transpile(case);
            assert!(result.is_err(), "Invalid input '{}' should produce an error", case);
        }
    }

    #[test]
    fn test_separate_parse_and_generate() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let code = "select(name) %>% filter(age > 18)";
        
        // Parse to AST
        let ast_result = transpiler.parse_dplyr(code);
        assert!(ast_result.is_ok(), "Parsing should succeed");
        
        let ast = ast_result.unwrap();
        assert!(ast.is_pipeline(), "Should be a pipeline");
        
        // Generate SQL from AST
        let sql_result = transpiler.generate_sql(&ast);
        assert!(sql_result.is_ok(), "SQL generation should succeed");
        
        let sql = sql_result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("WHERE"));
    }

    #[test]
    fn test_all_dialects() {
        let query = "select(name, age) %>% filter(age > 18)";
        let dialects: Vec<Box<dyn SqlDialect>> = vec![
            Box::new(PostgreSqlDialect::new()),
            Box::new(MySqlDialect::new()),
            Box::new(SqliteDialect::new()),
            Box::new(DuckDbDialect::new()),
        ];

        for dialect in dialects {
            let transpiler = Transpiler::new(dialect);
            let result = transpiler.transpile(query);
            assert!(result.is_ok(), "All dialects should handle basic queries");
        }
    }

    #[test]
    fn test_complex_pipeline() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let complex_query = r#"
            select(id, name, category, price) %>%
            filter(price > 10 & category == "electronics") %>%
            mutate(discounted_price = price * 0.9) %>%
            group_by(category) %>%
            summarise(avg_price = mean(discounted_price)) %>%
            arrange(desc(avg_price))
        "#;

        let result = transpiler.transpile(complex_query);
        assert!(result.is_ok(), "Complex pipeline should transpile successfully");
        
        let sql = result.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("GROUP BY"));
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("AVG"));
    }
}