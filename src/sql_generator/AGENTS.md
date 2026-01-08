# SQL GENERATOR KNOWLEDGE BASE

## OVERVIEW
Core engine for transpiling dplyr AST into dialect-specific SQL using a "QueryParts" assembly pattern.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| **New Dialect** | `dialect.rs` | Implement `SqlDialect` trait |
| **Mutation Logic** | `mutate_support.rs` | Dependency tracking and inlining |
| **SQL Assembly** | `assemble.rs` | Final string concatenation & ordering |
| **Join/SetOps** | `mod.rs` | Native vs. Subquery transformations |
| **Func Mapping** | `dialect.rs` | `translate_common_function` helper |

## CONVENTIONS
- **Tidy First**: Extract logic from `mod.rs` to specialized helpers as complexity grows.
- **Dialect Abstraction**: Use `self.dialect` for all identifier quoting and function mapping.
- **Incremental Building**: Populate `QueryParts` during operation processing; assemble only at the end.
- **Mutation Inlining**: Prefer inlining `mutate` expressions in `SELECT` over forced subqueries where possible.
- **Trait-Based Dispatch**: Dialect-specific behavior (e.g., `EXCLUDE` in DuckDB) must be gated by trait methods.

## ANTI-PATTERNS
- **NO Raw SQL Concatenation**: Use `dialect.quote_identifier` and `dialect.quote_string`.
- **NO Dialect Hardcoding**: Do not check `dialect_name()` in `mod.rs` if a trait method can handle it.
- **NO Side Effects**: `process_operation` should only modify `QueryParts` or return errors.
- **NO Nested Assembly**: Avoid recursive `generate()` calls; use subquery helpers for joins/mutates.
