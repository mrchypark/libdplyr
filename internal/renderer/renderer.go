package renderer

import (
	"fmt"
	"strings"

	"github.com/mrchypark/libdplyr/internal/ast"
)

// Renderer 인터페이스는 AST 노드를 SQL 문자열로 렌더링하는 메서드를 정의합니다.
type Renderer interface {
	Render(node ast.Node, tableName string) (string, error)
}

// NewRenderer는 주어진 방언에 맞는 렌더러 인스턴스를 반환합니다.
func NewRenderer(dialect ast.TargetDialect) (Renderer, error) {
	switch dialect {
	case ast.DuckDBDialect:
		return &duckDBRenderer{}, nil
	case ast.PostgreSQLDialect:
		return &postgreSQLRenderer{}, nil
	case ast.MySQLDialect:
		return &mySQLRenderer{}, nil
	case ast.SQLiteDialect:
		return &sqliteRenderer{}, nil
	default:
		return nil, fmt.Errorf("unsupported SQL dialect: %s", dialect)
	}
}

// duckDBRenderer는 DuckDB 방언에 특화된 렌더러 구현체입니다.
type duckDBRenderer struct{}

// postgreSQLRenderer는 PostgreSQL 방언에 특화된 렌더러 구현체입니다.
type postgreSQLRenderer struct{}

// mySQLRenderer는 MySQL 방언에 특화된 렌더러 구현체입니다.
type mySQLRenderer struct{}

// sqliteRenderer는 SQLite 방언에 특화된 렌더러 구현체입니다.
type sqliteRenderer struct{}

// Render 함수는 AST를 받아 SQL 문자열로 변환합니다.
func (r *duckDBRenderer) Render(node ast.Node, tableName string) (string, error) {
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
	case *ast.FilterStmt:
		binaryExpr, ok := n.Condition.(*ast.BinaryExpr)
		if !ok {
			return "", fmt.Errorf("unsupported condition type in FilterStmt: %T", n.Condition)
		}
		leftIdent, ok := binaryExpr.Left.(*ast.Identifier)
		if !ok {
			return "", fmt.Errorf("unsupported left operand type in BinaryExpr: %T", binaryExpr.Left)
		}
		rightLiteral, ok := binaryExpr.Right.(*ast.Literal)
		if !ok {
			return "", fmt.Errorf("unsupported right operand type in BinaryExpr: %T", binaryExpr.Right)
		}

		op := binaryExpr.Op
		if op == "==" {
			op = "="
		}

		// Handle string literals by quoting them
		rightValue := rightLiteral.Value
		if strings.HasPrefix(rightValue, `"`) && strings.HasSuffix(rightValue, `"`) {
			rightValue = fmt.Sprintf("'%s'", strings.Trim(rightValue, `"`))
		}

		return fmt.Sprintf("WHERE %s %s %s", leftIdent.Name, op, rightValue), nil
	case *ast.ArrangeStmt:
		var columnNames []string
		for _, col := range n.Columns {
			if ident, ok := col.(*ast.Identifier); ok {
				columnNames = append(columnNames, ident.Name)
			} else if funcCall, ok := col.(*ast.FuncCallExpr); ok && funcCall.Name == "desc" && len(funcCall.Args) == 1 {
				if descIdent, ok := funcCall.Args[0].(*ast.Identifier); ok {
					columnNames = append(columnNames, fmt.Sprintf("%s DESC", descIdent.Name))
				} else {
					return "", fmt.Errorf("unsupported argument type for desc() in ArrangeStmt: %T", funcCall.Args[0])
				}
			} else {
				return "", fmt.Errorf("unsupported column type in ArrangeStmt: %T", col)
			}
		}
		return fmt.Sprintf("ORDER BY %s", strings.Join(columnNames, ", ")), nil
	case *ast.GroupByStmt:
		var columnNames []string
		for _, col := range n.Columns {
			if ident, ok := col.(*ast.Identifier); ok {
				columnNames = append(columnNames, ident.Name)
			} else {
				return "", fmt.Errorf("unsupported column type in GroupByStmt: %T", col)
			}
		}
		return fmt.Sprintf("GROUP BY %s", strings.Join(columnNames, ", ")), nil
	case *ast.SummariseStmt:
		var aggregations []string
		for _, agg := range n.Aggregations {
			// Render the expression part of the aggregation
			var exprStr string
			if funcCall, ok := agg.Expr.(*ast.FuncCallExpr); ok {
				var args []string
				for _, arg := range funcCall.Args {
					if ident, ok := arg.(*ast.Identifier); ok {
						args = append(args, ident.Name)
					} else {
						return "", fmt.Errorf("unsupported argument type in FuncCallExpr: %T", arg)
					}
				}
				exprStr = fmt.Sprintf("%s(%s)", funcCall.Name, strings.Join(args, ", "))
			} else {
				return "", fmt.Errorf("unsupported expression type in Aggregation: %T", agg.Expr)
			}
			aggregations = append(aggregations, fmt.Sprintf("%s AS %s", exprStr, agg.Name))
		}
		return strings.Join(aggregations, ", "), nil
	default:
		return "", fmt.Errorf("unsupported AST node type")
	}
}

func (r *postgreSQLRenderer) Render(node ast.Node, tableName string) (string, error) {
	return "", fmt.Errorf("PostgreSQL dialect not yet supported")
}

func (r *mySQLRenderer) Render(node ast.Node, tableName string) (string, error) {
	return "", fmt.Errorf("MySQL dialect not yet supported")
}

func (r *sqliteRenderer) Render(node ast.Node, tableName string) (string, error) {
	return "", fmt.Errorf("SQLite dialect not yet supported")
}