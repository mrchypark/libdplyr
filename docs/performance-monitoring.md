# 성능 모니터링 가이드

## 개요

이 문서는 DuckDB dplyr 확장의 성능 모니터링 시스템을 설명합니다. R6-AC1 및 R6-AC2 요구사항에 따라 성능 목표 달성을 지속적으로 검증하고 성능 회귀를 방지합니다.

## 성능 목표 (R6-AC1)

### 핵심 성능 지표
- **단순 쿼리**: P95 < 2ms
- **복잡 쿼리**: P95 < 15ms  
- **확장 로딩**: P95 < 50ms

### 캐싱 효율성 (R6-AC2)
- 캐시 히트가 캐시 미스보다 최소 2배 빠름
- 반복 쿼리에 대한 효과적인 성능 향상
- 메모리 사용량 최적화

## 성능 테스트 구조

### 1. 단위 성능 테스트
```rust
// libdplyr_c/benches/transpile_benchmark.rs
#[test]
fn test_simple_query_performance_target() {
    // P95 < 2ms 검증
}

#[test] 
fn test_complex_query_performance_target() {
    // P95 < 15ms 검증
}

#[test]
fn test_cache_effectiveness() {
    // 캐시 효율성 검증
}
```

### 2. 벤치마크 카테고리

#### 변환 성능 벤치마크
- **단순 쿼리**: 기본 dplyr 연산 (select, filter, mutate 등)
- **복잡 쿼리**: 3-5단계 파이프라인
- **에러 처리**: 잘못된 입력 처리 성능
- **캐싱**: 캐시 히트 vs 미스 비교
- **옵션 영향**: 다양한 설정의 성능 영향
- **메모리 패턴**: 소/중/대형 쿼리 성능
- **동시성 시뮬레이션**: 빠른 전환 및 혼합 워크로드

#### 확장 로딩 벤치마크
- **콜드 로딩**: 새로운 DuckDB 인스턴스마다
- **웜 로딩**: 연결 재사용으로 여러 번 로딩
- **사용과 함께 로딩**: 로딩 후 즉시 기능 테스트
- **초기화 오버헤드**: 확장 유무 비교

## 자동화된 성능 모니터링

### GitHub Actions 워크플로우

#### 1. 성능 테스트 워크플로우 (`.github/workflows/performance.yml`)
```yaml
# 트리거
- push: main, develop 브랜치
- pull_request: main 브랜치  
- schedule: 매일 오전 2시 UTC
- workflow_dispatch: 수동 실행

# 플랫폼
- Linux x86_64
- macOS ARM64
- Windows x86_64
```

#### 2. 실행 단계
1. **환경 설정**: Rust, CMake, DuckDB CLI
2. **빌드**: Release 모드로 컴포넌트 빌드
3. **성능 검증**: 단위 테스트로 목표 달성 확인
4. **벤치마크 실행**: Criterion으로 상세 측정
5. **결과 처리**: JSON 및 HTML 리포트 생성
6. **회귀 검사**: 이전 결과와 비교
7. **리포트 생성**: 종합 성능 리포트

### 로컬 성능 테스트

#### 스크립트 실행
```bash
# 전체 성능 테스트 실행
./scripts/run-performance-tests.sh

# 사용자 정의 설정
BENCHMARK_DURATION=60 SAMPLE_SIZE=2000 ./scripts/run-performance-tests.sh

# 결과 확인
open benchmark-results/criterion/transpile_benchmark/report/index.html
```

#### 개별 벤치마크 실행
```bash
cd libdplyr_c

# 변환 성능 벤치마크
cargo bench --bench transpile_benchmark

# 확장 로딩 벤치마크  
cargo bench --bench extension_loading_benchmark

# 특정 벤치마크만 실행
cargo bench --bench transpile_benchmark -- simple_transpile
```

## 성능 분석 및 해석

### 벤치마크 결과 해석

#### Criterion 출력 이해
```
simple_transpile/simple/0  time:   [1.2345 ms 1.2567 ms 1.2789 ms]
                          change: [-2.1% -1.5% -0.9%] (p = 0.00 < 0.05)
                          Performance has improved.
```

