package libdplyr

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/mrchypark/libdplyr/internal/ast"
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

func TestTranspileArrange(t *testing.T) {
	tests := []struct {
		name        string
		dplyrQuery  string
		expectedSQL string
	}{
		{
			name:        "single column",
			dplyrQuery:  "my_table %>% arrange(col_a)",
			expectedSQL: "SELECT * FROM my_table ORDER BY col_a",
		},
		{
			name:        "multiple columns",
			dplyrQuery:  "my_table %>% arrange(col_a, col_b)",
			expectedSQL: "SELECT * FROM my_table ORDER BY col_a, col_b",
		},
		{
			name:        "descending column",
			dplyrQuery:  "my_table %>% arrange(desc(col_a))",
			expectedSQL: "SELECT * FROM my_table ORDER BY col_a DESC",
		},
		{
			name:        "multiple columns with descending",
			dplyrQuery:  "my_table %>% arrange(col_a, desc(col_b))",
			expectedSQL: "SELECT * FROM my_table ORDER BY col_a, col_b DESC",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			opts := &Options{
				Target: ast.DuckDBDialect,
			}
			actualSQL, err := Transpile(tt.dplyrQuery, opts)
			assert.NoError(t, err)
			assert.Equal(t, tt.expectedSQL, actualSQL)
		})
	}
}

func TestTranspileErrorHandling(t *testing.T) {
	dplyrQuery := "invalid_table %>% select(col_a)"
	opts := &Options{
		Target: ast.DuckDBDialect,
	}

	_, err := Transpile(dplyrQuery, opts)
	assert.Error(t, err)
	assert.Contains(t, err.Error(), "parsing error")
}
