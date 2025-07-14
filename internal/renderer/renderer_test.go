package renderer

import (
	"testing"

	"github.com/mrchypark/libdplyr/internal/ast"
	"github.com/stretchr/testify/assert"
)

func TestRenderSelectStatement(t *testing.T) {
	renderer, err := NewRenderer(ast.DuckDBDialect)
	assert.NoError(t, err)

	// Test case 1: Single column
	selectStmt1 := &ast.SelectStmt{
		Columns: []ast.Expr{
			&ast.Identifier{Name: "col_a"},
		},
	}
	expectedSQL1 := "SELECT col_a FROM test_table"
	actualSQL1, err1 := renderer.Render(selectStmt1, "test_table")
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
	actualSQL2, err2 := renderer.Render(selectStmt2, "another_table")
	assert.NoError(t, err2)
	assert.Equal(t, expectedSQL2, actualSQL2)
}

func TestRenderFilterStatement(t *testing.T) {
	renderer, err := NewRenderer(ast.DuckDBDialect)
	assert.NoError(t, err)

	tests := []struct {
		name        string
		filterStmt  *ast.FilterStmt
		expectedSQL string
	}{
		{
			name: "numeric comparison",
			filterStmt: &ast.FilterStmt{
				Condition: &ast.BinaryExpr{
					Left:  &ast.Identifier{Name: "price"},
					Op:    ">",
					Right: &ast.Literal{Value: "100"},
				},
			},
			expectedSQL: "WHERE price > 100",
		},
		{
			name: "string equality",
			filterStmt: &ast.FilterStmt{
				Condition: &ast.BinaryExpr{
					Left:  &ast.Identifier{Name: "region"},
					Op:    "==",
					Right: &ast.Literal{Value: `"US"`},
				},
			},
			expectedSQL: "WHERE region = 'US'",
		},
		{
			name: "less than or equal",
			filterStmt: &ast.FilterStmt{
				Condition: &ast.BinaryExpr{
					Left:  &ast.Identifier{Name: "age"},
					Op:    "<=",
					Right: &ast.Literal{Value: "30"},
				},
			},
			expectedSQL: "WHERE age <= 30",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			actualSQL, err := renderer.Render(tt.filterStmt, "")
			assert.NoError(t, err)
			assert.Equal(t, tt.expectedSQL, actualSQL)
		})
	}
}

func TestRenderGroupByStatement(t *testing.T) {
	renderer, err := NewRenderer(ast.DuckDBDialect)
	assert.NoError(t, err)

	tests := []struct {
		name        string
		groupByStmt *ast.GroupByStmt
		expectedSQL string
	}{
		{
			name: "single column",
			groupByStmt: &ast.GroupByStmt{
				Columns: []ast.Expr{
					&ast.Identifier{Name: "category"},
				},
			},
			expectedSQL: "GROUP BY category",
		},
		{
			name: "multiple columns",
			groupByStmt: &ast.GroupByStmt{
				Columns: []ast.Expr{
					&ast.Identifier{Name: "category"},
					&ast.Identifier{Name: "region"},
				},
			},
			expectedSQL: "GROUP BY category, region",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			actualSQL, err := renderer.Render(tt.groupByStmt, "")
			assert.NoError(t, err)
			assert.Equal(t, tt.expectedSQL, actualSQL)
		})
	}
}

func TestRenderSummariseStatement(t *testing.T) {
	renderer, err := NewRenderer(ast.DuckDBDialect)
	assert.NoError(t, err)

	tests := []struct {
		name          string
		summariseStmt *ast.SummariseStmt
		expectedSQL   string
	}{
		{
			name: "single aggregation",
			summariseStmt: &ast.SummariseStmt{
				Aggregations: []*ast.Aggregation{
					{
						Name: "avg_price",
						Expr: &ast.FuncCallExpr{Name: "mean", Args: []ast.Expr{&ast.Identifier{Name: "price"}}},
					},
				},
			},
			expectedSQL: "mean(price) AS avg_price",
		},
		{
			name: "multiple aggregations",
			summariseStmt: &ast.SummariseStmt{
				Aggregations: []*ast.Aggregation{
					{
						Name: "avg_price",
						Expr: &ast.FuncCallExpr{Name: "mean", Args: []ast.Expr{&ast.Identifier{Name: "price"}}},
					},
					{
						Name: "total_sales",
						Expr: &ast.FuncCallExpr{Name: "sum", Args: []ast.Expr{&ast.Identifier{Name: "sales"}}},
					},
				},
			},
			expectedSQL: "mean(price) AS avg_price, sum(sales) AS total_sales",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			actualSQL, err := renderer.Render(tt.summariseStmt, "")
			assert.NoError(t, err)
			assert.Equal(t, tt.expectedSQL, actualSQL)
		})
	}
}
