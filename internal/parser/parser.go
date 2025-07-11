package parser

import (
	"github.com/alecthomas/participle/v2"
	"github.com/alecthomas/participle/v2/lexer"
	"github.com/mrchypark/libdplyr/internal/ast"
)

// DplyrLexer defines the lexer for dplyr-like syntax.
var DplyrLexer = lexer.MustSimple([]lexer.SimpleRule{
	{Name: "Ident", Pattern: `[a-zA-Z_][a-zA-Z0-9_]*`},
	{Name: "Punct", Pattern: `[(),%]`},
	{Name: "Whitespace", Pattern: `\s+`},
})

// DplyrProgram represents the top-level structure of a dplyr program.
type DplyrProgram struct {
	Select *SelectClause `  "select" "(" @@ ")"`
}

// SelectClause represents the "select(...)" part of the dplyr syntax.
type SelectClause struct {
	Columns []*Identifier `@@ ("," @@)*`
}

// Identifier represents an identifier in the parser's context.
type Identifier struct {
	Name string `@Ident`
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
