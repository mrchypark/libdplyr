# Design Document

## Overview

libdplyr은 R의 dplyr 문법을 SQL로 변환하는 Rust 기반 트랜스파일러입니다. 이 시스템은 lexical analysis, parsing, AST transformation, 그리고 SQL generation의 단계를 거쳐 dplyr 코드를 효율적인 SQL 쿼리로 변환합니다.

## Architecture

시스템은 다음과 같은 레이어드 아키텍처를 따릅니다:

```
┌─────────────────┐
│   CLI / API     │  ← 사용자 인터페이스
├─────────────────┤
│  SQL Generator  │  ← SQL 생성 및 방언 처리
├─────────────────┤
│ AST Transformer │  ← AST 최적화 및 변환
├─────────────────┤
│     Parser      │  ← dplyr 문법 파싱
├─────────────────┤
│     Lexer       │  ← 토큰화
└─────────────────┘
```

### 데이터 흐름

1. **Input**: dplyr 코드 문자열 또는 파일
2. **Lexing**: 문자열을 토큰으로 분해
3. **Parsing**: 토큰을 AST로 변환
4. **Transformation**: AST 최적화 및 검증
5. **Generation**: AST를 대상 SQL 방언으로 변환
6. **Output**: SQL 쿼리 문자열

## Components and Interfaces

### 1. Lexer Module (`src/lexer.rs`)

```rust
pub struct Lexer {
    input: String,
    position: usize,
    current_char: Option<char>,
}

pub enum Token {
    // dplyr 함수들
    Select,
    Filter,
    Mutate,
    Arrange,
    GroupBy,
    Summarise,
    
    // 연산자들
    Pipe,           // %>%
    Assignment,     // =
    Comparison(ComparisonOp),
    
    // 리터럴들
    Identifier(String),
    String(String),
    Number(f64),
    
    // 구조
    LeftParen,
    RightParen,
    Comma,
    EOF,
}

impl Lexer {
    pub fn new(input: String) -> Self;
    pub fn next_token(&mut self) -> Result<Token, LexError>;
}
```

### 2. Parser Module (`src/parser.rs`)

```rust
pub struct Parser {
    lexer: Lexer,
    current_token: Token,
}

#[derive(Debug, Clone)]
pub enum DplyrNode {
    Pipeline {
        operations: Vec<DplyrOperation>,
    },
    DataSource {
        name: String,
    },
}

#[derive(Debug, Clone)]
pub enum DplyrOperation {
    Select {
        columns: Vec<ColumnExpr>,
    },
    Filter {
        condition: Expr,
    },
    Mutate {
        assignments: Vec<Assignment>,
    },
    Arrange {
        columns: Vec<OrderExpr>,
    },
    GroupBy {
        columns: Vec<String>,
    },
    Summarise {
        aggregations: Vec<Aggregation>,
    },
}

impl Parser {
    pub fn new(lexer: Lexer) -> Self;
    pub fn parse(&mut self) -> Result<DplyrNode, ParseError>;
}
```

### 3. SQL Generator Module (`src/sql_generator.rs`)

```rust
pub trait SqlDialect {
    fn quote_identifier(&self, name: &str) -> String;
    fn limit_clause(&self, limit: usize) -> String;
    fn string_concat(&self, left: &str, right: &str) -> String;
}

pub struct PostgreSqlDialect;
pub struct MySqlDialect;
pub struct SqliteDialect;

pub struct SqlGenerator {
    dialect: Box<dyn SqlDialect>,
}

impl SqlGenerator {
    pub fn new(dialect: Box<dyn SqlDialect>) -> Self;
    pub fn generate(&self, ast: &DplyrNode) -> Result<String, GenerationError>;
    
    fn generate_select(&self, select: &SelectOp) -> String;
    fn generate_where(&self, filter: &FilterOp) -> String;
    fn generate_group_by(&self, group_by: &GroupByOp) -> String;
    fn generate_order_by(&self, arrange: &ArrangeOp) -> String;
}
```

### 4. CLI Module (`src/cli.rs`)

```rust
use clap::{Arg, Command};

pub struct CliArgs {
    pub input_file: Option<String>,
    pub output_file: Option<String>,
    pub dialect: SqlDialectType,
    pub pretty_print: bool,
}

#[derive(Debug, Clone)]
pub enum SqlDialectType {
    PostgreSql,
    MySql,
    Sqlite,
}

pub fn parse_args() -> CliArgs;
pub fn run_cli() -> Result<(), Box<dyn std::error::Error>>;
```

### 5. Library API (`src/lib.rs`)

```rust
pub use crate::parser::{DplyrNode, Parser};
pub use crate::sql_generator::{SqlGenerator, SqlDialect};
pub use crate::lexer::Lexer;

pub struct Transpiler {
    generator: SqlGenerator,
}

impl Transpiler {
    pub fn new(dialect: Box<dyn SqlDialect>) -> Self;
    
    pub fn transpile(&self, dplyr_code: &str) -> Result<String, TranspileError>;
    
    pub fn parse_dplyr(&self, code: &str) -> Result<DplyrNode, ParseError>;
    
    pub fn generate_sql(&self, ast: &DplyrNode) -> Result<String, GenerationError>;
}

#[derive(Debug)]
pub enum TranspileError {
    LexError(LexError),
    ParseError(ParseError),
    GenerationError(GenerationError),
}
```

## Data Models

### AST 노드 구조

