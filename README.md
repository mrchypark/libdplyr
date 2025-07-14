# libdplyr

## Project Vision
`libdplyr` is a Go library designed to transpile dplyr-like syntax into SQL queries. Our goal is to provide a flexible and extensible tool that allows data analysts and engineers to write data manipulation logic in a familiar dplyr style, which then gets converted into optimized SQL for various database systems. This project aims to bridge the gap between R's powerful dplyr package and the SQL world, enabling more efficient and readable data workflows.

## Installation
To install `libdplyr`, use `go get`:

```bash
go get github.com/mrchypark/libdplyr
```

## Usage

### Transpiling dplyr queries to SQL

You can use the `Transpile` function in your Go applications:

```go
package main

import (
	"fmt"
	"log"

	"github.com/mrchypark/libdplyr"
	"github.com/mrchypark/libdplyr/internal/ast"
)

func main() {
	// Example: Select columns
	dplyrQuery := "my_table %>% select(col_a, col_b)"
	opts := &libdplyr.Options{
		Target: ast.DuckDBDialect,
	}
	sql, err := libdplyr.Transpile(dplyrQuery, opts)
	if err != nil {
		log.Fatalf("Failed to transpile: %v", err)
	}
	fmt.Printf("Dplyr: %s\nSQL: %s\n\n", dplyrQuery, sql)

	// Example: Filter rows
	dplyrQuery = "my_table %>% filter(price > 100)"
	sql, err = libdplyr.Transpile(dplyrQuery, opts)
	if err != nil {
		log.Fatalf("Failed to transpile: %v", err)
	}
	fmt.Printf("Dplyr: %s\nSQL: %s\n\n", dplyrQuery, sql)

	// Example: Arrange (order by)
	dplyrQuery = "my_table %>% arrange(col_a, desc(col_b))"
	sql, err = libdplyr.Transpile(dplyrQuery, opts)
	if err != nil {
		log.Fatalf("Failed to transpile: %v", err)
	}
	fmt.Printf("Dplyr: %s\nSQL: %s\n\n", dplyrQuery, sql)

	// Example: Group by and Summarise
	dplyrQuery = "my_table %>% group_by(category) %>% summarise(avg_price = mean(price))"
	sql, err = libdplyr.Transpile(dplyrQuery, opts)
	if err != nil {
		log.Fatalf("Failed to transpile: %v", err)
	}
	fmt.Printf("Dplyr: %s\nSQL: %s\n\n", dplyrQuery, sql)
}
```

### Supported dplyr Functions
Currently, `libdplyr` supports the following dplyr verbs and their corresponding SQL translations:

- `select(...)`: Translates to `SELECT ... FROM ...`
- `filter(...)`: Translates to `WHERE ...` clauses with support for binary operators (`>`, `<`, `==`, `!=`, `>=`, `<=`) and string/numeric literals.
- `arrange(...)`: Translates to `ORDER BY ...` with support for `desc()` for descending order.
- `group_by(...)`: Translates to `GROUP BY ...`
- `summarise(...)`: Translates to aggregate functions (e.g., `mean()`, `sum()`) with aliasing.

### CLI Usage
You can also use the `libdplyr` command-line tool:

```bash
# Transpile from stdin to stdout
echo "my_table %>% select(col_a)" | libdplyr

# Transpile from a file to stdout
libdplyr --input examples/simple_select.dplyr --table my_data

# Transpile from a file to an output file
libdplyr --input examples/simple_select.dplyr --table my_data --output output.sql

# Specify SQL dialect (e.g., postgres, mysql, sqlite, duckdb)
libdplyr --input examples/simple_select.dplyr --table my_data --dialect postgres
```
