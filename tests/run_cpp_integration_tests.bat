@echo off
REM DuckDB Extension C++ Integration Test Runner (Windows)
REM
REM This script runs the C++ integration tests for the DuckDB dplyr extension
REM Requirements: R7-AC1, R7-AC3, R2-AC2, R5-AC1

setlocal enabledelayedexpansion

REM Configuration
if "%BUILD_DIR%"=="" set BUILD_DIR=build
set EXTENSION_NAME=dplyr
set TEST_TIMEOUT=180

echo DuckDB dplyr Extension - C++ Integration Test Runner
echo ==================================================

REM Check if build directory exists
if not exist "%BUILD_DIR%" (
    echo Error: Build directory '%BUILD_DIR%' not found
    echo Please run 'mkdir build && cd build && cmake .. && cmake --build .' first
    exit /b 1
)

REM Check if extension was built
set EXTENSION_PATH=%BUILD_DIR%\%EXTENSION_NAME%.duckdb_extension
if not exist "%EXTENSION_PATH%" (
    echo Error: Extension not found at '%EXTENSION_PATH%'
    echo Please build the extension first with 'cmake --build .' in the build directory
    exit /b 1
)

REM Check if test executable exists
set TEST_EXECUTABLE=%BUILD_DIR%\duckdb_extension_integration_test.exe
if not exist "%TEST_EXECUTABLE%" (
    echo Error: Test executable not found at '%TEST_EXECUTABLE%'
    echo Please build tests with 'cmake --build . --target duckdb_extension_integration_test' in the build directory
    exit /b 1
)

REM Check if DuckDB is available
where duckdb >nul 2>&1
if errorlevel 1 (
    echo Warning: DuckDB CLI not found in PATH
    echo Some tests may fail if DuckDB is not available
)

echo ✓ Build directory found: %BUILD_DIR%
echo ✓ Extension found: %EXTENSION_PATH%
echo ✓ Test executable found: %TEST_EXECUTABLE%
echo.

REM Set environment variables
set DUCKDB_EXTENSION_PATH=%BUILD_DIR%
set GTEST_COLOR=1

REM Track test results
set TOTAL_TESTS=0
set PASSED_TESTS=0

echo Starting C++ Integration Tests...
echo.

REM R7-AC1: DuckDB extension loading and functionality tests
set /a TOTAL_TESTS+=1
echo Running Extension Loading (R7-AC1) tests...
"%TEST_EXECUTABLE%" --gtest_filter="DuckDBExtensionTest.ExtensionLoadingSuccess:DuckDBExtensionTest.DplyrKeywordRecognition:DuckDBExtensionTest.TableFunctionEntryPoint" --gtest_color=yes
if !errorlevel! equ 0 (
    echo ✓ Extension Loading tests passed
    set /a PASSED_TESTS+=1
) else (
    echo ✗ Extension Loading tests failed
)
echo.

REM R2-AC2: Standard SQL integration and mixing tests
set /a TOTAL_TESTS+=1
echo Running SQL Integration (R2-AC2) tests...
"%TEST_EXECUTABLE%" --gtest_filter="DuckDBExtensionTest.StandardSqlMixingWithCTE:DuckDBExtensionTest.SubqueryIntegration:DuckDBExtensionTest.JoinWithDplyrResults" --gtest_color=yes
if !errorlevel! equ 0 (
    echo ✓ SQL Integration tests passed
    set /a PASSED_TESTS+=1
) else (
    echo ✗ SQL Integration tests failed
)
echo.

REM R7-AC3: Crash prevention and error handling tests
set /a TOTAL_TESTS+=1
echo Running Crash Prevention (R7-AC3) tests...
"%TEST_EXECUTABLE%" --gtest_filter="DuckDBExtensionTest.InvalidDplyrSyntaxNoCrash:DuckDBExtensionTest.NullPointerHandling:DuckDBExtensionTest.LargeInputHandling:DuckDBExtensionTest.ConcurrentAccessSafety:DuckDBExtensionTest.MemoryLeakPrevention" --gtest_color=yes
if !errorlevel! equ 0 (
    echo ✓ Crash Prevention tests passed
    set /a PASSED_TESTS+=1
) else (
    echo ✗ Crash Prevention tests failed
)
echo.

REM R4-AC2: Smoke tests
set /a TOTAL_TESTS+=1
echo Running Smoke Tests (R4-AC2)...
"%TEST_EXECUTABLE%" --gtest_filter="DuckDBExtensionTest.SmokeTestBasicOperations" --gtest_color=yes
if !errorlevel! equ 0 (
    echo ✓ Smoke Tests passed
    set /a PASSED_TESTS+=1
) else (
    echo ✗ Smoke Tests failed
)
echo.

REM Error message quality tests
set /a TOTAL_TESTS+=1
echo Running Error Message Quality tests...
"%TEST_EXECUTABLE%" --gtest_filter="DuckDBExtensionTest.ErrorMessageQuality" --gtest_color=yes
if !errorlevel! equ 0 (
    echo ✓ Error Message Quality tests passed
    set /a PASSED_TESTS+=1
) else (
    echo ✗ Error Message Quality tests failed
)
echo.

REM Performance and stability tests
set /a TOTAL_TESTS+=1
echo Running Performance ^& Stability tests...
"%TEST_EXECUTABLE%" --gtest_filter="DuckDBExtensionTest.BasicPerformanceStability:DuckDBExtensionTest.ComplexQueryStability" --gtest_color=yes
if !errorlevel! equ 0 (
    echo ✓ Performance ^& Stability tests passed
    set /a PASSED_TESTS+=1
) else (
    echo ✗ Performance ^& Stability tests failed
)
echo.

REM DuckDB integration tests
set /a TOTAL_TESTS+=1
echo Running DuckDB Integration tests...
"%TEST_EXECUTABLE%" --gtest_filter="DuckDBExtensionTest.DuckDBSpecificFeatures" --gtest_color=yes
if !errorlevel! equ 0 (
    echo ✓ DuckDB Integration tests passed
    set /a PASSED_TESTS+=1
) else (
    echo ✗ DuckDB Integration tests failed
)
echo.

REM Summary
echo ==================================================
echo Test Summary
echo ==================================================

if !PASSED_TESTS! equ !TOTAL_TESTS! (
    echo ✓ All test categories passed (!PASSED_TESTS!/!TOTAL_TESTS!)
    echo.
    echo 🎉 C++ Integration Tests: SUCCESS
    echo.
    echo Requirements verified:
    echo   ✓ R7-AC1: DuckDB extension loading and functionality
    echo   ✓ R7-AC3: Crash prevention and error handling
    echo   ✓ R2-AC2: Standard SQL integration and mixing
    echo   ✓ R4-AC2: Smoke tests for basic functionality
    echo   ✓ R5-AC1: %%^>%% pipeline-based entry point
    exit /b 0
) else (
    echo ✗ Some test categories failed (!PASSED_TESTS!/!TOTAL_TESTS! passed)
    echo.
    echo ❌ C++ Integration Tests: FAILED
    echo.
    echo Please check the test output above for details.
    echo Common issues:
    echo   - Extension not properly built or loaded
    echo   - DuckDB version compatibility issues
    echo   - Missing dependencies (libdplyr_c, DuckDB)
    echo   - FFI boundary issues
    echo   - Memory management problems
    exit /b 1
)
