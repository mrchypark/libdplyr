# dplyr syntax support

Audited against the current parser and SQL generator on 2026-08-30. The primary
DuckDB target is 1.5.5; PostgreSQL, MySQL, SQLite, and DuckDB share the portable
subset unless a dialect exception is noted.

Status meanings:

- **Supported**: the documented form is parsed, generated, and covered by tests.
- **Partial**: only the forms listed below are supported; other forms fail closed.
- **Unsupported**: the parser or generator intentionally rejects the feature.

## Schema-aware compilation

The additive `Transpiler::transpile_with_schema(code, &schema)` API binds a
pipeline to an ordered input schema. It returns SQL, ordered output columns,
and the number of SELECT stages before database optimization. Column references
use stable internal identities; computed columns are evaluated in successive
stages rather than substituted into later expressions.

`Transpiler::transpile_with_schemas(code, &[SourceSchema])` extends that API to
every source a pipeline reads, which is what joins and set operations need. The
first entry describes the pipeline source; a source-less pipeline uses it. Each
later entry describes the table named by a `*_join()` or set operation, in the
order those operations appear. Each source is scanned exactly once and the
compiled query is a single SELECT. `Transpiler::required_sources(code)` returns
those source names in input order for callers that build the schemas themselves.

The CLI enables this path with `--schema schema.json`:

```json
{
  "source": "sales",
  "columns": [
    {"name": "region", "data_type": "VARCHAR", "nullable": true},
    {"name": "amount", "data_type": "DOUBLE", "nullable": true}
  ]
}
```

`--schema` accepts either a single source object or an array of them, so a
multi-source pipeline needs no separate flag:

```json
[
  {"source": "users", "columns": [{"name": "id"}, {"name": "name"}]},
  {"source": "orders", "columns": [{"name": "user_id"}, {"name": "qty"}, {"name": "price"}]}
]
```

```bash
libdplyr --schema schemas.json -d sqlite -t \
  'users %>% inner_join(orders, by = c("id" = "user_id")) %>% mutate(rev = price * qty) %>% group_by(name) %>% summarise(total = sum(rev)) %>% filter(total > 100) %>% arrange(desc(total))'
```

```sql
SELECT "name", "total" FROM (SELECT "name", SUM("rev") AS "total"
  FROM (SELECT "id", "name", "qty", "price", ("price" * "qty") AS "rev"
    FROM (SELECT "__libdplyr_left"."id" AS "id", "__libdplyr_left"."name" AS "name",
                 "__libdplyr_right"."qty" AS "qty", "__libdplyr_right"."price" AS "price"
          FROM (SELECT "id", "name" FROM "users") AS "__libdplyr_left"
          INNER JOIN (SELECT "user_id", "qty", "price" FROM "orders") AS "__libdplyr_right"
            ON "__libdplyr_left"."id" = "__libdplyr_right"."user_id") AS "q1") AS "q2"
  GROUP BY "name") AS "q3"
 WHERE ("total" > 100) ORDER BY "total" DESC
```

The single-source CLI example:

```bash
libdplyr --schema schema.json -d sqlite -t \
  'sales %>% mutate(net = amount * 0.9, tax = net * 0.1) %>% group_by(region) %>% summarise(total = sum(tax)) %>% filter(total > 100) %>% arrange(desc(total))'
```

The schema-aware path supports identifier selection and aliases, dependent and
overwriting mutations, portable renames, filters after aggregation, repeated
aggregation, grouped window aggregates, window-result filters,
distinct followed by further operations, counts of computed columns, joins, and
set operations. Grouped selection retains omitted group keys. `summarise()`
drops the last grouping level; `count()` restores the input groups. An earlier
sort value is retained internally when selection or mutation removes it from
the visible output. Hidden compiler columns do not appear in the returned
schema.

Input names are resolved exactly against the supplied schema. Explicit pipeline
sources must match `schema.source`; source-less pipelines use that source. The
caller must supply current metadata. Direct copies and renames retain supplied
type/nullability metadata; computed expressions with unknown result types return
unset metadata. SQL NULL and aggregate behavior follow the existing backend
contract, including NULL-inclusive `n_distinct()` in summaries.

Single-source limits: computed selection, tidy-select, mutation of grouping
columns, window `n_distinct()`, and target-table assignments fail closed on this
path. Distinct over a removed or overwritten sort key is also rejected. Window
helpers retain their explicit ordering arguments; an earlier `arrange()` is not
inferred as a window order. There is no SQL-stage optimizer yet, and generation
is bounded to 64 SELECT stages. The shared parser also bounds expression nesting
and AST depth to 64 and rejects overly long binary chains during construction.
The schema-free API and its limits below remain available.

### Aggregate arguments

On this path a summary argument may be an expression, so
`summarise(total = sum(price * qty))` aggregates the product directly instead of
requiring a preceding `mutate()`. Bare identifiers must still be grouped or
aggregated, an aggregate may not contain another aggregate, and window
functions inside `summarise()` are rejected.

### Joins

All six equality joins are supported: `inner`, `left`, `right`, `full`, `semi`,
and `anti`. Keys come from `by = "key"`, `by = c("left" = "right", ...)`,
or a `by` expression built from identifier comparisons joined by `&`.
MySQL rejects `full_join()` because it has no native full join.

Non-key columns that collide on both sides become `name.x` and `name.y`, which
is dplyr's default `suffix = c(".x", ".y")`. A name that would still collide
gains a repeated suffix rather than silently dropping a column.

`right_join()` and `full_join()` coalesce an equality key into one output column
named after the left key, so an unmatched right row keeps its own key value:

