package parser

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/mrchypark/libdplyr/internal/ast"
)

func TestSelectSingleColumn(t *testing.T) {
	input := "my_table %>% select(col_a)"
	p, err := NewDplyrParser()
	assert.NoError(t, err)

	program, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, program)

	pipeline := program.Pipeline
	assert.NotNil(t, pipeline)
	assert.Len(t, pipeline.Steps, 1)

	stmt := pipeline.Steps[0]
	assert.NotNil(t, stmt.Select)
	selectClause := stmt.Select

	selectStmt := selectClause.ToAST()
	assert.Len(t, selectStmt.Columns, 1)

	ident, ok := selectStmt.Columns[0].(*ast.Identifier)
	assert.True(t, ok)
	assert.Equal(t, "col_a", ident.Name)
}

func TestSelectMultipleColumns(t *testing.T) {
	input := "my_table %>% select(col_a, col_b)"
	p, err := NewDplyrParser()
	assert.NoError(t, err)

	program, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, program)

	pipeline := program.Pipeline
	assert.NotNil(t, pipeline)
	assert.Len(t, pipeline.Steps, 1)

	stmt := pipeline.Steps[0]
	assert.NotNil(t, stmt.Select)
	selectClause := stmt.Select

	selectStmt := selectClause.ToAST()
	assert.Len(t, selectStmt.Columns, 2)

	ident1, ok1 := selectStmt.Columns[0].(*ast.Identifier)
	assert.True(t, ok1)
	assert.Equal(t, "col_a", ident1.Name)

	ident2, ok2 := selectStmt.Columns[1].(*ast.Identifier)
	assert.True(t, ok2)
	assert.Equal(t, "col_b", ident2.Name)
}

func TestSelectWithWhitespace(t *testing.T) {
	input := "my_table %>% select(  col_a  ,  col_b  )"
	p, err := NewDplyrParser()
	assert.NoError(t, err)

	program, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, program)

	pipeline := program.Pipeline
	assert.NotNil(t, pipeline)
	assert.Len(t, pipeline.Steps, 1)

	stmt := pipeline.Steps[0]
	assert.NotNil(t, stmt.Select)
	selectClause := stmt.Select

	selectStmt := selectClause.ToAST()
	assert.Len(t, selectStmt.Columns, 2)

	ident1, ok1 := selectStmt.Columns[0].(*ast.Identifier)
	assert.True(t, ok1)
	assert.Equal(t, "col_a", ident1.Name)

	ident2, ok2 := selectStmt.Columns[1].(*ast.Identifier)
	assert.True(t, ok2)
	assert.Equal(t, "col_b", ident2.Name)
}

func TestPipelineWithSelect(t *testing.T) {
	input := "my_table %>% select(col_a)"
	p, err := NewDplyrParser()
	assert.NoError(t, err)

	program, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, program)

	pipelineAST := program.Pipeline.ToAST()
	assert.NotNil(t, pipelineAST.Table)
	assert.Equal(t, "my_table", pipelineAST.Table.Name)
	assert.Len(t, pipelineAST.Steps, 1)

	selectStmt, ok := pipelineAST.Steps[0].(*ast.SelectStmt)
	assert.True(t, ok)
	assert.Len(t, selectStmt.Columns, 1)

	ident, ok := selectStmt.Columns[0].(*ast.Identifier)
	assert.True(t, ok)
	assert.Equal(t, "col_a", ident.Name)
}

func TestFilterParsing(t *testing.T) {
	tests := []struct {
		name  string
		input string
		expectedLeft  string
		expectedOp    string
		expectedRight string
	}{
		{
			name:  "numeric comparison",
			input: "my_table %>% filter(price > 100)",
			expectedLeft:  "price",
			expectedOp:    ">",
			expectedRight: "100",
		},
		{
			name:  "string equality",
			input: `my_table %>% filter(region == "US")`,
			expectedLeft:  "region",
			expectedOp:    "==",
			expectedRight: `"US"`,
		},
		{
			name:  "less than or equal",
			input: "my_table %>% filter(age <= 30)",
			expectedLeft:  "age",
			expectedOp:    "<=",
			expectedRight: "30",
		},
	}

	for _, tt := range tests {
					t.Run(tt.name, func(t *testing.T) {
				p, err := NewDplyrParser()
				assert.NoError(t, err)

				program, err := p.Parse(tt.input)
				assert.NoError(t, err)
				assert.NotNil(t, program)

				pipeline := program.Pipeline
				assert.NotNil(t, pipeline)
				assert.Len(t, pipeline.Steps, 1)

				stmt := pipeline.Steps[0]
				assert.NotNil(t, stmt.Filter)
				filterClause := stmt.Filter

				filterStmt := filterClause.ToAST()
			binaryExpr, ok := filterStmt.Condition.(*ast.BinaryExpr)
			assert.True(t, ok)

			leftIdent, ok := binaryExpr.Left.(*ast.Identifier)
			assert.True(t, ok)
			assert.Equal(t, tt.expectedLeft, leftIdent.Name)
			assert.Equal(t, tt.expectedOp, binaryExpr.Op)

			rightLiteral, ok := binaryExpr.Right.(*ast.Literal)
			assert.True(t, ok)
			assert.Equal(t, tt.expectedRight, rightLiteral.Value)
		})
	}
}

