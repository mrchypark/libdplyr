package parser

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/mrchypark/libdplyr/internal/ast"
)

func TestSelectSingleColumn(t *testing.T) {
	input := "select(col_a)"
	p, err := NewDplyrParser()
	assert.NoError(t, err)

	programEntry, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, programEntry)

	selectClause, ok := programEntry.(*SelectClause)
	assert.True(t, ok, "Expected a SelectClause")
	assert.NotNil(t, selectClause)

	// Convert the parsed SelectClause to ast.SelectStmt for assertion
	selectStmt := selectClause.ToAST()
	assert.Len(t, selectStmt.Columns, 1)

	ident, ok := selectStmt.Columns[0].(*ast.Identifier)
	assert.True(t, ok)
	assert.Equal(t, "col_a", ident.Name)
}

func TestSelectMultipleColumns(t *testing.T) {
	input := "select(col_a, col_b)"
	p, err := NewDplyrParser()
	assert.NoError(t, err)

	programEntry, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, programEntry)

	selectClause, ok := programEntry.(*SelectClause)
	assert.True(t, ok, "Expected a SelectClause")
	assert.NotNil(t, selectClause)

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
	input := "select(  col_a  ,  col_b  )"
	p, err := NewDplyrParser()
	assert.NoError(t, err)

	programEntry, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, programEntry)

	selectClause, ok := programEntry.(*SelectClause)
	assert.True(t, ok, "Expected a SelectClause")
	assert.NotNil(t, selectClause)

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

	programEntry, err := p.Parse(input)
	assert.NoError(t, err)
	assert.NotNil(t, programEntry)

	pipeline, ok := programEntry.(*Pipeline)
	assert.True(t, ok, "Expected a Pipeline")
	assert.NotNil(t, pipeline)

	pipelineAST := pipeline.ToAST()
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