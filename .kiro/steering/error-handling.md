---
inclusion: always
---

# 에러 처리 및 디버깅 가이드라인

## 에러 타입 계층 구조

### 단계별 에러 정의
- **LexError**: 토큰화 과정의 에러 (문자 인식, 문자열 파싱 등)
- **ParseError**: 구문 분석 과정의 에러 (문법 오류, 예상치 못한 토큰 등)
- **GenerationError**: SQL 생성 과정의 에러 (지원되지 않는 연산, 방언 호환성 등)
- **TranspileError**: 전체 변환 과정을 포괄하는 통합 에러

### 에러 메시지 작성 원칙
- 영어로 작성하되 명확하고 구체적으로
- 위치 정보(position) 반드시 포함
- 사용자가 문제를 해결할 수 있는 힌트 제공
- 에러 컨텍스트 정보 포함 (예: 어떤 함수에서, 어떤 연산 중)

## 에러 처리 패턴

### Result 타입 사용
```rust
// 좋은 예시
pub fn parse_expression(&mut self) -> ParseResult<Expr> {
    match self.current_token {
        Token::Identifier(name) => {
            // 처리 로직
            Ok(expr)
        }
        _ => Err(ParseError::UnexpectedToken {
            expected: "expression".to_string(),
            found: format!("{}", self.current_token),
            position: self.position,
        })
    }
}
```

### 에러 전파 및 변환
```rust
// From 트레이트를 활용한 에러 변환
impl From<LexError> for ParseError {
    fn from(err: LexError) -> Self {
        ParseError::LexError(err)
    }
}
```

## CLI 에러 메시지 현지화

### 사용자 친화적 메시지
- CLI에서는 한국어로 에러 설명 제공
- 해결 방법 힌트 포함
- 예시 코드나 올바른 사용법 제시

```rust
pub fn print_error(error: &TranspileError) {
    eprintln!("오류: {}", error);
    
    match error {
        TranspileError::LexError(_) => {
            eprintln!("힌트: 입력 코드의 문법을 확인해주세요.");
            eprintln!("      특히 문자열 따옴표나 특수 문자를 확인해보세요.");
        }
        TranspileError::ParseError(_) => {
            eprintln!("힌트: dplyr 함수의 사용법을 확인해주세요.");
            eprintln!("      예: data %>% select(col1, col2) %>% filter(col1 > 10)");
        }
        TranspileError::GenerationError(_) => {
            eprintln!("힌트: 선택한 SQL 방언에서 지원되지 않는 기능일 수 있습니다.");
            eprintln!("      다른 방언을 시도해보거나 더 간단한 표현식을 사용해보세요.");
        }
    }
}
```

## 디버깅 지원

### 로깅 및 트레이싱
- 개발 중에는 상세한 디버그 정보 출력
- 프로덕션에서는 필요한 정보만 로깅
- AST 구조 시각화 기능 제공

### 테스트 중 에러 검증
```rust
#[test]
fn test_specific_error_case() {
    let result = parser.parse_invalid_syntax();
    
    match result {
        Err(ParseError::UnexpectedToken { expected, found, position }) => {
            assert_eq!(expected, "identifier");
            assert_eq!(found, "number");
            assert_eq!(position, 5);
        }
        other => panic!("예상된 에러가 발생하지 않음: {:?}", other),
    }
}
```

## 에러 복구 전략

### 부분적 파싱 지원
- 가능한 경우 에러 후에도 파싱 계속 진행
- 여러 에러를 한 번에 보고
- 사용자가 전체 코드를 수정할 수 있도록 지원

### 제안 시스템
- 오타나 잘못된 함수명에 대한 제안 제공
- 비슷한 함수명이나 올바른 문법 제시