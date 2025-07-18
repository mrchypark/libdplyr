#!/bin/bash

# Test script for install.sh
# This script tests various aspects of the installation script without actually installing

set -e

RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[0;33m"
CLEAR="\033[0m"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_SCRIPT="$SCRIPT_DIR/install.sh"

test_count=0
passed_count=0
failed_count=0

# Test helper functions
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_exit_code="${3:-0}"
    
    test_count=$((test_count + 1))
    echo -e "${YELLOW}Test $test_count: $test_name${CLEAR}"
    
    if eval "$test_command" >/dev/null 2>&1; then
        local actual_exit_code=$?
    else
        local actual_exit_code=$?
    fi
    
    if [ "$actual_exit_code" -eq "$expected_exit_code" ]; then
        echo -e "${GREEN}  ✅ PASSED${CLEAR}"
        passed_count=$((passed_count + 1))
    else
        echo -e "${RED}  ❌ FAILED (expected exit code $expected_exit_code, got $actual_exit_code)${CLEAR}"
        failed_count=$((failed_count + 1))
    fi
    echo
}

# Test script existence and permissions
echo "Testing install.sh script..."
echo

run_test "Script exists and is executable" \
    "[ -x '$INSTALL_SCRIPT' ]"

run_test "Help option works" \
    "'$INSTALL_SCRIPT' --help"

run_test "Dry run mode works" \
    "'$INSTALL_SCRIPT' --dry-run"

run_test "Invalid option returns error" \
    "'$INSTALL_SCRIPT' --invalid-option" 2

run_test "Version option requires argument" \
    "'$INSTALL_SCRIPT' --version" 2

run_test "Directory option requires argument" \
    "'$INSTALL_SCRIPT' --dir" 2

run_test "Debug option works in dry-run" \
    "DEBUG=true '$INSTALL_SCRIPT' --dry-run"

run_test "Custom version in dry-run" \
    "'$INSTALL_SCRIPT' --dry-run --version v1.0.0"

run_test "Custom directory in dry-run" \
    "'$INSTALL_SCRIPT' --dry-run --dir /tmp/test"

# Test environment variable handling
run_test "Environment variable LIBDPLYR_VERSION" \
    "LIBDPLYR_VERSION=v1.0.0 '$INSTALL_SCRIPT' --dry-run"

run_test "Environment variable INSTALL_DIR" \
    "INSTALL_DIR=/tmp/test '$INSTALL_SCRIPT' --dry-run"

# Summary
echo "═══════════════════════════════════════════════════════════════"
echo -e "${YELLOW}Test Summary:${CLEAR}"
echo "  Total tests: $test_count"
echo -e "  Passed: ${GREEN}$passed_count${CLEAR}"
echo -e "  Failed: ${RED}$failed_count${CLEAR}"

if [ "$failed_count" -eq 0 ]; then
    echo -e "${GREEN}All tests passed! ✅${CLEAR}"
    exit 0
else
    echo -e "${RED}Some tests failed! ❌${CLEAR}"
    exit 1
fi