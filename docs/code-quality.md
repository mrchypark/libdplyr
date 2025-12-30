# 코드 품질 가이드

이 문서는 libdplyr DuckDB 확장의 코드 품질 표준과 검사 도구에 대한 가이드입니다.

## 개요

**요구사항 R7-AC4**에 따라 다음과 같은 코드 품질 검사를 수행합니다:
- 코드 커버리지 측정 (70% 이상 목표)
- Rust 및 C++ 정적 분석
- 메모리 누수 검사 (Valgrind 등)
- 보안 취약점 스캔
- 성능 벤치마킹

## 품질 표준

### 코드 커버리지
- **목표**: 70% 이상
- **핵심 로직**: 95% 이상
- **도구**: `cargo-llvm-cov`, Codecov
- **측정 범위**: Rust 단위 테스트, 통합 테스트

### 정적 분석
#### Rust
- **포맷팅**: `rustfmt` (필수)
- **린팅**: `clippy` (모든 경고 해결)
- **보안**: `cargo-audit` (취약점 없음)
- **의존성**: `cargo-deny` (라이선스 호환성)
- **Unsafe 코드**: `cargo-geiger` (최소화)

#### C++
- **표준**: C++17
- **정적 분석**: `cppcheck`, `clang-tidy`
- **포맷팅**: 일관된 스타일 (`.clang-format`)
- **메모리 안전성**: Valgrind, AddressSanitizer

### 성능 기준
- **단순 쿼리**: <2ms (R6-AC1)
- **복잡 쿼리**: <15ms (R6-AC1)
- **확장 로딩**: <50ms (R6-AC2)
- **메모리 증가**: <10% (반복 실행 시)

## 도구 설치

### 자동 설치
```bash
# 모든 품질 도구 설치
./scripts/install-quality-tools.sh
```

### 수동 설치
```bash
# Rust 컴포넌트
rustup component add rustfmt clippy llvm-tools-preview

# Cargo 도구
cargo install cargo-audit cargo-deny cargo-geiger cargo-llvm-cov cargo-outdated

# 시스템 도구 (Ubuntu/Debian)
sudo apt-get install valgrind cppcheck clang-tidy

# 시스템 도구 (macOS)
brew install cppcheck llvm
```

## 품질 검사 실행

### 전체 품질 검사
```bash
# 모든 품질 검사 실행
./scripts/quality-check.sh

# Windows
scripts\quality-check.bat
```

### 개별 검사

#### Rust 품질 검사
```bash
cd libdplyr_c

# 포맷팅 검사
cargo fmt --check

# 린팅
cargo clippy --all-targets --all-features -- -D warnings

# 테스트
cargo test --all-features

# 보안 감사
cargo audit

# 의존성 검사
cargo deny check

# 코드 커버리지
cargo llvm-cov --html --open
```

#### C++ 품질 검사
```bash
# 빌드 (분석 플래그 포함)
mkdir build && cd build
cmake .. -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DBUILD_CPP_TESTS=ON
cmake --build . --parallel

# 정적 분석
cppcheck --enable=all --project=compile_commands.json ../extension/
clang-tidy -p . ../extension/src/*.cpp

# 메모리 검사
valgrind --tool=memcheck --leak-check=full ./duckdb_extension_integration_test
```

#### 성능 벤치마크
```bash
cd libdplyr_c

# 벤치마크 실행
cargo bench

# 특정 벤치마크
cargo bench simple_transpile
cargo bench complex_transpile
```

## CI/CD 통합

### GitHub Actions 워크플로우
- **`ci.yml`**: 기본 빌드 및 테스트
- **`security.yml`**: 보안 스캔
- (성능 벤치마크는 로컬에서 `scripts/run-performance-tests.sh`로 실행)

### 자동 실행 조건
- **Push**: CI/CD + 보안 검사
- **Pull Request**: CI/CD + 보안 검사
- **스케줄**: 보안 검사 (매일)

### 품질 게이트
- 모든 테스트 통과
- 코드 커버리지 70% 이상
- 정적 분석 경고 없음
- 메모리 누수 없음
- 보안 취약점 없음

## 품질 메트릭

### 코드 커버리지
```bash
# HTML 리포트 생성
cargo llvm-cov --html --output-dir coverage-html

# LCOV 형식 (Codecov 업로드용)
cargo llvm-cov --lcov --output-path lcov.info

# 요약 정보
cargo llvm-cov report --summary-only
```