func TestArrangeParsing(t *testing.T) {
	tests := []struct {
		name        string
		input       string
		expectedAST *ast.ArrangeStmt
	}{
		// {
		// 	name:  "single column",
		// 	input: "my_table %>% arrange(col_a)",
		// 	expectedAST: &ast.ArrangeStmt{
		// 		Columns: []ast.Expr{
		// 			&ast.Identifier{Name: "col_a"},
		// 		},
		// 	},
		// },
		// {
		// 	name:  "multiple columns",
		// 	input: "my_table %>% arrange(col_a, col_b)",
		// 	expectedAST: &ast.ArrangeStmt{
		// 		Columns: []ast.Expr{
		// 			&ast.Identifier{Name: "col_a"},
		// 			&ast.Identifier{Name: "col_b"},
		// 		},
		// 	},
		// },
		{
			name:  "descending column",
			input: "my_table %>% arrange(desc(col_a))",
			expectedAST: &ast.ArrangeStmt{
				Columns: []ast.Expr{
					&ast.FuncCallExpr{Name: "desc", Args: []ast.Expr{&ast.Identifier{Name: "col_a"}}},
				},
			},
		},
		// {
		// 	name:  "multiple columns with descending",
		// 	input: "my_table %>% arrange(col_a, desc(col_b))",
		// 	expectedAST: &ast.ArrangeStmt{
		// 		Columns: []ast.Expr{
		// 			&ast.Identifier{Name: "col_a"},
		// 			&ast.FuncCallExpr{Name: "desc", Args: []ast.Expr{&ast.Identifier{Name: "col_b"}}},
		// 		},
		// 	},
		// },
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p, err := NewDplyrParser()
			assert.NoError(t, err)

			program, err := p.Parse(tt.input)
			assert.NoError(t, err)
			assert.NotNil(t, program)

			pipeline := program.Pipeline
			assert.NotNil(t, pipeline)
			assert.Len(t, pipeline.Steps, 1)

			stmt := pipeline.Steps[0]
			assert.NotNil(t, stmt.Arrange)
			arrangeClause := stmt.Arrange

			arrangeStmt := arrangeClause.ToAST()
			assert.Equal(t, tt.expectedAST, arrangeStmt)
		})
	}
}

func TestGroupByParsing(t *testing.T) {
	tests := []struct {
		name        string
		input       string
		expectedAST *ast.GroupByStmt
	}{
		{
			name:  "single column",
			input: "my_table %>% group_by(col_a)",
			expectedAST: &ast.GroupByStmt{
				Columns: []ast.Expr{
					&ast.Identifier{Name: "col_a"},
				},
			},
		},
		{
			name:  "multiple columns",
			input: "my_table %>% group_by(col_a, col_b)",
			expectedAST: &ast.GroupByStmt{
				Columns: []ast.Expr{
					&ast.Identifier{Name: "col_a"},
					&ast.Identifier{Name: "col_b"},
				},
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p, err := NewDplyrParser()
			assert.NoError(t, err)

			program, err := p.Parse(tt.input)
			assert.NoError(t, err)
			assert.NotNil(t, program)

			pipeline := program.Pipeline
			assert.NotNil(t, pipeline)
			assert.Len(t, pipeline.Steps, 1)

			stmt := pipeline.Steps[0]
			assert.NotNil(t, stmt.GroupBy)
			groupByClause := stmt.GroupBy

			groupByStmt := groupByClause.ToAST()
			assert.Equal(t, tt.expectedAST, groupByStmt)
		})
	}
}

func TestSummariseParsing(t *testing.T) {
	tests := []struct {
		name        string
		input       string
		expectedAST *ast.SummariseStmt
	}{
		{
			name:  "single aggregation",
			input: "my_table %>% summarise(avg_price = mean(price))",
			expectedAST: &ast.SummariseStmt{
				Aggregations: []*ast.Aggregation{
					{
						Name: "avg_price",
						Expr: &ast.FuncCallExpr{Name: "mean", Args: []ast.Expr{&ast.Identifier{Name: "price"}}},
					},
				},
			},
		},
		{
			name:  "multiple aggregations",
			input: "my_table %>% summarise(avg_price = mean(price), total_sales = sum(sales))",
			expectedAST: &ast.SummariseStmt{
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
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p, err := NewDplyrParser()
			assert.NoError(t, err)

			program, err := p.Parse(tt.input)
			assert.NoError(t, err)
			assert.NotNil(t, program)

			pipeline := program.Pipeline
			assert.NotNil(t, pipeline)
			assert.Len(t, pipeline.Steps, 1)

			stmt := pipeline.Steps[0]
			assert.NotNil(t, stmt.Summarise)
			summariseClause := stmt.Summarise

			summariseStmt := summariseClause.ToAST()
			assert.Equal(t, tt.expectedAST, summariseStmt)
		})
	}
}