package parser

import (
	"github.com/alecthomas/participle/v2"
	"github.com/alecthomas/participle/v2/lexer"
	"github.com/mrchypark/libdplyr/internal/ast"
)

// DplyrLexer defines the lexer for dplyr-like syntax.
var DplyrLexer = lexer.MustSimple([]lexer.SimpleRule{
	{Name: "Ident", Pattern: `[a-zA-Z_][a-zA-Z0-9_]*`},
	{Name: "String", Pattern: `"[^"\\]*(\\.[^"\\]*)*"`},
	{Name: "Float", Pattern: `[0-9]+\.[0-9]*([eE][-+]?[0-9]+)?`},
	{Name: "Int", Pattern: `[0-9]+`},
	{Name: "Pipe", Pattern: `%>%`},
	{Name: "Punct", Pattern: `[(),%><=!]+`},
	{Name: "Whitespace", Pattern: `\s+`},
	{Name: "EOL", Pattern: `[\n\r]+`},
})

// DplyrProgram represents the top-level structure of a dplyr program.
type DplyrProgram struct {
	Pipeline *Pipeline `@@`
}

// Pipeline represents a dplyr pipeline.
type Pipeline struct {
	Table *Identifier `@@`
	Steps []Stmt      `("%>%" @@)*`
}

// Stmt is a statement in the dplyr pipeline.
type Stmt struct {
	Select *SelectClause `( "select" "(" @@ ")" )`
	Filter *FilterClause `| ( "filter" "(" @@ ")" )`
	Arrange *ArrangeClause `| ( "arrange" "(" @@ ")" )`
	GroupBy *GroupByClause `| ( "group_by" "(" @@ ")" )`
	Summarise *SummariseClause `| ( "summarise" "(" @@ ")" )`
}

// SelectClause represents the "select(...)" part of the dplyr syntax.
type SelectClause struct {
	Columns []*Identifier `@@ ("," @@)*`
}

// FilterClause represents the "filter(...)" part of the dplyr syntax.
type FilterClause struct {
	Condition *BinaryExpr `@@`
}

// Identifier represents an identifier in the parser's context.
type Identifier struct {
	Name string `@Ident`
}

// BinaryExpr represents a binary expression in the parser's context.
type BinaryExpr struct {
	Left  *Identifier `@@`
	Op    string      `@(">" | "<" | "==" | "!=" | ">=" | "<=")`
	Right *Literal    `@@`
}

// Literal represents a literal value in the parser's context.
type Literal struct {
	Value string `@Ident | @String | @Float | @Int`
}

// ArrangeClause represents the "arrange(...)" part of the dplyr syntax.
type ArrangeClause struct {
	Columns []*ArrangeColumn `@@ ("," @@)*`
}

// ArrangeColumn represents a column in an arrange clause, possibly with a descending indicator.
type ArrangeColumn struct {
	FuncCall   *FuncCallExpr `( @@`
	Identifier *Identifier   `| @@ )`
}
type GroupByClause struct {
	Columns []*Identifier `@@ ("," @@)*`
}

// SummariseClause represents the "summarise(...)" part of the dplyr syntax.
type SummariseClause struct {
	Aggregations []*Aggregation `@@ ("," @@)*`
}

// Aggregation represents a single aggregation in a summarise clause.
type Aggregation struct {
	Name *Identifier `@@ "="`
	Expr *FuncCallExpr `@@`
}

// FuncCallExpr represents a function call in the parser's context.
type FuncCallExpr struct {
	Name *Identifier `@@`
	_    string      `@"("` // Explicitly match the opening parenthesis
	Args []*Identifier `( @@ ("," @@)* )? ")"`
}



// DplyrParser represents the parser for dplyr-like syntax.
type DplyrParser struct {
	parser *participle.Parser[DplyrProgram]
}

// NewDplyrParser creates a new DplyrParser instance.
func NewDplyrParser() (*DplyrParser, error) {
	parser, err := participle.Build[DplyrProgram](
		participle.Lexer(DplyrLexer),
		participle.Elide("Whitespace"),
	)
	if err != nil {
		return nil, err
	}
	return &DplyrParser{parser: parser}, nil
}

