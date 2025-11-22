# CI Failure Prevention Guide

This document outlines common CI failures and how to prevent them.

## Common Issues and Solutions

### 1. Rust Formatting Failures

**Problem**: `cargo fmt --check` fails in CI

**Prevention**:
```bash
# Always run before committing
cargo fmt --all
```

**Why it happens**: Different developers might have different rustfmt versions or forget to format

**Solution**: Install the pre-commit hook:
```bash
ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

### 2. Pattern Matching with Missing Fields

**Problem**: After adding a field to a struct (e.g., `source` to `DplyrNode::Pipeline`), pattern matches fail

**Example Error**:
```
error[E0027]: pattern does not mention field `source`
```

**Prevention**:
- When adding fields to structs, search for ALL pattern matches:
  ```bash
  rg "DplyrNode::Pipeline \{" --type rust
  ```
- Add `..` to pattern matches that don't need the new field:
  ```rust
  // Before
  DplyrNode::Pipeline { operations, location } => { ... }
  
  // After
  DplyrNode::Pipeline { operations, location, .. } => { ... }
  ```

**Files to check**:
- `src/parser.rs` (tests)
- `src/sql_generator.rs` (tests)
- `examples/transpiler_usage.rs`
- Any other files with pattern matching

### 3. Benchmark Import Issues

**Problem**: Benchmark files can't find functions from the crate

**Why it happens**: Benchmarks are separate binaries, not part of the crate

**Solution**: Use the full crate name:
```rust
// ❌ Wrong (in benchmark files)
use crate::{dplyr_compile, dplyr_free_string, DplyrOptions};

// ✅ Correct (in benchmark files)
use libdplyr_c::{dplyr_compile, dplyr_free_string, DplyrOptions};
```

**Import Order**: `cargo fmt` enforces alphabetical order:
```rust
use criterion::{...};  // External crates first (alphabetical)
use libdplyr_c::{...}; // Then project crates
use std::{...};        // Then std
```

### 4. Test Files vs Benchmark Files

**Test files** (`#[cfg(test)]` or in `tests/`):
- Can use `crate::` or `super::`
- Part of the same crate

**Benchmark files** (in `benches/` with `harness = false`):
- Separate binaries
- Must use full crate name (e.g., `libdplyr_c::`)

### 5. DuckDB Version Compatibility

**Problem**: Extension fails to load due to version mismatch

**Prevention**:
- Keep `extension_config.cmake` and `CMakeLists.txt` in sync
- Update both `DUCKDB_EXTENSION_MIN_SUPPORTED` and `GIT_TAG`
- Test with the target DuckDB version locally

## Pre-Commit Checklist

Before every commit, run:

```bash
# 1. Format code
cargo fmt --all

# 2. Check for warnings
cargo clippy --all-targets --all-features -- -D warnings

# 3. Run tests
cargo test --all

# 4. If you modified structs, check pattern matches
rg "DplyrNode::" --type rust | grep -v "//"
```

## CI Workflow Understanding

### Workflows that run on every push:
1. **CI/CD Pipeline** - Build and test on multiple platforms
2. **Code Quality Analysis** - Clippy, formatting, security
3. **Performance Testing** - Rust benchmarks
4. **Performance Benchmarks** - Integration benchmarks
5. **Security Checks** - Dependency audits

### Key CI checks:
- `cargo fmt --all -- --check` (formatting)
- `cargo clippy` (linting)
- `cargo test` (unit tests)
- `cargo bench` (benchmarks compile)
- `make test` (SQL tests)

## Debugging CI Failures

### View recent failures:
```bash
gh run list --limit 5
```

### View specific failure logs:
```bash
gh run view <run-id> --log-failed
```

### Common failure patterns:

1. **Formatting**: Run `cargo fmt --all`
2. **Missing fields**: Add `..` to pattern matches
3. **Import errors**: Check if using correct crate name
4. **Test failures**: Run `cargo test` locally first

## Best Practices

1. **Always run tests locally** before pushing
2. **Use the pre-commit hook** to catch issues early
3. **Check CI status** before merging PRs
4. **Keep dependencies updated** regularly
5. **Document breaking changes** in commit messages

## Quick Reference

```bash
# Install pre-commit hook
ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit

# Run all checks manually
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
make test

# Check for pattern matching issues after struct changes
rg "DplyrNode::Pipeline \{" --type rust
rg "DplyrOperation::" --type rust | grep "match\|if let"

# View CI status
gh run list --limit 5
gh run watch  # Watch current run
```
