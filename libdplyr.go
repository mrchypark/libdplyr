package libdplyr

import (
	"fmt"
	"strings"

	"github.com/alecthomas/participle/v2"
	"github.com/mrchypark/libdplyr/internal/ast"
	"github.com/mrchypark/libdplyr/internal/parser"
	"github.com/mrchypark/libdplyr/internal/renderer"
)

// Options는 트랜스파일링 과정을 제어하는 옵션을 담습니다.
type Options struct {
	// Target은 생성할 SQL의 방언을 지정합니다. (기본값: DuckDBDialect)
	Target ast.TargetDialect
	// TableName은 FROM 절에 사용될 테이블 이름을 지정합니다.
	TableName string
}

// Transpile은 dplyr 문자열을 SQL로 변환합니다.
// 이 함수가 libdplyr 라이브러리의 핵심 공개 API입니다.
func Transpile(dplyrQuery string, opts *Options) (string, error) {
	if opts == nil {
		opts = &Options{Target: ast.DuckDBDialect} // 기본 옵션
	}

	// 1. 파싱: 문자열 -> AST
	p, err := parser.NewDplyrParser()
	if err != nil {
		return "", fmt.Errorf("parser initialization error: %w", err)
	}
	parsedProgram, err := p.Parse(dplyrQuery)
	if err != nil {
		if pErr, ok := err.(participle.Error);
			ok {
			return "", fmt.Errorf("parsing error at %s:%d:%d: %w", pErr.Position().Filename, pErr.Position().Line, pErr.Position().Column, pErr)
		}
		return "", fmt.Errorf("parsing error: %w", err)
	}

	// Convert parser's DplyrProgram to ast.Pipeline
	pipelineAST := parsedProgram.Pipeline.ToAST()

	// Extract table name from pipeline
	tableName := pipelineAST.Table.Name

	// Create a renderer for the target dialect
	rendererInstance, err := renderer.NewRenderer(opts.Target)
	if err != nil {
		return "", fmt.Errorf("renderer initialization error: %w", err)
	}

	// Build the SQL query step by step
	var sqlParts []string
	var selectClauseRendered bool

	for _, step := range pipelineAST.Steps {
		switch s := step.(type) {
		case *ast.SelectStmt:
			sql, err := rendererInstance.Render(s, tableName)
			if err != nil {
				return "", fmt.Errorf("rendering select statement error: %w", err)
			}
			sqlParts = append(sqlParts, sql)
			selectClauseRendered = true
		case *ast.FilterStmt:
			sql, err := rendererInstance.Render(s, "") // Table name not needed for WHERE clause
			if err != nil {
				return "", fmt.Errorf("rendering filter statement error: %w", err)
			}
			sqlParts = append(sqlParts, sql)
		case *ast.ArrangeStmt:
			sql, err := rendererInstance.Render(s, "") // Table name not needed for ORDER BY clause
			if err != nil {
				return "", fmt.Errorf("rendering arrange statement error: %w", err)
			}
			sqlParts = append(sqlParts, sql)
		case *ast.GroupByStmt:
			sql, err := rendererInstance.Render(s, "") // Table name not needed for GROUP BY clause
			if err != nil {
				return "", fmt.Errorf("rendering group by statement error: %w", err)
			}
			sqlParts = append(sqlParts, sql)
		case *ast.SummariseStmt:
			sql, err := rendererInstance.Render(s, "") // Table name not needed for SUMMARISE clause
			if err != nil {
				return "", fmt.Errorf("rendering summarise statement error: %w", err)
			}
			sqlParts = append(sqlParts, sql)
		default:
			return "", fmt.Errorf("unsupported AST statement type: %T", s)
		}
	}

	// If no select clause was rendered, default to SELECT *
	if !selectClauseRendered {
		sqlParts = append([]string{fmt.Sprintf("SELECT * FROM %s", tableName)}, sqlParts...)
	}

	return strings.Join(sqlParts, " "), nil
}
