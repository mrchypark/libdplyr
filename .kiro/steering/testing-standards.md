---
inclusion: always
---

# 테스트 표준 및 품질 보증

## 테스트 작성 원칙

### 단위 테스트 가이드라인
- 각 모듈의 공개 함수는 최소 1개 이상의 테스트 케이스 필요
- 정상 케이스와 에러 케이스 모두 테스트
- 테스트 함수명은 `test_` 접두사 + 기능 설명 (영어)
- `assert!`, `assert_eq!`, `assert_ne!` 매크로 적극 활용

### 통합 테스트 구조
- `tests/integration_tests.rs`에서 전체 파이프라인 테스트
- 각 SQL 방언별로 동일한 dplyr 코드 테스트
- 복잡한 체이닝 연산 테스트 포함
- 에러 시나리오 테스트 (잘못된 문법, 빈 입력 등)

### 벤치마크 테스트
- `benches/transpile_benchmark.rs`에서 성능 측정
- 입력 크기별 성능 비교 (작은/중간/큰 입력)
- 방언별 성능 비교
- 파싱 단계별 성능 분석

## 테스트 데이터 관리

### 테스트 케이스 예시
```rust
// 좋은 테스트 케이스 예시
#[test]
fn test_simple_select_with_filter() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "select(name, age) %>% filter(age > 18)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok(), "변환이 성공해야 합니다: {:?}", result);
    
    let sql = result.unwrap();
    let normalized = normalize_sql(&sql);
    
    assert!(normalized.contains("SELECT"));
    assert!(normalized.contains("WHERE"));
    assert!(normalized.contains("\"AGE\" > 18"));
}
```

### 에러 테스트 패턴
```rust
#[test]
fn test_invalid_syntax_error() {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "invalid_function(test)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_err(), "잘못된 문법은 오류를 반환해야 합니다");
    
    match result.unwrap_err() {
        TranspileError::ParseError(_) => {}, // 예상된 에러 타입
        other => panic!("예상치 못한 에러 타입: {:?}", other),
    }
}
```

## 코드 커버리지 목표
- 전체 코드 커버리지 80% 이상 유지
- 핵심 변환 로직은 95% 이상 커버리지
- 에러 처리 경로도 테스트에 포함

## 테스트 실행 명령어
- `cargo test` - 모든 테스트 실행
- `cargo test --lib` - 라이브러리 테스트만 실행
- `cargo test --test integration_tests` - 통합 테스트만 실행
- `cargo bench` - 벤치마크 실행