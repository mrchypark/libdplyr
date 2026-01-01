# Release Notes - v0.2.0

## 🎉 DuckDB Extension Release

This release marks a major milestone with full DuckDB extension integration, enabling native dplyr syntax support directly within DuckDB.

---

## ✨ New Features

### DuckDB Community Extension Integration

- **Community Extension Descriptor**: Added `descriptor.yml` for DuckDB community extensions registration
- **Extension CI Tools**: Integrated DuckDB extension-ci-tools workflow for automated testing and deployment
- **DuckDB Submodule**: Added DuckDB v1.4.0/v1.4.2 as submodule for extension development
- **Community PR Template**: Added standardized PR template for community contributions

### Native DuckDB Extension

- **C++ Extension Implementation**: Complete C++ extension with ParserExtension and OperatorExtension
- **Native dplyr Syntax**: Execute bare dplyr pipelines directly in DuckDB without SQL conversion
  ```sql
  -- Now supported natively in DuckDB:
  df %>% filter(age > 18) %>% select(name, age) %>% arrange(age)
  ```
- **Embedded Pipelines**: Support for embedded dplyr syntax within SQL queries
- **Table Function**: Execute real dplyr pipelines through DuckDB table functions

### C FFI Library (libdplyr_c)

- **Complete C API**: Full C FFI interface for DuckDB integration
- **Caching System**: Intelligent caching for transpilation results
- **Error Handling**: Comprehensive error codes and handling
- **Version API**: Added `dplyr_version()` function to FFI library
- **Performance Tests**: Built-in performance testing framework

### Testing & Quality Assurance

- **Smoke Tests**: Multi-platform smoke tests (Linux, macOS, Windows)
- **C++ Integration Tests**: Comprehensive integration test suite
- **SQL Logic Tests**: Expanded dplyr sqllogictest coverage
- **clang-tidy Integration**: Static analysis replacing CodeQL C++ for faster CI
- **Security Workflows**: Automated security scanning and auditing

### Documentation

- **Code Quality Guide** (`docs/code-quality.md`): Best practices and quality standards
- **Packaging Guide** (`docs/packaging.md`): Multi-platform packaging instructions
- **Performance Monitoring** (`docs/performance-monitoring.md`): Performance testing and benchmarking
- **Release Process** (`docs/release-process.md`): Comprehensive release workflow
- **Community Submission** (`docs/community-repo-submission.md`): Guide for DuckDB community extensions

---

## 🔧 Changes

### Extension Renaming

- Renamed extension from `dplyr_extension` to `dplyr` for consistency
- Updated all references and build configurations

### DuckDB Compatibility

- Updated to DuckDB v1.4.2 API compatibility
- Aligned with DuckDB extension standards and conventions

### CI/CD Improvements

- Simplified and consolidated CI workflows
- Merged integration benchmarks into performance workflow
- Enhanced release workflow with comprehensive metadata

### Build System

- Enforced no-RTTI flags for extension compatibility
- Improved CMake configuration for standalone builds
- Added support for Windows Ninja generator

---

## 🐛 Bug Fixes

### Windows Build Fixes

- **LNK2001/LNK2005 Errors**: Resolved linker errors by defining `DUCKDB_STATIC_BUILD`
- **DLL Entrypoint Export**: Fixed Windows DLL entrypoint with explicit `dllexport`
- **Static Linking**: Properly configured static linking with `duckdb_static.lib`
- **Test Paths**: Dynamic test executable and extension binary path detection
- **Ninja Generator**: Full support for Ninja build system on Windows

### CI/CD Fixes

- **clang-tidy Warnings**: Resolved all warnings (params, bools, ctor, enums, magic numbers)
- **macOS Cross-compilation**: Fixed x86_64 cross-architecture builds
- **Ubuntu Linker**: Resolved linker errors on Ubuntu platforms
- **CodeQL Build**: Fixed C++ CodeQL build failures

### Test Improvements

- Updated integration tests to handle exceptions properly
- Fixed cache effectiveness test determinism
- Improved smoke test robustness across platforms

### Dependency Management

- Fixed cargo-deny license warnings (MPL/Zlib/Unicode)
- Resolved duplicate dependency warnings

---

## 🚀 Performance Improvements

- **Faster CI**: Replaced CodeQL C++ with clang-tidy for ~50% faster CI runs
- **Optimized Extension Loading**: Improved binary discovery and loading
- **Cache Effectiveness**: Enhanced transpilation caching system

---

## 📦 New Files & Structure

### Extension Files

- `CMakeLists.txt` - Complete CMake build configuration
- `extension/src/dplyr.cpp` - C++ extension implementation (1165 lines)
- `extension/include/dplyr.h` - Extension header (706 lines)
- `extension_config.cmake` - Extension configuration
- `extension_version.h.in` - Version template

### C FFI Library

- `libdplyr_c/` - Complete C FFI library directory
  - `src/lib.rs` - Main FFI interface (2364 lines)
  - `src/cache.rs` - Caching system (796 lines)
  - `src/error.rs` - Error handling (461 lines)
  - `benches/` - Performance benchmarks

### Testing Infrastructure

- `tests/run_smoke_tests.sh` - Linux/macOS smoke tests
- `tests/run_smoke_tests.bat` - Windows smoke tests
- `tests/duckdb_extension_integration_test.cpp` - C++ integration tests
- `test/sql/dplyr.test` - SQL logic tests

### CI/CD

- `.github/workflows/MainDistributionPipeline.yml` - Distribution pipeline
- `.github/workflows/security.yml` - Security scanning
- `.clang-tidy` - Static analysis configuration
- `.cppcheck` - C++ checking configuration

### Scripts

- `scripts/create-release.sh` - Automated release creation
- `scripts/package-all-platforms.sh` - Multi-platform packaging
- `scripts/quality-check.sh` - Quality assurance automation
- `scripts/run-performance-tests.sh` - Performance testing

---

## 📊 Statistics

- **75 files changed**
- **19,614 insertions**, 511 deletions
- **120+ commits** since v0.1.2
- **5 new documentation guides**
- **Multi-platform support**: Linux, macOS, Windows (x86_64, ARM64)

---

## 🔗 Installation

### Using DuckDB Extension

```sql
INSTALL dplyr FROM community;
LOAD dplyr;

-- Use native dplyr syntax
SELECT * FROM (df %>% filter(age > 18) %>% select(name, age));
```

### Using CLI

```bash
# Download from releases
curl -L https://github.com/mrchypark/libdplyr/releases/download/v0.2.0/libdplyr-<platform>.tar.gz | tar xz
```

---

## 🙏 Acknowledgments

This release represents significant work in integrating libdplyr with the DuckDB ecosystem, making dplyr syntax a first-class citizen in DuckDB.

---

## 📝 Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for detailed changes.

**Full Diff**: [v0.1.2...v0.2.0](https://github.com/mrchypark/libdplyr/compare/v0.1.2...v0.2.0)
