#!/usr/bin/env python3
"""Execute the follow-up grammar surface against SQLite (stdlib only).

Covers tidyselect ranges/exclusions/helpers/where, across() in every supported
position, slice_min/max/sample, and the join options (natural, suffix, keep,
na_matches). The compiler is driven through the CLI's ``--schema`` flag the same
way tests/relational_execution.py does it, but this file owns its own fixtures
so the existing execution suite stays untouched.

Expected rows are hand-written SQL written from R semantics, not from compiler
output. Rows are compared as multisets together with the output column names,
except where a case declares an order or a sampling cardinality.

    python3 tests/followup_execution.py [path/to/libdplyr]
"""

import collections
import json
import os
import sqlite3
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SCRATCH = os.path.join(REPO, "target")

# Same shape as the existing harness' `data`: repeated group keys, NULL numerics,
# a zero and a negative, so ordering and NULL handling stay observable.
DATA_SOURCE = "data"
DATA_ROWS = [
    (1, "a", 10, 1),
    (2, "a", 10, None),
    (3, "a", None, 4),
    (4, "b", 5, 2),
    (5, "b", 0, None),
    (6, None, -3, 7),
    (7, "a", 20, 9),
    (8, "b", 1, 8),
    (9, "a", 30, 20),
]

# Slice fixture. `val` holds a tie (two rows of group "a" at 3 and two of group
# "b" at 8), a NULL (row 6) and a unique maximum (row 8), so with_ties, na_rm
# and per-group slicing are all observable. `alt` is NULL on row 3, which makes
# an ORDER BY over NULLs reachable through order_by.
SLICE_SOURCE = "slice_data"
SLICE_ROWS = [
    (1, "a", 5, 10),
    (2, "a", 3, 30),
    (3, "a", 3, None),
    (4, "b", 8, 8),
    (5, "b", 8, 1),
    (6, "b", None, 9),
    (7, "c", 1, 7),
    (8, "c", 9, 2),
    (9, "c", 4, 4),
    (10, "c", 2, 5),
]

# Ten NULL-free rows, so `prop` slices land on whole row counts instead of
# depending on how a fractional head count is rounded.
CLEAN_SOURCE = "slice_clean"
CLEAN_ROWS = [(i, i) for i in range(1, 11)]

# Join fixtures. The right side repeats the `id` key with a NULL, repeats the
# `grp` name (so a suffix is visible) and carries a row whose grp differs from
# the left row that shares its id (so natural_join and by-id joins disagree).
JOIN_SOURCE = "join_data"
JOIN_OTHER = "join_other"
JOIN_ROWS = [(1, "a", 10), (2, "b", 20), (None, "c", 30)]
JOIN_OTHER_ROWS = [(1, "a", "t1"), (None, "c", "t2"), (3, "z", "t3")]

# Tidyselect fixture. The names are chosen so every selector form has something
# to bite on: x / x2 and y / y2 separate prefix from suffix matches, `grp`
# matches no prefix, and `id` is a non-numeric first column so a positional
# range never coincides with a name range by accident.
TIDY_SOURCE = "tidy_data"
TIDY_ROWS = [
    (1, 10, 11, 20, 21, "a"),
    (2, 12, 13, 22, 23, "b"),
    (3, 14, 15, 24, 25, None),
]


def _schema(source, columns):
    return {
        "source": source,
        "columns": [
            {"name": name, "data_type": data_type, "nullable": True}
            for name, data_type in columns
        ],
    }


SCHEMAS = {
    DATA_SOURCE: _schema(
        DATA_SOURCE,
        [("id", "integer"), ("grp", "text"), ("x", "integer"), ("y", "integer")],
    ),
    SLICE_SOURCE: _schema(
        SLICE_SOURCE,
        [("id", "integer"), ("grp", "text"), ("val", "integer"), ("alt", "integer")],
    ),
    CLEAN_SOURCE: _schema(CLEAN_SOURCE, [("id", "integer"), ("val", "integer")]),
    JOIN_SOURCE: _schema(
        JOIN_SOURCE, [("id", "integer"), ("grp", "text"), ("v", "integer")]
    ),
    JOIN_OTHER: _schema(
        JOIN_OTHER, [("id", "integer"), ("grp", "text"), ("tag", "text")]
    ),
    TIDY_SOURCE: _schema(
        TIDY_SOURCE,
        [("id", "integer"), ("x", "integer"), ("x2", "integer"),
         ("y", "integer"), ("y2", "integer"), ("grp", "text")],
    ),
}

ROWS_BY_SOURCE = {
    DATA_SOURCE: DATA_ROWS,
    SLICE_SOURCE: SLICE_ROWS,
    CLEAN_SOURCE: CLEAN_ROWS,
    JOIN_SOURCE: JOIN_ROWS,
    JOIN_OTHER: JOIN_OTHER_ROWS,
    TIDY_SOURCE: TIDY_ROWS,
}


def find_binary(argv):
    if len(argv) > 1:
        return argv[1]
    for profile in ("debug", "release"):
        path = os.path.join(REPO, "target", profile, "libdplyr")
        if os.path.isfile(path) and os.access(path, os.X_OK):
            return path
    return None


def run_binary(binary, args):
    return subprocess.run(
        [binary] + args, capture_output=True, text=True, timeout=120, check=False
    )


def supports_schema(binary):
    if not (os.path.isfile(binary) and os.access(binary, os.X_OK)):
        return False
    try:
        out = run_binary(binary, ["--help"])
    except OSError:
        return False
    return "--schema" in (out.stdout + out.stderr)


def write_schema_file(sources, name):
    if isinstance(sources, str):
        payload = SCHEMAS[sources]
    else:
        payload = [SCHEMAS[source] for source in sources]
    path = os.path.join(SCRATCH, name)
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(payload, handle)
    return path


