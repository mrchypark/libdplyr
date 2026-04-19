# AGENTS - libdplyr_c (C FFI Bindings)

**Generated:** 2026-01-08
**Domain:** FFI / C Bindings / Memory Safety

## OVERVIEW
C FFI bindings for libdplyr providing a generic ABI for DuckDB extensions and other C-compatible environments with strict safety and memory controls.

## WHERE TO LOOK
| Task | Location | Role |
|------|----------|------|
| **Entry Point** | `src/compile.rs` | `dplyr_compile` and `dplyr_compile_query` |
| **FFI Safety** | `src/ffi_safety.rs`| Pointer helper utilities |
| **Memory** | `src/memory.rs` | `dplyr_free_string` and allocation helpers |
| **Error Mapping**| `src/error.rs` | C-compatible error codes & messages |
| **Exports** | `src/lib.rs` | Central `#[no_mangle]` re-exports |

## CONVENTIONS
- **Prefixing**: All exported C symbols MUST use the `dplyr_` prefix (e.g., `dplyr_compile`).
- **Panic Boundary**: EVERY `#[no_mangle]` function MUST use `panic::catch_unwind` to prevent unwinding into C.
- **Boundary Validation**: Validate nulls, UTF-8, structure, and security rules in `src/compile.rs`; keep that logic canonical.
- **Manual Ownership**: Use `CString::into_raw()` for returned strings; callers MUST use `dplyr_free_string`.
- **Return Style**: Return `i32` status codes (0 = Success); use `out` pointers for complex results.
- **Dialect Selection**: The C API is generic; `DplyrOptions.dialect` chooses SQL generation, defaulting to DuckDB for compatibility.
- **C ABI**: Use `extern "C"` and `#[no_mangle]` for all exported functions.

## ANTI-PATTERNS
- **NO Uncaught Panics**: Never allow a Rust panic to cross the FFI boundary (causes UB).
- **NO Raw Rust Types**: Never expose `String`, `Vec`, or complex Rust enums to C; use opaque pointers or C-compatible structs.
- **NO Implicit Cleanup**: Do not rely on Rust's `Drop` for memory shared with C; provide explicit `free` functions.
- **NO Blind Deref**: Never dereference a C-provided pointer without null checks and validity verification.
- **NO Extension Logic Drift**: Query rewriting for parser hooks belongs here so C++ adapters stay thin.
- **NO Repeating Logic**: Core transpilation logic stays in the parent crate; this crate only handles the boundary.

## COMMANDS
```bash
# Build C library
cargo build --package libdplyr_c --release

# Run FFI-specific benchmarks
cargo bench --package libdplyr_c
```