### 정적 분석 메트릭
- **Clippy 경고**: 0개 목표
- **cppcheck 오류**: 0개 목표
- **보안 취약점**: 0개 목표
- **라이선스 위반**: 0개 목표

### 성능 메트릭
```bash
# 벤치마크 결과 JSON 출력
cargo bench -- --output-format json > benchmark-results.json

# 성능 회귀 검사
python scripts/check-performance-regression.py
```

## 문제 해결

### 일반적인 문제

#### 코드 커버리지 낮음
```bash
# 누락된 테스트 식별
cargo llvm-cov report --show-missing-lines

# 특정 모듈 커버리지 확인
cargo llvm-cov report --summary-only --ignore-filename-regex="tests/"
```

#### Clippy 경고
```bash
# 자동 수정 가능한 경고 수정
cargo clippy --fix

# 특정 경고 억제 (최후 수단)
#[allow(clippy::specific_lint)]
```

#### 메모리 누수
```bash
# 상세한 Valgrind 출력
valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes ./your-program

# AddressSanitizer 사용
export RUSTFLAGS="-Z sanitizer=address"
cargo build --target x86_64-unknown-linux-gnu
```

#### 성능 회귀
```bash
# 벤치마크 비교
cargo bench -- --save-baseline main
git checkout feature-branch
cargo bench -- --baseline main
```

### 디버깅 도구

#### 코드 분석
```bash
# 의존성 트리
cargo tree

# 중복 의존성 확인
cargo tree --duplicates

# 빌드 시간 분석
cargo build --timings
```

#### 메모리 분석
```bash
# 힙 프로파일링
valgrind --tool=massif ./your-program

# 캐시 분석
valgrind --tool=cachegrind ./your-program
```

## 설정 파일

### `.clang-tidy`
C++ 정적 분석 규칙 정의
```yaml
Checks: '*,-readability-magic-numbers'
WarningsAsErrors: ''
```

### `.cppcheck`
C++ 정적 분석 설정
```ini
enable=all
inconclusive
suppress=missingIncludeSystem
```

### `codecov.yml`
코드 커버리지 설정
```yaml
coverage:
  range: 70..100
  status:
    project:
      default:
        target: 70%
```

### `libdplyr_c/deny.toml`
의존성 정책 설정
```toml
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause"]
deny = ["GPL-2.0", "GPL-3.0"]
```

## 품질 리포트

### 자동 생성 리포트
- **커버리지 리포트**: `coverage-html/index.html`
- **벤치마크 리포트**: `target/criterion/report/index.html`
- **정적 분석**: CI 아티팩트에서 다운로드

### 수동 리포트 생성
```bash
# 종합 품질 리포트
./scripts/generate-quality-report.sh

# 특정 메트릭 추출
cargo llvm-cov report --json > coverage-report.json
cargo audit --json > security-report.json
```

## 기여자 가이드

### 코드 제출 전 체크리스트
- [ ] `cargo fmt` 실행
- [ ] `cargo clippy` 경고 해결
- [ ] 테스트 추가/업데이트
- [ ] 커버리지 70% 이상 유지
- [ ] 벤치마크 회귀 없음
- [ ] 보안 취약점 없음

### PR 품질 검사
```bash
# PR 제출 전 전체 검사
./scripts/quality-check.sh

# 특정 변경사항 검사
cargo clippy --fix --allow-dirty
cargo test
```

### 지속적 개선
- 정기적인 의존성 업데이트
- 성능 벤치마크 모니터링
- 코드 커버리지 향상
- 정적 분석 규칙 강화

## 참고 자료

### 도구 문서
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- [cargo-audit](https://github.com/RustSec/rustsec/tree/main/cargo-audit)
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
- [Valgrind](https://valgrind.org/docs/manual/)
- [cppcheck](http://cppcheck.sourceforge.net/)

### 품질 표준
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [C++ Core Guidelines](https://isocpp.github.io/CppCoreGuidelines/)
- [Google C++ Style Guide](https://google.github.io/styleguide/cppguide.html)

### 성능 최적화
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)

이 가이드를 따라 높은 품질의 코드를 유지하고 지속적으로 개선해나가세요.
