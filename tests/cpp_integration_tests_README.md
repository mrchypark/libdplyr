# DuckDB Extension C++ Integration Tests

이 문서는 DuckDB dplyr 확장의 C++ 통합 테스트에 대한 설명입니다.

## 개요

C++ 통합 테스트는 다음 요구사항을 검증합니다:
- **R7-AC1**: DuckDB 확장 로딩 및 기능 테스트
- **R7-AC3**: 크래시 방지 및 에러 처리 테스트  
- **R2-AC2**: 표준 SQL과의 혼용 테스트
- **R4-AC2**: 스모크 테스트 (기본 기능 검증)
- **R5-AC1**: `%>%` 파이프라인 기반 진입점 테스트 (DPLYR 키워드는 미지원)

## 테스트 구조

### 테스트 파일
- `tests/duckdb_extension_integration_test.cpp`: 메인 C++ 테스트 파일
- `tests/run_cpp_integration_tests.sh`: Linux/macOS 테스트 실행 스크립트
- `tests/run_cpp_integration_tests.bat`: Windows 테스트 실행 스크립트

### 테스트 카테고리

#### 1. Extension Loading Tests (R7-AC1)
- `ExtensionLoadingSuccess`: 확장 로딩 성공 테스트
- `DplyrKeywordRecognition`: DPLYR 키워드 거부(미지원) 테스트
- `TableFunctionEntryPoint`: 테이블 함수 진입점 테스트

#### 2. SQL Integration Tests (R2-AC2)
- `StandardSqlMixingWithCTE`: CTE와 dplyr 혼용 테스트
- `SubqueryIntegration`: 서브쿼리 통합 테스트
- `JoinWithDplyrResults`: JOIN과 dplyr 결과 혼용 테스트

#### 3. Crash Prevention Tests (R7-AC3)
- `InvalidDplyrSyntaxNoCrash`: 잘못된 구문으로 인한 크래시 방지
- `NullPointerHandling`: NULL 포인터 처리
- `LargeInputHandling`: 대용량 입력 처리 (DoS 방지)
- `ConcurrentAccessSafety`: 동시 접근 안전성
- `MemoryLeakPrevention`: 메모리 누수 방지

#### 4. Error Handling Tests
- `ErrorMessageQuality`: 에러 메시지 품질 (R1-AC3 준수)

#### 5. Performance & Stability Tests
- `BasicPerformanceStability`: 기본 성능 안정성 (R6-AC1)
- `ComplexQueryStability`: 복잡한 쿼리 안정성

#### 6. DuckDB Integration Tests
- `DuckDBSpecificFeatures`: DuckDB 특화 기능 통합

#### 7. Smoke Tests (R4-AC2)
- `SmokeTestBasicOperations`: 기본 연산 스모크 테스트

## 빌드 및 실행

### 전제 조건
1. CMake 3.15 이상
2. C++17 호환 컴파일러
3. DuckDB 설치 또는 소스 빌드
4. Google Test (자동 다운로드됨)
5. Rust 툴체인 (libdplyr_c 빌드용)

### 빌드 방법

```bash
# 프로젝트 루트에서
mkdir build
cd build
cmake .. -DBUILD_CPP_TESTS=ON
cmake --build . --target duckdb_extension_integration_test
```

### 테스트 실행

#### Linux/macOS
```bash
# 스크립트 사용 (권장)
./tests/run_cpp_integration_tests.sh

# 또는 직접 실행
cd build
export DUCKDB_EXTENSION_PATH=$(pwd)
./duckdb_extension_integration_test
```

#### Windows
```cmd
REM 스크립트 사용 (권장)
tests\run_cpp_integration_tests.bat

REM 또는 직접 실행
cd build
set DUCKDB_EXTENSION_PATH=%cd%
duckdb_extension_integration_test.exe
```

#### CMake 테스트 실행
```bash
cd build
ctest -R cpp_integration
```

### 특정 테스트 카테고리 실행

```bash
# Extension loading tests만 실행
./duckdb_extension_integration_test --gtest_filter="*ExtensionLoading*"

# Crash prevention tests만 실행  
./duckdb_extension_integration_test --gtest_filter="*Crash*"

# 상세한 출력
./duckdb_extension_integration_test --gtest_filter="*" --gtest_color=yes -v
```

## 테스트 환경 설정

