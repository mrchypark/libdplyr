---
inclusion: always
---

# 코드 품질 및 유지보수성 가이드라인

- code first and import with ide and check correct

## 코드 스타일 표준

### Rust 관례 준수
- `rustfmt`를 사용한 자동 포맷팅 적용
- `clippy` 린터 경고 모두 해결
- 네이밍 컨벤션 엄격 준수 (snake_case, PascalCase)
- 공개 API는 반드시 문서 주석 포함

### 문서화 표준
```rust
/// Brief description of the function
///
/// More detailed explanation if needed.
///
/// # Arguments
///
/// * `param1` - Description of parameter 1
/// * `param2` - Description of parameter 2
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// Description of possible errors
///
/// # Examples
///
/// ```
/// use libdplyr::Transpiler;
/// let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
/// let result = transpiler.transpile("select(name)");
/// assert!(result.is_ok());
/// ```
pub fn example_function(param1: &str, param2: usize) -> Result<String, Error> {
    // Implementation
}
```

### 코드 구조 원칙
- 함수는 50줄 이하로 제한 (복잡한 로직은 분할)
- 중첩 깊이 4단계 이하 유지
- 매직 넘버 사용 금지 (상수로 정의)
- 불변성(immutability) 우선 적용

## 에러 처리 품질

### 에러 타입 설계
```rust
// 좋은 에러 설계 예시
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ParseError {
    #[error("Unexpected token: expected '{expected}', found '{found}' at position {position}")]
    UnexpectedToken {
        expected: String,
        found: String,
        position: usize,
    },
    
    #[error("Invalid operation '{operation}' at position {position}: {reason}")]
    InvalidOperation {
        operation: String,
        position: usize,
        reason: String,
    },
}
```

### 에러 컨텍스트 보존
```rust
// 에러 체이닝으로 컨텍스트 보존
pub fn parse_complex_expression(&mut self) -> ParseResult<Expr> {
    self.parse_primary_expression()
        .map_err(|e| ParseError::InvalidExpression {
            expr: "complex expression".to_string(),
            position: self.position,
            source: Box::new(e),
        })
}
```

## 테스트 품질 보장

### 테스트 커버리지 목표
- 라인 커버리지 85% 이상
- 브랜치 커버리지 80% 이상
- 모든 공개 함수 테스트 필수
- 에러 경로 테스트 포함

### 테스트 구조화
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // 테스트 헬퍼 함수들
    fn create_test_transpiler() -> Transpiler {
        Transpiler::new(Box::new(PostgreSqlDialect))
    }
    
    fn normalize_sql(sql: &str) -> String {
        sql.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()
    }
    
    // 정상 케이스 테스트
    mod success_cases {
        use super::*;
        
        #[test]
        fn test_simple_select() {
            // 테스트 구현
        }
    }
    
    // 에러 케이스 테스트
    mod error_cases {
        use super::*;
        
        #[test]
        fn test_invalid_syntax() {
            // 에러 테스트 구현
        }
    }
}
```

## 성능 품질 관리

### 벤치마크 기준
- 단순 쿼리 변환: 1ms 이하
- 복잡한 쿼리 변환: 10ms 이하
- 메모리 사용량: 입력 크기의 3배 이하
- 성능 회귀 허용 범위: 5% 이내

### 프로파일링 정기 실행
```rust
// 성능 크리티컬 함수에 대한 벤치마크
#[bench]
fn bench_parse_complex_query(b: &mut Bencher) {
    let transpiler = create_test_transpiler();
    let complex_query = create_complex_test_query();
    
    b.iter(|| {
        black_box(transpiler.transpile(&complex_query))
    });
}
```

## 코드 리뷰 체크리스트

### 기능성 검토
- [ ] 요구사항을 정확히 구현했는가?
- [ ] 모든 에지 케이스를 고려했는가?
- [ ] 에러 처리가 적절한가?
- [ ] 테스트 케이스가 충분한가?

### 코드 품질 검토
- [ ] 코드가 읽기 쉽고 이해하기 쉬운가?
- [ ] 네이밍이 명확하고 일관성 있는가?
- [ ] 중복 코드가 없는가?
- [ ] 성능상 문제가 없는가?

### 보안 검토
- [ ] 입력 검증이 적절한가?
- [ ] SQL 인젝션 가능성은 없는가?
- [ ] 메모리 안전성이 보장되는가?

## 리팩토링 가이드라인

### 리팩토링 시점
- 함수가 50줄을 초과할 때
- 중복 코드가 3회 이상 발견될 때
- 복잡도가 과도하게 높을 때
- 테스트하기 어려운 구조일 때

### 리팩토링 원칙
```rust
// Before: 복잡한 함수
pub fn complex_function(input: &str) -> Result<String, Error> {
    // 100줄의 복잡한 로직
}

// After: 분할된 함수들
pub fn complex_function(input: &str) -> Result<String, Error> {
    let parsed = parse_input(input)?;
    let processed = process_data(parsed)?;
    let result = format_output(processed)?;
    Ok(result)
}

fn parse_input(input: &str) -> Result<ParsedData, Error> {
    // 파싱 로직
}

fn process_data(data: ParsedData) -> Result<ProcessedData, Error> {
    // 처리 로직
}

fn format_output(data: ProcessedData) -> Result<String, Error> {
    // 포맷팅 로직
}
```

## 의존성 관리

### 크레이트 선택 기준
- 활발한 유지보수 (최근 6개월 내 업데이트)
- 충분한 다운로드 수 (월 10만 이상)
- 좋은 문서화
- 보안 취약점 없음

### 의존성 업데이트 정책
- 보안 패치: 즉시 적용
- 마이너 업데이트: 월 1회 검토
- 메이저 업데이트: 분기별 검토
- 호환성 테스트 필수

## 지속적 통합 (CI) 품질 게이트

### 필수 통과 조건
```yaml
# .github/workflows/ci.yml 예시
- name: Run tests
  run: cargo test --all-features
  
- name: Check formatting
  run: cargo fmt -- --check
  
- name: Run clippy
  run: cargo clippy -- -D warnings
  
- name: Check documentation
  run: cargo doc --no-deps --document-private-items
  
- name: Run benchmarks
  run: cargo bench --no-run
```

### 품질 메트릭 모니터링
- 코드 커버리지 추적
- 복잡도 메트릭 모니터링
- 의존성 취약점 스캔
- 성능 회귀 감지