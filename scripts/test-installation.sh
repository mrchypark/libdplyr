#!/bin/bash

# Test script for verifying installation after release
# This script tests the installation process and basic functionality

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CLEAR='\033[0m'

# Configuration
REPO_URL="https://github.com/yourusername/libdplyr"
INSTALL_SCRIPT_URL="https://raw.githubusercontent.com/yourusername/libdplyr/main/install.sh"
TEST_DIR="/tmp/libdplyr-install-test"

echo -e "${BLUE}libdplyr Installation Test Script${CLEAR}"
echo ""

# Function to run a command and check its result
run_check() {
    local cmd="$1"
    local desc="$2"
    
    echo -e "${BLUE}Testing: $desc${CLEAR}"
    if eval "$cmd"; then
        echo -e "${GREEN}✓ $desc passed${CLEAR}"
    else
        echo -e "${RED}✗ $desc failed${CLEAR}"
        return 1
    fi
    echo ""
}

# Function to test libdplyr functionality
test_libdplyr_functionality() {
    echo -e "${YELLOW}=== Testing libdplyr functionality ===${CLEAR}"
    echo ""
    
    # Test version command
    run_check "libdplyr --version" "Version command"
    
    # Test help command
    run_check "libdplyr --help" "Help command"
    
    # Test basic transpilation
    echo -e "${BLUE}Testing basic transpilation${CLEAR}"
    local test_input="select(name, age) %>% filter(age > 18)"
    local result
    result=$(echo "$test_input" | libdplyr --dialect postgresql 2>/dev/null)
    
    if [[ $result == *"SELECT"* ]] && [[ $result == *"WHERE"* ]]; then
        echo -e "${GREEN}✓ Basic transpilation works${CLEAR}"
    else
        echo -e "${RED}✗ Basic transpilation failed${CLEAR}"
        echo "Input: $test_input"
        echo "Output: $result"
        return 1
    fi
    echo ""
    
    # Test validation mode
    run_check "echo 'select(name, age)' | libdplyr --validate-only" "Validation mode"
    
    # Test JSON output
    echo -e "${BLUE}Testing JSON output${CLEAR}"
    local json_result
    json_result=$(echo "select(name)" | libdplyr --json 2>/dev/null)
    
    if [[ $json_result == *"success"* ]] && [[ $json_result == *"sql"* ]]; then
        echo -e "${GREEN}✓ JSON output works${CLEAR}"
    else
        echo -e "${RED}✗ JSON output failed${CLEAR}"
        echo "Output: $json_result"
        return 1
    fi
    echo ""
    
    # Test different dialects
    local dialects=("postgresql" "mysql" "sqlite" "duckdb")
    for dialect in "${dialects[@]}"; do
        if echo "select(name)" | libdplyr --dialect "$dialect" >/dev/null 2>&1; then
            echo -e "${GREEN}✓ $dialect dialect works${CLEAR}"
        else
            echo -e "${YELLOW}⚠ $dialect dialect failed${CLEAR}"
        fi
    done
    echo ""
}

# Function to test installation from script
test_script_installation() {
    echo -e "${YELLOW}=== Testing Script Installation ===${CLEAR}"
    echo ""
    
    # Create test directory
    mkdir -p "$TEST_DIR"
    cd "$TEST_DIR"
    
    # Download and run installation script
    echo -e "${BLUE}Downloading installation script${CLEAR}"
    if curl -sSL "$INSTALL_SCRIPT_URL" -o install.sh; then
        echo -e "${GREEN}✓ Installation script downloaded${CLEAR}"
    else
        echo -e "${RED}✗ Failed to download installation script${CLEAR}"
        return 1
    fi
    
    # Make script executable
    chmod +x install.sh
    
    # Run installation script with custom directory
    local install_dir="$TEST_DIR/bin"
    echo -e "${BLUE}Running installation script${CLEAR}"
    if ./install.sh --dir="$install_dir"; then
        echo -e "${GREEN}✓ Installation script completed${CLEAR}"
    else
        echo -e "${RED}✗ Installation script failed${CLEAR}"
        return 1
    fi
    
    # Check if binary was installed
    if [ -x "$install_dir/libdplyr" ]; then
        echo -e "${GREEN}✓ Binary installed successfully${CLEAR}"
    else
        echo -e "${RED}✗ Binary not found after installation${CLEAR}"
        return 1
    fi
    
    # Test the installed binary
    echo -e "${BLUE}Testing installed binary${CLEAR}"
    if "$install_dir/libdplyr" --version >/dev/null 2>&1; then
        echo -e "${GREEN}✓ Installed binary works${CLEAR}"
    else
        echo -e "${RED}✗ Installed binary failed${CLEAR}"
        return 1
    fi
    
    echo ""
}