- **시간 범위**: [최소값 평균값 최대값]
- **변화율**: 이전 실행 대비 성능 변화
- **통계적 유의성**: p-value로 변화의 신뢰도

#### 성능 목표 달성 확인
```bash
# 성능 검증 테스트 실행
cargo test --release performance_tests -- --nocapture

# 출력 예시:
# Simple query P95: 1.8ms ✅ (target: 2ms)
# Complex query P95: 12.3ms ✅ (target: 15ms)  
# Extension loading P95: 42ms ✅ (target: 50ms)
```

### 성능 회귀 감지

#### 회귀 기준
- **경미한 회귀**: 5-10% 성능 저하 → 경고
- **심각한 회귀**: 10% 이상 성능 저하 → 실패
- **목표 위반**: 성능 목표 초과 → 실패

#### 회귀 대응 절차
1. **즉시 조치**: CI/CD 파이프라인에서 실패 알림
2. **원인 분석**: 성능 프로파일링으로 병목점 식별
3. **수정 적용**: 성능 최적화 또는 코드 수정
4. **재검증**: 수정 후 성능 테스트 재실행

## 성능 최적화 가이드

### 일반적인 최적화 영역

#### 1. 변환 성능 최적화
```rust
// 문자열 할당 최소화
fn optimize_string_handling() {
    // ❌ 비효율적
    let result = format!("SELECT {}", columns.join(", "));
    
    // ✅ 효율적  
    let mut result = String::with_capacity(estimated_size);
    result.push_str("SELECT ");
    result.push_str(&columns.join(", "));
}

// 캐시 활용 최적화
fn optimize_caching() {
    // 캐시 키 최적화
    // 캐시 크기 조정
    // 캐시 만료 정책
}
```

#### 2. 메모리 사용량 최적화
```rust
// 메모리 할당 패턴 최적화
fn optimize_memory_allocation() {
    // 사전 할당으로 재할당 방지
    // 불필요한 복사 제거
    // 스택 할당 우선 사용
}
```

#### 3. 확장 로딩 최적화
```cpp
// 초기화 시간 최적화
void optimize_extension_loading() {
    // 지연 초기화 적용
    // 필수 컴포넌트만 사전 로딩
    // 병렬 초기화 고려
}
```

### 성능 프로파일링

#### Rust 프로파일링 도구
```bash
# CPU 프로파일링
cargo install flamegraph
cargo flamegraph --bench transpile_benchmark

# 메모리 프로파일링  
cargo install heaptrack
heaptrack target/release/deps/transpile_benchmark-*

# 상세 분석
cargo install cargo-profdata
cargo profdata -- merge -sparse default.profraw -o default.profdata
```

#### 성능 병목점 식별
1. **CPU 집약적 작업**: 파싱, AST 변환, SQL 생성
2. **메모리 할당**: 문자열 생성, 데이터 구조 복사
3. **I/O 작업**: 파일 읽기, 네트워크 통신 (해당 시)
4. **동기화**: 캐시 접근, 스레드 안전성

## 성능 모니터링 대시보드

### 메트릭 수집

#### 핵심 지표
```json
{
  "timestamp": "2024-08-29T12:00:00Z",
  "commit": "abc123",
  "platform": "linux-x86_64",
  "metrics": {
    "simple_query_p95_ms": 1.8,
    "complex_query_p95_ms": 12.3,
    "extension_loading_p95_ms": 42.0,
    "cache_hit_ratio": 0.85,
    "cache_effectiveness_ratio": 2.3
  },
  "targets": {
    "simple_query_target_ms": 2.0,
    "complex_query_target_ms": 15.0,
    "extension_loading_target_ms": 50.0
  },
  "status": "PASS"
}
```

#### 트렌드 분석
- **성능 추세**: 시간에 따른 성능 변화
- **플랫폼 비교**: 다양한 OS/아키텍처 간 성능 차이
- **버전 비교**: 릴리스 간 성능 변화
- **회귀 패턴**: 성능 저하 발생 빈도 및 원인

### 알림 시스템

