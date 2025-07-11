package libdplyr

import (
	"fmt"

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
		return "", fmt.Errorf("parsing error: %w", err)
	}

	// Convert parser's DplyrProgram to ast.Node
	var astNode ast.Node
	var tableName string

	// Expecting a direct SelectClause from the simplified parser
	if parsedProgram.Select != nil {
		astNode = parsedProgram.Select.ToAST()
		if opts.TableName == "" {
			return "", fmt.Errorf("table name must be provided in options for direct select statements")
		}
		tableName = opts.TableName
	} else {
		return "", fmt.Errorf("unsupported dplyr program structure: expected a direct select statement")
	}

	// 2. 렌더링: AST -> SQL
	sql, err := renderer.Render(astNode, opts.Target, tableName)
	if err != nil {
		return "", fmt.Errorf("rendering error: %w", err)
	}

	return sql, nil
}