# Function to test manual download
test_manual_download() {
    echo -e "${YELLOW}=== Testing Manual Download ===${CLEAR}"
    echo ""
    
    # Detect platform
    local os
    local arch
    case "$(uname -s)" in
        Linux*)
            os="linux"
            ;;
        Darwin*)
            os="macos"
            ;;
        *)
            echo -e "${YELLOW}⚠ Unsupported OS for manual download test${CLEAR}"
            return 0
            ;;
    esac
    
    case "$(uname -m)" in
        x86_64)
            arch="x86_64"
            ;;
        arm64|aarch64)
            arch="aarch64"
            ;;
        *)
            echo -e "${YELLOW}⚠ Unsupported architecture for manual download test${CLEAR}"
            return 0
            ;;
    esac
    
    # Get latest release info
    echo -e "${BLUE}Getting latest release information${CLEAR}"
    local latest_release
    latest_release=$(curl -s "https://api.github.com/repos/yourusername/libdplyr/releases/latest")
    
    if [ $? -ne 0 ]; then
        echo -e "${YELLOW}⚠ Could not fetch release information${CLEAR}"
        return 0
    fi
    
    local tag_name
    tag_name=$(echo "$latest_release" | grep '"tag_name":' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/')
    
    if [ -z "$tag_name" ]; then
        echo -e "${YELLOW}⚠ Could not parse tag name${CLEAR}"
        return 0
    fi
    
    echo "Latest release: $tag_name"
    
    # Download binary
    local archive_name="libdplyr-${os}-${arch}.tar.gz"
    local download_url="$REPO_URL/releases/download/$tag_name/$archive_name"
    
    echo -e "${BLUE}Downloading $archive_name${CLEAR}"
    if curl -sSL "$download_url" -o "$archive_name"; then
        echo -e "${GREEN}✓ Archive downloaded${CLEAR}"
    else
        echo -e "${YELLOW}⚠ Could not download archive (may not exist yet)${CLEAR}"
        return 0
    fi
    
    # Extract and test
    echo -e "${BLUE}Extracting archive${CLEAR}"
    if tar -xzf "$archive_name"; then
        echo -e "${GREEN}✓ Archive extracted${CLEAR}"
    else
        echo -e "${RED}✗ Failed to extract archive${CLEAR}"
        return 1
    fi
    
    # Test extracted binary
    if [ -x "./libdplyr" ]; then
        echo -e "${GREEN}✓ Binary found in archive${CLEAR}"
        
        if ./libdplyr --version >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Downloaded binary works${CLEAR}"
        else
            echo -e "${RED}✗ Downloaded binary failed${CLEAR}"
            return 1
        fi
    else
        echo -e "${RED}✗ Binary not found in archive${CLEAR}"
        return 1
    fi
    
    echo ""
}

# Function to cleanup
cleanup() {
    echo -e "${BLUE}Cleaning up test directory${CLEAR}"
    rm -rf "$TEST_DIR"
    echo -e "${GREEN}✓ Cleanup complete${CLEAR}"
}

# Main test execution
main() {
    local test_type="${1:-all}"
    local failed=0
    
    case "$test_type" in
        "functionality")
            test_libdplyr_functionality || failed=1
            ;;
        "script")
            test_script_installation || failed=1
            ;;
        "manual")
            test_manual_download || failed=1
            ;;
        "all")
            # Test existing installation first
            if command -v libdplyr >/dev/null 2>&1; then
                test_libdplyr_functionality || failed=1
            else
                echo -e "${YELLOW}⚠ libdplyr not found in PATH, skipping functionality tests${CLEAR}"
                echo ""
            fi
            
            # Test installation methods
            test_script_installation || failed=1
            test_manual_download || failed=1
            ;;
        *)
            echo "Usage: $0 [functionality|script|manual|all]"
            echo ""
            echo "  functionality  - Test libdplyr functionality (requires libdplyr in PATH)"
            echo "  script        - Test installation script"
            echo "  manual        - Test manual download"
            echo "  all           - Run all tests (default)"
            exit 1
            ;;
    esac
    
    # Cleanup
    cleanup
    
    # Final result
    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}=== All tests passed! ===${CLEAR}"
        echo ""
        echo -e "${GREEN}✓ libdplyr installation and functionality verified${CLEAR}"
    else
        echo -e "${RED}=== Some tests failed ===${CLEAR}"
        echo ""
        echo -e "${RED}✗ Please check the output above for details${CLEAR}"
        exit 1
    fi
}

# Run main function with arguments
main "$@"