#### 알림 조건
- **성능 목표 위반**: 즉시 알림
- **심각한 회귀**: 10% 이상 성능 저하
- **연속 실패**: 3회 연속 성능 테스트 실패
- **플랫폼 불일치**: 플랫폼 간 큰 성능 차이

#### 알림 채널
- **GitHub Issues**: 자동 이슈 생성
- **PR 코멘트**: 풀 리퀘스트에 성능 결과 코멘트
- **Slack/Discord**: 팀 채널 알림
- **이메일**: 중요한 회귀 시 이메일 알림

## 성능 테스트 모범 사례

### 테스트 설계 원칙

#### 1. 현실적인 워크로드
```rust
// 실제 사용 패턴 반영
const REALISTIC_QUERIES: &[&str] = &[
    "select(name, age, salary)",  // 일반적인 컬럼 선택
    "filter(age > 25 & salary > 50000)",  // 복합 조건
    "group_by(department) %>% summarise(avg_salary = mean(salary))",  // 집계
];
```

#### 2. 통계적 유의성
```rust
// 충분한 샘플 크기와 반복
group.significance_level(0.1)
     .sample_size(1000)
     .measurement_time(Duration::from_secs(30));
```

#### 3. 환경 일관성
```bash
# 동일한 환경에서 테스트
export RUST_LOG=error  # 로깅 최소화
export CRITERION_SAMPLE_SIZE=1000
export CRITERION_MEASUREMENT_TIME=30
```

### 성능 테스트 유지보수

#### 정기 검토
- **월간**: 성능 목표 및 기준 검토
- **분기별**: 벤치마크 쿼리 업데이트
- **반기별**: 성능 테스트 전략 재평가

#### 테스트 업데이트
- **새 기능**: 새로운 dplyr 연산 추가 시 벤치마크 추가
- **성능 최적화**: 최적화 후 기준선 업데이트
- **플랫폼 지원**: 새 플랫폼 지원 시 테스트 확장

## 문제 해결

### 일반적인 성능 문제

#### 1. 성능 목표 미달성
```bash
# 문제 진단
cargo bench --bench transpile_benchmark -- --profile-time=60

# 프로파일링
cargo flamegraph --bench transpile_benchmark

# 최적화 적용 후 재테스트
cargo bench --bench transpile_benchmark
```

#### 2. 성능 회귀 발생
```bash
# 이전 커밋과 비교
git bisect start
git bisect bad HEAD
git bisect good <last-good-commit>

# 각 커밋에서 성능 테스트
cargo bench --bench transpile_benchmark
```

#### 3. 플랫폼 간 성능 차이
```bash
# 플랫폼별 상세 분석
cargo bench --bench transpile_benchmark -- --save-baseline platform-baseline

# 결과 비교
cargo bench --bench transpile_benchmark -- --load-baseline platform-baseline
```

### 성능 디버깅 도구

#### Rust 도구
- **flamegraph**: CPU 프로파일링
- **heaptrack**: 메모리 프로파일링  
- **perf**: 시스템 레벨 분석
- **valgrind**: 메모리 누수 검사

#### 시스템 도구
- **htop**: CPU/메모리 사용량 모니터링
- **iotop**: I/O 사용량 분석
- **strace**: 시스템 호출 추적

## 참고 자료

### 문서
- [Criterion.rs 사용자 가이드](https://bheisler.github.io/criterion.rs/book/)
- [Rust 성능 최적화 가이드](https://nnethercote.github.io/perf-book/)
- [DuckDB 확장 성능 가이드](https://duckdb.org/docs/extensions/overview)

### 도구
- [Criterion.rs](https://github.com/bheisler/criterion.rs) - 벤치마킹 프레임워크
- [flamegraph](https://github.com/flamegraph-rs/flamegraph) - CPU 프로파일링
- [cargo-profdata](https://github.com/Kobzol/cargo-profdata) - 프로파일 데이터 분석

### 성능 기준
- R6-AC1: 성능 목표 (2ms/15ms/50ms P95)
- R6-AC2: 캐싱 효율성 측정
- 업계 표준: 데이터베이스 확장 성능 벤치마크