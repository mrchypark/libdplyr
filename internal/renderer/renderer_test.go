package renderer

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/mrchypark/libdplyr/internal/ast"
)

func TestRenderSelectStatement(t *testing.T) {
	// Test case 1: Single column
	selectStmt1 := &ast.SelectStmt{
		Columns: []ast.Expr{
			&ast.Identifier{Name: "col_a"},
		},
	}
	expectedSQL1 := "SELECT col_a FROM test_table"
	actualSQL1, err1 := Render(selectStmt1, ast.DuckDBDialect, "test_table")
	assert.NoError(t, err1)
	assert.Equal(t, expectedSQL1, actualSQL1)

	// Test case 2: Multiple columns
	selectStmt2 := &ast.SelectStmt{
		Columns: []ast.Expr{
			&ast.Identifier{Name: "col_x"},
			&ast.Identifier{Name: "col_y"},
			&ast.Identifier{Name: "col_z"},
		},
	}
	expectedSQL2 := "SELECT col_x, col_y, col_z FROM another_table"
	actualSQL2, err2 := Render(selectStmt2, ast.DuckDBDialect, "another_table")
	assert.NoError(t, err2)
	assert.Equal(t, expectedSQL2, actualSQL2)
}