def transpile(binary, schema_path, code):
    proc = run_binary(binary, ["-d", "sqlite", "--schema", schema_path, "-t", code])
    if proc.returncode != 0:
        raise RuntimeError(
            "transpile failed for %r (exit %d): %s"
            % (code, proc.returncode, proc.stderr.strip() or proc.stdout.strip())
        )
    sql = proc.stdout.strip()
    while sql.endswith(";"):
        sql = sql[:-1].strip()
    if not sql:
        raise RuntimeError("empty SQL for %r" % code)
    return sql


def make_connection():
    conn = sqlite3.connect(":memory:")
    for source in ROWS_BY_SOURCE:
        columns = [
            (column["name"], column["data_type"].upper())
            for column in SCHEMAS[source]["columns"]
        ]
        types = ", ".join('"%s" %s' % (name, kind) for name, kind in columns)
        conn.execute('CREATE TABLE "%s" (%s)' % (source, types))
        conn.executemany(
            'INSERT INTO "%s" VALUES (%s)'
            % (source, ", ".join(["?"] * len(columns))),
            ROWS_BY_SOURCE[source],
        )
    return conn


def reference(sql):
    conn = make_connection()
    try:
        cursor = conn.execute(sql)
        return [d[0] for d in cursor.description], cursor.fetchall()
    finally:
        conn.close()


class Case(object):
    def __init__(
        self,
        name,
        code,
        expected_sql=None,
        ordered=False,
        sources=(),
        min_sqlite=None,
        sample_pool_sql=None,
        sample_count=None,
        sample_groups=None,
    ):
        self.name = name
        self.code = code
        # expected_sql None with no sampling expectation marks a rejection case.
        self.expected_sql = expected_sql
        self.ordered = ordered
        self.sources = tuple(sources)
        self.min_sqlite = min_sqlite
        # Sampling cases pin the row count and membership only, never which rows
        # the engine happens to draw.
        self.sample_pool_sql = sample_pool_sql
        self.sample_count = sample_count
        # Optional (column, expected per-group counts) check for a grouped
        # sample. It constrains how the draw splits across groups, never which
        # rows inside a group are drawn.
        self.sample_groups = sample_groups

    @property
    def rejected(self):
        return self.expected_sql is None and self.sample_pool_sql is None


def d(code):
    return "%s %%>%% %s" % (DATA_SOURCE, code)


