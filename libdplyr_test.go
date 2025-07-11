package libdplyr

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/mrchypark/libdplyr/internal/ast"
	"github.com/mrchypark/libdplyr/internal/parser"
)

func TestTranspileSelectWithTableName(t *testing.T) {
	dplyrQuery := "select(col_a)"
	opts := &Options{
		Target:    ast.DuckDBDialect,
		TableName: "my_custom_table",
	}

	expectedSQL := "SELECT col_a FROM my_custom_table"

	actualSQL, err := Transpile(dplyrQuery, opts)
	assert.NoError(t, err)
	assert.Equal(t, expectedSQL, actualSQL)
}

func TestTranspilePipeline(t *testing.T) {
	dplyrQuery := "my_table %>% select(col_a)"
	opts := &Options{
		Target: ast.DuckDBDialect,
	}

	expectedSQL := "SELECT col_a FROM my_table"

	actualSQL, err := Transpile(dplyrQuery, opts)
	assert.NoError(t, err)
	assert.Equal(t, expectedSQL, actualSQL)
}