// Parse parses the given dplyr input string into a DplyrProgram.
func (p *DplyrParser) Parse(input string) (*DplyrProgram, error) {
	program, err := p.parser.ParseString("", input)
	if err != nil {
		return nil, err
	}
	return program, nil
}

// ToAST converts the parser's Stmt to ast.Stmt
func (s *Stmt) ToAST() ast.Stmt {
	if s.Select != nil {
		return s.Select.ToAST()
	} else if s.Filter != nil {
		return s.Filter.ToAST()
	} else if s.Arrange != nil {
		return s.Arrange.ToAST()
	} else if s.GroupBy != nil {
		return s.GroupBy.ToAST()
	} else if s.Summarise != nil {
		return s.Summarise.ToAST()
	}
	return nil
}

// ToAST converts the parser's Pipeline to ast.Pipeline
func (p *Pipeline) ToAST() *ast.Pipeline {
	astSteps := make([]ast.Stmt, len(p.Steps))
	for i, step := range p.Steps {
		astSteps[i] = step.ToAST()
	}
	return &ast.Pipeline{
		Table: &ast.TableIdentifier{Name: p.Table.Name},
		Steps: astSteps,
	}
}

// ToAST converts the parser's Identifier to ast.Identifier
func (i *Identifier) ToAST() *ast.Identifier {
	return &ast.Identifier{Name: i.Name}
}

// ToAST converts the parser's SelectClause to ast.SelectStmt
func (s *SelectClause) ToAST() *ast.SelectStmt {
	astColumns := make([]ast.Expr, len(s.Columns))
	for i, col := range s.Columns {
		astColumns[i] = &ast.Identifier{Name: col.Name}
	}
	return &ast.SelectStmt{
		Columns: astColumns,
	}
}

// ToAST converts the parser's FilterClause to ast.FilterStmt
func (f *FilterClause) ToAST() *ast.FilterStmt {
	return &ast.FilterStmt{
		Condition: f.Condition.ToAST(),
	}
}

// ToAST converts the parser's BinaryExpr to ast.BinaryExpr
func (b *BinaryExpr) ToAST() *ast.BinaryExpr {
	return &ast.BinaryExpr{
		Left:  &ast.Identifier{Name: b.Left.Name},
		Op:    b.Op,
		Right: &ast.Literal{Value: b.Right.Value},
	}
}

// ToAST converts the parser's ArrangeClause to ast.ArrangeStmt
func (a *ArrangeClause) ToAST() *ast.ArrangeStmt {
	astColumns := make([]ast.Expr, len(a.Columns))
	for i, col := range a.Columns {
		var expr ast.Expr
		if col.FuncCall != nil {
			expr = col.FuncCall.ToAST()
		} else if col.Identifier != nil {
			expr = col.Identifier.ToAST()
		}
		astColumns[i] = expr
	}
	return &ast.ArrangeStmt{
		Columns: astColumns,
	}
}

// ToAST converts the parser's GroupByClause to ast.GroupByStmt
func (g *GroupByClause) ToAST() *ast.GroupByStmt {
	astColumns := make([]ast.Expr, len(g.Columns))
	for i, col := range g.Columns {
		astColumns[i] = &ast.Identifier{Name: col.Name}
	}
	return &ast.GroupByStmt{
		Columns: astColumns,
	}
}

// ToAST converts the parser's SummariseClause to ast.SummariseStmt
func (s *SummariseClause) ToAST() *ast.SummariseStmt {
	astAggregations := make([]*ast.Aggregation, len(s.Aggregations))
	for i, agg := range s.Aggregations {
		astAggregations[i] = agg.ToAST()
	}
	return &ast.SummariseStmt{
		Aggregations: astAggregations,
	}
}

// ToAST converts the parser's Aggregation to ast.Aggregation
func (a *Aggregation) ToAST() *ast.Aggregation {
	return &ast.Aggregation{
		Name: a.Name.ToAST().Name,
		Expr: a.Expr.ToAST(),
	}
}

// ToAST converts the parser's FuncCallExpr to ast.FuncCallExpr
func (f *FuncCallExpr) ToAST() *ast.FuncCallExpr {
	astArgs := make([]ast.Expr, len(f.Args))
	for i, arg := range f.Args {
		astArgs[i] = arg.ToAST()
	}
	return &ast.FuncCallExpr{
		Name: f.Name.Name,
		Args: astArgs,
	}
}

