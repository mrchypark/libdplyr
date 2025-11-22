# GitHub Actions 문제 추적 보고서

생성 시간: 2025-11-23 01:19 KST
최신 실행 ID: 19598023xxx (2025-11-22T16:12:39Z)

## 🔴 현재 실패 중인 워크플로우

### 1. CI/CD Pipeline - FAILED ❌
**Run ID**: 19598023028
**주요 문제**: CMake 설정 실패

```
CMake Error at extension_config.cmake:212 (duckdb_extension_load):
  Unknown CMake command "duckdb_extension_load".
```

**원인**: `BUILD_DUCKDB=OFF`로 설정되어 있어서 DuckDB extension helper 함수들이 로드되지 않음
**영향**: 모든 플랫폼 (ubuntu, macos, windows)에서 빌드 실패

---

### 2. Performance Testing - FAILED ❌
**Run ID**: 19598023032
**주요 문제**: 벤치마크 import 오류

```
error[E0432]: unresolved import `libdplyr_c`
  --> libdplyr_c/benches/transpile_benchmark.rs:13:5
   |
13 | use libdplyr_c::{dplyr_compile, dplyr_free_string, DplyrOptions};
   |     ^^^^^^^^^^ use of unresolved module or unlinked crate `libdplyr_c`
```

**제안**: `use libdplyr::{...}` 사용
**원인**: Cargo.toml의 package name이 `libdplyr_c`이지만, 벤치마크에서는 다른 이름으로 참조해야 함

---

### 3. Performance Benchmarks - FAILED ❌
**Run ID**: 19598023030
**주요 문제**: 동일한 벤치마크 import 오류

---

### 4. Security Checks - FAILED ❌
**Run ID**: 19598023026
**주요 문제**: deny.toml 설정 오류

```
error[unexpected-value]: expected '["all", "workspace", "transitive", "none"]'
   ┌─ /home/runner/work/libdplyr/libdplyr/libdplyr_c/deny.toml:22:17
   │
22 │ unmaintained = "warn"
   │                 ━━━━ unexpected value
```

**원인**: deny.toml의 `unmaintained` 필드가 잘못된 값 사용
**수정 필요**: `unmaintained`는 배열 값이 필요함

---

### 5. Code Quality Analysis - IN PROGRESS ⏳
**Run ID**: 19598023023
**상태**: 아직 실행 중

---

## 📊 문제 분류

### A. 빌드 시스템 문제 (Critical)
1. **CMake 설정 오류** - `duckdb_extension_load` 함수 없음
   - 파일: `extension_config.cmake:212`
   - 원인: `BUILD_DUCKDB=OFF` 설정
   - 해결: CI 워크플로우에서 `BUILD_DUCKDB=ON`으로 변경 필요

### B. Rust 벤치마크 문제 (Critical)
1. **Crate 이름 불일치**
   - 파일: `libdplyr_c/benches/transpile_benchmark.rs:13`
   - 현재: `use libdplyr_c::{...}`
   - 필요: crate 이름 확인 및 수정

### C. 보안 설정 문제 (Medium)
1. **deny.toml 설정 오류**
   - 파일: `libdplyr_c/deny.toml:22`
   - 현재: `unmaintained = "warn"`
   - 필요: 올바른 형식으로 수정

---

## 🔍 근본 원인 분석

### 1. 벤치마크 Import 문제의 근본 원인

**문제**: `libdplyr_c` crate를 벤치마크에서 import할 수 없음

**조사 필요 사항**:
- `libdplyr_c/Cargo.toml`의 `[package] name` 확인
- `[lib] name` 설정 확인
- 벤치마크가 어떤 crate 이름을 사용해야 하는지 확인

**가능한 해결책**:
1. Cargo.toml에 `[lib] name = "libdplyr_c"` 추가
2. 또는 벤치마크에서 올바른 crate 이름 사용
3. 또는 `extern crate` 사용

