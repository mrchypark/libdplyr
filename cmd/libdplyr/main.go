// libdplyr/cmd/libdplyr/main.go
package main

import (
	"flag"
	"fmt"
	"io/ioutil"
	"log"
	"os"

	"github.com/mrchypark/libdplyr"
	"github.com/mrchypark/libdplyr/internal/ast"
)

func main() {
	var inputFile string
	var outputFile string
	var tableName string
	var dialect string

	flag.StringVar(&inputFile, "input", "", "Input dplyr query file (default: stdin)")
	flag.StringVar(&outputFile, "output", "", "Output SQL file (default: stdout)")
	flag.StringVar(&tableName, "table", "", "Table name to use in SQL query")
	flag.StringVar(&dialect, "dialect", "duckdb", "SQL dialect (e.g., duckdb, postgres, mysql, sqlite)")

	flag.Parse()

	var inputQueryBytes []byte
	var err error

	if inputFile == "" {
		inputQueryBytes, err = ioutil.ReadAll(os.Stdin)
		if err != nil {
			log.Fatalf("Failed to read from stdin: %v", err)
		}
	} else {
		inputQueryBytes, err = ioutil.ReadFile(inputFile)
		if err != nil {
			log.Fatalf("Failed to read input file %s: %v", inputFile, err)
		}
	}

	inputQuery := string(inputQueryBytes)

	opts := &libdplyr.Options{
		Target:    ast.TargetDialect(dialect),
		TableName: tableName,
	}

	sql, err := libdplyr.Transpile(inputQuery, opts)
	if err != nil {
		log.Fatalf("Failed to transpile: %v", err)
	}

	if outputFile == "" {
		fmt.Println(sql)
	} else {
		err = ioutil.WriteFile(outputFile, []byte(sql), 0644)
		if err != nil {
			log.Fatalf("Failed to write output file %s: %v", outputFile, err)
		}
	}
}
