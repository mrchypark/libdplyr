---
inclusion: always
---

# 성능 최적화 가이드라인

## 메모리 관리 원칙

### 문자열 처리 최적화
- `String::clone()` 대신 `&str` 참조 활용
- 불필요한 문자열 할당 최소화
- `format!` 매크로 사용 시 미리 용량 예측하여 `String::with_capacity()` 사용

```rust
// 좋은 예시
fn generate_identifier(&self, name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 2);
    result.push('"');
    result.push_str(name);
    result.push('"');
    result
}

// 피해야 할 예시
fn generate_identifier_bad(&self, name: &str) -> String {
    format!("\"{}\"", name) // 매번 새로운 할당
}
```

### AST 노드 최적화
- `Box<T>` 사용으로 스택 오버플로우 방지
- `Rc<T>` 또는 `Arc<T>` 사용으로 공유 데이터 최적화
- 큰 구조체는 참조로 전달

### 벡터 및 컬렉션 최적화
- 예상 크기를 알 때는 `Vec::with_capacity()` 사용
- 반복자(Iterator) 체이닝 활용으로 중간 할당 제거
- `collect()` 호출 최소화

## 파싱 성능 최적화

### 토큰화 최적화
- 문자별 매칭 대신 패턴 매칭 활용
- 미리 계산된 키워드 해시맵 사용
- 불필요한 문자 복사 방지

```rust
// 최적화된 키워드 매칭
lazy_static! {
    static ref KEYWORDS: HashMap<&'static str, Token> = {
        let mut m = HashMap::new();
        m.insert("select", Token::Select);
        m.insert("filter", Token::Filter);
        m.insert("mutate", Token::Mutate);
        // ...
        m
    };
}

fn match_keyword(identifier: &str) -> Option<Token> {
    KEYWORDS.get(identifier).cloned()
}
```

### 파서 최적화
- 재귀 깊이 제한으로 스택 오버플로우 방지
- 백트래킹 최소화
- 미리 계산된 우선순위 테이블 사용

## SQL 생성 최적화

### 문자열 빌더 패턴
```rust
struct SqlBuilder {
    query: String,
    capacity_hint: usize,
}

impl SqlBuilder {
    fn new(capacity_hint: usize) -> Self {
        Self {
            query: String::with_capacity(capacity_hint),
            capacity_hint,
        }
    }
    
    fn append_select(&mut self, columns: &[String]) -> &mut Self {
        self.query.push_str("SELECT ");
        self.query.push_str(&columns.join(", "));
        self
    }
    
    fn append_from(&mut self, table: &str) -> &mut Self {
        self.query.push_str("\nFROM ");
        self.query.push_str(table);
        self
    }
}
```

### 방언별 최적화
- 방언별 특화된 최적화 규칙 적용
- 캐싱 가능한 변환 결과 저장
- 방언별 함수 매핑 테이블 미리 계산

## 벤치마킹 및 프로파일링

### 성능 측정 포인트
- 전체 변환 시간
- 단계별 처리 시간 (렉싱, 파싱, 생성)
- 메모리 사용량
- 입력 크기별 확장성

### 성능 회귀 방지
- CI/CD에서 벤치마크 자동 실행
- 성능 저하 임계값 설정 (예: 10% 이상 느려지면 경고)
- 메모리 사용량 모니터링

### 프로파일링 도구 활용
```bash
# CPU 프로파일링
cargo bench --bench transpile_benchmark

# 메모리 프로파일링
valgrind --tool=massif target/release/libdplyr

# Rust 전용 프로파일러
cargo install flamegraph
cargo flamegraph --bench transpile_benchmark
```

## 대용량 입력 처리

### 스트리밍 처리
- 큰 입력 파일에 대한 청크 단위 처리
- 메모리 사용량 제한 설정
- 점진적 파싱 지원

### 병렬 처리 고려사항
- 독립적인 쿼리들의 병렬 변환
- 스레드 안전성 보장
- 공유 상태 최소화

## 컴파일 시간 최적화

### 의존성 관리
- 필요한 기능만 활성화 (feature flags)
- 컴파일 시간이 긴 크레이트 사용 최소화
- 조건부 컴파일 활용

### 모듈 구조 최적화
- 순환 의존성 방지
- 인터페이스와 구현 분리
- 제네릭 사용 최적화