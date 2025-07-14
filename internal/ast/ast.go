// libdplyr/internal/ast/ast.go

package ast

// TargetDialect는 SQL 변환의 목표 방언을 정의합니다.
type TargetDialect string

const (
	PostgreSQLDialect TargetDialect = "postgres"
	MySQLDialect      TargetDialect = "mysql"
	SQLiteDialect     TargetDialect = "sqlite"
	DuckDBDialect     TargetDialect = "duckdb"
)

// Node는 AST의 모든 노드가 구현하는 기본 인터페이스입니다.
type Node interface{}

// Stmt는 하나의 dplyr 동사(verb)에 해당하는 구문(statement)입니다. (e.g., select, filter)
type Stmt interface {
	Node
	isStmt() // Stmt 인터페이스를 만족시키기 위한 마커 메소드
}

// Expr는 값, 변수, 연산 등을 나타내는 표현식(expression)입니다.
type Expr interface {
	Node
	isExpr() // Expr 인터페이스를 만족시키기 위한 마커 메소드
}

// --- 구문(Statements) ---

// Pipeline은 %>% 로 연결된 전체 쿼리를 나타냅니다.
type Pipeline struct {
	Table *TableIdentifier `@@`
	Steps []Stmt           `("%>%" @@)*`
}

func (p *Pipeline) isStmt() {}

// SelectStmt는 select() 구문을 나타냅니다.
type SelectStmt struct {
	Columns []Expr
}

func (s *SelectStmt) isStmt() {}

// FilterStmt는 filter() 구문을 나타냅니다.
type FilterStmt struct {
	Condition Expr
}

func (f *FilterStmt) isStmt() {}

// ArrangeStmt는 arrange() 구문을 나타냅니다.
type ArrangeStmt struct {
	Columns []Expr
}

func (a *ArrangeStmt) isStmt() {}

// GroupByStmt는 group_by() 구문을 나타냅니다.
type GroupByStmt struct {
	Columns []Expr
}

func (g *GroupByStmt) isStmt() {}

// SummariseStmt는 summarise() 구문을 나타냅니다.
type SummariseStmt struct {
	Aggregations []*Aggregation
}

func (s *SummariseStmt) isStmt() {}

// Aggregation은 summarise() 내의 단일 집계 표현식을 나타냅니다. (e.g., avg_price = mean(price))
type Aggregation struct {
	Name  string
	Expr Expr
}

// --- 표현식(Expressions) ---

// Identifier는 변수나 칼럼 이름을 나타냅니다. (e.g., price)
type Identifier struct {
	Name string
}

func (i *Identifier) isExpr() {}

// TableIdentifier는 테이블 이름을 나타냅니다.
type TableIdentifier struct {
	Name string
}

func (t *TableIdentifier) isExpr() {}

// Literal은 숫자나 문자열 같은 상수 값을 나타냅니다. (e.g., 100, "apple")
type Literal struct {
	Value string
}

func (l *Literal) isExpr() {}

// BinaryExpr는 이항 연산을 나타냅니다. (e.g., price > 100)
type BinaryExpr struct {
	Left  Expr
	Op    string // e.g., ">", "==", "+"
	Right Expr
}

func (b *BinaryExpr) isExpr() {}

// FuncCallExpr는 함수 호출을 나타냅니다. (e.g., n(), mean(price))
type FuncCallExpr struct {
	Name string
	Args []Expr
}

func (f *FuncCallExpr) isExpr() {}
