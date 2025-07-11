package renderer

import (
	"fmt"
	"strings"

	"github.com/mrchypark/libdplyr/internal/ast"
)

// Render 함수는 AST를 받아 SQL 문자열로 변환합니다.
func Render(node ast.Node, dialect ast.TargetDialect, tableName string) (string, error) {
	switch n := node.(type) {
	case *ast.SelectStmt:
		var columnNames []string
		for _, col := range n.Columns {
			if ident, ok := col.(*ast.Identifier); ok {
				columnNames = append(columnNames, ident.Name)
			} else {
				return "", fmt.Errorf("unsupported column type in SelectStmt: %T", col)
			}
		}
		columns := strings.Join(columnNames, ", ")
		return fmt.Sprintf("SELECT %s FROM %s", columns, tableName), nil
	default:
		return "", fmt.Errorf("unsupported AST node type")
	}
}