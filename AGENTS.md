# PROJECT KNOWLEDGE BASE

**Generated:** 2026-01-08
**Commit:** (Head)
**Branch:** main

## OVERVIEW
High-performance Rust-based transpiler converting R dplyr syntax to SQL. Supports multiple dialects (Postgres, MySQL, SQLite, DuckDB) via CLI and C++ extensions.

## STRUCTURE
```
libdplyr/
├── libdplyr_c/       # C FFI bindings (crate)
├── extension/        # DuckDB extension (C++)
├── src/
│   ├── sql_generator/ # Dialect-specific logic
│   ├── parser/       # AST & Parser logic
│   └── cli/          # CLI infrastructure
└── tests/            # Integration tests
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| **Core Logic** | `src/lib.rs` | Entry point |
| **Parsing** | `src/parser/parse.rs` | Recursive descent parser |
| **SQL Gen** | `src/sql_generator/` | Dialect logic |
| **FFI** | `libdplyr_c/src/lib.rs` | C API surface |
| **Extension** | `extension/src/dplyr.cpp` | DuckDB C++ hooks |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `Transpiler` | Struct | `src/lib.rs` | Main facade |
| `DplyrNode` | Enum | `src/parser/ast.rs` | AST Root |
| `SqlGenerator`| Struct | `src/sql_generator/mod.rs` | SQL Builder |
| `PostgreSqlDialect` | Struct | `src/sql_generator/dialect.rs` | Dialect impl |

## CONVENTIONS
- **TDD**: Strict Red -> Green -> Refactor cycle.
- **Traceability**: Link code to requirements via `R#-AC#` comments.
- **FFI Safety**: Catch panics at C boundary (`libdplyr_c`).
- **Pinning**: Submodules/Dependencies pinned to exact commits.

## ANTI-PATTERNS (THIS PROJECT)
- **NO Unwraps**: Use `Result` or `expect` with context.
- **NO Dynamic SQL**: Use AST-based generation.
- **NO Raw Pointers**: Wrap in `ffi_safety` helpers.
- **NO Console Log**: Use `tracing` or `debug_logger`.

## UNIQUE STYLES
- **Korean Localization**: Error messages support Korean.
- **WASM Builds**: Emscripten support baked into Makefile.
- **Embedded Pipeline**: `(| ... |)` syntax in C++ extension.

## COMMANDS
```bash
# Build
cargo build --release
make extension # Build DuckDB extension

# Test
cargo test                  # All Rust tests
cargo test --test integration_tests
tests/run_smoke_tests.sh    # Fast verification
```

## NOTES
- **Complexity**: `src/parser/parse.rs` and `extension/src/dplyr.cpp` are hotspots.
- **Mutation**: CTE/Subquery support for `mutate` is limited.
- **Validation**: Error positioning needs improvement.