SELECT_CASES = [
    # Positional ranges: 1:3 stops before the fourth column.
    Case("select_positional_range", d("select(1:3)"),
         'SELECT "id", "grp", "x" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_positional_range_tail", d("select(2:4)"),
         'SELECT "grp", "x", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_name_range", d("select(x:y)"),
         'SELECT "x", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_union_of_range_and_position", d("select(1:2, 4)"),
         'SELECT "id", "grp", "y" FROM "data"', sources=(DATA_SOURCE,)),
    # Exclusions: negative positions and negative names drop columns.
    Case("select_excludes_first_position", d("select(-1)"),
         'SELECT "grp", "x", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_excludes_two_positions", d("select(-1, -2)"),
         'SELECT "x", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_excludes_a_named_column", d("select(-x)"),
         'SELECT "id", "grp", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_range_then_exclusion_inside_it", d("select(1:3, -2)"),
         'SELECT "id", "x" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_repeated_column_yields_it_once", d("select(1, x)"),
         'SELECT "id", "x" FROM "data"', sources=(DATA_SOURCE,)),
    # Explicit order is honoured, not sorted back into schema order.
    Case("select_reorders_columns_to_the_requested_order", d("select(y, x)"),
         'SELECT "y", "x" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_reorders_a_range_with_a_column", d("select(y, 1:2)"),
         'SELECT "y", "id", "grp" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_reorders_three_columns", d("select(y, grp, id)"),
         'SELECT "y", "grp", "id" FROM "data"', sources=(DATA_SOURCE,)),
    # A rename must read its source under the old name, not the new one.
    Case("select_rename_reads_the_old_name", d("select(new = x)"),
         'SELECT "x" AS "new" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_rename_of_a_positional_column", d("select(new = 1)"),
         'SELECT "id" AS "new" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_rename_keeps_the_position_of_its_source",
         d("select(grp, new = x, y)"),
         'SELECT "grp", "x" AS "new", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_rename_of_a_range_member", d("select(x, new = y)"),
         'SELECT "x", "y" AS "new" FROM "data"', sources=(DATA_SOURCE,)),
    # Helpers.
    Case("select_starts_with", d("select(starts_with('x'))"),
         'SELECT "x" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_ends_with", d("select(ends_with('p'))"),
         'SELECT "grp" FROM "data"', sources=(DATA_SOURCE,)),
    # contains() is a literal substring test, unlike matches() which is a regex.
    Case("select_contains_is_a_literal_substring", d("select(contains('g'))"),
         'SELECT "grp" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_matches_is_a_regex", d("select(matches('^[xy]$'))"),
         'SELECT "x", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_everything_keeps_column_order", d("select(everything())"),
         'SELECT "id", "grp", "x", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_where_numeric_keeps_position", d("select(where(is.numeric))"),
         'SELECT "id", "x", "y" FROM "data"', sources=(DATA_SOURCE,)),
    Case("select_where_character_picks_grp", d("select(where(is.character))"),
         'SELECT "grp" FROM "data"', sources=(DATA_SOURCE,)),
    # Selector logic: intersection, union and negation over the helper set.
    # x / x2 match starts_with("x") and x2 / y2 match ends_with("2"), so the
    # two helpers overlap on x2 alone and the union adds y2 to the prefix set.
    Case("select_intersection_of_helpers",
         "%s %%>%% select(starts_with('x') & ends_with('2'))" % TIDY_SOURCE,
         'SELECT "x2" FROM "%s"' % TIDY_SOURCE, sources=(TIDY_SOURCE,)),
    Case("select_intersection_of_a_prefix_with_a_negated_suffix",
         "%s %%>%% select(starts_with('x') & !ends_with('2'))" % TIDY_SOURCE,
         'SELECT "x" FROM "%s"' % TIDY_SOURCE, sources=(TIDY_SOURCE,)),
    Case("select_intersection_of_a_prefix_with_a_negated_name",
         "%s %%>%% select(starts_with('x') & !x2)" % TIDY_SOURCE,
         'SELECT "x" FROM "%s"' % TIDY_SOURCE, sources=(TIDY_SOURCE,)),
    Case("select_union_of_helpers",
         "%s %%>%% select(starts_with('x') | ends_with('2'))" % TIDY_SOURCE,
         'SELECT "x", "x2", "y2" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_union_with_a_column_outside_both_helpers",
         "%s %%>%% select(starts_with('x') | grp)" % TIDY_SOURCE,
         'SELECT "x", "x2", "grp" FROM "%s"' % TIDY_SOURCE, sources=(TIDY_SOURCE,)),
    Case("select_negated_helper_excludes_every_match",
         "%s %%>%% select(!starts_with('x'))" % TIDY_SOURCE,
         'SELECT "id", "y", "y2", "grp" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_intersection_with_a_range",
         "%s %%>%% select(x:y & !y2)" % TIDY_SOURCE,
         'SELECT "x", "x2", "y" FROM "%s"' % TIDY_SOURCE, sources=(TIDY_SOURCE,)),
    Case("select_intersection_with_a_negative_selector",
         "%s %%>%% select(starts_with('x') & -c(x2))" % TIDY_SOURCE,
         'SELECT "x" FROM "%s"' % TIDY_SOURCE, sources=(TIDY_SOURCE,)),
    # Negative selectors: all-negative drops everything, all-negative next to a
    # positive one drops only the named columns.
    Case("select_all_negative_inside_c_drops_everything",
         "%s %%>%% select(c(-grp))" % TIDY_SOURCE,
         'SELECT "id", "x", "x2", "y", "y2" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_all_negative_with_positions_drops_everything",
         "%s %%>%% select(c(-1, -grp))" % TIDY_SOURCE,
         'SELECT "x", "x2", "y", "y2" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_all_negative_with_a_positive_sibling",
         "%s %%>%% select(c(-grp, x))" % TIDY_SOURCE,
         'SELECT "id", "x", "x2", "y", "y2" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_two_negative_columns_in_one_c",
         "%s %%>%% select(c(-grp, -y2))" % TIDY_SOURCE,
         'SELECT "id", "x", "x2", "y" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_negative_then_positive_siblings",
         "%s %%>%% select(c(-x2, x, y2))" % TIDY_SOURCE,
         'SELECT "id", "x", "y", "y2", "grp" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_negative_then_positive_in_two_c_calls",
         "%s %%>%% select(c(-x2), c(x, y2))" % TIDY_SOURCE,
         'SELECT "id", "x", "y", "y2", "grp" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_positive_then_negative_siblings",
         "%s %%>%% select(c(x, y2, -x2))" % TIDY_SOURCE,
         'SELECT "x", "y2" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_all_negative_nested_inside_a_positive_c",
         "%s %%>%% select(c(-c(grp, y2), x))" % TIDY_SOURCE,
         'SELECT "id", "x", "x2", "y" FROM "%s"' % TIDY_SOURCE,
         sources=(TIDY_SOURCE,)),
    Case("select_range_is_usable_downstream", d("select(1:3) %>% filter(x > 5)"),
         'SELECT "id", "grp", "x" FROM "data" WHERE "x" > 5',
         sources=(DATA_SOURCE,)),
    # Rejections: out-of-range positions, unknown names and unknown helpers.
    Case("select_position_zero_is_rejected", d("select(0)"), sources=(DATA_SOURCE,)),
    Case("select_position_past_the_end_is_rejected", d("select(1:5)"),
         sources=(DATA_SOURCE,)),
    Case("select_negative_position_past_the_front_is_rejected", d("select(-5)"),
         sources=(DATA_SOURCE,)),
    Case("select_unknown_column_is_rejected", d("select(nope)"), sources=(DATA_SOURCE,)),
    Case("select_unknown_range_start_is_rejected", d("select(nope:y)"),
         sources=(DATA_SOURCE,)),
    Case("select_intersection_with_an_unknown_column_is_rejected",
         d("select(starts_with('x') & nope)"), sources=(DATA_SOURCE,)),
    Case("select_union_with_an_unknown_helper_is_rejected",
         d("select(starts_with('x') | nope_all)"), sources=(DATA_SOURCE,)),
    Case("select_rename_of_an_unknown_column_is_rejected", d("select(new = nope)"),
         sources=(DATA_SOURCE,)),
    Case("select_chained_rename_assignment_is_rejected",
         d("select(new = renamed = x)"), sources=(DATA_SOURCE,)),
]


