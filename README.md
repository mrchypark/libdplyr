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

```bash
# Basic usage - read from stdin, output to stdout
echo "select(name, age) %>% filter(age > 18)" | libdplyr

# Specify SQL dialect
echo "select(name)" | libdplyr -d mysql

# Pretty format output
echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty

# JSON format output with metadata
echo "select(name)" | libdplyr --json

# Compact format (single line)
echo "select(name, age)" | libdplyr --compact

# Validate syntax only (no SQL generation)
echo "select(name, age)" | libdplyr --validate-only

# Verbose output with debug information
echo "select(name)" | libdplyr --verbose --debug
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

#### 🔗 Pipeline Integration

```bash
# Chain with other tools
cat analysis.R | libdplyr -d postgresql | psql -d mydb

# Conditional processing
if echo "select(invalid)" | libdplyr --validate-only; then
    echo "Valid syntax"
else
    echo "Syntax error"
fi

# Batch processing
find . -name "*.R" -exec sh -c 'libdplyr -i "$1" -o "${1%.R}.sql"' _ {} \;

# Process multiple queries
cat queries.txt | while read query; do
    echo "$query" | libdplyr --json
done
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

## Error Handling

libdplyr provides detailed error information:

```rust
use libdplyr::{Transpiler, TranspileError, PostgreSqlDialect};

let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
let result = transpiler.transpile("invalid_function(test)");

match result {
    Ok(sql) => println!("SQL: {}", sql),
    Err(TranspileError::LexError(e)) => {
        eprintln!("Tokenization error: {}", e);
    }
    Err(TranspileError::ParseError(e)) => {
        eprintln!("Parsing error: {}", e);
    }
    Err(TranspileError::GenerationError(e)) => {
        eprintln!("SQL generation error: {}", e);
    }
}
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