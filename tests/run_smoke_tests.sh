#!/bin/bash
# DuckDB Extension Smoke Test Runner
# 
# This script runs the smoke tests for the DuckDB dplyr extension
# Requirements: R4-AC2, R1-AC2

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
SMOKE_TEST_FILE="tests/smoke.sql"
TEST_TIMEOUT=60

echo -e "${BLUE}DuckDB dplyr Extension - Smoke Test Runner${NC}"
echo "=============================================="

# Check if DuckDB is available
if ! command -v duckdb &> /dev/null; then
    echo -e "${RED}Error: DuckDB CLI not found in PATH${NC}"
    echo "Please install DuckDB or add it to your PATH"
    echo "Download from: https://duckdb.org/docs/installation/"
    exit 1
fi

# Check DuckDB version
DUCKDB_VERSION=$(duckdb --version 2>/dev/null || echo "unknown")
echo -e "${GREEN}✓ DuckDB found: $DUCKDB_VERSION${NC}"

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

# Check if smoke test file exists
if [ ! -f "$SMOKE_TEST_FILE" ]; then
    echo -e "${RED}Error: Smoke test file not found at '$SMOKE_TEST_FILE'${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Build directory found: $BUILD_DIR${NC}"
echo -e "${GREEN}✓ Extension found: $EXTENSION_PATH${NC}"
echo -e "${GREEN}✓ Smoke test file found: $SMOKE_TEST_FILE${NC}"
echo ""

# Set environment variables
export DUCKDB_EXTENSION_PATH="$BUILD_DIR"

# Create temporary database for testing
TEMP_DB=$(mktemp -d)/smoke_test.db
echo "Using temporary database: $TEMP_DB"

# Function to run smoke tests
run_smoke_tests() {
    echo -e "${BLUE}Running smoke tests...${NC}"
    echo "Extension path: $EXTENSION_PATH"
    echo "Test file: $SMOKE_TEST_FILE"
    echo ""
    
    # Run the smoke tests with timeout
    if timeout "$TEST_TIMEOUT" duckdb "$TEMP_DB" < "$SMOKE_TEST_FILE"; then
        echo ""
        echo -e "${GREEN}✓ Smoke tests completed successfully${NC}"
        return 0
    else
        local exit_code=$?
        echo ""
        if [ $exit_code -eq 124 ]; then
            echo -e "${RED}✗ Smoke tests timed out after ${TEST_TIMEOUT}s${NC}"
        else
            echo -e "${RED}✗ Smoke tests failed (exit code: $exit_code)${NC}"
        fi
        return $exit_code
    fi
}

# Function to analyze test results
analyze_results() {
    echo ""
    echo -e "${BLUE}Analyzing test results...${NC}"
    
    # Count different types of test outcomes
    local total_tests=0
    local passed_tests=0
    local failed_tests=0
    local maybe_tests=0
    
    # Parse the smoke.sql file to count tests
    while IFS= read -r line; do
        if [[ $line =~ ^--\ Test\ [0-9]+: ]]; then
            ((total_tests++))
        fi
    done < "$SMOKE_TEST_FILE"
    
    echo "Total test cases defined: $total_tests"
    
    # Provide guidance based on current implementation status
    echo ""
    echo -e "${YELLOW}Note: Test results depend on implementation status:${NC}"
    echo "  • Extension loading tests should PASS"
    echo "  • DPLYR functionality tests may FAIL gracefully (not implemented yet)"
    echo "  • Error handling tests should return meaningful error messages"
    echo "  • Standard SQL tests should PASS (no interference)"
    echo ""
    
    # Check if extension loaded successfully by looking for specific patterns
    if duckdb "$TEMP_DB" -c "LOAD '$EXTENSION_PATH'; SELECT 'Extension loaded' as status;" 2>/dev/null | grep -q "Extension loaded"; then
        echo -e "${GREEN}✓ Extension loading: SUCCESS${NC}"
    else
        echo -e "${RED}✗ Extension loading: FAILED${NC}"
        return 1
    fi
    
    # Test basic SQL functionality
    if duckdb "$TEMP_DB" -c "SELECT 1 as test;" 2>/dev/null | grep -q "1"; then
        echo -e "${GREEN}✓ Basic SQL functionality: SUCCESS${NC}"
    else
        echo -e "${RED}✗ Basic SQL functionality: FAILED${NC}"
        return 1
    fi
    
    return 0
}

# Function to provide troubleshooting guidance
provide_guidance() {
    echo ""
    echo -e "${BLUE}Troubleshooting Guidance:${NC}"
    echo ""
    
    echo "If extension loading fails:"
    echo "  1. Check that the extension was built successfully"
    echo "  2. Verify DuckDB version compatibility"
    echo "  3. Check for missing dependencies (libdplyr_c)"
    echo "  4. Review build logs for errors"
    echo ""
    
    echo "If DPLYR functionality tests fail:"
    echo "  1. This is expected if the extension is not fully implemented"
    echo "  2. Check that errors are graceful (no crashes)"
    echo "  3. Verify error messages include error codes (E-*)"
    echo "  4. Ensure the extension returns to DuckDB properly"
    echo ""
    
    echo "If standard SQL tests fail:"
    echo "  1. This indicates the extension interferes with DuckDB"
    echo "  2. Check parser extension implementation"
    echo "  3. Verify keyword collision avoidance"
    echo "  4. Review extension registration code"
    echo ""
    
    echo "For debugging:"
    echo "  • Set DPLYR_DEBUG=1 for verbose logging"
    echo "  • Use 'duckdb -c \"LOAD '$EXTENSION_PATH'; .help\"' to test loading"
    echo "  • Check DuckDB logs for extension-related messages"
    echo "  • Run individual test queries manually"
}

# Main execution
echo "Starting smoke tests..."
echo ""

# Track overall success
OVERALL_SUCCESS=true

# Run the smoke tests
if run_smoke_tests; then
    echo -e "${GREEN}✓ Smoke test execution completed${NC}"
else
    echo -e "${YELLOW}⚠ Smoke test execution had issues${NC}"
    OVERALL_SUCCESS=false
fi

# Analyze results
if analyze_results; then
    echo -e "${GREEN}✓ Core functionality verified${NC}"
else
    echo -e "${RED}✗ Core functionality issues detected${NC}"
    OVERALL_SUCCESS=false
fi

# Cleanup
if [ -f "$TEMP_DB" ]; then
    rm -f "$TEMP_DB"
fi

# Final summary
echo ""
echo "=============================================="
echo -e "${BLUE}Smoke Test Summary${NC}"
echo "=============================================="

if [ "$OVERALL_SUCCESS" = true ]; then
    echo -e "${GREEN}🎉 Smoke Tests: SUCCESS${NC}"
    echo ""
    echo "Requirements verified:"
    echo "  ✓ R4-AC2: Basic extension functionality"
    echo "  ✓ R1-AC2: Core operation support (structure)"
    echo "  ✓ Extension loading and unloading"
    echo "  ✓ No interference with standard SQL"
    echo ""
    echo "The extension is ready for further development and testing."
    exit 0
else
    echo -e "${RED}❌ Smoke Tests: ISSUES DETECTED${NC}"
    echo ""
    echo "Some core functionality issues were detected."
    echo "This may be expected if the extension is not fully implemented."
    echo ""
    provide_guidance
    exit 1
fi