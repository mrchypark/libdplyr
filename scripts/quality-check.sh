#!/bin/bash
# Local Code Quality Check Script
# R7-AC4: Comprehensive code quality verification

set -e

# Keep noisy local logs out of the repo root.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$SCRIPT_DIR/housekeeping-logs.sh" || true

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
COVERAGE_TARGET=70
BUILD_DIR="${BUILD_DIR:-build}"

echo -e "${BLUE}🔍 libdplyr Code Quality Check${NC}"
echo "=================================="

# Track overall success
OVERALL_SUCCESS=true

# Function to run a check and track success
run_check() {
    local check_name="$1"
    local command="$2"

    echo -e "\n${BLUE}Running $check_name...${NC}"

    if eval "$command"; then
        echo -e "${GREEN}✅ $check_name: PASSED${NC}"
    else
        echo -e "${RED}❌ $check_name: FAILED${NC}"
        OVERALL_SUCCESS=false
    fi
}

# =============================================================================
# Rust Code Quality Checks
# =============================================================================

echo -e "\n${BLUE}📦 Rust Code Quality Checks${NC}"
echo "----------------------------"

# Check if we're in the right directory
if [ ! -f "libdplyr_c/Cargo.toml" ]; then
    echo -e "${RED}Error: Please run this script from the project root${NC}"
    exit 1
fi

LIBDPLYR_C_MANIFEST="libdplyr_c/Cargo.toml"

# Rust formatting
run_check "Rust Formatting" "cargo fmt --manifest-path ${LIBDPLYR_C_MANIFEST} --all -- --check"

# Rust clippy
run_check "Rust Clippy" "cargo clippy --manifest-path ${LIBDPLYR_C_MANIFEST} --all-targets --all-features -- -D warnings"

# Rust tests
run_check "Rust Unit Tests" "cargo test --manifest-path ${LIBDPLYR_C_MANIFEST} --all-features"

# Security audit
if command -v cargo-audit &> /dev/null; then
    run_check "Security Audit" "cargo audit --manifest-path ${LIBDPLYR_C_MANIFEST}"
else
    echo -e "${YELLOW}⚠️ cargo-audit not installed, skipping security audit${NC}"
fi

# Dependency check
if command -v cargo-deny &> /dev/null; then
    run_check "Dependency Check" "cargo deny check --manifest-path ${LIBDPLYR_C_MANIFEST}"
else
    echo -e "${YELLOW}⚠️ cargo-deny not installed, skipping dependency check${NC}"
fi

# Unsafe code analysis
if command -v cargo-geiger &> /dev/null; then
    echo -e "\n${BLUE}Running Unsafe Code Analysis...${NC}"
    cargo geiger --manifest-path ${LIBDPLYR_C_MANIFEST} || echo -e "${YELLOW}⚠️ cargo-geiger analysis completed with warnings${NC}"
else
    echo -e "${YELLOW}⚠️ cargo-geiger not installed, skipping unsafe code analysis${NC}"
fi

# Code coverage
if command -v cargo-llvm-cov &> /dev/null; then
    echo -e "\n${BLUE}Running Code Coverage Analysis...${NC}"

    # Generate coverage report
    cargo llvm-cov --manifest-path ${LIBDPLYR_C_MANIFEST} --all-features --workspace --lcov --output-path lcov.info
    cargo llvm-cov report --manifest-path ${LIBDPLYR_C_MANIFEST} --html --output-dir coverage-html

    # Extract coverage percentage
    COVERAGE_PERCENT=$(cargo llvm-cov report --manifest-path ${LIBDPLYR_C_MANIFEST} --summary-only | grep -E "TOTAL.*%" | awk '{print $NF}' | sed 's/%//')

    echo "Coverage: $COVERAGE_PERCENT%"

    if (( $(echo "$COVERAGE_PERCENT >= $COVERAGE_TARGET" | bc -l) )); then
        echo -e "${GREEN}✅ Code Coverage: PASSED ($COVERAGE_PERCENT% >= $COVERAGE_TARGET%)${NC}"
    else
        echo -e "${RED}❌ Code Coverage: FAILED ($COVERAGE_PERCENT% < $COVERAGE_TARGET%)${NC}"
        OVERALL_SUCCESS=false
    fi

    echo "Coverage report generated in coverage-html/"
else
    echo -e "${YELLOW}⚠️ cargo-llvm-cov not installed, skipping coverage analysis${NC}"
    echo "Install with: cargo install cargo-llvm-cov"
fi

# Benchmarks
echo -e "\n${BLUE}Running Performance Benchmarks...${NC}"
if cargo bench --manifest-path ${LIBDPLYR_C_MANIFEST} --no-run; then
    echo -e "${GREEN}✅ Benchmarks compile successfully${NC}"
    echo "Run 'cargo bench' to execute full benchmark suite"
else
    echo -e "${RED}❌ Benchmark compilation failed${NC}"
    OVERALL_SUCCESS=false
