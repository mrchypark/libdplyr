## CODEMAP: C++ Integration Tests

### Core Files (Direct Impact)
- `tests/duckdb_extension_integration_test.cpp`: MAIN target. Contains all C++ integration tests.
- `CMakeLists.txt`: Configures the build for `duckdb_extension_integration_test` target.
- `tests/run_cpp_integration_tests.sh`: Helper script to run these tests.

### Dependency Graph
```
duckdb_extension_integration_test.cpp
├── Imports
│   ├── <gtest/gtest.h> (Google Test Framework)
│   ├── "duckdb.hpp" (DuckDB C++ API)
│   ├── "duckdb/main/extension/extension_loader.hpp"
│   ├── "duckdb/parser/parser_extension.hpp"
│   └── "../extension/include/dplyr.h" (Extension Header)
├── Depends On
│   ├── libdplyr_c (Rust FFI static library)
│   ├── DuckDB Static Library
│   └── Google Test Library
└── Executed By
    ├── tests/run_cpp_integration_tests.sh (Unix)
    └── tests/run_cpp_integration_tests.bat (Windows)
```

### Impact Zones
| Zone | Risk Level | Files Affected | Test Coverage |
|------|------------|----------------|---------------|
| Test Logic | HIGH | `tests/duckdb_extension_integration_test.cpp` | Self-tested |
| Build Config | MEDIUM | `CMakeLists.txt` | Indirectly verified by build success |
| Execution | LOW | `tests/run_cpp_integration_tests.sh` | Shell script execution |

### Established Patterns
- **Test Fixture**: `class DuckDBExtensionTest : public ::testing::Test` handles setup/teardown.
- **In-Memory DB**: `db = make_uniq<DuckDB>(nullptr)` creates a fresh DB for each test.
- **Static Loading**: `db->LoadStaticExtension<dplyr::DplyrExtension>()` loads the extension manually.
- **Safe Query**: `safe_query()` helper catches exceptions to prevent crashes during tests.
- **Requirements Tracing**: Comments like `// R7-AC1: ...` trace back to requirements.

### Refactoring Opportunities (Inferred)
- The user hasn't specified *what* to refactor yet.
- Potential targets based on file content:
    - Extract `safe_query` and `normalize_sql` helpers to a separate utility class/file?
    - Parameterize tests for different data sizes?
    - Modernize `make_uniq` usage if needed?
    - Improve error message assertion logic (currently string matching)?

**WAITING FOR USER INSTRUCTION ON SPECIFIC REFACTORING GOAL.**
(Proceeding to Phase 3 assuming the goal is related to this file, or I will pause to ask).
