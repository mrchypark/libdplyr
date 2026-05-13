# DuckDB Extension Smoke Tests

이 문서는 DuckDB dplyr 확장의 스모크 테스트에 대한 설명입니다.

## 개요

스모크 테스트는 다음 요구사항을 검증합니다:
- **R4-AC2**: 기본 확장 기능 및 로딩 테스트
- **R1-AC2**: 최소 연산 집합 지원 (select, filter, mutate, arrange, group_by, summarise)
- **R5-AC1**: `%>%` 파이프라인 기반 진입점 테스트
- **Pipe syntax 설정**: `dplyr_pipe_syntax`, `|>` native pipe, 명시적 pipe 인자 테스트
- **R2-AC1**: 테이블 함수 진입점 테스트
- **R2-AC2**: 표준 SQL과의 혼용 테스트
- **R5-AC2**: 파서 충돌/오인식 방지 테스트

## 테스트 구조

### 테스트 파일
- `tests/smoke.sql`: 메인 스모크 테스트 SQL 파일
- `tests/run_smoke_tests.sh`: Linux/macOS 테스트 실행 스크립트
- `tests/run_smoke_tests.bat`: Windows 테스트 실행 스크립트

### 테스트 카테고리

#### 1. Extension Loading and Basic Verification (R4-AC2)
- 확장 로딩 성공 테스트
- 기본 SQL 기능 간섭 없음 확인
- 표준 SQL 함수 정상 동작 확인

#### 2. Implicit Pipeline Entry Point (R5-AC1)
- `%>%` 파이프라인 구문 인식 테스트
- 기본 select 연산 테스트
- 컬럼 이름 변경 테스트

#### 3. Minimum Operation Set (R1-AC2)
- **select**: 컬럼 선택 및 이름 변경
- **filter**: 조건부 행 필터링
- **mutate**: 새 컬럼 생성 및 변환
- **arrange**: 정렬 (오름차순/내림차순)
- **group_by**: 그룹화
- **summarise**: 집계 함수 (mean, count, sum 등)

#### 4. Table Function Entry Point (R2-AC1)
- `SELECT * FROM dplyr('code')` 구문 테스트
- `SELECT * FROM dplyr('code', 'native')` 명시적 pipe syntax 테스트
- 서브쿼리 컨텍스트에서 사용 테스트

#### 5. Chained Operations (Pipeline Testing)
- 단순 파이프라인: select + filter
- 복잡한 파이프라인: 모든 연산 조합
- 실제 데이터 처리 시나리오

#### 6. Standard SQL Integration (R2-AC2)
- CTE와 dplyr 혼용
- 서브쿼리에서 dplyr 사용
- JOIN과 dplyr 결과 혼용

#### 7. Error Handling and Edge Cases (R1-AC3, R7-AC3)
- 잘못된 dplyr 구문 처리
- 빈 입력 처리
- NULL 입력 처리
- 의미 있는 에러 메시지 확인

#### 8. Parser Collision Avoidance (R5-AC2)
- `%>%` 파이프라인 인식이 일반 SQL을 오인식하지 않음

#### 9. Pipe Syntax Configuration
- `SET dplyr_pipe_syntax = 'native'` 세션 설정 테스트
- 명시적 pipe 인자와 세션 기본값을 통한 native pipe table function 테스트
- `SET dplyr_pipe_syntax = 'magrittr'` 전환 후 기존 경로 유지 확인
- magrittr 람다 RHS 변형 테스트: `{ . %>% ... }`, `{ filter(., ...) %>% ... }`, `(. %>% ...)`, RHS dot placeholder
- native 람다 RHS 변형 테스트: `\(x)` 람다, 명시적 data 인자, 세션 기본 native pipe 설정

#### 10. Performance and Stability (R6-AC1)
- 중간 복잡도 쿼리 실행
- 반복 실행 안정성
- 캐싱 동작 확인

## 실행 방법

### 전제 조건
1. DuckDB CLI 설치
2. 확장 빌드 완료 (`build/dplyr.duckdb_extension`)
3. 실행 권한 설정 (Linux/macOS)

### 스크립트 실행

#### Linux/macOS
```bash
# 전체 스모크 테스트 실행
./tests/run_smoke_tests.sh

# 빌드 디렉토리 지정
BUILD_DIR=my_build ./tests/run_smoke_tests.sh
```

#### Windows
```cmd
REM 전체 스모크 테스트 실행
tests\run_smoke_tests.bat

REM 빌드 디렉토리 지정
set BUILD_DIR=my_build
tests\run_smoke_tests.bat
```

#### CMake 테스트 실행
```bash
cd build

# 모든 스모크 테스트
ctest -R smoke

# 특정 스모크 테스트
ctest -R smoke_test_comprehensive
ctest -R smoke_test_minimum_operations
ctest -R smoke_test_dplyr_keyword

# 개발 타겟 사용
make smoke-test          # 전체 스모크 테스트
make smoke-test-quick    # 빠른 로딩 테스트만
```

#### 수동 실행
```bash
cd build
export DUCKDB_EXTENSION_PATH=$(pwd)
duckdb :memory: < ../tests/smoke.sql
```

## 테스트 결과 해석

### 성공 시나리오
```
✓ Extension loaded successfully
✓ Basic SQL functionality: SUCCESS
✓ Core functionality verified
🎉 Smoke Tests: SUCCESS
```

### 부분 성공 시나리오 (구현 진행 중)
```
✓ Extension loading: SUCCESS
⚠ dplyr 파이프라인/테이블 함수 테스트가 FAIL 하면 에러 메시지 확인
✓ Standard SQL tests should PASS (no interference)
```

