#!/bin/bash
set -euo pipefail

# GitHub Actions Workflow Testing Script
# Tests workflow files for common issues and validates configuration

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKFLOWS_DIR="$PROJECT_ROOT/.github/workflows"

echo "🔍 Testing GitHub Actions Workflows"
echo "=================================="
echo "Project root: $PROJECT_ROOT"
echo "Workflows directory: $WORKFLOWS_DIR"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Test function
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo -n "Testing $test_name... "
    
    if eval "$test_command" >/dev/null 2>&1; then
        echo -e "${GREEN}PASS${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Test with detailed output
run_test_verbose() {
    local test_name="$1"
    local test_command="$2"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "Testing $test_name..."
    
    if eval "$test_command"; then
        echo -e "${GREEN}✅ PASS: $test_name${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        echo -e "${RED}❌ FAIL: $test_name${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Check if workflows directory exists
if [ ! -d "$WORKFLOWS_DIR" ]; then
    echo -e "${RED}❌ Workflows directory not found: $WORKFLOWS_DIR${NC}"
    exit 1
fi

echo "📋 Basic Workflow File Tests"
echo "----------------------------"

# Test 1: Check if workflow files exist
run_test "Main CI workflow exists" "[ -f '$WORKFLOWS_DIR/ci.yml' ]"
run_test "Release workflow exists" "[ -f '$WORKFLOWS_DIR/release.yml' ]"
run_test "Notification workflow exists" "[ -f '$WORKFLOWS_DIR/notification.yml' ]"
run_test "Dependabot auto-merge workflow exists" "[ -f '$WORKFLOWS_DIR/dependabot-auto-merge.yml' ]"

# Test 2: Check YAML syntax
echo ""
echo "📝 YAML Syntax Tests"
echo "-------------------"

for workflow_file in "$WORKFLOWS_DIR"/*.yml "$WORKFLOWS_DIR"/*.yaml; do
    if [ -f "$workflow_file" ]; then
        filename=$(basename "$workflow_file")
        run_test "YAML syntax for $filename" "python3 -c 'import yaml; yaml.safe_load(open(\"$workflow_file\"))'"
    fi
done

# Test 3: Check for required fields
echo ""
echo "🔍 Workflow Structure Tests"
echo "--------------------------"

check_workflow_field() {
    local file="$1"
    local field="$2"
    python3 -c "
import yaml
with open('$file', 'r') as f:
    workflow = yaml.safe_load(f)
    assert '$field' in workflow, 'Missing field: $field'
"
}

for workflow_file in "$WORKFLOWS_DIR"/*.yml; do
    if [ -f "$workflow_file" ]; then
        filename=$(basename "$workflow_file")
        run_test "Required fields in $filename" "check_workflow_field '$workflow_file' 'name' && check_workflow_field '$workflow_file' 'on' && check_workflow_field '$workflow_file' 'jobs'"
    fi
done

# Test 4: Check for security best practices
echo ""
echo "🔒 Security Tests"
echo "----------------"

check_no_hardcoded_secrets() {
    local file="$1"
    # Check for potential hardcoded secrets (basic patterns)
    if grep -q -E "(password|token|key)\s*[:=]\s*[\"'][^\"']{10,}[\"']" "$file"; then
        return 1
    fi
    return 0
}

for workflow_file in "$WORKFLOWS_DIR"/*.yml; do
    if [ -f "$workflow_file" ]; then
        filename=$(basename "$workflow_file")
        run_test "No hardcoded secrets in $filename" "check_no_hardcoded_secrets '$workflow_file'"
    fi
done

# Test 5: Check action versions
echo ""
echo "📦 Action Version Tests"
echo "----------------------"

check_action_versions() {
    local file="$1"
    # Check for actions without version tags
    if grep -q "uses:.*@main\|uses:.*@master" "$file"; then
        echo "Found unstable branch references in $file"
        return 1
    fi
    return 0
}

for workflow_file in "$WORKFLOWS_DIR"/*.yml; do
    if [ -f "$workflow_file" ]; then
        filename=$(basename "$workflow_file")
        run_test "Stable action versions in $filename" "check_action_versions '$workflow_file'"
    fi
done

# Test 6: Check for caching in Rust workflows
echo ""
echo "⚡ Performance Tests"
echo "------------------"

check_rust_caching() {
    local file="$1"
    # If workflow mentions Rust/Cargo, it should have caching
    if grep -q -i "rust\|cargo" "$file"; then
        if grep -q "actions/cache\|setup-rust-cache" "$file"; then
            return 0
        else
            echo "Rust workflow without caching found in $file"
            return 1
        fi
    fi
    return 0
}

for workflow_file in "$WORKFLOWS_DIR"/*.yml; do
    if [ -f "$workflow_file" ]; then
        filename=$(basename "$workflow_file")
        run_test "Rust workflows use caching in $filename" "check_rust_caching '$workflow_file'"
    fi
done

# Test 7: Validate with Python script (if available)
echo ""
echo "🐍 Advanced Validation Tests"
echo "---------------------------"

if command -v python3 >/dev/null 2>&1; then
    if [ -f "$SCRIPT_DIR/validate_workflows.py" ]; then
        run_test_verbose "Python workflow validator" "python3 '$SCRIPT_DIR/validate_workflows.py' --workflows-dir '$WORKFLOWS_DIR' --fail-on-error"
    else
        echo -e "${YELLOW}⚠️  Python validator script not found${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  Python3 not available for advanced validation${NC}"
fi

# Test 8: Check Dependabot configuration
echo ""
echo "🤖 Dependabot Configuration Tests"
echo "--------------------------------"

DEPENDABOT_CONFIG="$PROJECT_ROOT/.github/dependabot.yml"
run_test "Dependabot config exists" "[ -f '$DEPENDABOT_CONFIG' ]"

if [ -f "$DEPENDABOT_CONFIG" ]; then
    run_test "Dependabot config YAML syntax" "python3 -c 'import yaml; yaml.safe_load(open(\"$DEPENDABOT_CONFIG\"))'"
fi

# Test 9: Check custom actions
echo ""
echo "🎭 Custom Actions Tests"
echo "----------------------"

CUSTOM_ACTIONS_DIR="$PROJECT_ROOT/.github/actions"
if [ -d "$CUSTOM_ACTIONS_DIR" ]; then
    for action_dir in "$CUSTOM_ACTIONS_DIR"/*; do
        if [ -d "$action_dir" ]; then
            action_name=$(basename "$action_dir")
            action_yml="$action_dir/action.yml"
            run_test "Custom action $action_name has action.yml" "[ -f '$action_yml' ]"
            
            if [ -f "$action_yml" ]; then
                run_test "Custom action $action_name YAML syntax" "python3 -c 'import yaml; yaml.safe_load(open(\"$action_yml\"))'"
            fi
        fi
    done
else
    echo -e "${YELLOW}⚠️  No custom actions directory found${NC}"
fi

# Test 10: Check for common CI patterns
echo ""
echo "🔄 CI Pattern Tests"
echo "------------------"

check_ci_patterns() {
    local file="$1"
    local patterns_found=0
    
    # Check for common CI patterns
    if grep -q "checkout" "$file"; then
        patterns_found=$((patterns_found + 1))
    fi
    
    if grep -q "cache" "$file"; then
        patterns_found=$((patterns_found + 1))
    fi
    
    if grep -q "upload-artifact\|download-artifact" "$file"; then
        patterns_found=$((patterns_found + 1))
    fi
    
    # Should have at least 2 common patterns
    [ $patterns_found -ge 2 ]
}

for workflow_file in "$WORKFLOWS_DIR"/*.yml; do
    if [ -f "$workflow_file" ]; then
        filename=$(basename "$workflow_file")
        run_test "Common CI patterns in $filename" "check_ci_patterns '$workflow_file'"
    fi
done

# Summary
echo ""
echo "📊 Test Summary"
echo "==============="
echo -e "Total tests: ${BLUE}$TOTAL_TESTS${NC}"
echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed: ${RED}$FAILED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 All tests passed!${NC}"
    echo "Your GitHub Actions workflows are properly configured."
    exit 0
else
    echo ""
    echo -e "${RED}❌ Some tests failed.${NC}"
    echo "Please review and fix the issues above."
    exit 1
fi