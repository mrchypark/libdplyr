# EXTENSION KNOWLEDGE BASE (DuckDB)

**Location:** `extension/`
**Primary Language:** C++17

## OVERVIEW
C++ bridge integrating the Rust transpiler into DuckDB as a Parser Extension and Table Function.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| **Entry Point** | `extension/src/dplyr.cpp` | `dplyr_duckdb_cpp_init` |
| **Parser Logic** | `extension/src/dplyr.cpp` | `dplyr_parse`, `ReplaceEmbeddedPipelines` |
| **FFI Boundary** | `extension/include/dplyr.h` | C API definitions for Rust logic |
| **Table Function** | `extension/src/dplyr.cpp` | `dplyr_query` / `dplyr()` implementation |
| **Validation** | `extension/src/dplyr.cpp` | `DplyrInputValidator` (Security/DoS) |

## CONVENTIONS
- **Memory Management**: Use `duckdb::unique_ptr` and DuckDB's `Allocator`.
- **RAII FFI**: Always wrap `char*` from Rust in cleanup guards or immediate `dplyr_free_string` calls.
- **Error Mapping**: Map `DPLYR_ERROR_*` to `duckdb::ParserException` or `duckdb::IOException`.
- **Embedded Syntax**: Support `(| pipeline |)` for mixing dplyr within standard SQL.
- **Build System**: CMake-driven with Corrosion to bridge Rust's `libdplyr_c` static library.
- **Thread Safety**: All extension state must be thread-safe; use `thread_local` for caching if necessary.

## ANTI-PATTERNS
- **NO Raw Malloc**: Use DuckDB's memory manager to participate in query memory limits.
- **NO Leakage**: Never let `sql_output` or `error_output` from `dplyr_compile` escape without freeing.
- **NO Global State**: Avoid mutable globals; use `ClientContext` for session-specific state.
- **NO Unchecked Input**: Never pass raw user strings to Rust without `DplyrInputValidator` checks.
- **NO Standard IO**: Use DuckDB's internal logging/error mechanisms instead of `printf`/`std::cout`.

## BUILD COMMANDS
```bash
# Via Makefile (Recommended)
make extension

# Manual CMake
mkdir build && cd build
cmake .. -DDUCKDB_EXTENSION_NAME=dplyr
make
```