### 실패 시나리오
```
✗ Extension loading: FAILED
❌ Smoke Tests: ISSUES DETECTED
```

## 구현 단계별 예상 결과

### 1단계: 확장 구조만 구현
- ✅ 확장 로딩 성공
- ✅ DPLYR 키워드 거부 (의도된 동작)
- ✅ 표준 SQL 정상 동작

### 2단계: 파서 확장 구현
- ✅ 확장 로딩 성공
- ✅ `%>%` 파이프라인 인식
- ❌ 실제 변환 실패 (graceful)
- ✅ 표준 SQL 정상 동작

### 3단계: 기본 연산 구현
- ✅ 확장 로딩 성공
- ✅ `%>%` 파이프라인 인식
- ✅ 기본 select, filter 동작
- ❌ 복잡한 연산 실패 (graceful)
- ✅ 표준 SQL 정상 동작

### 4단계: 전체 구현 완료
- ✅ 모든 테스트 성공
- ✅ 에러 처리 완벽
- ✅ 성능 요구사항 충족

## 디버깅 가이드

### 확장 로딩 실패
```bash
# 확장 파일 존재 확인
ls -la build/dplyr.duckdb_extension

# 수동 로딩 테스트
duckdb -c "LOAD 'build/dplyr.duckdb_extension';"

# 의존성 확인 (Linux)
ldd build/dplyr.duckdb_extension

# 의존성 확인 (macOS)
otool -L build/dplyr.duckdb_extension
```

### dplyr 기능 테스트 실패
```bash
# 디버그 모드 활성화
export DPLYR_DEBUG=1
duckdb -c "LOAD 'build/dplyr.duckdb_extension'; CREATE TABLE __dplyr_test(x INTEGER); INSERT INTO __dplyr_test VALUES (1); SELECT * FROM dplyr('__dplyr_test %>% select(x)');"

# 개별 테스트 실행
duckdb -c "
LOAD 'build/dplyr.duckdb_extension';
CREATE TABLE test AS SELECT 1 as id;
SELECT * FROM dplyr('test %>% select(id)');
"
```

### 표준 SQL 간섭 문제
```bash
# 키워드 충돌 테스트
duckdb -c "
LOAD 'build/dplyr.duckdb_extension';
CREATE TABLE dplyr AS SELECT 1 as test;
SELECT test FROM dplyr;
"
```

### 성능 문제 진단
```bash
# 시간 측정
time duckdb -c "
LOAD 'build/dplyr.duckdb_extension';
CREATE TABLE perf_test AS SELECT i as id FROM range(1, 100000) as t(i);
SELECT * FROM dplyr('perf_test %>% filter(id > 10) %>% select(id)');
"

# 메모리 사용량 확인
valgrind --tool=massif duckdb -c "LOAD 'build/dplyr.duckdb_extension';"
```

## 환경 변수

- `BUILD_DIR`: 빌드 디렉토리 경로 (기본값: build)
- `DUCKDB_EXTENSION_PATH`: 확장 파일 경로 (자동 설정)
- `DPLYR_DEBUG`: 디버그 로깅 활성화 (1=활성화)
- `TEST_TIMEOUT`: 테스트 타임아웃 (초, 기본값: 60)

## CI/CD 통합

### GitHub Actions 예시
```yaml
- name: Run Smoke Tests
  run: |
    chmod +x tests/run_smoke_tests.sh
    ./tests/run_smoke_tests.sh
  env:
    BUILD_DIR: build
    DPLYR_DEBUG: 1

- name: Upload Test Results
  if: always()
  uses: actions/upload-artifact@v3
  with:
    name: smoke-test-results
    path: |
      smoke_test_*.log
      test_results.xml
```

### 테스트 결과 분석
```bash
# 테스트 로그 분석
grep -E "(✓|✗|ERROR|FAIL)" smoke_test.log

# 성능 메트릭 추출
grep -E "took [0-9]+ms" smoke_test.log

# 에러 패턴 분석
grep -E "E-[A-Z]+" smoke_test.log
```

## 확장 및 커스터마이징

### 새로운 테스트 추가
```sql
-- tests/smoke.sql에 추가
-- Test XX: New functionality test
statement maybe
SELECT * FROM dplyr('my_table %>% select(*)');
```

### 테스트 카테고리 추가
```bash
# run_smoke_tests.sh에 새 함수 추가
run_new_category_tests() {
    echo "Running new category tests..."
    # 테스트 로직
}
```

### 플랫폼별 테스트
```cmake
# CMakeLists.txt에 플랫폼별 테스트 추가
if(WIN32)
    add_test(NAME smoke_test_windows_specific ...)
elseif(APPLE)
    add_test(NAME smoke_test_macos_specific ...)
endif()
```

## 문제 보고

스모크 테스트 실패 시 다음 정보를 포함하여 이슈를 보고해주세요:

1. 운영체제 및 버전
2. DuckDB 버전 (`duckdb --version`)
3. 빌드 환경 (CMake, 컴파일러 버전)
4. 전체 테스트 로그
5. 실패한 특정 테스트 케이스
6. 재현 단계

## 참고 자료

- [DuckDB SQL Reference](https://duckdb.org/docs/sql/introduction)
- [DuckDB Extension Development](https://duckdb.org/docs/extensions/overview)
- [libdplyr Requirements](../specs/duckdb-extension/requirements.md)
- [libdplyr Design](../specs/duckdb-extension/design.md)
- [dplyr R Package Documentation](https://dplyr.tidyverse.org/)
