# libdplyr

A Rust-based transpiler that converts R dplyr syntax to SQL queries.

[![Crates.io](https://img.shields.io/crates/v/libdplyr.svg)](https://crates.io/crates/libdplyr)
[![Documentation](https://docs.rs/libdplyr/badge.svg)](https://docs.rs/libdplyr)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Overview

libdplyr enables R users to write database queries using familiar dplyr syntax and converts them to efficient SQL for execution. It supports multiple SQL dialects (PostgreSQL, MySQL, SQLite, DuckDB) for use across various database environments.

## Key Features

- **dplyr Syntax Support**: Full support for `select()`, `filter()`, `mutate()`, `arrange()`, `group_by()`, `summarise()` functions
- **Pipeline Operations**: Chain operations using the `%>%` pipe operator
- **Multiple SQL Dialects**: PostgreSQL, MySQL, SQLite, DuckDB support
- **Performance Optimized**: Efficient parsing and SQL generation
- **CLI Tool**: Direct command-line usage
- **Library API**: Integration into Rust projects

## Installation

### 🚀 Quick Install (Recommended)

Install the latest version automatically:

```bash
curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh
```

Install a specific version:

```bash
LIBDPLYR_VERSION=v1.0.0 curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh
```

### 📦 Supported Platforms

- **Linux**: x86_64, ARM64 (aarch64)
- **macOS**: Intel (x86_64), Apple Silicon (ARM64)  
- **Windows**: x86_64

### 🔧 Manual Installation

1. Download the binary for your platform from [Releases](https://github.com/libdplyr/libdplyr/releases)
2. Move the binary to a directory in your PATH
3. Make it executable: `chmod +x libdplyr`

### 📚 Install via Cargo

```bash
# Use as library
cargo add libdplyr

# Install CLI tool from source
cargo install libdplyr
```

### 🛠 Build from Source

```bash
git clone https://github.com/libdplyr/libdplyr.git
cd libdplyr
cargo build --release
```

## Usage

### As a Library

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create transpiler using PostgreSQL dialect
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    
    // Convert dplyr code to SQL
    let dplyr_code = r#"
        select(name, age, salary) %>%
        filter(age >= 18) %>%
        arrange(desc(salary))
    "#;
    
    let sql = transpiler.transpile(dplyr_code)?;
    println!("{}", sql);
    
    Ok(())
}
```

### As a CLI Tool

#### 🔄 stdin/stdout Pipeline (Recommended)

The most efficient way to use libdplyr is through stdin/stdout pipelines:

```bash
# Basic usage - read from stdin, output to stdout
echo "select(name, age) %>% filter(age > 18)" | libdplyr

# Specify SQL dialect (postgresql, mysql, sqlite, duckdb)
echo "select(name)" | libdplyr --dialect mysql
echo "select(name)" | libdplyr -d postgresql

# Output formatting options
echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty
echo "select(name, age)" | libdplyr --compact
echo "select(name)" | libdplyr --json

# Validation mode - check syntax without generating SQL
echo "select(name, age)" | libdplyr --validate-only

# Verbose and debug modes for troubleshooting
echo "select(name)" | libdplyr --verbose
echo "select(name)" | libdplyr --debug
echo "select(name)" | libdplyr --verbose --debug

# Combine options
echo "select(name)" | libdplyr --json --verbose --dialect mysql
```

#### 📊 Output Format Examples

**Default Format:**
```bash
$ echo "select(name, age) %>% filter(age > 18)" | libdplyr
SELECT "name", "age" FROM "data" WHERE "age" > 18
```

**Pretty Format:**
```bash
$ echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty
SELECT "name", "age"
FROM "data"
WHERE "age" > 18
```

**Compact Format:**
```bash
$ echo "select(name, age) %>% filter(age > 18)" | libdplyr --compact
SELECT "name","age" FROM "data" WHERE "age">18
```

**JSON Format:**
```bash
$ echo "select(name)" | libdplyr --json
{
  "success": true,
  "sql": "SELECT \"name\" FROM \"data\"",
  "metadata": {
    "dialect": "postgresql",
    "timestamp": 1640995200,
    "stats": {
      "total_time_us": 1250,
      "input_size_bytes": 13,
      "output_size_bytes": 25
    },
    "version": "0.1.0"
  }
}
```

**Validation Only:**
```bash
$ echo "select(name, age)" | libdplyr --validate-only
Valid dplyr syntax

$ echo "invalid_syntax(" | libdplyr --validate-only
Validation failed: Unexpected token: expected 'expression' but found EOF
```

#### 📁 File-based Operations

```bash
# Read from file and convert
libdplyr -i query.R -o query.sql -d postgresql

# Read from file, output to stdout
libdplyr -i query.R --pretty

# Direct text input
libdplyr -t "select(name, age) %>% filter(age > 18)" -d mysql
```

#### 🔗 Pipeline Integration & Best Practices

**Database Integration:**
```bash
# PostgreSQL integration
echo "select(name, age) %>% filter(age > 18)" | libdplyr -d postgresql | psql -d mydb

# MySQL integration  
echo "select(name) %>% arrange(name)" | libdplyr -d mysql | mysql -u user -p database

# SQLite integration
echo "select(count = n()) %>% group_by(category)" | libdplyr -d sqlite | sqlite3 data.db

# DuckDB integration
echo "select(name, value) %>% filter(value > 100)" | libdplyr -d duckdb | duckdb mydata.db
```

**Validation and Error Handling:**
```bash
# Validate before processing
if echo "select(name, age)" | libdplyr --validate-only --json | jq -r '.success'; then
    echo "select(name, age)" | libdplyr -d postgresql | psql -d mydb
else
    echo "Invalid dplyr syntax"
fi

# Handle errors gracefully
echo "invalid_syntax(" | libdplyr --json 2>/dev/null | jq -r '.error.message // "No error"'

# Check exit codes
echo "select(name)" | libdplyr --validate-only
echo "Exit code: $?"  # 0 for success, 4 for validation error
```

**Batch Processing:**
```bash
# Process multiple files
find . -name "*.R" -exec sh -c '
    echo "Processing: $1"
    if libdplyr -i "$1" --validate-only; then
        libdplyr -i "$1" -o "${1%.R}.sql" -d postgresql --pretty
        echo "✓ Converted: ${1%.R}.sql"
    else
        echo "✗ Invalid syntax in: $1"
    fi
' _ {} \;

# Process queries from a file
while IFS= read -r query; do
    echo "Query: $query"
    echo "$query" | libdplyr --json | jq -r '.sql // .error.message'
    echo "---"
done < queries.txt

# Parallel processing with xargs
cat queries.txt | xargs -I {} -P 4 sh -c 'echo "{}" | libdplyr --compact'
```

**Performance Optimization:**
```bash
# Use compact format for minimal output
echo "select(name)" | libdplyr --compact

# Validate syntax first for large batches
echo "select(name, age)" | libdplyr --validate-only && \
echo "select(name, age)" | libdplyr -d postgresql

# Use appropriate dialect for your database
echo "select(name)" | libdplyr -d mysql    # for MySQL
echo "select(name)" | libdplyr -d sqlite   # for SQLite
```

**Monitoring and Logging:**
```bash
# Verbose mode for debugging
echo "select(name)" | libdplyr --verbose --debug 2>debug.log

# JSON output for structured logging
echo "select(name)" | libdplyr --json | jq '{
    success: .success,
    processing_time: .metadata.stats.total_time_us,
    dialect: .metadata.dialect
}'

# Performance monitoring
time echo "select(name, age) %>% filter(age > 18)" | libdplyr --compact
```

#### 🆘 Help and Options

```bash
# Show help
libdplyr --help

# Show version
libdplyr --version

# List supported dialects
libdplyr --help | grep -A 10 "dialect"
```

## Supported dplyr Functions

### Core Functions

- `select(col1, col2, ...)` - Column selection
- `filter(condition)` - Row filtering
- `mutate(new_col = expression)` - Create/modify columns
- `arrange(col1, desc(col2))` - Sorting
- `group_by(col1, col2)` - Grouping
- `summarise(stat = function(col))` - Aggregation

### Aggregate Functions

- `mean()` / `avg()` - Average
- `sum()` - Sum
- `count()` / `n()` - Count
- `min()` / `max()` - Minimum/Maximum

## SQL Dialect Examples

### PostgreSQL

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
let dplyr_code = "select(name, age) %>% filter(age > 18)";
let sql = transpiler.transpile(dplyr_code)?;

// Result:
// SELECT "name", "age" 
// FROM data 
// WHERE "age" > 18
```

### MySQL

```rust
use libdplyr::{Transpiler, MySqlDialect};

let transpiler = Transpiler::new(Box::new(MySqlDialect::new()));
let dplyr_code = "select(name, age) %>% filter(age > 18)";
let sql = transpiler.transpile(dplyr_code)?;

// Result:
// SELECT `name`, `age` 
// FROM data 
// WHERE `age` > 18
```

### SQLite

```rust
use libdplyr::{Transpiler, SqliteDialect};

let transpiler = Transpiler::new(Box::new(SqliteDialect::new()));
let dplyr_code = "group_by(department) %>% summarise(avg_salary = mean(salary))";
let sql = transpiler.transpile(dplyr_code)?;

// Result:
// SELECT "department", AVG("salary") AS "avg_salary"
// FROM data
// GROUP BY "department"
```

### DuckDB

```rust
use libdplyr::{Transpiler, DuckDbDialect};

let transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));
let dplyr_code = "select(name, salary) %>% mutate(bonus = salary * 0.1)";
let sql = transpiler.transpile(dplyr_code)?;

// Result:
// SELECT "name", "salary", ("salary" * 0.1) AS "bonus"
// FROM data
```

## Complex Examples

### Multi-Operation Pipeline

```rust
let dplyr_code = r#"
    select(employee_id, name, department, salary, hire_date) %>%
    filter(salary >= 50000 & department == "Engineering") %>%
    mutate(
        annual_bonus = salary * 0.15,
        years_employed = 2024 - year(hire_date)
    ) %>%
    arrange(desc(salary), name) %>%
    group_by(department) %>%
    summarise(
        total_employees = n(),
        avg_salary = mean(salary),
        max_bonus = max(annual_bonus)
    )
"#;

let sql = transpiler.transpile(dplyr_code)?;
```

### Conditional Filtering

```rust
let dplyr_code = r#"
    select(product_name, category, price, stock_quantity) %>%
    filter(
        (category == "Electronics" & price > 100) |
        (category == "Books" & stock_quantity > 50)
    ) %>%
    arrange(category, desc(price))
"#;

let sql = transpiler.transpile(dplyr_code)?;
```

## Error Handling & Troubleshooting

### 🔍 Common Error Types

libdplyr provides detailed error information with specific exit codes:

| Exit Code | Error Type | Description |
|-----------|------------|-------------|
| 0 | Success | Operation completed successfully |
| 1 | General Error | Unspecified error occurred |
| 2 | Invalid Arguments | Command line arguments are invalid |
| 3 | I/O Error | File or stdin/stdout operations failed |
| 4 | Validation Error | dplyr syntax validation failed |
| 5 | Transpilation Error | SQL generation failed |
| 6 | Configuration Error | Invalid configuration or settings |

### 🛠 Troubleshooting Guide

**Syntax Errors:**
```bash
# Check syntax before processing
$ echo "invalid_function(" | libdplyr --validate-only
Validation failed: Unexpected token: expected 'expression' but found EOF

# Get detailed error information with JSON
$ echo "invalid_function(" | libdplyr --json
{
  "success": false,
  "error": {
    "error_type": "parse",
    "message": "Unexpected token: expected 'expression' but found EOF",
    "suggestions": [
      "Check dplyr function syntax and arguments",
      "Ensure proper use of pipe operator (%>%)",
      "Verify function names are spelled correctly"
    ]
  }
}
```

**Common Syntax Issues:**
```bash
# Missing closing parenthesis
$ echo "select(name" | libdplyr --validate-only
# Fix: echo "select(name)" | libdplyr --validate-only

# Invalid function name
$ echo "invalid_func(name)" | libdplyr --validate-only  
# Fix: echo "select(name)" | libdplyr --validate-only

# Incomplete pipe operation
$ echo "select(name) %>%" | libdplyr --validate-only
# Fix: echo "select(name) %>% filter(age > 18)" | libdplyr --validate-only

# Missing quotes for string literals
$ echo "filter(name == John)" | libdplyr --validate-only
# Fix: echo "filter(name == \"John\")" | libdplyr --validate-only
```

**File I/O Issues:**
```bash
# File not found
$ libdplyr -i nonexistent.R
# Fix: Check file path and permissions

# Permission denied
$ libdplyr -i protected.R -o /root/output.sql
# Fix: Use accessible output directory or run with proper permissions

# Empty input
$ echo "" | libdplyr
# Fix: Provide valid dplyr code
```

**Dialect-Specific Issues:**
```bash
# Unsupported dialect
$ echo "select(name)" | libdplyr -d unsupported_db
# Fix: Use supported dialects: postgresql, mysql, sqlite, duckdb

# Dialect-specific function not supported
$ echo "select(name) %>% mutate(id = uuid())" | libdplyr -d sqlite
# Fix: Check dialect documentation for supported functions
```

### 🐛 Debug Mode

Use debug mode for detailed troubleshooting:

```bash
# Enable verbose and debug output
$ echo "select(name, age)" | libdplyr --verbose --debug 2>debug.log

# Debug output includes:
# - Tokenization steps
# - AST structure
# - SQL generation process
# - Performance metrics
```

### 📊 Error Handling in Code

**Rust Library:**
```rust
use libdplyr::{Transpiler, TranspileError, PostgreSqlDialect};

let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
let result = transpiler.transpile("invalid_function(test)");

match result {
    Ok(sql) => println!("SQL: {}", sql),
    Err(TranspileError::LexError(e)) => {
        eprintln!("Tokenization error: {}", e);
        // Handle lexical errors (invalid characters, malformed strings)
    }
    Err(TranspileError::ParseError(e)) => {
        eprintln!("Parsing error: {}", e);
        // Handle syntax errors (invalid dplyr functions, wrong arguments)
    }
    Err(TranspileError::GenerationError(e)) => {
        eprintln!("SQL generation error: {}", e);
        // Handle unsupported operations for specific SQL dialects
    }
    Err(TranspileError::IoError(e)) => {
        eprintln!("I/O error: {}", e);
        // Handle file read/write errors
    }
    Err(TranspileError::ValidationError(e)) => {
        eprintln!("Validation error: {}", e);
        // Handle validation-specific errors
    }
    Err(TranspileError::ConfigurationError(e)) => {
        eprintln!("Configuration error: {}", e);
        // Handle configuration issues
    }
}
```

**Shell Scripts:**
```bash
#!/bin/bash

# Function to handle libdplyr errors
process_dplyr() {
    local query="$1"
    local dialect="${2:-postgresql}"
    
    # Validate first
    if ! echo "$query" | libdplyr --validate-only >/dev/null 2>&1; then
        echo "Error: Invalid dplyr syntax in query: $query" >&2
        return 4
    fi
    
    # Process with error handling
    local result
    result=$(echo "$query" | libdplyr -d "$dialect" 2>&1)
    local exit_code=$?
    
    case $exit_code in
        0) echo "$result" ;;
        2) echo "Error: Invalid arguments or unsupported dialect: $dialect" >&2 ;;
        3) echo "Error: I/O operation failed" >&2 ;;
        4) echo "Error: Validation failed for query: $query" >&2 ;;
        5) echo "Error: SQL generation failed" >&2 ;;
        *) echo "Error: Unknown error (code: $exit_code)" >&2 ;;
    esac
    
    return $exit_code
}

# Usage example
process_dplyr "select(name, age) %>% filter(age > 18)" "postgresql"
```

### 🔧 Performance Issues

**Slow Processing:**
```bash
# Use compact format for faster processing
echo "select(name)" | libdplyr --compact

# Validate syntax first for large batches
echo "select(name)" | libdplyr --validate-only && \
echo "select(name)" | libdplyr

# Monitor performance
time echo "complex_query" | libdplyr --json | jq '.metadata.stats.total_time_us'
```

**Memory Issues:**
```bash
# Process large files in chunks
split -l 1000 large_queries.txt chunk_
for chunk in chunk_*; do
    while read -r query; do
        echo "$query" | libdplyr --compact
    done < "$chunk"
done
```

## Performance

libdplyr is optimized for high performance:

- **Fast Parsing**: Efficient tokenization and AST generation
- **Memory Efficiency**: Minimal memory allocation
- **Benchmarking**: Performance measurement with `cargo bench`

```bash
# Run benchmarks
cargo bench

# Performance profiling
cargo bench --bench transpile_benchmark
```

## Development and Contributing

### Development Setup

```bash
# Clone repository
git clone https://github.com/your-repo/libdplyr.git
cd libdplyr

# Install dependencies and build
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Testing

```bash
# Run all tests
cargo test

# Test specific module
cargo test --lib lexer

# Integration tests
cargo test --test integration_tests

# Documentation tests
cargo test --doc
```

### Code Quality

```bash
# Code formatting
cargo fmt

# Linting
cargo clippy

# Generate documentation
cargo doc --open
```

## License

This project is distributed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! You can contribute in the following ways:

1. Issue reports
2. Feature suggestions
3. Pull requests
4. Documentation improvements

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed contribution guidelines.

## Support

- **Documentation**: [docs.rs/libdplyr](https://docs.rs/libdplyr)
- **Issue Tracker**: [GitHub Issues](https://github.com/your-repo/libdplyr/issues)
- **Discussions**: [GitHub Discussions](https://github.com/your-repo/libdplyr/discussions)

## Related Projects

- [dplyr](https://dplyr.tidyverse.org/) - Original R package
- [polars](https://www.pola.rs/) - Rust-based dataframe library
- [datafusion](https://arrow.apache.org/datafusion/) - Rust-based query engine