```sql
COALESCE("__libdplyr_left"."id", "__libdplyr_right"."user_id") AS "id"
```

This matches dplyr's default `keep = NULL`. The `keep` option
are not implemented, so there is no way to retain both key columns.

Key columns are matched with SQL `=`, which **never matches NULL**, matching
[dbplyr's default `na_matches = "never"`](https://dbplyr.tidyverse.org/reference/join.tbl_sql.html).
This affects matching only, independently of the coalescing above: a
NULL key row never joins, and key coalescing still fills from the surviving
side.

Rejected on this path: duplicate keys within one `by`, non-identifier join
comparisons, the `join_by()` wrapper, non-equality predicates (inequality, rolling,
overlap, nearest), and natural joins without `by`. Common-key discovery is not
implemented, so `by` is required.

### Set operations

`union()`, `intersect()`, and `setdiff()` align their inputs **by column name**,
not position, so the right table may list the same columns in a different
order. Inputs with differing column names are rejected rather than filled with
NULL. Membership compares visible values only, so an earlier `arrange()` does
not change the result.

These verbs remove duplicates, as dplyr's do, and `union_all`, `symdiff`, and
`setequal` are not implemented.

The additive C API `dplyr_compile_with_schema` accepts the same schema JSON, in
either the single-object or the array form, and returns strings with the
existing `dplyr_free_string` ownership contract. Bound SQL is not cached, so
each request uses its supplied schema. The companion
`dplyr_compile_with_schema_and_pipe_syntax` takes an explicit pipe mode.

DuckDB automatically discovers all source schemas in `dplyr(code[, pipe_config])`,
direct pipelines, and embedded `(| ... |)` pipelines. Discovery binds a native
`SELECT *` through a child of the current binder; it does not execute the source.
This includes caller-visible TEMP tables, CTEs, views, and uncommitted changes.
Queries require rebinding so catalog changes refresh the discovered columns.
`dplyr_required_sources` exposes the ordered source list to C callers, and
`dplyr_prepare_query_with_pipe_syntax` defers query compilation to this bind step.
Standalone Rust and CLI compilation still require supplied schema metadata.

DuckDB also exposes `dplyr_with_schema(code, schema_json[, pipe_config])`, with the
same single-object or array schema JSON:

```sql
SELECT * FROM dplyr_with_schema(
  'users %>% inner_join(orders, by = c("id" = "user_id"))',
  '[{"source":"users","columns":[{"name":"id"}]},{"source":"orders","columns":[{"name":"user_id"}]}]'
);
```

The generated SELECT is bound in the caller's context, including TEMP tables,
CTEs, and uncommitted transaction changes. This explicit API uses caller-provided
metadata. The optional third
argument overrides pipe mode; otherwise the existing DuckDB pipe setting and
environment fallback apply.

## Verbs in the schema-free API

| Status | Syntax | Supported range and limits |
|---|---|---|
| Supported | Pipes | `%>%` or `|>` according to the configured pipe mode, including the implemented magrittr/native lambda RHS forms. |
| Partial | `select()` | `*`, identifiers, expressions, and aliases. No tidy-select helpers, negative selection, `where()`, or `everything()`. |
| Partial | `filter()` | One boolean expression using comparisons, `&`, `\|`, arithmetic, and supported helpers. No comma-separated predicates or `.by`. |
| Partial | `mutate()` | `name = expression` assignments. Assignments that reference earlier mutated or aliased columns, or redefine those names, are rejected until subquery stages are supported. Independent grouped window expressions remain supported. No `.by`, `.keep`, `.before`, `.after`, or `NULL` column deletion. |
| Partial | `rename()` | `new = old`; currently depends on DuckDB-style `* EXCLUDE`, so it is not portable to every dialect or every preceding projection. |
| Partial | `arrange()` | Identifier keys and `asc(identifier)` / `desc(identifier)` only. |
| Partial | `group_by()` | Identifier keys only; no computed keys, `.add`, or `.drop`. |
| Partial | `summarise()` / `summarize()` | Optional alias plus one supported aggregate over an identifier. Only `arrange()` and grouping metadata changes may follow; filters, projections, further aggregation, and other downstream operations require an unimplemented subquery stage and are rejected. No `.by`, `across()`, arbitrary scalar output, or `reframe()` semantics. |
| Partial | `distinct()` | `distinct()` or identifier keys. Expressions and `.keep_all` are rejected; operations after `distinct()` and `group_by() %>% distinct()` require an unimplemented query stage. |
| Partial | `count()` | Identifier-only keys, merged with current groups. No `wt`, `sort`, `name`, `.drop`, computed/join-derived keys, or arbitrary downstream verbs; `arrange()` is allowed. |
| Partial | `tally()` | Argument-free, unweighted tally over current groups. The same downstream restrictions as `count()` apply. |
| Partial | `*_join()` | `inner`, `left`, `right`, `full`, `semi`, and `anti` with explicit `by = "key"` or string-only `by = c("same", "left" = "right", ...)`. Conditions are equality predicates joined with `AND`; non-DuckDB semi/anti joins lower to `EXISTS` / `NOT EXISTS`. No default common-key discovery, `join_by()`, inequality/rolling/overlap joins, or dplyr join options such as `suffix`, `keep`, `relationship`, `multiple`, `unmatched`, and `na_matches`. |
| Partial | Set operations | `union(table)`, `intersect(table)`, and `setdiff(table)` with an identifier right table. All downstream operations, including another set operation, are rejected until subquery stages are supported. No `union_all`, `symdiff`, or `setequal`. |

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