```rust
// 표현식 타입들
#[derive(Debug, Clone)]
pub enum Expr {
    Identifier(String),
    Literal(LiteralValue),
    Binary {
        left: Box<Expr>,
        operator: BinaryOp,
        right: Box<Expr>,
    },
    Function {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    And,
    Or,
    Plus,
    Minus,
    Multiply,
    Divide,
}

// 컬럼 표현식
#[derive(Debug, Clone)]
pub struct ColumnExpr {
    pub expr: Expr,
    pub alias: Option<String>,
}

// 정렬 표현식
#[derive(Debug, Clone)]
pub struct OrderExpr {
    pub column: String,
    pub direction: OrderDirection,
}

#[derive(Debug, Clone)]
pub enum OrderDirection {
    Asc,
    Desc,
}
```

### SQL 방언 설정

```rust
#[derive(Debug, Clone)]
pub struct DialectConfig {
    pub identifier_quote: char,
    pub string_quote: char,
    pub supports_limit: bool,
    pub supports_offset: bool,
    pub case_sensitive: bool,
}

impl DialectConfig {
    pub fn postgresql() -> Self {
        DialectConfig {
            identifier_quote: '"',
            string_quote: '\'',
            supports_limit: true,
            supports_offset: true,
            case_sensitive: false,
        }
    }
    
    pub fn mysql() -> Self {
        DialectConfig {
            identifier_quote: '`',
            string_quote: '\'',
            supports_limit: true,
            supports_offset: true,
            case_sensitive: false,
        }
    }
    
    pub fn sqlite() -> Self {
        DialectConfig {
            identifier_quote: '"',
            string_quote: '\'',
            supports_limit: true,
            supports_offset: true,
            case_sensitive: false,
        }
    }
}
```

## Error Handling

### 오류 타입 계층

```rust
#[derive(Debug, thiserror::Error)]
pub enum LexError {
    #[error("Unexpected character: {0}")]
    UnexpectedCharacter(char),
    #[error("Unterminated string literal")]
    UnterminatedString,
    #[error("Invalid number format: {0}")]
    InvalidNumber(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Unexpected token: expected {expected}, found {found}")]
    UnexpectedToken { expected: String, found: String },
    #[error("Invalid dplyr operation: {0}")]
    InvalidOperation(String),
    #[error("Missing required argument for {function}")]
    MissingArgument { function: String },
}

#[derive(Debug, thiserror::Error)]
pub enum GenerationError {
    #[error("Unsupported operation for dialect: {operation}")]
    UnsupportedOperation { operation: String },
    #[error("Invalid column reference: {column}")]
    InvalidColumnReference { column: String },
    #[error("Complex expression not supported: {expr}")]
    ComplexExpression { expr: String },
}
```

### 오류 복구 전략

1. **Lexer 오류**: 잘못된 문자를 건너뛰고 다음 유효한 토큰 찾기
2. **Parser 오류**: 동기화 포인트(세미콜론, 파이프 연산자)까지 건너뛰기
3. **Generation 오류**: 지원되지 않는 기능에 대한 명확한 메시지 제공

## Testing Strategy

### 단위 테스트 구조

```rust
#[cfg(test)]
mod tests {
    use super::*;

    mod lexer_tests {
        #[test]
        fn test_tokenize_select() { /* ... */ }
        
        #[test]
        fn test_tokenize_pipe_operator() { /* ... */ }
        
        #[test]
        fn test_tokenize_string_literals() { /* ... */ }
    }

    mod parser_tests {
        #[test]
        fn test_parse_simple_select() { /* ... */ }
        
        #[test]
        fn test_parse_chained_operations() { /* ... */ }
        
        #[test]
        fn test_parse_complex_filter() { /* ... */ }
    }

    mod generator_tests {
        #[test]
        fn test_generate_postgresql_select() { /* ... */ }
        
        #[test]
        fn test_generate_mysql_select() { /* ... */ }
        
        #[test]
        fn test_generate_sqlite_select() { /* ... */ }
    }
}
```

### 통합 테스트

```rust
// tests/integration_tests.rs
use libdplyr::Transpiler;

#[test]
fn test_end_to_end_transpilation() {
    let dplyr_code = r#"
        data %>%
        select(name, age) %>%
        filter(age > 18) %>%
        arrange(desc(age))
    "#;
    
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let sql = transpiler.transpile(dplyr_code).unwrap();
    
    let expected = r#"
        SELECT "name", "age"
        FROM "data"
        WHERE "age" > 18
        ORDER BY "age" DESC
    "#;
    
    assert_eq!(normalize_sql(&sql), normalize_sql(expected));
}
```

### 테스트 데이터 세트

1. **기본 dplyr 패턴**: 각 함수의 기본 사용법
2. **복잡한 체이닝**: 여러 연산의 조합
3. **에지 케이스**: 빈 데이터, 특수 문자, 긴 식별자
4. **오류 케이스**: 잘못된 문법, 지원되지 않는 기능
5. **성능 테스트**: 큰 AST, 복잡한 쿼리

### 벤치마크

```rust
// benches/transpile_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use libdplyr::Transpiler;

fn benchmark_simple_transpile(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "data %>% select(name, age) %>% filter(age > 18)";
    
    c.bench_function("simple transpile", |b| {
        b.iter(|| transpiler.transpile(black_box(dplyr_code)))
    });
}

criterion_group!(benches, benchmark_simple_transpile);
criterion_main!(benches);
```