fi

# =============================================================================
# C++ Code Quality Checks
# =============================================================================

echo -e "\n${BLUE}🔧 C++ Code Quality Checks${NC}"
echo "---------------------------"

# Check if build directory exists
if [ ! -d "$BUILD_DIR" ]; then
    echo "Creating build directory..."
    mkdir -p "$BUILD_DIR"
fi

# Configure CMake with analysis flags
cd "$BUILD_DIR"
if [ ! -f "CMakeCache.txt" ]; then
    echo "Configuring CMake..."
    cmake .. \
        -DCMAKE_BUILD_TYPE=Debug \
        -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
        -DBUILD_CPP_TESTS=ON \
        -DBUILD_DUCKDB=OFF
fi

# Build the project
run_check "C++ Build" "cmake --build . --parallel"

# cppcheck static analysis
if command -v cppcheck &> /dev/null; then
    echo -e "\n${BLUE}Running cppcheck...${NC}"
    cppcheck \
        --enable=all \
        --inconclusive \
        --project=compile_commands.json \
        --suppress=missingIncludeSystem \
        --suppress=unmatchedSuppression \
        ../extension/ || echo -e "${YELLOW}⚠️ cppcheck completed with warnings${NC}"
else
    echo -e "${YELLOW}⚠️ cppcheck not installed, skipping C++ static analysis${NC}"
fi

# clang-tidy analysis
if command -v clang-tidy &> /dev/null && command -v run-clang-tidy &> /dev/null; then
    echo -e "\n${BLUE}Running clang-tidy...${NC}"
    run-clang-tidy \
        -header-filter='extension/.*' \
        -j $(nproc) \
        || echo -e "${YELLOW}⚠️ clang-tidy completed with warnings${NC}"
else
    echo -e "${YELLOW}⚠️ clang-tidy not installed, skipping C++ linting${NC}"
fi

cd ..

# =============================================================================
# Integration Tests
# =============================================================================

echo -e "\n${BLUE}🧪 Integration Tests${NC}"
echo "--------------------"

# C++ integration tests
if [ -f "$BUILD_DIR/duckdb_extension_integration_test" ]; then
    cd "$BUILD_DIR"
    export DUCKDB_EXTENSION_PATH=$(pwd)

    run_check "C++ Integration Tests" "./duckdb_extension_integration_test"

    cd ..
else
    echo -e "${YELLOW}⚠️ C++ integration tests not built${NC}"
fi

# Smoke tests
if [ -f "tests/run_smoke_tests.sh" ]; then
    export BUILD_DIR="$BUILD_DIR"
    run_check "Smoke Tests" "./tests/run_smoke_tests.sh"
else
    echo -e "${YELLOW}⚠️ Smoke tests not found${NC}"
fi

# =============================================================================
# Memory Analysis (if available)
# =============================================================================

if command -v valgrind &> /dev/null; then
    echo -e "\n${BLUE}🧠 Memory Analysis${NC}"
    echo "------------------"

    cd "$BUILD_DIR"
    export DUCKDB_EXTENSION_PATH=$(pwd)

    echo "Running Valgrind memory check..."
    if valgrind \
        --tool=memcheck \
        --leak-check=full \
        --error-exitcode=1 \
        --quiet \
        duckdb :memory: -c "LOAD './dplyr.duckdb_extension'; SELECT 'Memory test' as result;" \
        > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Memory Analysis: PASSED${NC}"
    else
        echo -e "${RED}❌ Memory Analysis: FAILED${NC}"
        echo "Run with full output: valgrind --tool=memcheck --leak-check=full duckdb :memory: -c \"LOAD './dplyr.duckdb_extension';\""
        OVERALL_SUCCESS=false
    fi

    cd ..
else
    echo -e "${YELLOW}⚠️ Valgrind not available, skipping memory analysis${NC}"
fi

# =============================================================================
# Summary
# =============================================================================

echo -e "\n${BLUE}📋 Quality Check Summary${NC}"
echo "========================="

if [ "$OVERALL_SUCCESS" = true ]; then
    echo -e "${GREEN}🎉 All quality checks passed!${NC}"
    echo ""
    echo "✅ Code formatting and linting"
    echo "✅ Unit and integration tests"
    echo "✅ Security and dependency checks"
    echo "✅ Static analysis"
    echo "✅ Memory safety (if available)"
    echo ""
    echo "Your code meets the quality standards for libdplyr."
    exit 0
else
    echo -e "${RED}❌ Some quality checks failed${NC}"
    echo ""
    echo "Please address the issues above before submitting your changes."
    echo ""
    echo "Common fixes:"
    echo "  • Run 'cargo fmt' to fix formatting"
    echo "  • Run 'cargo clippy --fix' to auto-fix linting issues"
    echo "  • Add tests to improve coverage"
    echo "  • Update dependencies to fix security issues"
    echo ""
    exit 1
fi
