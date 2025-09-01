#!/bin/bash
# DuckDB Extension C++ Integration Test Runner
# 
# This script runs the C++ integration tests for the DuckDB dplyr extension
# Requirements: R7-AC1, R7-AC3, R2-AC2, R5-AC1

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BUILD_DIR="${BUILD_DIR:-build}"
EXTENSION_NAME="dplyr"
TEST_TIMEOUT=180

echo -e "${BLUE}DuckDB dplyr Extension - C++ Integration Test Runner${NC}"
echo "=================================================="

# Check if build directory exists
if [ ! -d "$BUILD_DIR" ]; then
    echo -e "${RED}Error: Build directory '$BUILD_DIR' not found${NC}"
    echo "Please run 'mkdir build && cd build && cmake .. && make' first"
    exit 1
fi

# Check if extension was built
EXTENSION_PATH="$BUILD_DIR/${EXTENSION_NAME}.duckdb_extension"
if [ ! -f "$EXTENSION_PATH" ]; then
    echo -e "${RED}Error: Extension not found at '$EXTENSION_PATH'${NC}"
    echo "Please build the extension first with 'make' in the build directory"
    exit 1
fi

# Check if test executable exists
TEST_EXECUTABLE="$BUILD_DIR/duckdb_extension_integration_test"
if [ ! -f "$TEST_EXECUTABLE" ]; then
    echo -e "${RED}Error: Test executable not found at '$TEST_EXECUTABLE'${NC}"
    echo "Please build tests with 'make duckdb_extension_integration_test' in the build directory"
    exit 1
fi

# Check if DuckDB is available
if ! command -v duckdb &> /dev/null; then
    echo -e "${YELLOW}Warning: DuckDB CLI not found in PATH${NC}"
    echo "Some tests may fail if DuckDB is not available"
fi

echo -e "${GREEN}✓ Build directory found: $BUILD_DIR${NC}"
echo -e "${GREEN}✓ Extension found: $EXTENSION_PATH${NC}"
echo -e "${GREEN}✓ Test executable found: $TEST_EXECUTABLE${NC}"
echo ""

# Set environment variables
export DUCKDB_EXTENSION_PATH="$BUILD_DIR"
export GTEST_COLOR=1

# Function to run test category
run_test_category() {
    local category_name="$1"
    local test_filter="$2"
    local timeout="${3:-60}"
    
    echo -e "${BLUE}Running $category_name tests...${NC}"
    
    if timeout "$timeout" "$TEST_EXECUTABLE" --gtest_filter="$test_filter" --gtest_color=yes; then
        echo -e "${GREEN}✓ $category_name tests passed${NC}"
        return 0
    else
        local exit_code=$?
        if [ $exit_code -eq 124 ]; then
            echo -e "${RED}✗ $category_name tests timed out after ${timeout}s${NC}"
        else
            echo -e "${RED}✗ $category_name tests failed (exit code: $exit_code)${NC}"
        fi
        return $exit_code
    fi
}

# Test categories based on requirements
echo "Starting C++ Integration Tests..."
echo ""

# Track test results
TOTAL_TESTS=0
PASSED_TESTS=0

# R7-AC1: DuckDB extension loading and functionality tests
TOTAL_TESTS=$((TOTAL_TESTS + 1))
if run_test_category "Extension Loading (R7-AC1)" \
    "DuckDBExtensionTest.ExtensionLoadingSuccess:DuckDBExtensionTest.DplyrKeywordRecognition:DuckDBExtensionTest.TableFunctionEntryPoint" \
    60; then
    PASSED_TESTS=$((PASSED_TESTS + 1))
fi
echo ""

# R2-AC2: Standard SQL integration and mixing tests  
TOTAL_TESTS=$((TOTAL_TESTS + 1))
if run_test_category "SQL Integration (R2-AC2)" \
    "DuckDBExtensionTest.StandardSqlMixingWithCTE:DuckDBExtensionTest.SubqueryIntegration:DuckDBExtensionTest.JoinWithDplyrResults" \
    60; then
    PASSED_TESTS=$((PASSED_TESTS + 1))
fi
echo ""

# R7-AC3: Crash prevention and error handling tests
TOTAL_TESTS=$((TOTAL_TESTS + 1))
if run_test_category "Crash Prevention (R7-AC3)" \
    "DuckDBExtensionTest.InvalidDplyrSyntaxNoCrash:DuckDBExtensionTest.NullPointerHandling:DuckDBExtensionTest.LargeInputHandling:DuckDBExtensionTest.ConcurrentAccessSafety:DuckDBExtensionTest.MemoryLeakPrevention" \
    120; then
    PASSED_TESTS=$((PASSED_TESTS + 1))
fi
echo ""

# R4-AC2: Smoke tests
TOTAL_TESTS=$((TOTAL_TESTS + 1))
if run_test_category "Smoke Tests (R4-AC2)" \
    "DuckDBExtensionTest.SmokeTestBasicOperations" \
    30; then
    PASSED_TESTS=$((PASSED_TESTS + 1))
fi
echo ""

# Error message quality tests
TOTAL_TESTS=$((TOTAL_TESTS + 1))
if run_test_category "Error Message Quality" \
    "DuckDBExtensionTest.ErrorMessageQuality" \
    30; then
    PASSED_TESTS=$((PASSED_TESTS + 1))
fi
echo ""

# Performance and stability tests
TOTAL_TESTS=$((TOTAL_TESTS + 1))
if run_test_category "Performance & Stability" \
    "DuckDBExtensionTest.BasicPerformanceStability:DuckDBExtensionTest.ComplexQueryStability" \
    60; then
    PASSED_TESTS=$((PASSED_TESTS + 1))
fi
echo ""

# DuckDB integration tests
TOTAL_TESTS=$((TOTAL_TESTS + 1))
if run_test_category "DuckDB Integration" \
    "DuckDBExtensionTest.DuckDBSpecificFeatures" \
    30; then
    PASSED_TESTS=$((PASSED_TESTS + 1))
fi
echo ""

# Summary
echo "=================================================="
echo -e "${BLUE}Test Summary${NC}"
echo "=================================================="

if [ $PASSED_TESTS -eq $TOTAL_TESTS ]; then
    echo -e "${GREEN}✓ All test categories passed ($PASSED_TESTS/$TOTAL_TESTS)${NC}"
    echo ""
    echo -e "${GREEN}🎉 C++ Integration Tests: SUCCESS${NC}"
    echo ""
    echo "Requirements verified:"
    echo "  ✓ R7-AC1: DuckDB extension loading and functionality"
    echo "  ✓ R7-AC3: Crash prevention and error handling"
    echo "  ✓ R2-AC2: Standard SQL integration and mixing"
    echo "  ✓ R4-AC2: Smoke tests for basic functionality"
    echo "  ✓ R5-AC1: DPLYR keyword-based entry point"
    exit 0
else
    echo -e "${RED}✗ Some test categories failed ($PASSED_TESTS/$TOTAL_TESTS passed)${NC}"
    echo ""
    echo -e "${RED}❌ C++ Integration Tests: FAILED${NC}"
    echo ""
    echo "Please check the test output above for details."
    echo "Common issues:"
    echo "  - Extension not properly built or loaded"
    echo "  - DuckDB version compatibility issues"
    echo "  - Missing dependencies (libdplyr_c, DuckDB)"
    echo "  - FFI boundary issues"
    echo "  - Memory management problems"
    exit 1
fi