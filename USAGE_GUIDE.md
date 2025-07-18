# libdplyr Usage Guide

This comprehensive guide covers all aspects of using libdplyr, from basic usage to advanced pipeline integration.

## Table of Contents

- [Quick Start](#quick-start)
- [CLI Options Reference](#cli-options-reference)
- [Input/Output Modes](#inputoutput-modes)
- [SQL Dialects](#sql-dialects)
- [Output Formats](#output-formats)
- [Pipeline Integration](#pipeline-integration)
- [Error Handling](#error-handling)
- [Performance Tips](#performance-tips)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Quick Start

### Basic Usage

```bash
# Convert dplyr code to SQL
echo "select(name, age) %>% filter(age > 18)" | libdplyr

# Use specific SQL dialect
echo "select(name)" | libdplyr --dialect mysql

# Pretty format output
echo "select(name, age)" | libdplyr --pretty
```

### Common Patterns

```bash
# Validate syntax before processing
echo "select(name)" | libdplyr --validate-only && \
echo "select(name)" | libdplyr --dialect postgresql

# Process file and save output
libdplyr -i query.R -o query.sql --pretty

# JSON output with metadata
echo "select(name)" | libdplyr --json | jq '.sql'
```

## CLI Options Reference

### Input Options

| Option | Short | Description | Example |
|--------|-------|-------------|---------|
| `--input` | `-i` | Read from file | `libdplyr -i query.R` |
| `--text` | `-t` | Direct text input | `libdplyr -t "select(name)"` |
| (stdin) | | Read from stdin (default) | `echo "select(name)" \| libdplyr` |

### Output Options

| Option | Short | Description | Example |
|--------|-------|-------------|---------|
| `--output` | `-o` | Write to file | `libdplyr -o output.sql` |
| `--pretty` | `-p` | Pretty format | `libdplyr --pretty` |
| `--compact` | `-c` | Compact format | `libdplyr --compact` |
| `--json` | `-j` | JSON format | `libdplyr --json` |

### Processing Options

| Option | Short | Description | Example |
|--------|-------|-------------|---------|
| `--dialect` | `-d` | SQL dialect | `libdplyr -d mysql` |
| `--validate-only` | | Syntax validation only | `libdplyr --validate-only` |
| `--verbose` | `-v` | Verbose output | `libdplyr --verbose` |
| `--debug` | | Debug information | `libdplyr --debug` |

### Information Options

| Option | Description | Example |
|--------|-------------|---------|
| `--help` | Show help | `libdplyr --help` |
| `--version` | Show version | `libdplyr --version` |

## Input/Output Modes

### 1. Stdin/Stdout Mode (Recommended)

**Best for:** Pipeline integration, interactive use

```bash
# Basic usage
echo "select(name, age)" | libdplyr

# With options
echo "select(name)" | libdplyr --dialect mysql --pretty

# Pipeline integration
cat query.R | libdplyr --compact | mysql -u user -p database
```

**Advantages:**
- Fast and efficient
- Works well in Unix pipelines
- No temporary files needed
- Supports all output formats

### 2. File Mode

**Best for:** Batch processing, permanent storage

```bash
# Input and output files
libdplyr -i input.R -o output.sql

# Input file, stdout output
libdplyr -i query.R --pretty

# Multiple files
for file in *.R; do
    libdplyr -i "$file" -o "${file%.R}.sql"
done
```

**Advantages:**
- Permanent file storage
- Batch processing
- Clear audit trail

### 3. Text Mode

**Best for:** Quick testing, scripting

```bash
# Direct text input
libdplyr -t "select(name, age) %>% filter(age > 18)"

# With output file
libdplyr -t "select(name)" -o query.sql --dialect sqlite
```

**Advantages:**
- No intermediate files
- Good for scripting
- Quick testing

## SQL Dialects

### Supported Dialects

| Dialect | Identifier | Description |
|---------|------------|-------------|
| PostgreSQL | `postgresql`, `postgres`, `pg` | Default dialect |
| MySQL | `mysql` | MySQL/MariaDB |
| SQLite | `sqlite` | SQLite database |
| DuckDB | `duckdb`, `duck` | DuckDB analytics |

### Dialect Examples

**PostgreSQL (Default):**
```bash
$ echo "select(name, age)" | libdplyr
SELECT "name", "age" FROM "data"

$ echo "select(name, age)" | libdplyr -d postgresql
SELECT "name", "age" FROM "data"
```

**MySQL:**
```bash
$ echo "select(name, age)" | libdplyr -d mysql
SELECT `name`, `age` FROM `data`
```

**SQLite:**
```bash
$ echo "select(name, age)" | libdplyr -d sqlite
SELECT "name", "age" FROM "data"
```

**DuckDB:**
```bash
$ echo "select(name, age)" | libdplyr -d duckdb
SELECT "name", "age" FROM "data"
```

### Dialect-Specific Features

**String Concatenation:**
```bash
# PostgreSQL
echo "mutate(full_name = first_name + last_name)" | libdplyr -d postgresql
# Result: SELECT (first_name || last_name) AS full_name FROM data

# MySQL  
echo "mutate(full_name = first_name + last_name)" | libdplyr -d mysql
# Result: SELECT CONCAT(first_name, last_name) AS full_name FROM data
```

## Output Formats

### 1. Default Format

**Best for:** General use, readable output

```bash
$ echo "select(name, age) %>% filter(age > 18)" | libdplyr
SELECT "name", "age" FROM "data" WHERE "age" > 18
```

### 2. Pretty Format

**Best for:** Human readability, documentation

```bash
$ echo "select(name, age) %>% filter(age > 18)" | libdplyr --pretty
SELECT "name", "age"
FROM "data"
WHERE "age" > 18
```

### 3. Compact Format

**Best for:** Minimal output, performance

```bash
$ echo "select(name, age) %>% filter(age > 18)" | libdplyr --compact
SELECT "name","age" FROM "data" WHERE "age">18
```

### 4. JSON Format

**Best for:** Programmatic processing, metadata

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

**JSON Error Format:**
```bash
$ echo "invalid_syntax(" | libdplyr --json
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

## Pipeline Integration

### Database Integration

**PostgreSQL:**
```bash
# Direct execution
echo "select(name, age) %>% filter(age > 18)" | \
  libdplyr -d postgresql | \
  psql -d mydb

# With error handling
if echo "select(name)" | libdplyr --validate-only; then
    echo "select(name)" | libdplyr -d postgresql | psql -d mydb
else
    echo "Invalid dplyr syntax"
fi
```

**MySQL:**
```bash
# Direct execution
echo "select(name) %>% arrange(name)" | \
  libdplyr -d mysql | \
  mysql -u user -p database

# Batch processing
cat queries.txt | while read query; do
    echo "$query" | libdplyr -d mysql | mysql -u user -p database
done
```

**SQLite:**
```bash
# Direct execution
echo "select(count = n()) %>% group_by(category)" | \
  libdplyr -d sqlite | \
  sqlite3 data.db

# File processing
libdplyr -i analysis.R -d sqlite | sqlite3 results.db
```

### Validation Workflows

```bash
# Validate before processing
validate_and_process() {
    local query="$1"
    local dialect="${2:-postgresql}"
    
    if echo "$query" | libdplyr --validate-only --json | jq -e '.success' >/dev/null; then
        echo "$query" | libdplyr -d "$dialect"
    else
        echo "Error: Invalid syntax in query: $query" >&2
        return 1
    fi
}

# Usage
validate_and_process "select(name, age)" "mysql"
```

### Batch Processing

```bash
# Process multiple files
process_dplyr_files() {
    local dialect="${1:-postgresql}"
    
    find . -name "*.R" | while read file; do
        echo "Processing: $file"
        if libdplyr -i "$file" --validate-only; then
            libdplyr -i "$file" -o "${file%.R}.sql" -d "$dialect" --pretty
            echo "✓ Converted: ${file%.R}.sql"
        else
            echo "✗ Invalid syntax in: $file"
        fi
    done
}

# Usage
process_dplyr_files mysql
```

### Parallel Processing

```bash
# Process queries in parallel
cat queries.txt | xargs -I {} -P 4 sh -c '
    echo "Processing: {}"
    echo "{}" | libdplyr --compact
'

# Parallel file processing
find . -name "*.R" | xargs -I {} -P 4 sh -c '
    libdplyr -i "{}" -o "{}.sql" --pretty
'
```

## Error Handling

### Exit Codes

| Code | Meaning | Description |
|------|---------|-------------|
| 0 | Success | Operation completed successfully |
| 1 | General Error | Unspecified error occurred |
| 2 | Invalid Arguments | Command line arguments are invalid |
| 3 | I/O Error | File or stdin/stdout operations failed |
| 4 | Validation Error | dplyr syntax validation failed |
| 5 | Transpilation Error | SQL generation failed |
| 6 | Configuration Error | Invalid configuration or settings |

### Error Handling in Scripts

```bash
#!/bin/bash

# Function with comprehensive error handling
safe_transpile() {
    local query="$1"
    local dialect="${2:-postgresql}"
    local output_file="$3"
    
    # Validate syntax first
    if ! echo "$query" | libdplyr --validate-only >/dev/null 2>&1; then
        echo "Error: Invalid dplyr syntax" >&2
        return 4
    fi
    
    # Transpile with error handling
    local result
    if [ -n "$output_file" ]; then
        result=$(echo "$query" | libdplyr -d "$dialect" -o "$output_file" 2>&1)
    else
        result=$(echo "$query" | libdplyr -d "$dialect" 2>&1)
    fi
    
    local exit_code=$?
    
    case $exit_code in
        0) echo "Success: $result" ;;
        2) echo "Error: Invalid arguments or unsupported dialect: $dialect" >&2 ;;
        3) echo "Error: I/O operation failed" >&2 ;;
        4) echo "Error: Validation failed" >&2 ;;
        5) echo "Error: SQL generation failed" >&2 ;;
        *) echo "Error: Unknown error (code: $exit_code)" >&2 ;;
    esac
    
    return $exit_code
}
```

### JSON Error Processing

```bash
# Extract error information from JSON
process_with_json_errors() {
    local query="$1"
    local result
    
    result=$(echo "$query" | libdplyr --json 2>/dev/null)
    
    if echo "$result" | jq -e '.success' >/dev/null; then
        # Success - extract SQL
        echo "$result" | jq -r '.sql'
    else
        # Error - extract error message
        local error_msg=$(echo "$result" | jq -r '.error.message // "Unknown error"')
        local suggestions=$(echo "$result" | jq -r '.error.suggestions[]? // empty')
        
        echo "Error: $error_msg" >&2
        if [ -n "$suggestions" ]; then
            echo "Suggestions:" >&2
            echo "$suggestions" | sed 's/^/  - /' >&2
        fi
        return 1
    fi
}
```

## Performance Tips

### 1. Use Appropriate Output Format

```bash
# Fastest - compact format
echo "select(name)" | libdplyr --compact

# Balanced - default format  
echo "select(name)" | libdplyr

# Slowest - pretty format (but most readable)
echo "select(name)" | libdplyr --pretty
```

### 2. Validate Before Processing

```bash
# Efficient validation workflow
if echo "$query" | libdplyr --validate-only; then
    echo "$query" | libdplyr -d postgresql
fi
```

### 3. Batch Processing Optimization

```bash
# Process in chunks for large datasets
split -l 1000 large_queries.txt chunk_
for chunk in chunk_*; do
    while read -r query; do
        echo "$query" | libdplyr --compact
    done < "$chunk"
done
```

### 4. Memory Management

```bash
# For very large files, process line by line
while IFS= read -r query; do
    echo "$query" | libdplyr --compact
done < large_file.txt
```

## Best Practices

### 1. Always Validate First

```bash
# Good practice
echo "$query" | libdplyr --validate-only && \
echo "$query" | libdplyr -d postgresql

# Even better with error handling
if echo "$query" | libdplyr --validate-only; then
    echo "$query" | libdplyr -d postgresql | psql -d mydb
else
    echo "Invalid syntax, skipping query"
fi
```

### 2. Use Appropriate Dialect

```bash
# Match your target database
echo "select(name)" | libdplyr -d mysql      # for MySQL
echo "select(name)" | libdplyr -d sqlite     # for SQLite
echo "select(name)" | libdplyr -d postgresql # for PostgreSQL
```

### 3. Handle Errors Gracefully

```bash
# Robust error handling
process_query() {
    local query="$1"
    local dialect="$2"
    
    local result
    result=$(echo "$query" | libdplyr -d "$dialect" --json 2>/dev/null)
    
    if echo "$result" | jq -e '.success' >/dev/null; then
        echo "$result" | jq -r '.sql'
        return 0
    else
        echo "Failed to process query: $query" >&2
        echo "$result" | jq -r '.error.message' >&2
        return 1
    fi
}
```

### 4. Use JSON for Programmatic Processing

```bash
# Extract metadata for monitoring
echo "select(name)" | libdplyr --json | jq '{
    success: .success,
    processing_time: .metadata.stats.total_time_us,
    dialect: .metadata.dialect,
    input_size: .metadata.stats.input_size_bytes
}'
```

### 5. Optimize for Your Use Case

**Interactive Use:**
```bash
# Pretty format for readability
echo "select(name, age)" | libdplyr --pretty
```

**Pipeline Integration:**
```bash
# Compact format for efficiency
echo "select(name)" | libdplyr --compact | mysql -u user -p db
```

**Debugging:**
```bash
# Verbose output for troubleshooting
echo "select(name)" | libdplyr --verbose --debug 2>debug.log
```

## Troubleshooting

### Common Issues

**1. Empty Output**
```bash
# Problem
echo "" | libdplyr
# Solution: Provide valid dplyr code
echo "select(name)" | libdplyr
```

**2. Syntax Errors**
```bash
# Problem
echo "select(name" | libdplyr
# Solution: Check parentheses
echo "select(name)" | libdplyr
```

**3. Unsupported Dialect**
```bash
# Problem
echo "select(name)" | libdplyr -d unsupported
# Solution: Use supported dialect
echo "select(name)" | libdplyr -d postgresql
```

**4. File Not Found**
```bash
# Problem
libdplyr -i nonexistent.R
# Solution: Check file path
libdplyr -i existing_file.R
```

### Debug Mode

```bash
# Enable debug output
echo "select(name, age)" | libdplyr --debug --verbose 2>debug.log

# Check debug log
cat debug.log
```

### Performance Issues

```bash
# Monitor processing time
time echo "complex_query" | libdplyr

# Use JSON to get detailed timing
echo "select(name)" | libdplyr --json | jq '.metadata.stats'
```

### Getting Help

```bash
# Show help
libdplyr --help

# Show version
libdplyr --version

# Validate syntax
echo "your_query" | libdplyr --validate-only
```

---

For more information, see the [README](README.md) or visit the [documentation](https://docs.rs/libdplyr).