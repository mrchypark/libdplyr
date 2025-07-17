43E# libdplyr Usage Guide

This guide explains how to effectively use libdplyr step by step.

## Table of Contents

1. [Installation and Setup](#installation-and-setup)
2. [Basic Usage](#basic-usage)
3. [Advanced Features](#advanced-features)
4. [SQL Dialect-Specific Usage](#sql-dialect-specific-usage)
5. [Performance Optimization](#performance-optimization)
6. [Error Handling](#error-handling)
7. [CLI Usage](#cli-usage)
8. [Integration Examples](#integration-examples)
9. [Troubleshooting](#troubleshooting)

## Installation and Setup

### Add to Rust Project

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
libdplyr = "0.1.0"
```

### Install CLI Tool

```bash
cargo install libdplyr
```

### Build from Development Environment

```bash
git clone https://github.com/your-repo/libdplyr.git
cd libdplyr
cargo build --release
```

## Basic Usage

### 1. Simple Conversion

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create transpiler
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    // Convert dplyr code to SQL
    let dplyr_code = "select(name, age) %>% filter(age > 18)";
    let sql = transpiler.transpile(dplyr_code)?;
    
    println!("Generated SQL:");
    println!("{}", sql);
    
    Ok(())
}
```

**Output:**
```sql
SELECT "name", "age"
FROM "data"
WHERE "age" > 18
```

### 2. Supported dplyr Functions

#### select() - Column Selection

```rust
// Basic column selection
let query1 = "select(name, age, salary)";

// Using aliases
let query2 = "select(employee_name = name, employee_age = age)";

// With function calls
let query3 = "select(name, upper(department))";
```

#### filter() - Row Filtering

```rust
// Simple condition
let query1 = "filter(age > 18)";

// Complex conditions
let query2 = "filter(age >= 18 & salary > 50000)";

// String comparison
let query3 = r#"filter(department == "Engineering" | department == "Sales")"#;

// Logical operators
let query4 = "filter((age > 25 & salary > 60000) | (age < 25 & salary > 40000))";
```

#### mutate() - Create New Columns

```rust
// Simple calculation
let query1 = "mutate(bonus = salary * 0.1)";

// Multiple column creation
let query2 = "mutate(bonus = salary * 0.1, tax = salary * 0.2, net = salary - tax)";

// Conditional calculation
let query3 = "mutate(category = if(age >= 18, \"Adult\", \"Minor\"))";
```

#### arrange() - Sorting

```rust
// Ascending sort
let query1 = "arrange(name)";

// Descending sort
let query2 = "arrange(desc(salary))";

// Multiple column sort
let query3 = "arrange(department, desc(salary), name)";
```

#### group_by() and summarise() - Grouping and Aggregation

```rust
// Basic grouping and aggregation
let query1 = "group_by(department) %>% summarise(avg_salary = mean(salary))";

// Multiple grouping
let query2 = "group_by(department, location) %>% summarise(count = n(), total_salary = sum(salary))";

// Multiple aggregate functions
let query3 = r#"
    group_by(department) %>% 
    summarise(
        employee_count = n(),
        avg_salary = mean(salary),
        min_salary = min(salary),
        max_salary = max(salary),
        total_salary = sum(salary)
    )
"#;
```

### 3. Pipeline Chaining

```rust
let complex_pipeline = r#"
    select(employee_id, name, department, salary, hire_date) %>%
    filter(salary >= 50000) %>%
    mutate(
        annual_bonus = salary * 0.15,
        years_employed = 2024 - year(hire_date),
        senior_employee = years_employed > 5
    ) %>%
    arrange(desc(salary), name) %>%
    group_by(department, senior_employee) %>%
    summarise(
        employee_count = n(),
        avg_salary = mean(salary),
        total_bonus = sum(annual_bonus)
    )
"#;

let sql = transpiler.transpile(complex_pipeline)?;
```

## Advanced Features

### 1. AST Manipulation

```rust
use libdplyr::{Transpiler, PostgreSqlDialect, DplyrNode, DplyrOperation};

let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

// Parse dplyr code to AST
let ast = transpiler.parse_dplyr("select(name, age) %>% filter(age > 18)")?;

// Inspect AST structure
match &ast {
    DplyrNode::Pipeline { operations, .. } => {
        println!("Pipeline with {} operations", operations.len());
        
        for (i, operation) in operations.iter().enumerate() {
            match operation {
                DplyrOperation::Select { columns, .. } => {
                    println!("Operation {}: SELECT with {} columns", i, columns.len());
                }
                DplyrOperation::Filter { .. } => {
                    println!("Operation {}: FILTER", i);
                }
                _ => {
                    println!("Operation {}: {}", i, operation.operation_name());
                }
            }
        }
    }
    DplyrNode::DataSource { name, .. } => {
        println!("Data source: {}", name);
    }
}

// Generate SQL from AST
let sql = transpiler.generate_sql(&ast)?;
println!("Generated SQL: {}", sql);
```

### 2. Custom Error Handling

```rust
use libdplyr::{Transpiler, TranspileError, PostgreSqlDialect};

fn safe_transpile(dplyr_code: &str) -> Result<String, String> {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    match transpiler.transpile(dplyr_code) {
        Ok(sql) => Ok(sql),
        Err(TranspileError::LexError(e)) => {
            Err(format!("Syntax error: {}. Check string quotes or special characters.", e))
        }
        Err(TranspileError::ParseError(e)) => {
            Err(format!("Parse error: {}. Check dplyr function usage.", e))
        }
        Err(TranspileError::GenerationError(e)) => {
            Err(format!("SQL generation error: {}. Try a different dialect.", e))
        }
    }
}

// Usage example
match safe_transpile("select(name, age) %>% filter(age > 18)") {
    Ok(sql) => println!("Success: {}", sql),
    Err(msg) => eprintln!("Error: {}", msg),
}
```

### 3. Batch Processing

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

fn batch_transpile(queries: Vec<&str>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    let mut results = Vec::new();
    
    for (i, query) in queries.iter().enumerate() {
        match transpiler.transpile(query) {
            Ok(sql) => {
                println!("Query {}: Success", i + 1);
                results.push(sql);
            }
            Err(e) => {
                eprintln!("Query {}: Error - {}", i + 1, e);
                return Err(e.into());
            }
        }
    }
    
    Ok(results)
}

// Usage example
let queries = vec![
    "select(name, age)",
    "filter(age > 18)",
    "group_by(department) %>% summarise(count = n())",
];

match batch_transpile(queries) {
    Ok(sql_queries) => {
        for (i, sql) in sql_queries.iter().enumerate() {
            println!("SQL {}: {}", i + 1, sql);
        }
    }
    Err(e) => eprintln!("Batch processing failed: {}", e),
}
```

## SQL Dialect-Specific Usage

### PostgreSQL

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));

// PostgreSQL features:
// - Identifiers: "column_name"
// - String concatenation: ||
// - Standard SQL function support

let examples = vec![
    ("Basic query", "select(name, age) %>% filter(age > 18)"),
    ("String concatenation", r#"mutate(full_name = first_name || " " || last_name)"#),
    ("Aggregate functions", "group_by(dept) %>% summarise(avg_sal = mean(salary))"),
];

for (desc, query) in examples {
    println!("{}: {}", desc, transpiler.transpile(query)?);
}
```

### MySQL

```rust
use libdplyr::{Transpiler, MySqlDialect};

let transpiler = Transpiler::new(Box::new(MySqlDialect::new()));

// MySQL features:
// - Identifiers: `column_name`
// - String concatenation: CONCAT()
// - MySQL-specific function support

let examples = vec![
    ("Basic query", "select(name, age) %>% filter(age > 18)"),
    ("String concatenation", r#"mutate(full_name = concat(first_name, " ", last_name))"#),
    ("Aggregate functions", "group_by(dept) %>% summarise(avg_sal = mean(salary))"),
];

for (desc, query) in examples {
    println!("{}: {}", desc, transpiler.transpile(query)?);
}
```

### SQLite

```rust
use libdplyr::{Transpiler, SqliteDialect};

let transpiler = Transpiler::new(Box::new(SqliteDialect::new()));

// SQLite features:
// - Identifiers: "column_name"
// - String concatenation: ||
// - Lightweight database optimization

let examples = vec![
    ("Basic query", "select(name, age) %>% filter(age > 18)"),
    ("Simple aggregation", "group_by(category) %>% summarise(count = n())"),
];

for (desc, query) in examples {
    println!("{}: {}", desc, transpiler.transpile(query)?);
}
```

### DuckDB

```rust
use libdplyr::{Transpiler, DuckDbDialect};

let transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));

// DuckDB features:
// - Identifiers: "column_name"
// - Analytical function support (MEDIAN, MODE, etc.)
// - PostgreSQL compatibility

let examples = vec![
    ("Basic query", "select(name, age) %>% filter(age > 18)"),
    ("Median calculation", "group_by(dept) %>% summarise(median_salary = median(salary))"),
    ("Mode calculation", "group_by(category) %>% summarise(common_status = mode(status))"),
];

for (desc, query) in examples {
    println!("{}: {}", desc, transpiler.transpile(query)?);
}
```

## Performance Optimization

### 1. Transpiler Reuse

```rust
use std::sync::Arc;
use libdplyr::{Transpiler, PostgreSqlDialect};

// Create transpiler once and reuse
let transpiler = Arc::new(Transpiler::new(Box::new(PostgreSqlDialect::new())));

// Use across multiple threads
let queries = vec![
    "select(name, age)",
    "filter(age > 18)",
    "group_by(dept) %>% summarise(count = n())",
];

let handles: Vec<_> = queries.into_iter().map(|query| {
    let transpiler = Arc::clone(&transpiler);
    std::thread::spawn(move || {
        transpiler.transpile(query)
    })
}).collect();

for handle in handles {
    match handle.join().unwrap() {
        Ok(sql) => println!("SQL: {}", sql),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### 2. Memory-Efficient Processing

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

fn process_large_batch(queries: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    // Process in streaming fashion to minimize memory usage
    for (i, query) in queries.iter().enumerate() {
        let sql = transpiler.transpile(query)?;
        
        // Process immediately and release from memory
        println!("Query {}: {}", i + 1, sql);
        
        // Save results to file if needed
        // std::fs::write(format!("query_{}.sql", i + 1), sql)?;
    }
    
    Ok(())
}
```

### 3. Performance Measurement

```rust
use std::time::Instant;
use libdplyr::{Transpiler, PostgreSqlDialect};

fn benchmark_transpilation() -> Result<(), Box<dyn std::error::Error>> {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    let queries = vec![
        "select(name, age)",
        "select(name, age) %>% filter(age > 18)",
        "group_by(dept) %>% summarise(avg_salary = mean(salary))",
        // More complex queries...
    ];
    
    for query in queries {
        let start = Instant::now();
        let _sql = transpiler.transpile(query)?;
        let duration = start.elapsed();
        
        println!("Query: {} - Time: {:?}", query, duration);
    }
    
    Ok(())
}
```

## Error Handling

### 1. Detailed Error Information Usage

```rust
use libdplyr::{Transpiler, TranspileError, PostgreSqlDialect};

fn detailed_error_handling(dplyr_code: &str) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    match transpiler.transpile(dplyr_code) {
        Ok(sql) => {
            println!("Success: {}", sql);
        }
        Err(TranspileError::LexError(e)) => {
            eprintln!("Tokenization error: {}", e);
            eprintln!("Solutions:");
            eprintln!("  - Check if string quotes are properly closed");
            eprintln!("  - Verify special characters or escape sequences");
            eprintln!("  - Check pipe operator %>% format");
        }
        Err(TranspileError::ParseError(e)) => {
            eprintln!("Parse error: {}", e);
            eprintln!("Solutions:");
            eprintln!("  - Check dplyr function names (select, filter, mutate, etc.)");
            eprintln!("  - Verify function arguments and parentheses matching");
            eprintln!("  - Check pipe operator placement");
        }
        Err(TranspileError::GenerationError(e)) => {
            eprintln!("SQL generation error: {}", e);
            eprintln!("Solutions:");
            eprintln!("  - Try a different SQL dialect");
            eprintln!("  - Split into simpler expressions");
            eprintln!("  - Check supported function list");
        }
    }
}

// Usage examples
detailed_error_handling("select(name, age) %>% filter(age > 18)"); // Success
detailed_error_handling("invalid_function(test)"); // Parse error
detailed_error_handling("select(\"unterminated"); // Tokenization error
```

### 2. Error Recovery Strategy

```rust
use libdplyr::{Transpiler, TranspileError, PostgreSqlDialect, MySqlDialect, SqliteDialect};

fn try_multiple_dialects(dplyr_code: &str) -> Result<String, String> {
    let dialects: Vec<(&str, Box<dyn libdplyr::SqlDialect>)> = vec![
        ("PostgreSQL", Box::new(PostgreSqlDialect::new())),
        ("MySQL", Box::new(MySqlDialect::new())),
        ("SQLite", Box::new(SqliteDialect::new())),
    ];
    
    let mut errors = Vec::new();
    
    for (name, dialect) in dialects {
        let transpiler = Transpiler::new(dialect);
        
        match transpiler.transpile(dplyr_code) {
            Ok(sql) => {
                println!("Successful dialect: {}", name);
                return Ok(sql);
            }
            Err(e) => {
                errors.push(format!("{}: {}", name, e));
            }
        }
    }
    
    Err(format!("Failed in all dialects:\n{}", errors.join("\n")))
}
```

## CLI Usage

### 1. Basic Commands

```bash
# Direct code input
libdplyr -t "select(name, age) %>% filter(age > 18)"

# Read from file
libdplyr -i input.R -o output.sql

# Specify dialect
libdplyr -t "select(name)" -d mysql

# Pretty output
libdplyr -t "select(name) %>% filter(age > 18)" -p
```

### 2. Using Pipes

```bash
# Read from standard input
echo "select(name, age)" | libdplyr

# Pipe file contents
cat query.R | libdplyr -d postgresql -p

# Process multiple queries
find . -name "*.R" -exec cat {} \; | libdplyr -d mysql
```

### 3. Batch Processing Script

```bash
#!/bin/bash

# Convert multiple dplyr files to SQL
for file in *.R; do
    echo "Processing $file..."
    libdplyr -i "$file" -o "${file%.R}.sql" -d postgresql -p
    
    if [ $? -eq 0 ]; then
        echo "✓ Successfully converted $file"
    else
        echo "✗ Failed to convert $file"
    fi
done
```

### 4. Advanced CLI Usage

```bash
# Run with error logging
libdplyr -i complex_query.R -d mysql 2> error.log

# Save to file only on success
libdplyr -t "select(name)" && echo "Success" || echo "Failed"

# Convert to multiple dialects simultaneously
for dialect in postgresql mysql sqlite duckdb; do
    libdplyr -i query.R -o "query_${dialect}.sql" -d "$dialect" -p
done
```

## Integration Examples

### 1. Web Service Integration

```rust
use libdplyr::{Transpiler, PostgreSqlDialect, TranspileError};
use std::sync::Arc;

// Transpiler service for web applications
pub struct DplyrService {
    transpiler: Arc<Transpiler>,
}

impl DplyrService {
    pub fn new() -> Self {
        Self {
            transpiler: Arc::new(Transpiler::new(Box::new(PostgreSqlDialect::new()))),
        }
    }
    
    pub fn convert_to_sql(&self, dplyr_code: &str) -> Result<String, String> {
        self.transpiler
            .transpile(dplyr_code)
            .map_err(|e| format!("Conversion failed: {}", e))
    }
    
    pub fn validate_syntax(&self, dplyr_code: &str) -> Result<(), String> {
        self.transpiler
            .parse_dplyr(dplyr_code)
            .map(|_| ())
            .map_err(|e| format!("Syntax error: {}", e))
    }
}

// Usage example (e.g., with Actix-web)
/*
use actix_web::{web, App, HttpResponse, HttpServer, Result};

async fn convert_dplyr(
    service: web::Data<DplyrService>,
    dplyr_code: String,
) -> Result<HttpResponse> {
    match service.convert_to_sql(&dplyr_code) {
        Ok(sql) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "sql": sql
        }))),
        Err(error) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": error
        }))),
    }
}
*/
```

### 2. Database Query Builder

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

pub struct QueryBuilder {
    transpiler: Transpiler,
    base_table: String,
}

impl QueryBuilder {
    pub fn new(base_table: &str) -> Self {
        Self {
            transpiler: Transpiler::new(Box::new(PostgreSqlDialect::new())),
            base_table: base_table.to_string(),
        }
    }
    
    pub fn build_query(&self, dplyr_operations: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Construct full query including base table name
        let full_query = if dplyr_operations.contains("select") {
            dplyr_operations.to_string()
        } else {
            format!("select(*) %>% {}", dplyr_operations)
        };
        
        let mut sql = self.transpiler.transpile(&full_query)?;
        
        // Replace default table name with actual table name
        sql = sql.replace("\"data\"", &format!("\"{}\"", self.base_table));
        
        Ok(sql)
    }
    
    pub fn build_count_query(&self, dplyr_operations: &str) -> Result<String, Box<dyn std::error::Error>> {
        let count_query = format!("{} %>% summarise(total_count = n())", dplyr_operations);
        self.build_query(&count_query)
    }
}

// Usage example
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = QueryBuilder::new("employees");
    
    // Query filtered employees
    let query1 = builder.build_query("filter(salary > 50000) %>% select(name, salary)")?;
    println!("Query 1: {}", query1);
    
    // Average salary by department
    let query2 = builder.build_query("group_by(department) %>% summarise(avg_salary = mean(salary))")?;
    println!("Query 2: {}", query2);
    
    // Count employees matching criteria
    let count_query = builder.build_count_query("filter(age >= 18 & salary > 40000)")?;
    println!("Count Query: {}", count_query);
    
    Ok(())
}
```

### 3. Configuration-Based Transpiler

```rust
use libdplyr::{Transpiler, PostgreSqlDialect, MySqlDialect, SqliteDialect, DuckDbDialect, SqlDialect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TranspilerConfig {
    pub default_dialect: String,
    pub enable_pretty_print: bool,
    pub max_query_length: usize,
    pub timeout_seconds: u64,
}

impl Default for TranspilerConfig {
    fn default() -> Self {
        Self {
            default_dialect: "postgresql".to_string(),
            enable_pretty_print: false,
            max_query_length: 10000,
            timeout_seconds: 30,
        }
    }
}

pub struct ConfigurableTranspiler {
    config: TranspilerConfig,
}

impl ConfigurableTranspiler {
    pub fn new(config: TranspilerConfig) -> Self {
        Self { config }
    }
    
    pub fn from_file(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string(config_path)?;
        let config: TranspilerConfig = serde_json::from_str(&config_str)?;
        Ok(Self::new(config))
    }
    
    fn create_dialect(&self) -> Box<dyn SqlDialect> {
        match self.config.default_dialect.as_str() {
            "mysql" => Box::new(MySqlDialect::new()),
            "sqlite" => Box::new(SqliteDialect::new()),
            "duckdb" => Box::new(DuckDbDialect::new()),
            _ => Box::new(PostgreSqlDialect::new()),
        }
    }
    
    pub fn transpile(&self, dplyr_code: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Validate query length
        if dplyr_code.len() > self.config.max_query_length {
            return Err(format!("Query too long: {} > {}", 
                dplyr_code.len(), self.config.max_query_length).into());
        }
        
        let transpiler = Transpiler::new(self.create_dialect());
        let sql = transpiler.transpile(dplyr_code)?;
        
        if self.config.enable_pretty_print {
            Ok(self.format_sql(&sql))
        } else {
            Ok(sql)
        }
    }
    
    fn format_sql(&self, sql: &str) -> String {
        sql.replace(" FROM ", "\nFROM ")
           .replace(" WHERE ", "\nWHERE ")
           .replace(" GROUP BY ", "\nGROUP BY ")
           .replace(" ORDER BY ", "\nORDER BY ")
    }
}

// Usage example
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load from configuration file
    let transpiler = ConfigurableTranspiler::from_file("config.json")?;
    
    let queries = vec![
        "select(name, age)",
        "filter(age > 18)",
        "group_by(department) %>% summarise(count = n())",
    ];
    
    for query in queries {
        match transpiler.transpile(query) {
            Ok(sql) => println!("SQL: {}", sql),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}
```

## Troubleshooting

### Common Errors

#### 1. "Unexpected character" Error

```
Error: Unexpected character: '@' (position: 15)
```

**Cause:** Using unsupported special characters
**Solution:** Use only characters supported by R/dplyr

```rust
// Incorrect example
"select(@column)"

// Correct example
"select(column)"
```

#### 2. "Unterminated string literal" Error

```
Error: Unterminated string literal (start position: 10)
```

**Cause:** String quotes not properly closed
**Solution:** Check string quotes

```rust
// Incorrect example
r#"filter(name == "John)"#

// Correct example
r#"filter(name == "John")"#
```

#### 3. "Invalid pipe operator" Error

```
Error: Invalid pipe operator: '%>' (position: 12)
```

**Cause:** Incorrect pipe operator format
**Solution:** Use `%>%` format

```rust
// Incorrect example
"select(name) %> filter(age > 18)"

// Correct example
"select(name) %>% filter(age > 18)"
```

#### 4. "Unexpected token" Error

```
Error: Unexpected token: expected 'identifier' but found 'number' (position: 8)
```

**Cause:** Grammar structure error
**Solution:** Check dplyr function syntax

```rust
// Incorrect example
"select(123)"

// Correct example
"select(column_123)"
```

### Performance Issue Resolution

#### 1. High Memory Usage

```rust
// Problem: Processing large queries at once
let huge_query = "select(col1, col2, ..., col1000) %>% ...";

// Solution: Split queries into smaller units
let queries = vec![
    "select(col1, col2, col3)",
    "filter(condition1)",
    "mutate(new_col = expression)",
];
```

#### 2. Slow Processing Speed

```rust
// Problem: Creating new transpiler every time
for query in queries {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    let sql = transpiler.transpile(query)?;
}

// Solution: Reuse transpiler
let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
for query in queries {
    let sql = transpiler.transpile(query)?;
}
```

### Debugging Tips

#### 1. Check AST Structure

```rust
let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
let ast = transpiler.parse_dplyr("select(name) %>% filter(age > 18)")?;
println!("AST: {:#?}", ast);
```

#### 2. Step-by-Step Debugging

```rust
// Step 1: Check tokenization
let lexer = libdplyr::Lexer::new("select(name)".to_string());
// Check tokens...

// Step 2: Check parsing
let ast = transpiler.parse_dplyr("select(name)")?;
println!("Parsed AST: {:?}", ast);

// Step 3: Check SQL generation
let sql = transpiler.generate_sql(&ast)?;
println!("Generated SQL: {}", sql);
```

#### 3. Using Logging

```rust
use log::{info, warn, error};

fn debug_transpile(dplyr_code: &str) -> Result<String, Box<dyn std::error::Error>> {
    info!("Starting transpilation for: {}", dplyr_code);
    
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    match transpiler.transpile(dplyr_code) {
        Ok(sql) => {
            info!("Transpilation successful");
            info!("Generated SQL: {}", sql);
            Ok(sql)
        }
        Err(e) => {
            error!("Transpilation failed: {}", e);
            Err(e.into())
        }
    }
}
```

This guide will help you use libdplyr effectively. If you have additional questions or issues, please contact us through GitHub Issues.