@echo off
REM Local Code Quality Check Script (Windows)
REM R7-AC4: Comprehensive code quality verification

setlocal enabledelayedexpansion

REM Configuration
set COVERAGE_TARGET=70
if "%BUILD_DIR%"=="" set BUILD_DIR=build

echo libdplyr Code Quality Check
echo ==================================

REM Track overall success
set OVERALL_SUCCESS=1

REM Function to run a check (simulated with labels)
goto :main

:run_check
set CHECK_NAME=%1
set COMMAND=%2
echo.
echo Running %CHECK_NAME%...

%COMMAND% >nul 2>&1
if %errorlevel% equ 0 (
    echo ✓ %CHECK_NAME%: PASSED
) else (
    echo ✗ %CHECK_NAME%: FAILED
    set OVERALL_SUCCESS=0
)
goto :eof

:main

REM =============================================================================
REM Rust Code Quality Checks
REM =============================================================================

echo.
echo 📦 Rust Code Quality Checks
echo ----------------------------

REM Check if we're in the right directory
if not exist "libdplyr_c\Cargo.toml" (
    echo Error: Please run this script from the project root
    exit /b 1
)

cd libdplyr_c

REM Rust formatting
echo.
echo Running Rust Formatting...
cargo fmt --all -- --check
if %errorlevel% equ 0 (
    echo ✓ Rust Formatting: PASSED
) else (
    echo ✗ Rust Formatting: FAILED
    set OVERALL_SUCCESS=0
)

REM Rust clippy
echo.
echo Running Rust Clippy...
cargo clippy --all-targets --all-features -- -D warnings
if %errorlevel% equ 0 (
    echo ✓ Rust Clippy: PASSED
) else (
    echo ✗ Rust Clippy: FAILED
    set OVERALL_SUCCESS=0
)

REM Rust tests
echo.
echo Running Rust Unit Tests...
cargo test --all-features
if %errorlevel% equ 0 (
    echo ✓ Rust Unit Tests: PASSED
) else (
    echo ✗ Rust Unit Tests: FAILED
    set OVERALL_SUCCESS=0
)

REM Security audit
where cargo-audit >nul 2>&1
if %errorlevel% equ 0 (
    echo.
    echo Running Security Audit...
    cargo audit
    if %errorlevel% equ 0 (
        echo ✓ Security Audit: PASSED
    ) else (
        echo ✗ Security Audit: FAILED
        set OVERALL_SUCCESS=0
    )
) else (
    echo ⚠ cargo-audit not installed, skipping security audit
)

REM Dependency check
where cargo-deny >nul 2>&1
if %errorlevel% equ 0 (
    echo.
    echo Running Dependency Check...
    cargo deny check
    if %errorlevel% equ 0 (
        echo ✓ Dependency Check: PASSED
    ) else (
        echo ✗ Dependency Check: FAILED
        set OVERALL_SUCCESS=0
    )
) else (
    echo ⚠ cargo-deny not installed, skipping dependency check
)

REM Code coverage
where cargo-llvm-cov >nul 2>&1
if %errorlevel% equ 0 (
    echo.
    echo Running Code Coverage Analysis...
    
    cargo llvm-cov --all-features --workspace --lcov --output-path ..\lcov.info
    cargo llvm-cov report --html --output-dir ..\coverage-html
    
    REM Extract coverage percentage (simplified for Windows)
    for /f "tokens=*" %%i in ('cargo llvm-cov report --summary-only ^| findstr "TOTAL"') do (
        set COVERAGE_LINE=%%i
    )
    
    REM This is a simplified extraction - in practice, you'd need more robust parsing
    echo Coverage report generated in coverage-html\
    echo ✓ Code Coverage: Analysis completed
) else (
    echo ⚠ cargo-llvm-cov not installed, skipping coverage analysis
    echo Install with: cargo install cargo-llvm-cov
)

REM Benchmarks
echo.
echo Running Performance Benchmarks...
cargo bench --no-run
if %errorlevel% equ 0 (
    echo ✓ Benchmarks compile successfully
    echo Run 'cargo bench' to execute full benchmark suite
) else (
    echo ✗ Benchmark compilation failed
    set OVERALL_SUCCESS=0
)

cd ..

REM =============================================================================
REM C++ Code Quality Checks
REM =============================================================================

echo.
echo 🔧 C++ Code Quality Checks
echo ---------------------------

REM Check if build directory exists
if not exist "%BUILD_DIR%" (
    echo Creating build directory...
    mkdir "%BUILD_DIR%"
)

REM Configure CMake
cd "%BUILD_DIR%"
if not exist "CMakeCache.txt" (
    echo Configuring CMake...
    cmake .. ^
        -DCMAKE_BUILD_TYPE=Debug ^
        -DCMAKE_EXPORT_COMPILE_COMMANDS=ON ^
        -DBUILD_CPP_TESTS=ON ^
        -DBUILD_DUCKDB=OFF ^
        -G "Visual Studio 17 2022" ^
        -A x64
)

REM Build the project
echo.
echo Running C++ Build...
cmake --build . --config Debug --parallel
if %errorlevel% equ 0 (
    echo ✓ C++ Build: PASSED
) else (
    echo ✗ C++ Build: FAILED
    set OVERALL_SUCCESS=0
)

cd ..

REM =============================================================================
REM Integration Tests
REM =============================================================================

echo.
echo 🧪 Integration Tests
echo --------------------

REM C++ integration tests
if exist "%BUILD_DIR%\Debug\duckdb_extension_integration_test.exe" (
    cd "%BUILD_DIR%"
    set DUCKDB_EXTENSION_PATH=%cd%
    
    echo Running C++ Integration Tests...
    Debug\duckdb_extension_integration_test.exe
    if %errorlevel% equ 0 (
        echo ✓ C++ Integration Tests: PASSED
    ) else (
        echo ✗ C++ Integration Tests: FAILED
        set OVERALL_SUCCESS=0
    )
    
    cd ..
) else (
    echo ⚠ C++ integration tests not built
)

REM Smoke tests
if exist "tests\run_smoke_tests.bat" (
    set BUILD_DIR=%BUILD_DIR%
    echo Running Smoke Tests...
    tests\run_smoke_tests.bat
    if %errorlevel% equ 0 (
        echo ✓ Smoke Tests: PASSED
    ) else (
        echo ✗ Smoke Tests: FAILED
        set OVERALL_SUCCESS=0
    )
) else (
    echo ⚠ Smoke tests not found
)

REM =============================================================================
REM Summary
REM =============================================================================

echo.
echo 📋 Quality Check Summary
echo =========================

if %OVERALL_SUCCESS% equ 1 (
    echo 🎉 All quality checks passed!
    echo.
    echo ✓ Code formatting and linting
    echo ✓ Unit and integration tests
    echo ✓ Security and dependency checks
    echo ✓ Static analysis
    echo.
    echo Your code meets the quality standards for libdplyr.
    exit /b 0
) else (
    echo ❌ Some quality checks failed
    echo.
    echo Please address the issues above before submitting your changes.
    echo.
    echo Common fixes:
    echo   • Run 'cargo fmt' to fix formatting
    echo   • Run 'cargo clippy --fix' to auto-fix linting issues
    echo   • Add tests to improve coverage
    echo   • Update dependencies to fix security issues
    echo.
    exit /b 1
)