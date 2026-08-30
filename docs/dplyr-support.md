# dplyr syntax support

Audited against the current parser and SQL generator on 2026-08-30. The primary
DuckDB target is 1.5.5; PostgreSQL, MySQL, SQLite, and DuckDB share the portable
subset unless a dialect exception is noted.

Status meanings:

- **Supported**: the documented form is parsed, generated, and covered by tests.
- **Partial**: only the forms listed below are supported; other forms fail closed.
- **Unsupported**: the parser or generator intentionally rejects the feature.

## Verbs

| Status | Syntax | Supported range and limits |
|---|---|---|
| Supported | Pipes | `%>%` or `|>` according to the configured pipe mode, including the implemented magrittr/native lambda RHS forms. |
| Partial | `select()` | `*`, identifiers, expressions, and aliases. No tidy-select helpers, negative selection, `where()`, or `everything()`. |
| Partial | `filter()` | One boolean expression using comparisons, `&`, `\|`, arithmetic, and supported helpers. No comma-separated predicates or `.by`. |
| Partial | `mutate()` | `name = expression` assignments. No `.by`, `.keep`, `.before`, `.after`, or `NULL` column deletion. Pipelines that require a new query stage may be rejected. |
| Partial | `rename()` | `new = old`; currently depends on DuckDB-style `* EXCLUDE`, so it is not portable to every dialect or every preceding projection. |
| Partial | `arrange()` | Identifier keys and `asc(identifier)` / `desc(identifier)` only. |
| Partial | `group_by()` | Identifier keys only; no computed keys, `.add`, or `.drop`. |
| Partial | `summarise()` / `summarize()` | Optional alias plus one supported aggregate over an identifier. No `.by`, `across()`, arbitrary scalar output, or `reframe()` semantics. |
| Partial | `distinct()` | `distinct()` or identifier keys. Expressions and `.keep_all` are rejected; operations after `distinct()` and `group_by() %>% distinct()` require an unimplemented query stage. |
| Partial | `count()` | Identifier-only keys, merged with current groups. No `wt`, `sort`, `name`, `.drop`, computed/join-derived keys, or arbitrary downstream verbs; `arrange()` is allowed. |
| Partial | `tally()` | Argument-free, unweighted tally over current groups. The same downstream restrictions as `count()` apply. |
| Partial | `*_join()` | `inner`, `left`, `right`, `full`, `semi`, and `anti` with explicit `by = "key"` or string-only `by = c("same", "left" = "right", ...)`. Conditions are equality predicates joined with `AND`; non-DuckDB semi/anti joins lower to `EXISTS` / `NOT EXISTS`. No default common-key discovery, `join_by()`, inequality/rolling/overlap joins, or dplyr join options such as `suffix`, `keep`, `relationship`, `multiple`, `unmatched`, and `na_matches`. |
| Partial | Set operations | `union(table)`, `intersect(table)`, and `setdiff(table)` with an identifier right table. No `union_all`, `symdiff`, or `setequal`. |

## Expressions and helpers

| Status | Category | Supported range and limits |
|---|---|---|
| Supported | Operators | `==`, `!=`, `<`, `<=`, `>`, `>=`, `&`, `\|`, `+`, `-`, `*`, `/`, and parentheses. |
| Supported | Aggregates | `mean` / `avg`, `sum`, `count`, `min`, `max`, `n`, and single-column `n_distinct`. `n_distinct` includes one NULL/NA group to match dplyr's default. DuckDB additionally supports `median` and `mode`. |
| Supported | Conditional | `ifelse`, `if_else`, and ordered `case_when(condition ~ value, ..., .default = value)`. Omitted `.default` becomes SQL `NULL`. |
| Partial | `case_when()` | No empty branch list, `.ptype`, `.size`, dynamic dots, or explicit dplyr prototype/coercion emulation. |
| Supported | Predicates / missing values | `between`, `is.na`, `coalesce`, `replace_na`, and `na.replace`. |
| Supported | Math / casts | `abs`, `round`, `floor`, `ceiling` / `ceil`, `sqrt`, `sign`, `exp`, `log`, `log10`, `mod`, trigonometric helpers, and `as.numeric` / `as.double` / `as.integer` / `as.character` / `as.logical`, subject to dialect support. |
| Supported | Strings | `concat`, `paste`, `paste0`, case conversion, `str_detect`, `str_length`, `str_trim`, `substr`, `nchar`, `nzchar`, and `trimws`, subject to dialect support. |
| Supported | Window helpers | `lead`, `lag`, `rank`, `dense_rank`, `ntile`, `row_number`, `first`, `last`, and `nth_value`; grouping keys become window partitions where implemented. |

## Not currently supported

The operation dispatcher does not implement `slice*`, `ungroup`, `relocate`,
`pull`, `reframe`, `rowwise`, `add_count`, `add_tally`, `bind_rows`, `bind_cols`,
`cross_join`, `nest_join`, `rows_*`, `across`, `if_any`, `if_all`, `pick`, or
tidy-select helpers. Common vector helpers such as `na_if`, `near`, `min_rank`,
`percent_rank`, `cume_dist`, and cumulative functions are also not implemented.

The authoritative implementation points are
[`src/parser/parse.rs`](../src/parser/parse.rs),
[`src/parser/ast.rs`](../src/parser/ast.rs),
[`src/sql_generator/mod.rs`](../src/sql_generator/mod.rs), and
[`src/sql_generator/dialect.rs`](../src/sql_generator/dialect.rs).