### 2. CMake 문제의 근본 원인

**문제**: DuckDB extension helper 함수들이 로드되지 않음

**원인**:
- CI 워크플로우가 `BUILD_DUCKDB=OFF`로 설정
- 이는 DuckDB를 빌드하지 않지만, extension helper도 로드하지 않음

**해결책**:
- `BUILD_DUCKDB=ON`으로 변경
- 또는 extension helper만 별도로 로드하는 방법 찾기

---

## ✅ 해결 우선순위

### Priority 1 (즉시 수정 필요)
1. ⚠️ **CMake 빌드 설정 수정**
   - 파일: `.github/workflows/ci.yml`
   - 변경: `BUILD_DUCKDB=OFF` → `BUILD_DUCKDB=ON`
   - 또는: `extension-ci-tools` 서브모듈 업데이트

2. ⚠️ **벤치마크 crate 이름 수정**
   - 파일: `libdplyr_c/benches/transpile_benchmark.rs`
   - 조사: 올바른 crate 이름 확인
   - 수정: import 문 수정

### Priority 2 (중요)
3. 🔧 **deny.toml 설정 수정**
   - 파일: `libdplyr_c/deny.toml`
   - 수정: `unmaintained` 필드 올바른 형식으로 변경

### Priority 3 (개선)
4. 📝 **CI 워크플로우 검증**
   - 로컬에서 CI와 동일한 설정으로 빌드 테스트
   - pre-commit hook에 CMake 빌드 체크 추가

---

## 🛠️ 권장 조치 사항

### 즉시 조치
```bash
# 1. Cargo.toml 확인
cat libdplyr_c/Cargo.toml | grep -A 5 "\[package\]"
cat libdplyr_c/Cargo.toml | grep -A 5 "\[lib\]"

# 2. deny.toml 확인
cat libdplyr_c/deny.toml | grep -A 2 "unmaintained"

# 3. CI 워크플로우 확인
grep -n "BUILD_DUCKDB" .github/workflows/ci.yml
```

### 수정 후 검증
```bash
# 로컬에서 벤치마크 컴파일 테스트
cd libdplyr_c
cargo bench --no-run

# CMake 빌드 테스트
mkdir -p build_test
cd build_test
cmake .. -DCMAKE_BUILD_TYPE=Release -DBUILD_CPP_TESTS=ON
```

---

## 📈 진행 상황 추적

### 이전 시도들
1. ❌ `crate::` 사용 → 함수를 찾을 수 없음
2. ❌ `libdplyr::` 사용 → crate에 함수가 export되지 않음
3. ❌ `libdplyr_c::` 사용 → crate를 찾을 수 없음

### 다음 시도
1. Cargo.toml 분석하여 정확한 crate 이름 확인
2. 필요시 Cargo.toml에 `[lib] name` 추가
3. CMake 빌드 설정 수정

---

## 🔄 재발 방지

### 추가할 체크
1. **Pre-commit hook에 추가**:
   - `cargo bench --no-run` (벤치마크 컴파일 체크)
   - CMake 설정 검증

2. **CI 개선**:
   - 빌드 실패 시 더 명확한 에러 메시지
   - 로컬 재현 가능한 빌드 스크립트 제공

3. **문서화**:
   - Cargo.toml 설정 가이드
   - 벤치마크 작성 가이드
   - CMake 빌드 옵션 설명

---

## 📝 참고 사항

### Cargo Bench Import 규칙
- 벤치마크 파일은 별도 바이너리
- `Cargo.toml`의 `[package] name`이 crate 이름
- `[lib] name`이 설정되면 그것이 import 이름
- `[lib] crate-type`이 `["staticlib", "cdylib"]`이면 Rust crate로 사용 불가능할 수 있음

### 해결 방향
1. `[lib] name` 명시적 설정
2. 또는 `crate-type`에 `"rlib"` 추가
3. 또는 벤치마크를 별도 crate로 분리
