# dplyr syntax support

Audited against the current parser and SQL generator on 2026-10-08. The primary
DuckDB target is 1.5.5; PostgreSQL, MySQL, SQLite, and DuckDB share the portable
subset unless a dialect exception is noted.

Status meanings:

- **Supported**: the documented form is parsed, generated, and covered by tests.
- **Partial**: only the forms listed below are supported. Some unsupported
  options and contexts still require the schema-aware API.
- **Unsupported**: the parser or generator intentionally rejects the feature.

The [dbplyr 2.6.0 parity audit](dbplyr-parity.md) records measured coverage,
intentional R semantics, and remaining limits. The previously identified option,
NULL, ranking, and window-order errors have been corrected.

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
order those operations appear. A source may occur in more than one SQL branch. The
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

`ungroup()` clears grouping for later operations without adding a SELECT stage.
Earlier grouped summaries keep their own GROUP BY. After `ungroup()`, summary
and window aggregates are global, selection no longer retains former group
keys, and those keys may be overwritten by `mutate()`.

Input names are resolved exactly against the supplied schema. Explicit pipeline
sources must match `schema.source`; source-less pipelines use that source. The
caller must supply current metadata. Direct copies and renames retain supplied
type/nullability metadata; computed expressions with unknown result types return
unset metadata. SQL NULL and aggregate behavior follow the existing backend
contract, including NULL-inclusive `n_distinct()` in summaries.

Computed `select()` expressions and target-table assignments remain unsupported.
Use `mutate()` before selection. Grouping columns can be overwritten and are
regrouped by their new values. `arrange()` supplies window order;
`window_order()` and `window_frame(from, to)` set explicit window context.
Whole-partition `n_distinct()` uses two stages and includes NULL by default.
Moving-frame distinct counts and portable moving-frame statistics are rejected.
The shared parser bounds expression depth to 64. The compiler bounds SELECT
stages to 64. The schema-free API retains its narrower limits below.

### Verb options

`filter()` accepts zero or several conditions, `.by`, and `filter_out()`.
`mutate()` and `transmute()` support `.by`, `.keep` (`all`, `used`, `unused`,
`none`), `.before`, `.after`, and NULL column deletion. Group columns cannot be
deleted while grouped. `mutate(x = NULL)` deletes a column; `mutate(x = NA)`
keeps the column and assigns SQL NULL, including inside `across()`.
`summarise()` supports `.by` and `.groups` (`drop`,
`drop_last`, `keep`). `.by` requires ungrouped input. Computed grouping keys,
`.add`, partial `ungroup()`, computed ordering, and `.by_group` are supported.
`relocate()` and `rename_with(tolower/toupper)` change column placement/names.
`distinct(..., .keep_all = TRUE)` keeps the first row under supplied ordering.
`count/tally/add_count/add_tally` accept `wt`, `sort`, and `name`. Unobserved
factor levels (`.drop = FALSE`) need domain metadata and remain unsupported.

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

This matches dplyr's default `keep = NULL` (also `FALSE`). `keep = TRUE`
retains both key columns and applies suffixes to colliding key names.
`suffix = c("_left", "_right")` sets the two name suffixes.