### 환경 변수
- `DUCKDB_EXTENSION_PATH`: 확장 파일 경로 (필수)
- `BUILD_DIR`: 빌드 디렉토리 (기본값: build)
- `GTEST_COLOR`: Google Test 색상 출력 (기본값: 1)

### 타임아웃 설정
- 기본 테스트: 60초
- 크래시 방지 테스트: 120초
- 전체 테스트 스위트: 180초

## 테스트 실패 시 디버깅

### 일반적인 문제들

1. **확장 로딩 실패**
   ```
   Error: Extension not found at 'build/dplyr.duckdb_extension'
   ```
   - 해결: 확장이 제대로 빌드되었는지 확인
   - `cmake --build . --target dplyr` 실행

2. **DuckDB 버전 호환성**
   ```
   Error: DuckDB version compatibility issues
   ```
   - 해결: DuckDB 버전 확인 및 업데이트
   - `duckdb --version` 확인

3. **FFI 경계 오류**
   ```
   Error: E-FFI: FFI boundary error
   ```
   - 해결: libdplyr_c 크레이트 재빌드
   - `cargo clean && cargo build` 실행

4. **메모리 관련 오류**
   ```
   Error: Memory leak or segmentation fault
   ```
   - 해결: Valgrind로 메모리 검사
   - `valgrind --tool=memcheck ./duckdb_extension_integration_test`

### 디버그 모드 실행

```bash
# 디버그 빌드
cmake .. -DCMAKE_BUILD_TYPE=Debug -DBUILD_CPP_TESTS=ON
cmake --build .

# GDB로 디버깅
gdb ./duckdb_extension_integration_test
(gdb) run --gtest_filter="*FailingTest*"
```

### 로그 출력 증가

```bash
# 상세한 Google Test 출력
./duckdb_extension_integration_test --gtest_color=yes --gtest_print_time=1

# DuckDB 디버그 로그 (환경에 따라)
export DUCKDB_DEBUG=1
export DPLYR_DEBUG=1
```

## CI/CD 통합

### GitHub Actions 예시

```yaml
- name: Run C++ Integration Tests
  run: |
    cd build
    export DUCKDB_EXTENSION_PATH=$(pwd)
    ctest -R cpp_integration --output-on-failure
```

### 테스트 결과 해석

- **성공**: 모든 요구사항 검증 완료
- **부분 실패**: 일부 기능 동작하지만 완전하지 않음
- **전체 실패**: 기본 기능 동작하지 않음

## 성능 벤치마킹

테스트에는 기본적인 성능 검증이 포함되어 있습니다:

- 단순 쿼리: 1초 이내 완료 (관대한 테스트 환경 기준)
- 복잡한 쿼리: 안정성 위주 검증
- 메모리 누수: 반복 실행 후 크래시 없음

더 정확한 성능 측정은 별도의 벤치마크 도구를 사용하세요.

## 기여 가이드라인

새로운 테스트 추가 시:

1. 해당 요구사항 (Rx-ACy) 명시
2. 테스트 이름에 기능 설명 포함
3. 실패 시 의미 있는 에러 메시지 제공
4. 타임아웃 설정 (크래시 방지)
5. 메모리 정리 확인

### 테스트 추가 예시

```cpp
TEST_F(DuckDBExtensionTest, NewFeatureTest) {
    // R8-AC1: Test new feature requirement
    auto result = safe_query("SELECT * FROM dplyr('my_table %>% select(*)')");
    
    ASSERT_NE(result, nullptr) << "New feature should not crash";
    
    if (result && !result->HasError()) {
        EXPECT_GT(result->RowCount(), 0) << "Should return results";
    } else if (result) {
        string error = result->GetError();
        EXPECT_FALSE(error.empty()) << "Should have meaningful error";
    }
}
```

## 문제 보고

테스트 실패 시 다음 정보를 포함하여 이슈를 보고해주세요:

1. 운영체제 및 버전
2. DuckDB 버전
3. Rust 버전
4. 빌드 로그
5. 테스트 실행 로그
6. 재현 단계

## 참고 자료

- [DuckDB Extension Development](https://duckdb.org/docs/extensions/overview)
- [Google Test Documentation](https://google.github.io/googletest/)
- [libdplyr Requirements](../specs/duckdb-extension/requirements.md)
- [libdplyr Design](../specs/duckdb-extension/design.md)
