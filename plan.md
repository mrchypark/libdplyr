#### 1단계: 파서(Parser)의 첫걸음 - `select` 구문 완성하기

가장 먼저, dplyr 문법을 분석하여 Go 구조체(AST)로 변환하는 파서의 핵심 골격을 만듭니다.

* **1-1. 프로젝트 구조 및 테스트 환경 설정**
    * `[x]` **Go 모듈 초기화**: `go mod init`으로 프로젝트를 시작하고, 파서 라이브러리 `participle`을 의존성에 추가합니다. (`go get github.com/alecthomas/participle/v2`)
    * `[x]` **AST(추상 구문 트리) 파일 생성**: `internal/ast/ast.go` 파일을 만들고, `select` 구문을 표현할 `SelectStmt`와 `Column` 구조체를 정의합니다.
    * `[x]` **파서 파일 및 테스트 파일 생성**: `internal/parser/parser.go`와 `parser_test.go` 파일을 생성합니다.

* **1-2. `select` 구문 파싱 (TDD 사이클)**
    * `[x]` **Test 1: 단일 컬럼 `select`**
        * **실패하는 테스트 작성**: `parser_test.go`에 `select(col_a)`를 파싱하여 `SelectStmt{ Columns: ["col_a"] }`와 같은 AST가 생성되는지 확인하는 테스트를 추가합니다.
        * **최소 기능 구현**: `participle`을 이용해 `SelectStmt` 구조체에 `@Ident`와 같은 태그를 추가하여 테스트를 통과시킵니다. `participle.MustBuild[SelectStmt]()`로 파서를 생성합니다.
    * `[x]` **Test 2: 다중 컬럼 `select`**
        * **실패하는 테스트 작성**: `select(col_a, col_b, col_c)` 구문을 처리하는 테스트를 추가합니다.
        * **기능 구현 및 리팩토링**: `Columns` 필드를 `[]string`으로 변경하고, `(@Ident ("," @Ident)*)`와 같은 반복(`*`) 및 그룹(`()`) 문법을 사용하여 다중 컬럼을 지원하도록 파서를 개선합니다.
    * `[x]` **Test 3: 공백 및 개행 처리**
        * **실패하는 테스트 작성**: `select(  col_a  , \n col_b )`와 같이 자유로운 공백을 허용하는지 테스트합니다.
        * **파서 옵션 추가**: `participle.Elide("Whitespace", "EOL")` 옵션을 파서 빌더에 추가하여 불필요한 공백과 개행 문자를 무시하도록 설정합니다. 이를 위해 `participle.Lexer()`를 이용한 커스텀 lexer 정의가 필요할 수 있습니다.

---

#### 2단계: 렌더러(Renderer) 구현 - `SELECT` SQL 문 생성

이제 파싱된 AST를 실제 SQL 문으로 변환하는 렌더러를 만듭니다.

* `[x]` **렌더러 및 테스트 파일 생성**: `internal/renderer/renderer.go` 및 `renderer_test.go` 파일을 생성합니다.
* `[x]` **Test 1: `SELECT` 절 렌더링**
    * **실패하는 테스트 작성**: `SelectStmt` AST를 입력받아 `SELECT col_a, col_b` 형태의 SQL 문자열을 반환하는지 검증하는 테스트를 추가합니다.
    * **기능 구현**: `SelectStmt`를 처리하는 렌더링 함수를 작성하고, `strings.Join()` 등을 활용하여 컬럼 목록을 만듭니다.
* `[x]` **Test 2: `FROM` 절 동적 처리**
    * **Transpile 함수 구조 변경**: `libdplyr.go`의 `Transpile` 함수가 `(dplyrQuery, tableName string)`을 인자로 받도록 수정합니다.
    * **실패하는 테스트 작성**: `libdplyr_test.go`에서 `Transpile` 함수가 `SELECT ... FROM my_table` 형태의 완전한 SQL을 생성하는지 테스트합니다.
    * **기능 구현**: 파서가 `my_table %>% select(...)`와 같은 파이프라인(`%>%`)을 인식하도록 문법을 확장하고, 렌더러가 이 정보를 `FROM` 절에 사용하도록 개선합니다. 이를 위해 `Pipeline`과 `TableIdentifier` AST 노드가 필요합니다.

---

