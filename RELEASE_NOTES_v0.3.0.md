# libdplyr v0.3.0 Release Notes

**Release Date:** January 8, 2026

---

## Overview

libdplyr v0.3.0 introduces **Join functionality** support, allowing you to perform SQL joins using familiar dplyr syntax. This release also includes code quality improvements, bug fixes, and dependency updates.

---

## What's New

### ✨ Added: Join Functionality

This release brings comprehensive join support to libdplyr:

- **`left_join()`** - Keep all rows from the left table, matching rows from the right
- **`inner_join()`** - Keep only rows with matches in both tables
- **`right_join()`** - Keep all rows from the right table, matching rows from the left
- **`full_join()`** - Keep all rows from both tables

#### Join Syntax

```r
# Join with by parameter
left_join(other, by = "id")
inner_join(other, by = c("key1", "key2"))

# Join with on expression
left_join(other, on = "table1.id = table2.id")
```

#### Example

```r
select(name, dept) %>%
  left_join(salaries, by = "dept")
```

Transpiles to:

```sql
SELECT "name", "dept"
FROM "data"
LEFT JOIN "salaries" ON "data"."dept" = "salaries"."dept"
```

---

## Changes

### Dependencies Updated
- **`lru`**: `0.12.5` → `0.16.3` (performance and compatibility improvements)

### Documentation
- Separated `INSTALL.md` from `README.md` for improved readability
- Updated `AGENTS.md` with current project structure and conventions

---

## Fixed

### Build & CI/CD
- **macOS Release Workflow**: Fixed checksum verification by using `shasum -a 256` instead of unavailable `sha256sum`
- **Windows Release Workflow**: Disabled ccache to resolve resource file compilation conflicts with Ninja
- **Release Workflow**: Added dynamic extension file path detection for Windows builds
- **CI Permissions**: Added `contents: write` permission for GitHub Actions release deployment

### Testing
- **C++ Integration Tests**: Fixed compilation errors introduced by join refactoring
- **Join Tests**: Updated to use `Transpiler` directly for consistent testing

### SQL Generation
- **ON Clause Generation**: Fixed correct `ON` clause generation when using `by` parameter in joins

---

## Refactored

### Code Quality
- **Clippy Warnings**: Resolved all reported warnings for improved code quality
- **Unused Code**: Removed unused fields and functions detected by cargo check
- **Parser/SQL Generator**: Refactored join-related logic to separate `by` column from `on` expression handling

### Examples
- Fixed compilation errors in examples caused by join refactoring

---

## Security

- **GitHub Security Workflow**: Improved security audit workflow configuration

---

## Compatibility

| Component | Minimum Version | Notes |
|-----------|----------------|-------|
| Rust | 1.70+ | For `edition = 2021` |
| DuckDB | 1.4.0+ | For extension compatibility |
| CMake | 3.25+ | For building extension |

---

## Installation

### Via Install Script (Linux/macOS)
```bash
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash
```

### Via PowerShell (Windows)
```powershell
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex
```

### Via Cargo
```bash
cargo install libdplyr
```

### From Source
```bash
git clone https://github.com/mrchypark/libdplyr.git
cd libdplyr
cargo build --release
```

---

## Resources

- **Documentation**: [https://docs.rs/libdplyr](https://docs.rs/libdplyr)
- **GitHub**: [https://github.com/mrchypark/libdplyr](https://github.com/mrchypark/libdplyr)
- **Crates.io**: [https://crates.io/crates/libdplyr](https://crates.io/crates/libdplyr)

---

## Contributors

Thank you to all contributors who made this release possible!

---

## Full Changelog

```
a07e5ec refactor: fix clippy warnings and code quality improvements
2c49794 refactor: remove unused code detected by cargo check
dab80d4 Merge pull request #3 from mrchypark/dependabot/cargo/lru-0.16.3
a7e43e4 add agents for sub
2d522ef refactor(docs): clean up documentation
81a1397 refactor(tests): fix c++ integration tests compilation
f0dc1d4 fix(examples): fix compilation errors from join refactor
d9d7381 fix(test): use Transpiler directly in join tests
5feca88 fix(sql_generator): generate correct ON clause for by parameter
13b7e63 refactor(parser): separate by column from on expression for join
3aa1284 feat: Add join functionality to libdplyr
8cb5a42 build(deps): bump lru from 0.12.5 to 0.16.3
aa8ab74 fix(ci): add contents: write permission to release workflow
a43a65b fix(ci): dynamic extension file path for Windows release
c989329 fix: resolve release workflow build errors
7c9ffe1 fix security
```

---

**Full changes since v0.2.0**: [Compare v0.2.0...v0.3.0](https://github.com/mrchypark/libdplyr/compare/v0.2.0...v0.3.0)
