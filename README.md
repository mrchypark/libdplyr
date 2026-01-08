# libdplyr

A Rust-based transpiler that converts R dplyr syntax to SQL queries.

[![Crates.io](https://img.shields.io/crates/v/libdplyr.svg)](https://crates.io/crates/libdplyr)
[![Documentation](https://docs.rs/libdplyr/badge.svg)](https://docs.rs/libdplyr)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/mrchypark/libdplyr/workflows/CI/badge.svg)](https://github.com/mrchypark/libdplyr/actions)
[![codecov](https://codecov.io/gh/mrchypark/libdplyr/branch/main/graph/badge.svg)](https://codecov.io/gh/mrchypark/libdplyr)

## Overview

libdplyr enables R users to write database queries using familiar dplyr syntax and converts them to efficient SQL for execution. It supports multiple SQL dialects (PostgreSQL, MySQL, SQLite, DuckDB) for use across various database environments.

## ✨ Key Features

- **dplyr Syntax Support**: Full support for `select()`, `filter()`, `mutate()`, `arrange()`, `group_by()`, `summarise()`
- **Pipeline Operations**: Chain operations using the `%>%` pipe operator
- **Multiple Dialects**: PostgreSQL, MySQL, SQLite, DuckDB
- **Performance**: High-performance Rust implementation
- **Dual Mode**: Use as a Rust library or standalone CLI tool

## 🚀 Quick Start

**Linux/macOS:**
```bash
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex
```

**Try it out:**
```bash
echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty
```

> **Note:** For detailed installation options, troubleshooting, and platform support, see the [Installation Guide](INSTALL.md).

## Usage

### As a CLI Tool

The most efficient way to use libdplyr is through stdin/stdout pipelines:

```bash
# Basic usage
echo "select(name, age) %>% filter(age > 18)" | libdplyr

# Specify dialect (postgres, mysql, sqlite, duckdb)
echo "select(name)" | libdplyr --dialect mysql

# Output formatting
echo "select(name)" | libdplyr --pretty
echo "select(name)" | libdplyr --json
echo "select(name)" | libdplyr --compact
```

### As a Rust Library

```rust
use libdplyr::{Transpiler, PostgreSqlDialect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
    let dplyr_code = "select(name, age) %>% filter(age > 18)";
    
    let sql = transpiler.transpile(dplyr_code)?;
    println!("{}", sql);
    Ok(())
}
```

## 📋 Supported Functions

libdplyr supports a wide range of dplyr verbs and R functions.

### Core Verbs
| Function | Description | Example |
| :--- | :--- | :--- |
| `select()` | Select/rename columns | `select(id, name)` |
| `filter()` | Filter rows | `filter(age > 18)` |
| `mutate()` | Create/modify columns | `mutate(total = price * qty)` |
| `rename()` | Rename columns | `rename(new = old)` |
| `arrange()` | Sort rows | `arrange(desc(date))` |
| `group_by()` | Group rows | `group_by(dept)` |
| `summarise()` | Aggregate data | `summarise(avg = mean(val))` |
| `*_join()` | Joins (inner, left, etc.) | `left_join(other, by="id")` |
| Set Ops | union, intersect, setdiff | `union(other)` |

### Helper Functions
*   **Aggregation**: `mean`, `sum`, `min`, `max`, `n`, `count`, `median`*, `mode`*
*   **Window**: `row_number`, `rank`, `lead`, `lag`, `ntile`
*   **Math**: `abs`, `sqrt`, `round`, `floor`, `log`, `exp`
*   **String**: `tolower`, `toupper`, `substr`, `trimws`
*   **Logic**: `ifelse`, `is.na`, `coalesce`

## Examples

### PostgreSQL
```rust
let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
let sql = transpiler.transpile("select(name) %>% filter(age > 18)")?;
// SELECT "name" FROM "data" WHERE "age" > 18
```

### Complex Pipeline
```rust
let code = r#"
    select(dept, salary) %>%
    filter(salary > 50000) %>%
    group_by(dept) %>%
    summarise(avg_sal = mean(salary)) %>%
    arrange(desc(avg_sal))
"#;
```

## Error Handling & Troubleshooting

libdplyr provides detailed error codes:
*   `Exit 0`: Success
*   `Exit 4`: Validation Error (syntax issues)
*   `Exit 5`: Transpilation Error (generation failed)

**Debug Mode:**
```bash
libdplyr --verbose --debug
```

For more troubleshooting details, see [INSTALL.md](INSTALL.md).

## Performance

libdplyr is optimized for speed using Rust's zero-cost abstractions.
Run benchmarks:
```bash
cargo bench
```

## Development

```bash
# Setup
git clone https://github.com/mrchypark/libdplyr.git
cd libdplyr
cargo build

# Test
cargo test
```

## Support

- **Documentation**: [docs.rs/libdplyr](https://docs.rs/libdplyr)
- **Installation**: [INSTALL.md](INSTALL.md)
- **Issues**: [GitHub Issues](https://github.com/mrchypark/libdplyr/issues)

## License

MIT License. See [LICENSE](LICENSE).
