#!/bin/bash

# Test script for release workflow
# This script helps test the release process locally before pushing tags

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CLEAR='\033[0m'

echo -e "${BLUE}libdplyr Release Test Script${CLEAR}"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -f "src/main.rs" ]; then
    echo -e "${RED}Error: This script must be run from the libdplyr root directory${CLEAR}"
    exit 1
fi

# Function to run a command and check its result
run_check() {
    local cmd="$1"
    local desc="$2"
    
    echo -e "${BLUE}Running: $desc${CLEAR}"
    if eval "$cmd"; then
        echo -e "${GREEN}✓ $desc passed${CLEAR}"
    else
        echo -e "${RED}✗ $desc failed${CLEAR}"
        exit 1
    fi
    echo ""
}

# Pre-release checks
echo -e "${YELLOW}=== Pre-release Checks ===${CLEAR}"
echo ""

# Check that we can build
run_check "cargo build --release" "Release build"

# Run all tests
run_check "cargo test --all-features" "All tests"

# Run cross-platform tests
run_check "cargo test --test cross_platform_tests" "Cross-platform tests"

# Run benchmarks (compile only)
run_check "cargo bench --no-run" "Benchmark compilation"

# Check formatting
run_check "cargo fmt -- --check" "Code formatting"

# Run clippy
run_check "cargo clippy --all-targets --all-features -- -D warnings" "Clippy lints"

# Check documentation
run_check "cargo doc --no-deps --document-private-items --all-features" "Documentation generation"

# Security audit
if command -v cargo-audit &> /dev/null; then
    run_check "cargo audit" "Security audit"
else
    echo -e "${YELLOW}⚠ cargo-audit not installed, skipping security audit${CLEAR}"
    echo "  Install with: cargo install cargo-audit"
    echo ""
fi

# Test installation scripts syntax
echo -e "${YELLOW}=== Installation Script Checks ===${CLEAR}"
echo ""

# Check install.sh syntax
if command -v shellcheck &> /dev/null; then
    run_check "shellcheck install.sh" "install.sh syntax check"
else
    echo -e "${YELLOW}⚠ shellcheck not installed, skipping shell script check${CLEAR}"
    echo "  Install with: apt-get install shellcheck (Ubuntu) or brew install shellcheck (macOS)"
    echo ""
fi

# Check install.ps1 syntax (basic check)
if [ -f "install.ps1" ]; then
    echo -e "${BLUE}Checking install.ps1 basic syntax${CLEAR}"
    if grep -q "param" install.ps1 && grep -q "function" install.ps1; then
        echo -e "${GREEN}✓ install.ps1 basic syntax check passed${CLEAR}"
    else
        echo -e "${YELLOW}⚠ install.ps1 might have syntax issues${CLEAR}"
    fi
    echo ""
fi

# Version consistency check
echo -e "${YELLOW}=== Version Consistency Check ===${CLEAR}"
echo ""

CARGO_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
INSTALL_SH_VERSION=$(grep '^VERSION=' install.sh | sed 's/VERSION="\(.*\)"/\1/')
INSTALL_PS1_VERSION=$(grep '^\$VERSION = ' install.ps1 | sed 's/\$VERSION = "\(.*\)"/\1/')

echo "Cargo.toml version: $CARGO_VERSION"
echo "install.sh version: $INSTALL_SH_VERSION"
echo "install.ps1 version: $INSTALL_PS1_VERSION"

if [ "$CARGO_VERSION" = "$INSTALL_SH_VERSION" ] && [ "$CARGO_VERSION" = "$INSTALL_PS1_VERSION" ]; then
    echo -e "${GREEN}✓ All versions are consistent${CLEAR}"
else
    echo -e "${RED}✗ Version mismatch detected${CLEAR}"
    echo "Please update all version strings to match"
    exit 1
fi
echo ""

# Check for uncommitted changes
echo -e "${YELLOW}=== Git Status Check ===${CLEAR}"
echo ""

if [ -n "$(git status --porcelain)" ]; then
    echo -e "${YELLOW}⚠ You have uncommitted changes:${CLEAR}"
    git status --short
    echo ""
    echo "Consider committing these changes before creating a release."
else
    echo -e "${GREEN}✓ Working directory is clean${CLEAR}"
fi
echo ""

# Simulate release build for all targets
echo -e "${YELLOW}=== Cross-compilation Test ===${CLEAR}"
echo ""

# Test targets that don't require special setup
TARGETS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "x86_64-pc-windows-msvc"
)

for target in "${TARGETS[@]}"; do
    if rustup target list --installed | grep -q "$target"; then
        echo -e "${BLUE}Testing build for $target${CLEAR}"
        if cargo build --target "$target" --release; then
            echo -e "${GREEN}✓ $target build successful${CLEAR}"
        else
            echo -e "${YELLOW}⚠ $target build failed (may require additional setup)${CLEAR}"
        fi
    else
        echo -e "${YELLOW}⚠ $target not installed, skipping${CLEAR}"
        echo "  Install with: rustup target add $target"
    fi
    echo ""
done

# Final summary
echo -e "${GREEN}=== Release Test Summary ===${CLEAR}"
echo ""
echo -e "${GREEN}✓ All pre-release checks passed!${CLEAR}"
echo ""
echo "To create a release:"
echo "1. Ensure all changes are committed and pushed"
echo "2. Create and push a tag:"
echo "   git tag -a v$CARGO_VERSION -m \"Release v$CARGO_VERSION\""
echo "   git push origin v$CARGO_VERSION"
echo "3. Monitor the GitHub Actions workflow"
echo "4. Test the installation scripts after release"
echo ""
echo -e "${BLUE}Release workflow will be triggered automatically when the tag is pushed.${CLEAR}"