#### 3단계: 핵심 동사(Verb) 확장 - `filter`와 `arrange`

`select`가 안정화되었으니, 데이터 조작의 핵심인 `filter`와 `arrange`를 추가합니다.

* **3-1. `filter` 동사 (WHERE 절)**
    * `[x]` **AST 정의**: `FilterStmt`, `Condition` 등 `WHERE` 절의 조건(`col_a > 100`)을 표현할 구조체를 `ast.go`에 정의합니다. 값의 종류(숫자, 문자열 등)를 다루기 위해 `participle.Union` 옵션을 고려하는 것이 좋습니다.
    * `[x]` **`filter` 파싱 테스트 추가**: `filter(price > 100)`와 `filter(region == "US")`를 파싱하는 테스트를 작성합니다.
    * `[x]` **AST 정의**: `FilterStmt`, `Condition` 등 `WHERE` 절의 조건(`col_a > 100`)을 표현할 구조체를 `ast.go`에 정의합니다. 값의 종류(숫자, 문자열 등)를 다루기 위해 `participle.Union` 옵션을 고려하는 것이 좋습니다.
    * `[x]` **`filter` 파싱 구현**: 이항 연산자(`>`, `<`, `==` 등)와 다양한 값 타입을 인식하도록 파서 문법을 확장합니다.
    * `[x]` **`filter` 렌더링 테스트 추가**: 파싱된 `FilterStmt`가 `WHERE price > 100`으로 변환되는지 테스트합니다.
    * `[x]` **`filter` 렌더링 구현**: 렌더러에 `FilterStmt` 처리 로직을 추가하여 `WHERE` 절을 생성하도록 합니다.

* **3-2. `arrange` 동사 (ORDER BY 절)**
    * `[x]` **`arrange` 파싱 및 렌더링 테스트 동시 작성**: `arrange(col_a, desc(col_b))`가 `ORDER BY col_a, col_b DESC`로 변환되는지 확인하는 엔드-투-엔드 테스트를 추가합니다.
    * `[x]` **`arrange` 기능 전체 구현**: `ArrangeStmt` AST를 정의하고, 파서가 `arrange`와 `desc()` 함수를 인식하도록 합니다. 렌더러는 이 AST를 기반으로 `ORDER BY` 절을 생성하도록 구현합니다.

---

#### 4단계: 고급 기능 및 완성도 높이기

프로젝트의 활용도를 높이고 실사용에 대비하기 위한 단계입니다.

* `[x]` **집계 함수 지원 (`group_by`, `summarise`)**
    * `[x]` **테스트 주도 구현**: `group_by(category) %>% summarise(avg_price = mean(price))` 코드가 `SELECT category, AVG(price) AS avg_price FROM ... GROUP BY category` SQL로 변환되는 과정을 TDD 사이클에 맞춰 구현합니다.
* `[x]` **SQL 방언(Dialect) 지원**
    * `[x]` **렌더러 전략 패턴 도입**: `Renderer`를 인터페이스로 만들고, `PostgreSQLRenderer`, `MySQLRenderer` 등 각 DB에 맞는 구현체를 만듭니다. `Transpile` 함수에 `Dialect` 옵션을 추가하여 원하는 렌더러를 선택할 수 있도록 합니다.
* `[x]` **CLI 및 에러 핸들링**
    * `[x]` **CLI 기능 확장**: 파일 입력(`--file`) 및 파라미터화된 쿼리 지원을 추가합니다.
    * `[x]` **사용자 친화적 에러 메시지**: `participle`의 에러 객체(`participle.Error`)가 제공하는 위치 정보(`lexer.Position`)를 활용하여, "line 5, column 10: 'filter' 다음에는 '('가 와야 합니다"와 같이 구체적인 에러 메시지를 생성하도록 로직을 개선합니다.
* `[ ]` **문서화 및 배포**
    * `[x]` **`README.md` 업데이트**: 프로젝트의 비전, 설치 및 사용법, 지원하는 dplyr 함수 목록을 상세하게 작성합니다.
    * `[x]` **CI/CD 파이프라인 설정**: GitHub Actions 등을 사용하여 테스트, 빌드, 릴리즈 과정을 자동화합니다.

이 로드맵이 `libdplyr` 프로젝트를 성공으로 이끄는 데 훌륭한 길잡이가 되기를 바랍니다. 💪