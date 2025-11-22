#!/bin/sh
# Pre-commit hook to prevent common CI failures
#
# Install this hook by running:
#   ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit
#   chmod +x .git/hooks/pre-commit

set -e

echo "🔍 Running pre-commit checks..."

# 1. Check Rust formatting
echo "📝 Checking Rust formatting..."
if ! cargo fmt --all -- --check; then
    echo "❌ Rust formatting check failed!"
    echo "💡 Run 'cargo fmt --all' to fix formatting issues"
    exit 1
fi

# 2. Check Rust clippy
echo "🔧 Running Rust clippy..."
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "❌ Clippy found issues!"
    echo "💡 Fix the clippy warnings before committing"
    exit 1
fi

# 3. Run Rust tests
echo "🧪 Running Rust tests..."
if ! cargo test --all; then
    echo "❌ Tests failed!"
    echo "💡 Fix failing tests before committing"
    exit 1
fi

# 4. Check for common mistakes in struct changes
echo "🔎 Checking for pattern matching issues..."
if git diff --cached --name-only | grep -q "src/parser.rs\|src/sql_generator.rs"; then
    echo "⚠️  Warning: You modified parser.rs or sql_generator.rs"
    echo "   If you added fields to DplyrNode::Pipeline, make sure to:"
    echo "   1. Update ALL pattern matches with '..' or the new field"
    echo "   2. Run 'cargo test' to catch missing fields"
    echo "   3. Check examples/transpiler_usage.rs for pattern matches"
fi

echo "✅ All pre-commit checks passed!"