ACROSS_CASES = [
    # A scalar .x lambda overwrites the selected columns in place.
    Case("across_scalar_lambda_overwrites_in_place",
         d("mutate(across(c(x, y), ~ .x * 2))"),
         'SELECT "id", "grp", "x" * 2 AS "x", "y" * 2 AS "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    # The bare . pronoun is the same as .x.
    Case("across_bare_dot_lambda", d("mutate(across(x, ~ . + 1))"),
         'SELECT "id", "grp", "x" + 1 AS "x", "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    # `.x` is bound per column, so selecting x and y and returning `.x` leaves
    # each column holding its own original value.
    Case("across_uses_a_snapshot_of_the_input_columns",
         d("mutate(across(c(x, y), ~ .x))"),
         'SELECT "id", "grp", "x", "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    # The snapshot reaches columns named outside the selector too: y must read
    # the original x, not the value x was just overwritten with. Each column
    # keeps its own identity, so y receives its own original y plus the
    # original x.
    Case("across_snapshot_reaches_an_outer_column",
         d("mutate(across(c(x, y), ~ .x + x))"),
         'SELECT "id", "grp", "x" + "x" AS "x", "y" + "x" AS "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    # The selector order does not reorder the output columns: across(c(y, x))
    # still writes x before y, because the frame keeps its own column order.
    Case("across_selector_order_does_not_reorder_the_output",
         d("mutate(across(c(y, x), ~ .x + x))"),
         'SELECT "id", "grp", "x" + "x" AS "x", "y" + "x" AS "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    Case("across_then_a_later_mutate_sees_the_overwritten_column",
         d("mutate(across(x, ~ .x * 2), k = x + 1)"),
         'SELECT "id", "grp", "x" * 2 AS "x", "y", "x" * 2 + 1 AS "k" '
         'FROM "data"', sources=(DATA_SOURCE,)),
    Case("across_named_list_columns_are_visible_to_a_later_mutate",
         d("mutate(across(x, list(dbl = ~ .x * 2)), k = x_dbl)"),
         'SELECT "id", "grp", "x", "y", "x" * 2 AS "x_dbl", "x" * 2 AS "k" '
         'FROM "data"', sources=(DATA_SOURCE,)),
    # A named list produces suffixed columns instead of overwriting.
    Case("across_named_list_adds_suffixed_columns",
         d("mutate(across(c(x, y), list(dbl = ~ .x * 2)))"),
         'SELECT "id", "grp", "x", "y", "x" * 2 AS "x_dbl", '
         '"y" * 2 AS "y_dbl" FROM "data"', sources=(DATA_SOURCE,)),
    # A later assignment in the same mutate sees an earlier one.
    Case("across_is_sequential_within_mutate",
         d("mutate(k = x * 2, across(x, ~ .x + k))"),
         # k has to exist before across() can read it, so the two assignments
         # nest rather than sharing one projection.
         'SELECT "id", "grp", "x" + "k" AS "x", "y", "k" FROM ('
         'SELECT "id", "grp", "x", "y", "x" * 2 AS "k" FROM "data")',
         sources=(DATA_SOURCE,)),
    # A bare function is accepted alongside a lambda; sum() over a column
    # holding NULL yields NULL, so the aggregate must not silently drop it.
    Case("across_bare_mean_is_accepted", d("mutate(across(c(x, y), mean))"),
         'SELECT "id", "grp", AVG("x") OVER () AS "x", AVG("y") OVER () AS "y" '
         'FROM "data"', sources=(DATA_SOURCE,)),
    # Grouped summarise: one aggregate per group and per selected column.
    Case("across_in_grouped_summarise",
         d("group_by(grp) %>% summarise(across(c(x, y), ~ sum(.x)))"),
         'SELECT "grp", SUM("x") AS "x", SUM("y") AS "y" FROM "data" '
         'GROUP BY "grp"', sources=(DATA_SOURCE,)),
    Case("across_named_list_in_grouped_summarise",
         d("group_by(grp) %>% summarise(across(x, list(total = ~ sum(.x))))"),
         'SELECT "grp", SUM("x") AS "x_total" FROM "data" GROUP BY "grp"',
         sources=(DATA_SOURCE,)),
    # Selection forms other than c(x, y).
    Case("across_with_a_positional_range_selector",
         d("mutate(across(3:4, ~ .x - 1))"),
         'SELECT "id", "grp", "x" - 1 AS "x", "y" - 1 AS "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    Case("across_with_a_name_range_selector", d("mutate(across(x:y, ~ .x * 0))"),
         'SELECT "id", "grp", "x" * 0 AS "x", "y" * 0 AS "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    Case("across_with_a_negative_selector", d("mutate(across(-grp, ~ .x + 1))"),
         'SELECT "id" + 1 AS "id", "grp", "x" + 1 AS "x", "y" + 1 AS "y" '
         'FROM "data"',
         sources=(DATA_SOURCE,)),
    Case("across_with_a_starts_with_selector",
         d("mutate(across(starts_with('x'), ~ .x * 3))"),
         'SELECT "id", "grp", "x" * 3 AS "x", "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    # Ungrouped summarise evaluates once, so across yields exactly one row.
    Case("across_in_ungrouped_summarise_is_one_row",
         d("summarise(across(c(x, y), ~ sum(.x)))"),
         'SELECT SUM("x") AS "x", SUM("y") AS "y" FROM "data"',
         sources=(DATA_SOURCE,)),
    # Rejections: an unnamed across() has no output name to write to.
    Case("named_across_output_is_rejected", d("mutate(out = across(c(x, y)))"),
         sources=(DATA_SOURCE,)),
    Case("named_across_output_is_rejected_in_summarise",
         d("summarise(out = across(x))"), sources=(DATA_SOURCE,)),
    Case("across_with_an_unknown_column_is_rejected",
         d("mutate(across(nope, ~ .x))"), sources=(DATA_SOURCE,)),
]


SLICE_CASES = [
    # Reference ordering over SLICE_ROWS, id in parens. na_rm defaults to TRUE,
    # so row 6 (val NULL) is not ranked unless na_rm = FALSE asks for it, and a
    # NULL then sorts LAST in both directions.
    #   asc  (na_rm=TRUE) : 1(7), 2(10), 3(2), 3(3), 4(9), 5(1), 8(4), 8(5), 9(8)
    #   desc (na_rm=TRUE) : 9(8), 8(4), 8(5), 5(1), 4(9), 3(2), 3(3), 2(10), 1(7)
    #   na_rm=FALSE appends NULL(6) to the end of either direction.
    # A rank slice selects a membership, so no case below asserts output order:
    # only a pipeline that ends in arrange() has a defined row order.
    Case("slice_min_defaults_to_one_row", "%s %%>%% slice_min(val)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" = 7'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_min_takes_n_smallest",
         "%s %%>%% slice_min(val, n = 2)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (7, 10)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    # The third smallest value is 3, which rows 2 and 3 both hold, so with_ties
    # widens the cut past the tie boundary to four rows.
    Case("slice_min_with_ties_widens_the_cut",
         "%s %%>%% slice_min(val, n = 3, with_ties = TRUE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (7, 10, 2, 3)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_min_without_ties_keeps_exactly_n",
         "%s %%>%% slice_min(val, n = 5)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" '
         'WHERE "id" IN (7, 10, 2, 3, 9)' % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_min_excludes_null_by_default",
         "%s %%>%% slice_min(val, n = 10)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "val" IS NOT NULL'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    # Rank 10 is the NULL row, so only a cut this deep can reach it.
    Case("slice_min_na_rm_false_puts_null_last",
         "%s %%>%% slice_min(val, n = 10, na_rm = FALSE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s"' % SLICE_SOURCE,
         sources=(SLICE_SOURCE,)),
    # NULL sorting last means a short cut never reaches it.
    Case("slice_min_na_rm_false_keeps_null_out_of_a_short_cut",
         "%s %%>%% slice_min(val, n = 3, na_rm = FALSE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (7, 10, 2, 3)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_max_defaults_to_one_row", "%s %%>%% slice_max(val)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" = 8'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    # The second largest value is 8, held by rows 4 and 5, so the plain cut
    # already returns both.
    Case("slice_max_takes_n_largest",
         "%s %%>%% slice_max(val, n = 2)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (8, 4, 5)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_max_with_ties_keeps_both_eights",
         "%s %%>%% slice_max(val, n = 2, with_ties = TRUE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (8, 4, 5)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_max_excludes_null_by_default",
         "%s %%>%% slice_max(val, n = 10)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "val" IS NOT NULL'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_max_na_rm_false_puts_null_last",
         "%s %%>%% slice_max(val, n = 10, na_rm = FALSE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s"' % SLICE_SOURCE,
         sources=(SLICE_SOURCE,)),
    Case("slice_max_na_rm_false_keeps_null_out_of_a_short_cut",
         "%s %%>%% slice_max(val, n = 3, na_rm = FALSE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (8, 4, 5)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    # A computed order expression is supported: the ranking runs on val * 2,
    # which preserves the ordering of val.
    Case("slice_min_with_a_computed_order",
         "%s %%>%% slice_min(val * 2, n = 2)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (7, 10)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_max_with_a_computed_order",
         "%s %%>%% slice_max(val * 2, n = 2)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (8, 4, 5)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_n_zero_returns_no_rows",
         "%s %%>%% slice_min(val, n = 0)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE 0' % SLICE_SOURCE,
         sources=(SLICE_SOURCE,)),
    # Order only becomes observable when the pipeline ends in arrange().
    Case("slice_then_arrange_pins_the_row_order",
         "%s %%>%% slice_min(val, n = 3, with_ties = TRUE) %%>%% arrange(val)"
         % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (7, 10, 2, 3) '
         'ORDER BY "val"' % SLICE_SOURCE, ordered=True, sources=(SLICE_SOURCE,)),
    # prop: dbplyr lowers slice_*(prop = p) to CUME_DIST() <= p, not a rounded
    # row count. CLEAN_ROWS holds ten distinct values, so the cumulative share
    # is 0.1 per row and every cut lands on a whole row count.
    Case("slice_min_prop_uses_cume_dist",
         "%s %%>%% slice_min(val, prop = 0.2, with_ties = TRUE)" % CLEAN_SOURCE,
         'SELECT "id", "val" FROM "%s" WHERE "id" IN (1, 2)' % CLEAN_SOURCE,
         sources=(CLEAN_SOURCE,)),
    Case("slice_max_prop_uses_cume_dist_descending",
         "%s %%>%% slice_max(val, prop = 0.3, with_ties = TRUE)" % CLEAN_SOURCE,
         'SELECT "id", "val" FROM "%s" WHERE "id" IN (8, 9, 10)' % CLEAN_SOURCE,
         sources=(CLEAN_SOURCE,)),
    Case("slice_min_prop_without_ties_keeps_the_cut",
         "%s %%>%% slice_min(val, prop = 0.2)" % CLEAN_SOURCE,
         'SELECT "id", "val" FROM "%s" WHERE "id" IN (1, 2)' % CLEAN_SOURCE,
         sources=(CLEAN_SOURCE,)),
    # Nine non-NULL rows remain. The first two cumulative shares are 1/9
    # and 2/9; the val=3 tie reaches 4/9, so a 0.3 cut excludes that tie.
    Case("slice_min_prop_does_not_widen_a_tie_above_the_cut",
         "%s %%>%% slice_min(val, prop = 0.2, with_ties = TRUE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" = 7'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_min_prop_excludes_a_tie_crossing_the_cut",
         "%s %%>%% slice_min(val, prop = 0.3, with_ties = TRUE)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (7, 10)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    # prop is a share, so the endpoints and an over-large value are all legal:
    # 0 selects nothing, 1 and 1.5 select all non-NULL ordering rows.
    Case("slice_min_prop_one_selects_every_row",
         "%s %%>%% slice_min(val, prop = 1)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE val IS NOT NULL' % SLICE_SOURCE,
         sources=(SLICE_SOURCE,)),
    Case("slice_max_prop_above_one_selects_every_row",
         "%s %%>%% slice_max(val, prop = 1.5)" % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE val IS NOT NULL' % SLICE_SOURCE,
         sources=(SLICE_SOURCE,)),
    Case("slice_min_prop_zero_selects_no_rows",
         "%s %%>%% slice_min(val, prop = 0)" % CLEAN_SOURCE,
         'SELECT "id", "val" FROM "%s" WHERE 0' % CLEAN_SOURCE,
         sources=(CLEAN_SOURCE,)),
    # Grouped slicing: one head per group. Grouping is observable through the
    # row set, so arrange() is appended where the key order matters.
    Case("grouped_slice_max_keeps_the_tied_group_heads",
         "%s %%>%% group_by(grp) %%>%% slice_max(val, n = 1, with_ties = TRUE)"
         % SLICE_SOURCE,
         # group a: 5(1); group b: 8(4),8(5); group c: 9(8).
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (1, 4, 5, 8)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("grouped_slice_max_na_rm_false_reaches_the_null_row",
         "%s %%>%% group_by(grp) %%>%% slice_max(val, n = 3, with_ties = TRUE, "
         "na_rm = FALSE)" % SLICE_SOURCE,
         # Only group b holds a NULL and it ranks last there, so row 6 joins
         # that group while groups a and c lose their third rank.
         'SELECT "id", "grp", "val", "alt" FROM "%s" '
         'WHERE "id" IN (1, 2, 3, 4, 5, 6, 8, 9, 10)' % SLICE_SOURCE,
         sources=(SLICE_SOURCE,)),
    Case("grouped_slice_min_keeps_the_tied_group_heads",
         "%s %%>%% group_by(grp) %%>%% slice_min(val, n = 1, with_ties = TRUE)"
         % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (2, 3, 4, 5, 7)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_by_groups_without_an_explicit_group_by",
         "%s %%>%% slice_max(val, n = 1, by = 'grp', with_ties = TRUE)"
         % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt" FROM "%s" WHERE "id" IN (1, 4, 5, 8)'
         % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    Case("slice_after_filter_and_before_mutate",
         "%s %%>%% filter(id > 2) %%>%% slice_min(val, n = 1) %%>%% mutate(tag = 'k')"
         % SLICE_SOURCE,
         'SELECT "id", "grp", "val", "alt", \'k\' AS "tag" FROM "%s" '
         'WHERE "id" = 7' % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
    # Sampling: cardinality and membership only, never the drawn rows.
    Case("slice_sample_n_draws_n_rows", "%s %%>%% slice_sample(n = 3)" % SLICE_SOURCE,
         sample_pool_sql='SELECT * FROM "%s"' % SLICE_SOURCE, sample_count=3,
         sources=(SLICE_SOURCE,)),
    Case("slice_sample_prop_draws_a_share",
         "%s %%>%% slice_sample(prop = 0.4)" % CLEAN_SOURCE,
         sample_pool_sql='SELECT * FROM "%s"' % CLEAN_SOURCE, sample_count=4,
         sources=(CLEAN_SOURCE,)),
    Case("slice_sample_prop_zero_draws_nothing",
         "%s %%>%% slice_sample(prop = 0)" % CLEAN_SOURCE,
         sample_pool_sql='SELECT * FROM "%s"' % CLEAN_SOURCE, sample_count=0,
         sources=(CLEAN_SOURCE,)),
    Case("slice_sample_prop_above_one_draws_everything",
         "%s %%>%% slice_sample(prop = 1.5)" % CLEAN_SOURCE,
         sample_pool_sql='SELECT * FROM "%s"' % CLEAN_SOURCE, sample_count=10,
         sources=(CLEAN_SOURCE,)),
    Case("slice_sample_grouped_draws_one_per_group",
         "%s %%>%% slice_sample(n = 1, by = 'grp')" % SLICE_SOURCE,
         sample_pool_sql='SELECT * FROM "%s"' % SLICE_SOURCE, sample_count=3,
         sample_groups=("grp", {1, 1, 1}), sources=(SLICE_SOURCE,)),
    Case("slice_sample_after_filter_stays_inside_the_filtered_pool",
         "%s %%>%% filter(id > 5) %%>%% slice_sample(n = 2)" % SLICE_SOURCE,
         sample_pool_sql='SELECT * FROM "%s" WHERE "id" > 5' % SLICE_SOURCE,
         sample_count=2, sources=(SLICE_SOURCE,)),
    # Rejections.
    Case("slice_min_on_an_unknown_column_is_rejected",
         "%s %%>%% slice_min(nope, n = 2)" % SLICE_SOURCE, sources=(SLICE_SOURCE,)),
]


JOIN_OPTION_CASES = [
    # by = 'id' needs grp suffixing because both sides own that name; id 1 is
    # the only match, since id 3 is missing on the left and NULL never matches.
    Case("join_suffixes_the_overlapping_name_by_default",
         "%s %%>%% inner_join(%s, by = 'id')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp" AS "grp.x", j."v", o."grp" AS "grp.y", '
         'o."tag" FROM "%s" AS j JOIN "%s" AS o ON j."id" = o."id"'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("join_suffix_argument_renames_both_sides",
         "%s %%>%% inner_join(%s, by = 'id', suffix = c('_l', '_r'))"
         % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp" AS "grp_l", j."v", o."grp" AS "grp_r", '
         'o."tag" FROM "%s" AS j JOIN "%s" AS o ON j."id" = o."id"'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    # An omitted `by` is the natural join: every shared column name becomes a
    # key, so id and grp are both constrained and the output keeps one copy of
    # each. Only row id 1 matches on both keys. Left id 2 has no right partner,
    # and right id 3 exists only on the right, so neither can reach an inner
    # join; the left row with a NULL id cannot match because its grp ('c') is
    # not NULL like the right row's.
    Case("inner_join_with_an_omitted_by_is_natural",
         "%s %%>%% inner_join(%s)" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp", j."v", o."tag" FROM "%s" AS j '
         'JOIN "%s" AS o ON j."id" = o."id" AND j."grp" = o."grp"'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("left_join_with_an_omitted_by_is_natural",
         "%s %%>%% left_join(%s)" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp", j."v", o."tag" FROM "%s" AS j '
         'LEFT JOIN "%s" AS o ON j."id" = o."id" AND j."grp" = o."grp"'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    # na_matches does not change the result here: the left NULL-id row carries
    # grp 'c' while the right NULL-id row's grp is NULL, so the grp key still
    # fails and only id 1 survives.
    Case("inner_join_omitted_by_with_na_matching",
         "%s %%>%% inner_join(%s, na_matches = 'na')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp", j."v", o."tag" FROM "%s" AS j JOIN "%s" AS o '
         'ON (j."id" = o."id" OR (j."id" IS NULL AND o."id" IS NULL)) '
         'AND (j."grp" = o."grp" OR (j."grp" IS NULL AND o."grp" IS NULL))'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    # keep = TRUE retains the right key column, and both keys are renamed.
    Case("join_keep_retains_the_right_key_column",
         "%s %%>%% left_join(%s, by = 'id', keep = TRUE)" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id" AS "id.x", j."grp" AS "grp.x", j."v", '
         'o."id" AS "id.y", o."grp" AS "grp.y", o."tag" FROM "%s" AS j '
         'LEFT JOIN "%s" AS o '
         'ON j."id" = o."id"' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
    # na_matches: 'never' is the default and NULL keys stay unmatched; 'na'
    # lets the two NULL keys pair up.
    Case("inner_join_na_matches_never_is_the_default",
         "%s %%>%% inner_join(%s, by = 'id')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp" AS "grp.x", j."v", o."grp" AS "grp.y", '
         'o."tag" FROM "%s" AS j JOIN "%s" AS o ON j."id" = o."id"'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("inner_join_na_matches_never_is_explicit",
         "%s %%>%% inner_join(%s, by = 'id', na_matches = 'never')"
         % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp" AS "grp.x", j."v", o."grp" AS "grp.y", '
         'o."tag" FROM "%s" AS j JOIN "%s" AS o ON j."id" = o."id"'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("inner_join_na_matches_na_matches_the_null_key",
         "%s %%>%% inner_join(%s, by = 'id', na_matches = 'na')"
         % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp" AS "grp.x", j."v", o."grp" AS "grp.y", '
         'o."tag" FROM "%s" AS j JOIN "%s" AS o ON j."id" = o."id" '
         'OR (j."id" IS NULL AND o."id" IS NULL)' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("left_join_na_matches_na_pads_the_unmatched_rows",
         "%s %%>%% left_join(%s, by = 'id', na_matches = 'na')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp" AS "grp.x", j."v", o."grp" AS "grp.y", '
         'o."tag" FROM "%s" AS j LEFT JOIN "%s" AS o ON j."id" = o."id" '
         'OR (j."id" IS NULL AND o."id" IS NULL)' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("right_join_na_matches_na_keeps_every_right_row",
         "%s %%>%% right_join(%s, by = 'id', na_matches = 'na')"
         % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT COALESCE(j."id", o."id") AS "id", j."grp" AS "grp.x", j."v", '
         'o."grp" AS "grp.y", o."tag" FROM "%s" AS j RIGHT JOIN "%s" AS o '
         'ON j."id" = o."id" OR (j."id" IS NULL AND o."id" IS NULL)'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER), min_sqlite=(3, 39)),
    Case("full_join_na_matches_na_unions_both_sides",
         "%s %%>%% full_join(%s, by = 'id', na_matches = 'na')"
         % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT COALESCE(j."id", o."id") AS "id", j."grp" AS "grp.x", j."v", '
         'o."grp" AS "grp.y", o."tag" FROM "%s" AS j FULL JOIN "%s" AS o '
         'ON j."id" = o."id" OR (j."id" IS NULL AND o."id" IS NULL)'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER), min_sqlite=(3, 39)),
    # semi/anti are left-preserving, so their key columns keep their own names.
    Case("semi_join_keeps_matching_left_rows_once",
         "%s %%>%% semi_join(%s, by = 'id')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp", j."v" FROM "%s" AS j WHERE j."id" IN '
         '(SELECT o."id" FROM "%s" AS o)' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("semi_join_na_matches_na_admits_the_null_key",
         "%s %%>%% semi_join(%s, by = 'id', na_matches = 'na')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp", j."v" FROM "%s" AS j WHERE EXISTS (SELECT 1 FROM '
         '"%s" AS o WHERE j."id" = o."id" OR (j."id" IS NULL AND o."id" IS NULL))'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("anti_join_keeps_unmatched_left_rows",
         "%s %%>%% anti_join(%s, by = 'id')" % (JOIN_SOURCE, JOIN_OTHER),
         # na_matches = 'never' is the default here, so the left NULL id stays
         # unmatched and only id 2 survives.
         'SELECT j."id", j."grp", j."v" FROM "%s" AS j WHERE NOT EXISTS (SELECT 1 '
         'FROM "%s" AS o WHERE j."id" = o."id")' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("anti_join_na_matches_na_drops_the_null_key",
         "%s %%>%% anti_join(%s, by = 'id', na_matches = 'na')" % (JOIN_SOURCE, JOIN_OTHER),
         # With NULL matching, the left NULL id finds a partner and is dropped.
         'SELECT j."id", j."grp", j."v" FROM "%s" AS j WHERE NOT EXISTS (SELECT 1 '
         'FROM "%s" AS o WHERE j."id" = o."id" OR (j."id" IS NULL AND o."id" IS '
         'NULL))' % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("semi_join_never_keeps_the_null_row_out",
         "%s %%>%% semi_join(%s, by = 'id')" % (JOIN_SOURCE, JOIN_OTHER),
         # Only id 1 matches under 'never'; the NULL id cannot match.
         'SELECT j."id", j."grp", j."v" FROM "%s" AS j WHERE j."id" IN '
         '(SELECT o."id" FROM "%s" AS o WHERE o."id" IS NOT NULL)'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("right_join_drops_unmatched_right_rows_without_na_matching",
         "%s %%>%% right_join(%s, by = 'id')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT COALESCE(j."id", o."id") AS "id", j."grp" AS "grp.x", j."v", '
         'o."grp" AS "grp.y", o."tag" FROM "%s" AS j RIGHT JOIN "%s" AS o '
         'ON j."id" = o."id"' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER), min_sqlite=(3, 39)),
    Case("full_join_drops_the_null_key_without_na_matching",
         "%s %%>%% full_join(%s, by = 'id')" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT COALESCE(j."id", o."id") AS "id", j."grp" AS "grp.x", j."v", '
         'o."grp" AS "grp.y", o."tag" FROM "%s" AS j FULL JOIN "%s" AS o '
         'ON j."id" = o."id"' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER), min_sqlite=(3, 39)),
    # Options compose with downstream verbs.
    Case("suffix_is_reachable_from_a_downstream_select",
         "%s %%>%% inner_join(%s, by = 'id', suffix = c('_l', '_r')) "
         "%%>%% select(id, grp_l, tag)" % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT j."id", j."grp" AS "grp_l", o."tag" FROM "%s" AS j JOIN "%s" AS o '
         'ON j."id" = o."id"' % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("natural_join_then_grouped_summarise",
         "%s %%>%% inner_join(%s) %%>%% group_by(tag) %%>%% summarise(n = n())"
         % (JOIN_SOURCE, JOIN_OTHER),
         'SELECT o."tag", COUNT(*) AS "n" FROM "%s" AS j JOIN "%s" AS o '
         'ON j."id" = o."id" AND j."grp" = o."grp" GROUP BY o."tag"'
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    # Rejections.
    Case("unknown_na_matches_value_is_rejected",
         "%s %%>%% inner_join(%s, by = 'id', na_matches = 'maybe')"
         % (JOIN_SOURCE, JOIN_OTHER), sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("unknown_join_suffix_column_is_rejected",
         "%s %%>%% inner_join(%s, by = 'id', suffix = '_l')" % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
    Case("join_omitted_by_with_an_unknown_source_is_rejected",
         "%s %%>%% inner_join(no_such_table)" % JOIN_SOURCE, sources=(JOIN_SOURCE,)),
    Case("join_keep_on_a_semi_join_is_rejected",
         "%s %%>%% semi_join(%s, by = 'id', keep = TRUE)" % (JOIN_SOURCE, JOIN_OTHER),
         sources=(JOIN_SOURCE, JOIN_OTHER)),
]


ALL_GROUPS = (
    ("select", SELECT_CASES),
    ("across", ACROSS_CASES),
    ("slice", SLICE_CASES),
    ("join", JOIN_OPTION_CASES),
)


def normalize(rows, ordered):
    return rows if ordered else sorted(rows, key=repr)


def run_case(case, binary, schema_path, conn, failures):
    try:
        sql = transpile(binary, schema_path, case.code)
    except RuntimeError as err:
        failures.append(str(err))
        return

    try:
        cursor = conn.execute(sql)
        actual_columns = [d[0] for d in cursor.description]
        actual_rows = cursor.fetchall()
    except sqlite3.Error as err:
        failures.append(
            "%s: SQL execution failed (%s)\n  dplyr: %s\n  sql: %s"
            % (case.name, err, case.code, sql)
        )
        return

    if case.sample_pool_sql is not None:
        _, pool = reference(case.sample_pool_sql)
        pool_cursor = conn.execute(case.sample_pool_sql)
        expected_columns = [d[0] for d in pool_cursor.description]
        if actual_columns != expected_columns:
            failures.append(
                "%s: output column names differ\n  dplyr: %s\n  expected: %r\n"
                "  actual: %r" % (case.name, case.code, expected_columns, actual_columns)
            )
            return
        if len(actual_rows) != case.sample_count:
            failures.append(
                "%s: expected %d sampled rows, got %d\n  dplyr: %s\n  sql: %s"
                % (case.name, case.sample_count, len(actual_rows), case.code, sql)
            )
            return
        # Membership, not identity: any drawn row must exist in the input pool,
        # and no pool row may be drawn more often than it occurs there.
        remaining = collections.Counter(pool)
        for row in actual_rows:
            if remaining[row] <= 0:
                failures.append(
                    "%s: sampled row %r is not in the input pool\n  dplyr: %s"
                    % (case.name, row, case.code)
                )
                return
            remaining[row] -= 1
        if case.sample_groups is not None:
            key, expected = case.sample_groups
            index = actual_columns.index(key)
            counts = collections.Counter(row[index] for row in actual_rows)
            if set(counts.values()) != set(expected):
                failures.append(
                    "%s: grouped sample split %r, expected every group to draw %r"
                    % (case.name, dict(counts), sorted(set(expected)))
                )
                return
        print("ok   %s" % case.name)
        return

    try:
        expected_columns, expected_rows = reference(case.expected_sql)
    except sqlite3.Error as err:
        failures.append(
            "%s: expectation SQL failed (%s): %s"
            % (case.name, err, case.expected_sql)
        )
        return
    if expected_columns != actual_columns:
        failures.append(
            "%s: output column names differ\n  dplyr: %s\n  expected: %r\n  actual: %r"
            % (case.name, case.code, expected_columns, actual_columns)
        )
        return
    if normalize(expected_rows, case.ordered) != normalize(actual_rows, case.ordered):
        failures.append(
            "%s: rows differ\n  dplyr: %s\n  sql: %s\n  expected: %r\n  actual: %r"
            % (case.name, case.code, sql, expected_rows, actual_rows)
        )
        return
    print("ok   %s" % case.name)


def run_rejection(case, binary, schema_path, failures):
    try:
        sql = transpile(binary, schema_path, case.code)
    except RuntimeError:
        print("ok   %s (rejected)" % case.name)
        return
    failures.append(
        "%s: expected rejection of %r, got:\n%s" % (case.name, case.code, sql)
    )


def main(argv):
    binary = find_binary(argv)
    if binary is None:
        print(
            "FAIL: libdplyr binary not found; build it or pass a path\n"
            "      looked for target/debug/libdplyr and target/release/libdplyr"
        )
        return 1
    if not supports_schema(binary):
        print("FAIL: %s does not expose --schema" % binary)
        return 1

    conn = make_connection()
    failures = []
    skipped = []
    schema_cache = {}

    def schema_for(case):
        key = case.sources
        if key not in schema_cache:
            schema_cache[key] = write_schema_file(
                list(key), "schema_followup_%s.json" % "_".join(key)
            )
        return schema_cache[key]

    try:
        for group, cases in ALL_GROUPS:
            for case in cases:
                if case.min_sqlite and sqlite3.sqlite_version_info < case.min_sqlite:
                    skipped.append(
                        "%s: SQLite %s predates %d.%d"
                        % (case.name, sqlite3.sqlite_version,
                           case.min_sqlite[0], case.min_sqlite[1])
                    )
                    continue
                path = schema_for(case)
                if case.rejected:
                    run_rejection(case, binary, path, failures)
                else:
                    run_case(case, binary, path, conn, failures)
    finally:
        conn.close()

    for reason in skipped:
        print("skip %s" % reason)

    if failures:
        print("\n%d failure(s):" % len(failures))
        for failure in failures:
            print("-", failure)
        return 1
    print("\nall follow-up execution cases passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