Key columns are matched with SQL `=`, which **never matches NULL**, matching
[dbplyr's default `na_matches = "never"`](https://dbplyr.tidyverse.org/reference/join.tbl_sql.html).
This affects matching only, independently of the coalescing above: a
NULL key row never joins, and key coalescing still fills from the surviving
side. `na_matches = "na"` instead matches two NULL keys, including in semi
and anti joins.

Omitting `by` or using `by = NULL` infers common visible columns. No common
columns is an error; use `cross_join()` for a Cartesian product. `join_by()`
supports equality, inequalities, `between`, `within`, `overlaps`, and one
`closest()` inequality. Rolling joins retain every closest tie. Non-equality
joins retain both key columns. The right operand may be a table pipeline;
nested joins inside that operand are not implemented.
Duplicate/computed key specifications are rejected.

`multiple = "all"` and `unmatched = "drop"` need no checks. `relationship`
(`one-to-one`, `one-to-many`, `many-to-one`, `many-to-many`) and
`unmatched = "error"` use actual-match validation through the execution API.
Pure compilation rejects options that require checks. `multiple = "first"`,
`"last"`, and `"any"` remain unsupported because candidate ordering has no
explicit contract. Validation rejects volatile inputs.

### Tidy selection and across

The schema-aware path expands selection against the current ordered columns.
It supports names, one-based positions, `a:b` / `1:3`, negative selection,
`!`, set intersection `&`, union `|`, and `c()`. Selection order is preserved;
repeated selections are deduplicated. Helpers include `everything()`,
`starts_with()`, `ends_with()`, `contains()`, `matches()`, `last_col()`,
`all_of(c("x", "y"))`, and `any_of(c("x", "missing"))`. `all_of()` rejects
missing names; `any_of()` skips them. Pattern helpers default to
`ignore.case = TRUE`. `matches()` uses Rust regular expressions.
`where(is.numeric/is.integer/is.double/is.character/is.logical)` uses supplied
SQL type metadata and rejects unknown types. Ordinary `select(new = old)`
renames one selected column. Group keys are restored if omitted.

`mutate(across(...))` and `summarise(across(...))` accept the same selectors,
a function name, a `~` lambda using `.x` or `.`, a single-parameter
`function(v)` expression, and `list()` of functions.
Named lists default to `{.col}_{.fn}`; one function defaults to `{.col}`.
`.names` templates accept `{.col}` and `{.fn}`. Group keys are excluded from
selection. Each across block reads one input stage, while later ordinary mutate
assignments see the updated columns. `cur_column()` returns the current name.
General R execution, arbitrary glue expressions, and `.unpack` are unsupported.
`if_any()` and `if_all()` expand predicates with the same selectors/lambdas.
Aggregates accept one literal boolean `na.rm` option. SQL aggregates remove
NULL for both values; this does not emulate R NA propagation. `n_distinct()`
includes NULL by default and removes it with `na.rm = TRUE`.

```r
sales %>% select(region, starts_with("amount"), -amount_tax)
sales %>% mutate(across(starts_with("amount"), ~ .x * 0.9))
sales %>% group_by(region) %>% summarise(across(starts_with("amount"), list(total = sum, avg = mean)))
```

### Slice verbs

`slice_min/max(order_by, n = 1)` use ranking windows. Ordering accepts an
expression or `tibble(x, y)` / `c(x, y)` tuple. `with_ties = TRUE` preserves
ties; `FALSE` selects row numbers. `na_rm` removes missing keys or sorts them
last. Nonnegative `n` is rounded down. `prop` selects a share of each group.

Positional `slice()`, `slice_head()`, `slice_tail()`, and `tail()` require
`arrange()` or `window_order()`. Positive indices preserve request order and
duplicates; negative indices exclude rows. Positive and negative indices cannot
be mixed. Single integer ranges use a range predicate without enumeration.
Ranges nested in `c()` are currently bounded to 100001 positions. Head/tail
support negative sizes and proportions. `head(n = 6)` uses SQL LIMIT and does
not require ordering. Deterministic membership requires a total sort order.

`slice_sample()` supports `n`, `prop`, `weight_by`, `replace`, and temporary
`by`. Weighted sampling uses an exponential random key; replacement sampling
uses an independent draw and cumulative weights. Zero weights are excluded.
Nonmissing, finite, nonnegative weights and positive totals are validated by
execution plans. An empty input produces no draws. Samples have no seed or
output-order guarantee. Backend random-number precision limits tiny probabilities.

### Set operations

`union/union_all/intersect/setdiff/bind_queries` align columns by name and fill
missing columns with NULL. Union removes duplicates; union_all and bind_queries
preserve them. Right operands can contain pipelines. `intersect/setdiff(all =
TRUE)`, `symdiff`, and `setequal` remain unsupported.

### Tidyr and rows

`pivot_longer()` supports selectors, `names_to`, and `values_to`.
`pivot_wider()` supports one `names_from`, one `values_from`, `id_cols`,
`names_prefix`, `values_fill`, `values_fn` (`max`, `min`, `sum`, `mean`, `n`),
and explicit literal `keys`. The default aggregator is max. Dynamic keys use
`execute_with_pivot_discovery()`; discovery and result must share one snapshot.
An empty key domain returns distinct ID columns. Zero-column results are rejected.
Other pivot options, multiple name/value columns, and pivot specs are not implemented.

`fill()` requires explicit order and supports down/up/downup/updown and `.by`.
`expand()` and `complete()` support column domains, `nesting()`, and explicit
literal domains for existing columns. `complete()` supports `fill` and `explicit`.
Completion matches NULL domain keys; this differs from dbplyr's default SQL join.
`replace_na(list(column = value))` changes named columns. `dbplyr_uncount()`
supports weights, `.remove`, and `.id`; dynamic weights require an execution plan.
It uses recursive SQL; MySQL receives a statement-specific recursion-limit hint.

`rows_append/insert/update/patch/upsert/delete` produce SELECT results and
preserve left-column order. Right columns must be a subset of left columns.
`by` names common keys, defaulting to the first right column. Update can write
NULL; patch fills only left NULLs. Insert uses `conflict` and update/delete use
`unmatched`, with `error`/`ignore` policies. Update/patch/upsert validate unique
right keys through an execution plan. Append, insert-ignore, and delete-ignore
can compile without checks. `in_place`, copying across databases, and DML are
not implemented. Runtime row operations use SQL equality for key matching.

### Bindings, native expressions, and execution

`transpile_with_bindings(code, schemas, &HashMap<String, serde_json::Value>)`
accepts scalar constants, selection arrays, and named selection objects.
Bare columns take precedence over binding names. `.data$name` explicitly names
a column; `.env$name` explicitly names a binding. Values become typed AST
literals and are quoted. Nonfinite or lossy integer bindings are rejected.
`!!/!!!`, quosures, and arbitrary R evaluation require a host frontend and are
rejected rather than treated as SQL logical negation.

Safe native function names pass through to the backend; function installation
and semantics remain the caller's responsibility. Schema-aware `sql("...")`
parses arithmetic and function-call expressions with this parser. It is not a
full SQL parser and cannot accept SQL CASE, subqueries, statements, or raw fragments.

`plan_with_schemas()` returns an `ExecutionPlan` containing result SQL and checks.
CLI `--schema schemas.json --execution-plan` prints that plan as JSON.
`execute(plan, adapter)` begins a stable snapshot, runs checks, buffers rows,
and commits. Failures trigger rollback; rollback failures preserve both errors.
Implement `SnapshotExecutor` for the database driver. A plain READ COMMITTED
transaction is insufficient. `PivotExecutor` adds scalar key discovery in that
same snapshot. The library does not bundle database drivers. The C API and
DuckDB SELECT extension do not run execution-plan checks.

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
| Supported | `ungroup()` | Argument-free. Clears grouping for future operations while preserving the GROUP BY of an earlier summary. It does not remove the existing downstream stage restrictions. |
| Partial | `summarise()` / `summarize()` | Optional alias plus one supported aggregate over an identifier. Only `arrange()` and grouping metadata changes may follow; filters, projections, further aggregation, and other downstream operations require an unimplemented subquery stage and are rejected. No `.by`, `across()`, arbitrary scalar output, or `reframe()` semantics. |
| Partial | `distinct()` | `distinct()` or identifier keys. Expressions and `.keep_all` are rejected; operations after `distinct()` and `group_by() %>% distinct()` require an unimplemented query stage. |
| Partial | `count()` | Identifier-only keys, merged with current groups. No `wt`, `sort`, `name`, `.drop`, computed/join-derived keys, or arbitrary downstream verbs; `arrange()` is allowed. |
| Partial | `tally()` | Argument-free, unweighted tally over current groups. The same downstream restrictions as `count()` apply. |
| Partial | `*_join()` | `inner`, `left`, `right`, `full`, `semi`, and `anti` with explicit `by = "key"` or string-only `by = c("same", "left" = "right", ...)`. Conditions are equality predicates joined with `AND`; non-DuckDB semi/anti joins lower to `EXISTS` / `NOT EXISTS`. No default common-key discovery, `join_by()`, inequality/rolling/overlap joins, or dplyr join options such as `suffix`, `keep`, `relationship`, `multiple`, `unmatched`, and `na_matches`. |
| Partial | Set operations | `union(table)`, `intersect(table)`, and `setdiff(table)` with an identifier right table. All downstream operations, including another set operation, are rejected until subquery stages are supported. No Extended `union_all()`, `symdiff`, or `setequal`. |

## Expressions and helpers

| Status | Category | Supported range and limits |
|---|---|---|
| Supported | Operators | `==`, `!=`, `<`, `<=`, `>`, `>=`, `&`, `\|`, `!`, binary and unary `+` / `-`, `*`, `/`, `^`, and parentheses. `^` uses the backend's `POWER` function; SQLite requires math-function support. |
| Partial | Membership | `x %in% c(literal, ...)` or a scalar literal RHS. Numeric, string, and boolean lists are supported separately, optionally with `NA`. No mixed-type vectors, column RHS, computed members, or named vector entries. |
| Supported | Aggregates | `mean` / `avg`, `sum`, `count`, `min`, `max`, `n`, and single-column `n_distinct`. `n_distinct` includes one NULL/NA group to match dplyr's default. DuckDB additionally supports `median` and `mode`. |
| Supported | Conditional | `ifelse`, `if_else`, and ordered `case_when(condition ~ value, ..., .default = value)`. Omitted `.default` becomes SQL `NULL`. |
| Partial | `case_when()` | No empty branch list, `.ptype`, `.size`, dynamic dots, or explicit dplyr prototype/coercion emulation. |
| Supported | Predicates / missing values | `between`, `is.na`, `coalesce`, `replace_na`, and `na.replace`. |
| Supported | Math / casts | `abs`, `round`, `floor`, `ceiling` / `ceil`, `sqrt`, `sign`, `exp`, `log`, `log10`, `mod`, trigonometric helpers, and `as.numeric` / `as.double` / `as.integer` / `as.character` / `as.logical`, subject to dialect support. |
| Supported | Strings | `concat`, `paste`, `paste0`, case conversion, `str_detect`, `str_length`, `str_trim`, `substr`, `nchar`, `nzchar`, and `trimws`, subject to dialect support. |
| Supported | Window helpers | `lead`, `lag`, `rank`, `dense_rank`, `ntile`, `row_number`, `first`, `last`, and `nth_value`; grouping keys become window partitions where implemented. |

Operators follow [R's precedence](https://stat.ethz.ch/R-manual/R-devel/library/base/html/Syntax.html):
`-2^2` means `-(2^2)`, powers associate right to left, and `!x %in% c(1, 2)`
means `!(x %in% c(1, 2))`. Expression depth limits also apply to unary and power
chains. `&&` and `||` use row-wise SQL AND/OR; they do not implement R scalar
short-circuit evaluation. `%%` uses floor-based remainder, including negative
and fractional values.

Membership never returns SQL NULL. A NULL input matches `NA` in the list and
otherwise returns FALSE. `c()`, `c(NULL)`, and scalar `NULL` are empty, so they
match no value. This preserves the missing-value and empty-vector behavior of
[R's `%in%`](https://stat.ethz.ch/R-manual/R-patched/library/base/html/match.html)
for the supported literal forms. SQL backends still determine operand type
conversion and string collation. General R vector evaluation and coercion are
not implemented.

## Remaining limits

There is no complete dbplyr/R interpreter. General R functions, host data
movement (`copy_to`, `compute`, `pull`), in-place writes, `rowwise`, `reframe`,
`nest_join`, and the unlisted tidyr option combinations remain unsupported.
`bind_rows/bind_cols` are not aliases for query set operations.
SQLite scalar math and portable sd require a build with math functions.
Backend versions and function availability still affect execution.

The authoritative implementation points are
[`src/parser/parse.rs`](../src/parser/parse.rs),
[`src/parser/ast.rs`](../src/parser/ast.rs),
[`src/sql_generator/mod.rs`](../src/sql_generator/mod.rs), and
[`src/sql_generator/dialect.rs`](../src/sql_generator/dialect.rs).
