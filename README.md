# libdplyr

A Rust-based transpiler that converts R dplyr syntax to SQL queries.

[![Crates.io](https://img.shields.io/crates/v/libdplyr.svg)](https://crates.io/crates/libdplyr)
[![Documentation](https://docs.rs/libdplyr/badge.svg)](https://docs.rs/libdplyr)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Overview

libdplyr enables R users to write database queries using familiar dplyr syntax and converts them to efficient SQL for execution. It supports multiple SQL dialects (PostgreSQL, MySQL, SQLite, DuckDB) for use across various database environments.

## 🚀 Quick Start

```bash
# Install (Linux/macOS) - One line installation
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash

# Install (Windows PowerShell) - One line installation
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex

# Verify installation
libdplyr --version

# Try it out
echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty
```

### 🎯 Installation Examples

**Standard Installation:**
```bash
# Linux/macOS - Install to /usr/local/bin
$ curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash
libdplyr Installer v0.1.0

Detected OS: linux, Architecture: x86_64
Checking dependencies...
✓ curl found
✓ tar found
Downloading libdplyr v0.1.0 for linux-x86_64...
✓ Download complete
✓ Extraction complete
Installing to /usr/local/bin...
✓ Installation complete
Verifying installation...
✓ libdplyr is working: libdplyr 0.1.0
✓ libdplyr is in PATH
libdplyr has been successfully installed to /usr/local/bin/libdplyr
Try it out:
  echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty

Thank you for installing libdplyr!
```

**Custom Directory Installation:**
```bash
# Install to user's home directory
$ curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --dir=$HOME/bin
libdplyr Installer v0.1.0

Detected OS: macos, Architecture: aarch64
Installing to /Users/username/bin...
✓ Installation complete
Warning: libdplyr is not in PATH
You may need to add /Users/username/bin to your PATH.
For example, add this to your ~/.bashrc or ~/.zshrc:
  export PATH="/Users/username/bin:$PATH"
```

**Windows Installation:**
```powershell
# Windows PowerShell installation
PS> Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex
libdplyr Installer v0.1.0

Downloading libdplyr v0.1.0 for windows-x86_64...
✓ Download complete
✓ Extraction complete
Installing to C:\Users\username\AppData\Local\Programs\libdplyr...
✓ Installation complete
Adding libdplyr to your PATH...
✓ Added to PATH
Verifying installation...
✓ libdplyr is working: libdplyr 0.1.0
libdplyr has been successfully installed to C:\Users\username\AppData\Local\Programs\libdplyr\libdplyr.exe
Try it out:
  echo 'select(name, age) %>% filter(age > 18)' | libdplyr --pretty

Thank you for installing libdplyr!
```

## Key Features

- **dplyr Syntax Support**: Full support for `select()`, `filter()`, `mutate()`, `arrange()`, `group_by()`, `summarise()` functions
- **Pipeline Operations**: Chain operations using the `%>%` pipe operator
- **Multiple SQL Dialects**: PostgreSQL, MySQL, SQLite, DuckDB support
- **Performance Optimized**: Efficient parsing and SQL generation
- **CLI Tool**: Direct command-line usage
- **Library API**: Integration into Rust projects

## Installation

### 🚀 Quick Install (Recommended)

**Linux/macOS:**
```bash
# Install latest version
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash

# Install to custom directory
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --dir=$HOME/bin

# Install specific version
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --version=v0.1.0
```

**Windows (PowerShell):**
```powershell
# Install latest version
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex

# Install to custom directory
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex -Dir "C:\Tools"
```

**Advanced Installation (with version management):**
```bash
# Download advanced installer
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/scripts/install-advanced.sh -o install-advanced.sh
chmod +x install-advanced.sh

# List available versions
./install-advanced.sh --list-versions

# Install specific version with auto-update
./install-advanced.sh --version=v0.1.0 --auto-update

# Install to custom directory with force reinstall
./install-advanced.sh --dir=$HOME/tools --force

# Check for updates
./install-advanced.sh --list-versions | head -1  # Shows latest version

# Uninstall completely
./install-advanced.sh --uninstall

# Dry run to see what would be done
./install-advanced.sh --dry-run --version=v0.2.0
```

### 📦 Supported Platforms

| Platform | Architecture | Status | Installation Method |
|----------|-------------|--------|-------------------|
| **Linux** | x86_64 | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **Linux** | ARM64 (aarch64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **macOS** | Intel (x86_64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **macOS** | Apple Silicon (ARM64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **Windows** | x86_64 | ✅ Fully Supported | `Irm ... \| iex` |
| **Windows** | ARM64 | ✅ Fully Supported | `Irm ... \| iex` |

### 🛠 Installation Options

The enhanced installation script provides comprehensive options for different use cases:

**Basic Installation:**
- 🔍 Automatic platform detection (Linux x86_64/ARM64, macOS Intel/Apple Silicon)
- 🌐 Network connectivity verification with detailed diagnostics
- 📦 Latest version installation with retry logic
- 📁 Smart directory selection (`/usr/local/bin` → `~/.local/bin` fallback)
- 🛣️ Automatic PATH configuration with shell detection
- ✅ Comprehensive installation verification

**Enhanced Features:**
- 🎯 **Dry-run mode**: Preview installation without changes (`--dry-run`)
- 🔧 **Custom installation directory**: `--dir /custom/path`
- 📌 **Specific version installation**: `--version v1.0.0`
- 🐛 **Debug mode**: Verbose output for troubleshooting (`--debug`)
- 🔒 **Permission handling**: Automatic fallback for permission issues
- 📊 **Progress indicators**: Visual progress bars and step tracking
- 🆘 **Enhanced error handling**: Detailed error messages with solutions
- 🔍 **Installation verification**: Multi-step verification process

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
git clone https://github.com/mrchypark/libdplyr.git
cd libdplyr
cargo build --release

# The binary will be available at target/release/libdplyr
# Copy it to a directory in your PATH
cp target/release/libdplyr /usr/local/bin/  # Linux/macOS
```

### ✅ Verify Installation

After installation, verify that libdplyr is working correctly:

```bash
# Check version
libdplyr --version

# Test basic functionality
echo "select(name, age)" | libdplyr --validate-only

# Test SQL generation
echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty
```

### 🔧 Installation Troubleshooting

The enhanced installation script provides comprehensive error handling and troubleshooting:

**🔍 Built-in Diagnostics:**
- **Network connectivity check**: Automatically tests multiple endpoints
- **Platform detection**: Detailed OS and architecture detection  
- **Permission verification**: Checks write permissions before installation
- **Dependency validation**: Verifies required tools (curl, tar, etc.)
- **Installation verification**: Multi-step verification process

**🚨 Enhanced Error Handling:**

1. **Network Issues (Auto-diagnosed):**
   ```bash
   # Script provides detailed network diagnostics
   ./install.sh --debug  # Shows connectivity test results
   
   # Manual troubleshooting if needed
   nslookup github.com    # Test DNS resolution
   curl -I https://github.com  # Test HTTPS connectivity
   ```

2. **Permission Issues (Auto-resolved):**
   ```bash
   # Script automatically tries fallback directory
   ./install.sh  # Tries /usr/local/bin → ~/.local/bin
   
   # Or specify custom directory
   ./install.sh --dir $HOME/bin
   
   # Interactive PATH configuration offered
   ```

3. **Platform Support (Auto-detected):**
   ```bash
   # Check platform compatibility
   ./install.sh --dry-run  # Shows detected platform
   
   # Supported: Linux (x86_64, ARM64), macOS (Intel, Apple Silicon)
   ```

4. **PATH Configuration (Auto-configured):**
   ```bash
   # Script offers automatic PATH setup
   ./install.sh  # Detects shell and offers to configure PATH
   
   # Manual setup if needed
   echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
   source ~/.bashrc
   ```

**🛠️ Advanced Troubleshooting:**

```bash
# Preview installation without changes
./install.sh --dry-run

# Get detailed debug information  
./install.sh --debug

# Test specific version
./install.sh --dry-run --version v1.0.0

# Check system compatibility
uname -a  # OS and architecture
which curl wget tar  # Required dependencies
```

**📞 Getting Help:**
- Use `./install.sh --help` for comprehensive options
- Run `./install.sh --dry-run` to preview installation
- Enable `--debug` for detailed troubleshooting information
- The script provides specific error codes and solutions for each issue type
- Report issues with full debug output for faster resolution

**Installation Script Options:**

The enhanced installation script now supports comprehensive options:

| Option | install.sh | install.ps1 | Description |
|--------|------------|-------------|-------------|
| `--help` / `-h` | ✅ | `-Help` | Show detailed help message |
| `--version VER` / `-v VER` | ✅ | `-Version VER` | Install specific version (e.g., v1.0.0) |
| `--dir PATH` / `-d PATH` | ✅ | `-Dir PATH` | Custom installation directory |
| `--dry-run` | ✅ | `-DryRun` | Preview installation without changes |
| `--debug` | ✅ | `-Debug` | Enable verbose debug output |

**Environment Variables:**
| Variable | Description | Default |
|----------|-------------|---------|
| `LIBDPLYR_VERSION` | Version to install | `latest` |
| `INSTALL_DIR` | Installation directory | `/usr/local/bin` (Unix) |
| `DEBUG` | Enable debug mode | `false` |

**Usage Examples:**
```bash
# Basic installation
./install.sh

# Install specific version
./install.sh --version v1.0.0

# Install to custom directory
./install.sh --dir $HOME/.local/bin

# Preview installation
./install.sh --dry-run --debug

# Using environment variables
LIBDPLYR_VERSION=v1.0.0 INSTALL_DIR=$HOME/bin ./install.sh
```

**Getting Help:**
- Check the [Installation Guide](INSTALL.md) for detailed instructions
- Report issues on [GitHub Issues](https://github.com/mrchypark/libdplyr/issues)
- Use `--help` flag for command-line options
- Test installation with `--dry-run` before actual installation

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

#### 🔄 Update and Maintenance

**Check for Updates:**
```bash
# Using advanced installer (recommended)
./scripts/install-advanced.sh --list-versions

# Check current version
libdplyr --version

# Update to latest version (basic installer)
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash

# Update with advanced installer
./scripts/install-advanced.sh --force  # Reinstall latest version

# Enable automatic updates (advanced installer only)
./scripts/install-advanced.sh --auto-update

# Compare versions
echo "Current: $(libdplyr --version)"
echo "Latest: $(curl -s https://api.github.com/repos/mrchypark/libdplyr/releases/latest | grep '"tag_name"' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/')"
```

**Uninstall:**
```bash
# Using advanced installer
./scripts/install-advanced.sh --uninstall

# Manual removal
rm -f /usr/local/bin/libdplyr  # Linux/macOS
# or remove from Windows installation directory
```

**Configuration Management:**
```bash
# Advanced installer saves configuration to ~/.config/libdplyr/install.conf
cat ~/.config/libdplyr/install.conf

# Enable auto-updates
./scripts/install-advanced.sh --auto-update
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

## ❓ Frequently Asked Questions

### Installation & Setup

**Q: Which installation method should I use?**
- **Basic users**: Use the one-line installers (`curl ... | bash` or `Irm ... | iex`)
- **Power users**: Use the advanced installer for version management and auto-updates
- **Developers**: Build from source with `cargo install --path .`

**Q: Can I install multiple versions of libdplyr?**
- The basic installers replace existing installations
- Use the advanced installer with different `--dir` options for multiple versions
- Or use version managers like `cargo install` with different toolchains

**Q: How do I update libdplyr?**
```bash
# Quick update (overwrites current installation)
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash

# Advanced update with version control
./scripts/install-advanced.sh --force

# Enable automatic updates
./scripts/install-advanced.sh --auto-update
```

**Q: How do I completely remove libdplyr?**
```bash
# Using advanced installer (recommended)
./scripts/install-advanced.sh --uninstall

# Manual removal
rm -f /usr/local/bin/libdplyr  # Linux/macOS
rm -rf ~/.config/libdplyr      # Remove config files

# Windows: Remove from installation directory and PATH
```

**Q: The installer says "permission denied" - what should I do?**
```bash
# Option 1: Install to user directory
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --dir=$HOME/.local/bin

# Option 2: Use sudo (not recommended for security)
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | sudo bash

# Option 3: Download and inspect script first
curl -O https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh
chmod +x install.sh
./install.sh --dir=$HOME/bin
```

### Usage & Troubleshooting

**Q: libdplyr command not found after installation**
```bash
# Check if it's installed
ls -la /usr/local/bin/libdplyr  # Linux/macOS
where libdplyr                  # Windows

# Add to PATH if needed
export PATH="/usr/local/bin:$PATH"  # Add to ~/.bashrc or ~/.zshrc

# Or use full path
/usr/local/bin/libdplyr --version
```

**Q: How do I check what version is installed?**
```bash
libdplyr --version
```

**Q: Can I use libdplyr without installing it?**
```bash
# Download and run directly
curl -L https://github.com/mrchypark/libdplyr/releases/download/v0.1.0/libdplyr-linux-x86_64.tar.gz | tar -xz
./libdplyr --version

# Or build from source
git clone https://github.com/mrchypark/libdplyr.git
cd libdplyr
cargo run -- --help
```

**Q: Which SQL dialect should I use?**
- **PostgreSQL**: Most feature-complete, recommended for complex queries
- **MySQL**: Good compatibility, use for MySQL/MariaDB databases
- **SQLite**: Lightweight, good for simple queries and embedded use
- **DuckDB**: Excellent for analytical queries and data science workflows

**Q: How do I report bugs or request features?**
- **Bugs**: [GitHub Issues](https://github.com/mrchypark/libdplyr/issues)
- **Features**: [GitHub Discussions](https://github.com/mrchypark/libdplyr/discussions)
- **Security**: Email security@libdplyr.org (if applicable)

## Support

- **Documentation**: [docs.rs/libdplyr](https://docs.rs/libdplyr)
- **Installation Guide**: [INSTALL.md](INSTALL.md)
- **Issue Tracker**: [GitHub Issues](https://github.com/mrchypark/libdplyr/issues)
- **Discussions**: [GitHub Discussions](https://github.com/mrchypark/libdplyr/discussions)
- **Releases**: [GitHub Releases](https://github.com/mrchypark/libdplyr/releases)
- **Changelog**: [CHANGELOG.md](CHANGELOG.md)

## Related Projects

- [dplyr](https://dplyr.tidyverse.org/) - Original R package
- [polars](https://www.pola.rs/) - Rust-based dataframe library
- [datafusion](https://arrow.apache.org/datafusion/) - Rust-based query engine