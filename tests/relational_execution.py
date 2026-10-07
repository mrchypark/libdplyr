#!/usr/bin/env python3
"""Execute schema-aware relational compiler output against SQLite (stdlib only).

The compiler is driven through the CLI's ``--schema`` flag; this harness never
edits the CLI, it only consumes its SQL output. Expected rows come from
hand-written SQL that is written independently of the compiler output, so a
regression in the compiler cannot silently rewrite the expectation.

    python3 tests/relational_execution.py [path/to/libdplyr]

``target/debug/libdplyr`` is preferred over a stale release build. A missing
binary or a missing ``--schema`` flag is a hard failure (exit 1), not a skip.
"""

import json
import os
import sqlite3
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
SCRATCH = os.path.join(REPO, "target")

# Deterministic fixture: repeated group keys, NULL numerics, zeros, negatives.
ROWS = [
    (1, "a", 10, 1),
    (2, "a", 10, None),
    (3, "a", None, 4),
    (4, "b", 5, 2),
    (5, "b", 0, None),
    (6, None, -3, 7),
    # Later ids carry larger x/y, so ordering by the pre-overwrite x DESC is
    # distinguishable from the incidental id order.
    (7, "a", 20, 9),
    (8, "b", 1, 8),
    (9, "a", 30, 20),
]

EMPTY_SOURCE = "empty_data"
COLUMNS = ["id", "grp", "x", "y"]
# Placeholder args so a %-formatted case can spell a literal % without doubling.
NO_ARGS = ()

# Join fixtures. `other` deliberately carries a duplicated key (2), a key with
# no counterpart on the left (10) and a NULL key, so row multiplication,
# unmatched sides and "NULL never matches" are all observable. It also repeats
# `grp`, which is the overlapping non-key column dplyr suffixes to .x/.y.
OTHER_SOURCE = "other"
OTHER_ROWS = [
    (1, "a", "t1"),
    (2, "b", "t2"),
    (2, None, "t3"),
    (4, None, "t4"),
    (10, "z", "t10"),
    (None, "z", "tnull"),
]

# Differently named keys, plus NULL keys on both sides.
LEFT_SOURCE = "left_side"
RIGHT_SOURCE = "right_side"
LEFT_ROWS = [(1, "p"), (2, "q"), (3, "r"), (10, "s"), (None, "t")]
RIGHT_ROWS = [(1, "P"), (2, "Q"), (2, "Q2"), (20, "R"), (None, "T")]

# Key-only fixtures: NULL keys cannot join on either side.
NULL_LEFT_SOURCE = "null_keys"
NULL_RIGHT_SOURCE = "null_keys_b"
NULL_LEFT_ROWS = [(1, "a"), (None, "b"), (2, "c")]
NULL_RIGHT_ROWS = [(1, "x"), (2, "y"), (2, "z"), (None, "w")]

# Suffix fixture: `v` is an overlapping non-key column, so the join output
# must be v.x / v.y. The right side also owns the literal name "v.x", which
# forces the suffix resolver to keep every output name unique.
SUFFIX_LEFT_SOURCE = "suffix_left"
SUFFIX_RIGHT_SOURCE = "suffix_right"
SUFFIX_CLASH_SOURCE = "suffix_clash"
SUFFIX_LEFT_ROWS = [(1, "a"), (2, "b")]
SUFFIX_RIGHT_ROWS = [(1, "x"), (2, "y"), (2, "z")]
SUFFIX_CLASH_ROWS = [(1, "cv", "cx"), (2, "cv", "cy")]

# Aggregate fixture. Every group's sum(x) divides evenly by its row count, so
# sum(x) / n() has the same value whether the SQL engine returns an integer or
# a float and the comparison stays exact.
AGG_SOURCE = "agg_data"
AGG_ROWS = [
    ("a", 2, 2),
    ("a", 4, 3),
    ("a", 3, None),
    ("b", 4, None),
    ("b", 8, 5),
    (None, 6, 6),
]

# Ratio fixtures. AGG_ROWS is chosen so sum(x) / n() divides evenly, which
# hides integer truncation; these do not, so a float-typed division is forced.
RATIO_SOURCE = "ratio_data"
RATIO_ROWS = [(1,), (2,)]
ODD_RATIO_SOURCE = "odd_ratio_data"
ODD_RATIO_ROWS = [("a", 1), ("a", 1), ("a", 2), ("b", 3), ("b", 6)]

# Set-operation fixtures. set_b declares its schema in the opposite column
# order and a compatible but different integer type for `key`.
SET_A_SOURCE = "set_a"
SET_B_SOURCE = "set_b"
SET_C_SOURCE = "set_c"
SET_D_SOURCE = "set_d"
SET_A_ROWS = [(1, "p"), (1, "p"), (2, "q"), (None, "n")]
SET_B_ROWS = [(2, "q"), (3, "r"), (None, "n"), (3, "r")]
SET_C_ROWS = [(1, "p"), (2, "q")]
SET_D_ROWS = [(1, "p", 9), (2, "q", 8)]


def _schema(source, columns):
    return {
        "source": source,
        "columns": [
            {"name": name, "data_type": data_type, "nullable": True}
            for name, data_type in columns
        ],
    }


SCHEMAS = {
    "data": _schema(
        "data",
        [("id", "integer"), ("grp", "text"), ("x", "integer"), ("y", "integer")],
    ),
    EMPTY_SOURCE: _schema(
        EMPTY_SOURCE,
        [("id", "integer"), ("grp", "text"), ("x", "integer"), ("y", "integer")],
    ),
    OTHER_SOURCE: _schema(
        OTHER_SOURCE, [("id", "integer"), ("grp", "text"), ("tag", "text")]
    ),
    LEFT_SOURCE: _schema(LEFT_SOURCE, [("lid", "integer"), ("lval", "text")]),
    RIGHT_SOURCE: _schema(RIGHT_SOURCE, [("rid", "integer"), ("rval", "text")]),
    NULL_LEFT_SOURCE: _schema(NULL_LEFT_SOURCE, [("kid", "integer"), ("v", "text")]),
    NULL_RIGHT_SOURCE: _schema(NULL_RIGHT_SOURCE, [("kid", "integer"), ("w", "text")]),
    SUFFIX_LEFT_SOURCE: _schema(SUFFIX_LEFT_SOURCE, [("kid", "integer"), ("v", "text")]),
    SUFFIX_RIGHT_SOURCE: _schema(
        SUFFIX_RIGHT_SOURCE, [("kid", "integer"), ("v", "text")]
    ),
    SUFFIX_CLASH_SOURCE: _schema(
        SUFFIX_CLASH_SOURCE, [("kid", "integer"), ("v", "text"), ("v.x", "text")]
    ),
    AGG_SOURCE: _schema(
        AGG_SOURCE, [("g", "text"), ("x", "integer"), ("y", "integer")]
    ),
    RATIO_SOURCE: _schema(RATIO_SOURCE, [("v", "integer")]),
    ODD_RATIO_SOURCE: _schema(ODD_RATIO_SOURCE, [("g", "text"), ("v", "integer")]),
    SET_A_SOURCE: _schema(SET_A_SOURCE, [("key", "integer"), ("val", "text")]),
    # Reordered metadata and a compatible-but-different integer type.
    SET_B_SOURCE: _schema(SET_B_SOURCE, [("val", "text"), ("key", "bigint")]),
    SET_C_SOURCE: _schema(SET_C_SOURCE, [("key", "integer"), ("other", "text")]),
    SET_D_SOURCE: _schema(
        SET_D_SOURCE, [("key", "integer"), ("val", "text"), ("extra", "integer")]
    ),
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
    """Writes one schema object, or a JSON array of them, to a file."""
    path = os.path.join(SCRATCH, name)
    if isinstance(sources, str):
        payload = SCHEMAS[sources]
    else:
        payload = [SCHEMAS[source] for source in sources]
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(payload, handle)
    return path


def transpile(binary, schema_path, code):
    proc = run_binary(
        binary, ["-d", "sqlite", "--schema", schema_path, "-t", code]
    )
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

    def table(source, columns, rows):
        # Column names are quoted: a suffix fixture deliberately declares a
        # dotted name such as "v.x", which SQLite would otherwise reject.
        types = ", ".join('"%s" %s' % (name, kind) for name, kind in columns)
        conn.execute('CREATE TABLE "%s" (%s)' % (source, types))
        conn.executemany(
            'INSERT INTO "%s" VALUES (%s)'
            % (source, ", ".join(["?"] * len(columns))),
            rows,
        )

    for source, rows in (
        ("data", ROWS),
        (EMPTY_SOURCE, []),
        (OTHER_SOURCE, OTHER_ROWS),
        (LEFT_SOURCE, LEFT_ROWS),
        (RIGHT_SOURCE, RIGHT_ROWS),
        (NULL_LEFT_SOURCE, NULL_LEFT_ROWS),
        (NULL_RIGHT_SOURCE, NULL_RIGHT_ROWS),
        (SUFFIX_LEFT_SOURCE, SUFFIX_LEFT_ROWS),
        (SUFFIX_RIGHT_SOURCE, SUFFIX_RIGHT_ROWS),
        (SUFFIX_CLASH_SOURCE, SUFFIX_CLASH_ROWS),
        (AGG_SOURCE, AGG_ROWS),
        (RATIO_SOURCE, RATIO_ROWS),
        (ODD_RATIO_SOURCE, ODD_RATIO_ROWS),
        (SET_A_SOURCE, SET_A_ROWS),
        (SET_B_SOURCE, SET_B_ROWS),
        (SET_C_SOURCE, SET_C_ROWS),
        (SET_D_SOURCE, SET_D_ROWS),
    ):
        columns = [(column["name"], column["data_type"].upper()) for column in SCHEMAS[source]["columns"]]
        # set_b declares its schema in a different order than its storage order.
        if source == SET_B_SOURCE:
            rows = [(value, key) for key, value in rows]
        table(source, columns, rows)
    return conn


def reference(sql):
    conn = make_connection()
    try:
        cursor = conn.execute(sql)
        return [description[0] for description in cursor.description], cursor.fetchall()
    finally:
        conn.close()


class Case(object):
    def __init__(
        self,
        name,
        code,
        expected_sql,
        ordered=False,
        empty=False,
        sources=("data",),
        min_sqlite=None,
    ):
        self.name = name
        self.code = code
        self.expected_sql = expected_sql
        # arrange() makes row order significant; otherwise compare multisets.
        self.ordered = ordered
        self.empty = empty
        self.sources = sources
        # SQLite only grew RIGHT/FULL JOIN in 3.39; older builds cannot check
        # those cases at all, so they are reported as skipped, not failed.
        self.min_sqlite = min_sqlite


CASES = [
    Case(
        "literal_summary_on_nonempty_input_is_one_row",
        "data %>% summarise(k = 1) %>% distinct()",
        'SELECT 1 AS "k"',
    ),
    Case(
        "filter_excludes_null",
        "data %>% filter(x > 5)",
        'SELECT "id", "grp", "x", "y" FROM "data" WHERE "x" > 5',
    ),
    Case(
        "dependent_mutate",
        "data %>% mutate(a = x * 2, b = a + 1) %>% arrange(id)",
        'SELECT "id", "grp", "x", "y", "a", "b" FROM ('
        'SELECT "id", "grp", "x", "y", "a", "a" + 1 AS "b" FROM ('
        'SELECT "id", "grp", "x", "y", "x" * 2 AS "a" FROM "data"'
        ') AS t0) AS t1 ORDER BY "id"',
        ordered=True,
    ),
    Case(
        "mutate_then_filter_derived",
        "data %>% mutate(t = x + y) %>% filter(t > 10) %>% arrange(id)",
        'SELECT "id", "grp", "x", "y", "t" FROM ('
        'SELECT "id", "grp", "x", "y", "x" + "y" AS "t" FROM "data"'
        ') WHERE "t" > 10 ORDER BY "id"',
    ),
    Case(
        "grouped_summarise",
        "data %>% group_by(grp) %>% summarise(n = n(), total = sum(x)) %>% arrange(grp)",
        'SELECT "grp", COUNT(*) AS "n", SUM("x") AS "total" FROM "data" '
        'GROUP BY "grp" ORDER BY "grp"',
        ordered=True,
    ),
    Case(
        "filter_after_summarise",
        "data %>% group_by(grp) %>% summarise(n = n()) %>% filter(n >= 3) %>% arrange(grp)",
        'SELECT "grp", COUNT(*) AS "n" FROM "data" GROUP BY "grp" '
        'HAVING COUNT(*) >= 3 ORDER BY "grp"',
        ordered=True,
    ),
    Case(
        "count_derived_column",
        "data %>% group_by(grp) %>% count() %>% arrange(grp)",
        'SELECT "grp", COUNT(*) AS "n" FROM "data" GROUP BY "grp" ORDER BY "grp"',
        ordered=True,
    ),
    Case(
        "rename_binding",
        "data %>% rename(value = x) %>% filter(value > 5) %>% arrange(id)",
        'SELECT "id", "grp", "x" AS "value", "y" FROM "data" WHERE "x" > 5 '
        'ORDER BY "id"',
        ordered=True,
    ),
    Case(
        "distinct_then_filter",
        "data %>% distinct(grp) %>% filter(grp != 'a')",
        'SELECT DISTINCT "grp" FROM "data" WHERE "grp" != \'a\'',
    ),
    Case(
        "overwrite_mutate",
        "data %>% mutate(v = x) %>% mutate(v = y) %>% arrange(id)",
        'SELECT "id", "grp", "x", "y", "y" AS "v" FROM "data" ORDER BY "id"',
        ordered=True,
    ),
    Case(
        "grouped_select_keeps_keys",
        "data %>% group_by(grp) %>% select(grp, y) %>% summarise(n = n()) %>% arrange(grp)",
        'SELECT "grp", COUNT(*) AS "n" FROM (SELECT "grp", "y" FROM "data") '
        'GROUP BY "grp" ORDER BY "grp"',
        ordered=True,
    ),
    Case(
        "late_group_by_on_derived_key",
        "data %>% summarise(total = sum(x)) %>% group_by(total) %>% summarise(grand = sum(total))",
        'SELECT "total", SUM("total") AS "grand" FROM '
        '(SELECT SUM("x") AS "total" FROM "data") AS t GROUP BY "total"',
    ),
    Case(
        "sort_survives_select_dropping_key",
        "data %>% arrange(desc(x)) %>% select(id, grp)",
        'SELECT "id", "grp" FROM "data" ORDER BY "x" DESC',
        ordered=True,
    ),
    Case(
        "sort_survives_overwriting_key",
        "data %>% arrange(desc(x)) %>% mutate(x = y)",
        # The sort must keep using the pre-overwrite x, so the original value is
        # projected under its own name and ordered by before being shadowed.
        'SELECT "id", "grp", "y" AS "x", "y" FROM ('
        'SELECT "id", "grp", "x" AS "orig_x", "y" FROM "data"'
        ') AS t ORDER BY "orig_x" DESC',
        ordered=True,
    ),
    Case(
        "n_distinct_is_null_inclusive",
        "data %>% group_by(grp) %>% summarise(u = n_distinct(x)) %>% arrange(grp)",
        'SELECT "grp", COUNT(DISTINCT "x") + '
        'CASE WHEN COUNT(*) > COUNT("x") THEN 1 ELSE 0 END AS "u" '
        'FROM "data" GROUP BY "grp" ORDER BY "grp"',
        ordered=True,
    ),
    Case(
        "grouped_mean_mutate",
        "data %>% group_by(grp) %>% mutate(mean = mean(x)) %>% filter(mean > 5) %>% arrange(grp)",
        'SELECT "id", "grp", "x", "y", "m" AS "mean" FROM ('
        'SELECT "id", "grp", "x", "y", AVG("x") OVER (PARTITION BY "grp") AS "m" '
        'FROM "data") WHERE "m" > 5 ORDER BY "grp"',
        ordered=True,
    ),
    Case(
        "filter_against_window_mean",
        "data %>% group_by(grp) %>% filter(x > mean(x)) %>% arrange(grp, id)",
        'SELECT "id", "grp", "x", "y" FROM ('
        'SELECT "id", "grp", "x", "y", "x" > AVG("x") OVER (PARTITION BY "grp") AS "__m" '
        'FROM "data"'
        ') AS t WHERE "__m" ORDER BY "grp", "id"',
        ordered=True,
    ),
    Case(
        "hidden_order_survives_window_filter",
        "data %>% arrange(desc(x)) %>% select(id, grp, y) %>% group_by(grp) %>% filter(y > mean(y))",
        # The sort key survives two projections and a window filter, and is
        # applied last, so the ordering is the pre-overwrite x DESC.
        'SELECT "id", "grp", "y" FROM ('
        'SELECT "id", "grp", "y", "orig_x" AS "__ord", '
        '"y" > AVG("y") OVER (PARTITION BY "grp") AS "__keep" FROM ('
        'SELECT "id", "grp", "y", "x" AS "orig_x" FROM "data"'
        ') AS t0'
        ') AS t1 WHERE "__keep" ORDER BY "__ord" DESC',
        ordered=True,
    ),
    Case(
        "named_scalar_with_window_aggregate",
        "data %>% mutate(r = round(x = mean(x), digits = 1)) %>% arrange(id)",
        'SELECT "id", "grp", "x", "y", ROUND("m", 1) AS "r" FROM ('
        'SELECT "id", "grp", "x", "y", AVG("x") OVER () AS "m" FROM "data"'
        ') AS t ORDER BY "id"',
        ordered=True,
    ),
    Case(
        "n_distinct_window_is_rejected",
        "data %>% filter(n_distinct(x) > 1)",
        None,
    ),
    Case(
        "join_is_rejected",
        'data %>% inner_join(other, by = "id")',
        None,
    ),
    Case(
        "union_is_rejected",
        "data %>% union(other)",
        None,
    ),
    Case(
        "computed_select_is_rejected",
        "data %>% select(z = x + 1)",
        None,
    ),
]

EMPTY_CASES = [
    Case(
        "literal_summary_on_empty_input_is_one_row",
        EMPTY_SOURCE + " %>% summarise(k = 1)",
        'SELECT 1 AS "k"',
    ),
    Case(
        "empty_input_filter",
        EMPTY_SOURCE + " %>% filter(x > 1)",
        'SELECT "id", "grp", "x", "y" FROM "%s" WHERE "x" > 1' % EMPTY_SOURCE,
        empty=True,
    ),
    Case(
        "empty_input_summarise",
        EMPTY_SOURCE + " %>% group_by(grp) %>% summarise(n = n())",
        'SELECT "grp", COUNT(*) AS "n" FROM "%s" GROUP BY "grp"' % EMPTY_SOURCE,
        empty=True,
    ),
    Case(
        "empty_input_mutate",
        EMPTY_SOURCE + " %>% mutate(t = x + y)",
        'SELECT "id", "grp", "x", "y", "x" + "y" AS "t" FROM "%s"' % EMPTY_SOURCE,
        empty=True,
    ),
]


# --- Joins -----------------------------------------------------------------
# Every expectation is written against the fixture rows above, independently of
# the compiler's SQL. Non-equality predicates are out of scope, so only `by`
# keys appear here.

JOIN_CASES = [
    Case(
        "inner_join_multiplies_duplicate_keys",
        'data %>% inner_join(other, by = "id") %>% arrange(id, tag)',
        'SELECT d."id", d."grp" AS "grp.x", d."x", d."y", o."grp" AS "grp.y", '
        'o."tag" FROM "data" AS d JOIN "other" AS o ON d."id" = o."id" '
        'ORDER BY d."id", o."tag"',
        ordered=True,
        sources=("data", OTHER_SOURCE),
    ),
    Case(
        "inner_join_drops_unmatched_rows",
        'data %>% inner_join(other, by = "id")',
        'SELECT d."id", d."grp" AS "grp.x", d."x", d."y", o."grp" AS "grp.y", '
        'o."tag" FROM "data" AS d JOIN "other" AS o ON d."id" = o."id"',
        sources=("data", OTHER_SOURCE),
    ),
    Case(
        "left_join_keeps_all_left_rows",
        'data %>% left_join(other, by = "id")',
        'SELECT d."id", d."grp" AS "grp.x", d."x", d."y", o."grp" AS "grp.y", '
        'o."tag" FROM "data" AS d LEFT JOIN "other" AS o ON d."id" = o."id"',
        sources=("data", OTHER_SOURCE),
    ),
    Case(
        "right_join_keeps_all_right_rows",
        'data %>% right_join(other, by = "id")',
        # The key is coalesced into one output column, same as a full join: a
        # right-only row reports its own key value (10).
        'SELECT COALESCE(d."id", o."id") AS "id", d."grp" AS "grp.x", d."x", '
        'd."y", o."grp" AS "grp.y", o."tag" FROM "data" AS d '
        'RIGHT JOIN "other" AS o ON d."id" = o."id"',
        sources=("data", OTHER_SOURCE),
        min_sqlite=(3, 39),
    ),
    Case(
        "full_join_coalesces_the_key",
        'data %>% full_join(other, by = "id")',
        'SELECT COALESCE(d."id", o."id") AS "id", d."grp" AS "grp.x", d."x", '
        'd."y", o."grp" AS "grp.y", o."tag" FROM "data" AS d '
        'FULL JOIN "other" AS o ON d."id" = o."id"',
        sources=("data", OTHER_SOURCE),
        min_sqlite=(3, 39),
    ),
    Case(
        "semi_join_keeps_left_once_per_match",
        'data %>% semi_join(other, by = "id")',
        'SELECT d."id", d."grp", d."x", d."y" FROM "data" AS d '
        'WHERE d."id" IN (SELECT o."id" FROM "other" AS o)',
        sources=("data", OTHER_SOURCE),
    ),
    Case(
        "anti_join_drops_every_matched_key",
        'data %>% anti_join(other, by = "id")',
        'SELECT d."id", d."grp", d."x", d."y" FROM "data" AS d '
        'WHERE NOT EXISTS (SELECT 1 FROM "other" AS o WHERE d."id" = o."id")',
        sources=("data", OTHER_SOURCE),
    ),
    Case(
        "null_key_never_matches",
        '%s %%>%% inner_join(%s, by = "kid")'
        % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        'SELECT l."kid", l."v", r."w" FROM "%s" AS l JOIN "%s" AS r '
        'ON l."kid" = r."kid"' % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        sources=(NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
    ),
    Case(
        "null_key_survives_left_join",
        '%s %%>%% left_join(%s, by = "kid")'
        % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        'SELECT l."kid", l."v", r."w" FROM "%s" AS l LEFT JOIN "%s" AS r '
        'ON l."kid" = r."kid"' % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        sources=(NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
    ),
    Case(
        "semi_join_with_duplicate_right_rows",
        '%s %%>%% semi_join(%s, by = "kid")'
        % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        'SELECT l."kid", l."v" FROM "%s" AS l WHERE l."kid" IN '
        '(SELECT r."kid" FROM "%s" AS r)' % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        sources=(NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
    ),
    Case(
        "anti_join_with_duplicate_right_rows",
        '%s %%>%% anti_join(%s, by = "kid")'
        % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        # The duplicated right key (2) still matches, and the left NULL key
        # never matches anything, so that one row survives. NOT EXISTS rather
        # than NOT IN: NOT IN yields NULL for a NULL left key and would drop it.
        'SELECT l."kid", l."v" FROM "%s" AS l WHERE NOT EXISTS (SELECT 1 FROM "%s" AS r '
        'WHERE l."kid" = r."kid")'
        % (NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
        sources=(NULL_LEFT_SOURCE, NULL_RIGHT_SOURCE),
    ),
    Case(
        "different_key_names_join",
        '%s %%>%% inner_join(%s, by = c("lid" = "rid"))'
        % (LEFT_SOURCE, RIGHT_SOURCE),
        'SELECT l."lid", l."lval", r."rval" FROM "%s" AS l JOIN "%s" AS r '
        'ON l."lid" = r."rid"' % (LEFT_SOURCE, RIGHT_SOURCE),
        sources=(LEFT_SOURCE, RIGHT_SOURCE),
    ),
    Case(
        "different_key_names_full_join_coalesces",
        '%s %%>%% full_join(%s, by = c("lid" = "rid"))'
        % (LEFT_SOURCE, RIGHT_SOURCE),
        'SELECT COALESCE(l."lid", r."rid") AS "lid", l."lval", r."rval" '
        'FROM "%s" AS l FULL JOIN "%s" AS r ON l."lid" = r."rid"'
        % (LEFT_SOURCE, RIGHT_SOURCE),
        sources=(LEFT_SOURCE, RIGHT_SOURCE),
        min_sqlite=(3, 39),
    ),
    Case(
        "same_name_key_full_join_keeps_one_key_column",
        'data %>% full_join(other, by = "id") %>% '
        "select(id, grp.x, grp.y, tag)",
        # The coalesced key must be reachable by its own name, exactly once,
        # after the join rather than only inside the join projection.
        'SELECT COALESCE(d."id", o."id") AS "id", d."grp" AS "grp.x", '
        'o."grp" AS "grp.y", o."tag" FROM "data" AS d '
        'FULL JOIN "other" AS o ON d."id" = o."id"',
        sources=("data", OTHER_SOURCE),
        min_sqlite=(3, 39),
    ),
    Case(
        "join_suffix_suffixes_the_overlapping_column",
        # `v` exists on both sides, so the output must expose v.x / v.y and a
        # downstream verb must be able to name them. Row order is compared as
        # a multiset: ORDER BY over a join output is ambiguous in SQLite.
        '%s %%>%% inner_join(%s, by = "kid") %%>%% select(kid, v.x, v.y)'
        % (SUFFIX_LEFT_SOURCE, SUFFIX_RIGHT_SOURCE),
        'SELECT l."kid", l."v" AS "v.x", r."v" AS "v.y" FROM "%s" AS l '
        'JOIN "%s" AS r ON l."kid" = r."kid"'
        % (SUFFIX_LEFT_SOURCE, SUFFIX_RIGHT_SOURCE),
        sources=(SUFFIX_LEFT_SOURCE, SUFFIX_RIGHT_SOURCE),
    ),
    Case(
        "join_suffix_collision_stays_unique",
        # The right side already owns the name a suffix would produce, so the
        # resolver must push the overlapping column further out and never emit
        # the same output name twice. The pinned aliases make any regression in
        # collision handling visible, since the harness compares column names
        # as well as rows.
        '%s %%>%% inner_join(%s, by = "kid")'
        % (SUFFIX_LEFT_SOURCE, SUFFIX_CLASH_SOURCE),
        'SELECT l."kid", l."v" AS "v.x.x", r."v" AS "v.y", r."v.x" FROM "%s" AS l '
        'JOIN "%s" AS r ON l."kid" = r."kid"'
        % (SUFFIX_LEFT_SOURCE, SUFFIX_CLASH_SOURCE),
        sources=(SUFFIX_LEFT_SOURCE, SUFFIX_CLASH_SOURCE),
    ),
    Case(
        "mutate_filter_group_after_join",
        'data %%>%% inner_join(other, by = "id") %%>%% mutate(total = x + y) '
        "%%>%% group_by(tag) %%>%% summarise(n = n()) %%>%% arrange(tag)" % NO_ARGS,
        'SELECT o."tag", COUNT(*) AS "n" FROM "data" AS d JOIN "other" AS o '
        'ON d."id" = o."id" GROUP BY o."tag" ORDER BY o."tag"',
        ordered=True,
        sources=("data", OTHER_SOURCE),
    ),
    Case(
        "filter_after_join_uses_the_right_column",
        'data %>% left_join(other, by = "id") %>% filter(tag == "t2")',
        'SELECT d."id", d."grp" AS "grp.x", d."x", d."y", o."grp" AS "grp.y", '
        'o."tag" FROM "data" AS d LEFT JOIN "other" AS o ON d."id" = o."id" '
        'WHERE o."tag" = \'t2\'',
        sources=("data", OTHER_SOURCE),
    ),
    Case(
        "join_on_a_non_equality_predicate_is_rejected",
        "data %>% inner_join(other, by = x > 1)",
        None,
        sources=("data", OTHER_SOURCE),
    ),
]


# --- Set operations --------------------------------------------------------
# `set_b` declares (val, key) while storing (key, val): the compiler must align
# by name, not by the schema order it was handed.

SET_CASES = [
    Case(
        "union_aligns_by_name_not_input_order",
        "%s %%>%% union(%s)" % (SET_A_SOURCE, SET_B_SOURCE),
        'SELECT "key", "val" FROM "%s" UNION SELECT "key", "val" FROM "%s"'
        % (SET_A_SOURCE, SET_B_SOURCE),
        sources=(SET_A_SOURCE, SET_B_SOURCE),
    ),
    Case(
        "intersect_aligns_by_name",
        "%s %%>%% intersect(%s)" % (SET_A_SOURCE, SET_B_SOURCE),
        'SELECT "key", "val" FROM "%s" INTERSECT SELECT "key", "val" FROM "%s"'
        % (SET_A_SOURCE, SET_B_SOURCE),
        sources=(SET_A_SOURCE, SET_B_SOURCE),
    ),
    Case(
        "setdiff_aligns_by_name",
        "%s %%>%% setdiff(%s)" % (SET_A_SOURCE, SET_B_SOURCE),
        'SELECT "key", "val" FROM "%s" EXCEPT SELECT "key", "val" FROM "%s"'
        % (SET_A_SOURCE, SET_B_SOURCE),
        sources=(SET_A_SOURCE, SET_B_SOURCE),
    ),
    Case(
        "union_drops_duplicates_and_keeps_null",
        "%s %%>%% union(%s)" % (SET_A_SOURCE, SET_B_SOURCE),
        # set_a holds (1,p) twice and set_b holds (3,r) twice; dplyr's union()
        # is distinct, so both pairs collapse. NULL is a value for set
        # operations and survives rather than disappearing.
        'SELECT "key", "val" FROM "%s" UNION SELECT "key", "val" FROM "%s"'
        % (SET_A_SOURCE, SET_B_SOURCE),
        sources=(SET_A_SOURCE, SET_B_SOURCE),
    ),
    Case(
        "intersect_treats_null_as_a_matching_value",
        "%s %%>%% intersect(%s)" % (SET_A_SOURCE, SET_B_SOURCE),
        'SELECT "key", "val" FROM "%s" INTERSECT SELECT "key", "val" FROM "%s"'
        % (SET_A_SOURCE, SET_B_SOURCE),
        sources=(SET_A_SOURCE, SET_B_SOURCE),
    ),
    Case(
        "set_op_then_operations_are_allowed",
        "%s %%>%% union(%s) %%>%% filter(val != 'p')" % (SET_A_SOURCE, SET_B_SOURCE),
        'SELECT "key", "val" FROM (SELECT "key", "val" FROM "%s" UNION SELECT "key", '
        '"val" FROM "%s") WHERE "val" != \'p\'' % (SET_A_SOURCE, SET_B_SOURCE),
        sources=(SET_A_SOURCE, SET_B_SOURCE),
    ),
    Case(
        "set_op_then_group_and_summarise",
        "%s %%>%% union(%s) %%>%% group_by(val) %%>%% summarise(n = n())"
        % (SET_A_SOURCE, SET_B_SOURCE),
        'SELECT "val", COUNT(*) AS "n" FROM (SELECT "key", "val" FROM "%s" '
        'UNION SELECT "key", "val" FROM "%s") GROUP BY "val"'
        % (SET_A_SOURCE, SET_B_SOURCE),
        sources=(SET_A_SOURCE, SET_B_SOURCE),
    ),
    Case(
        "mismatched_column_names_are_rejected",
        "%s %%>%% union(%s)" % (SET_A_SOURCE, SET_C_SOURCE),
        None,
        sources=(SET_A_SOURCE, SET_C_SOURCE),
    ),
    Case(
        "mismatched_column_count_is_rejected",
        "%s %%>%% union(%s)" % (SET_A_SOURCE, SET_D_SOURCE),
        None,
        sources=(SET_A_SOURCE, SET_D_SOURCE),
    ),
    Case(
        "unknown_set_source_is_rejected",
        "%s %%>%% union(no_such_table)" % SET_A_SOURCE,
        None,
        sources=(SET_A_SOURCE,),
    ),
]


# --- Compound aggregates ----------------------------------------------------
# Every AGG_ROWS group's sum(x) divides evenly by n(), so sum(x) / n() is exact
# whether SQLite hands back an integer or a float.

AGG_CASES = [
    Case(
        "sum_of_a_product",
        "%s %%>%% group_by(g) %%>%% summarise(p = sum(x * y))" % AGG_SOURCE,
        'SELECT "g", SUM("x" * "y") AS "p" FROM "%s" GROUP BY "g"' % AGG_SOURCE,
        sources=(AGG_SOURCE,),
    ),
    Case(
        "sum_divided_by_count",
        "%s %%>%% group_by(g) %%>%% summarise(m = sum(x) / n())" % AGG_SOURCE,
        'SELECT "g", SUM("x") / COUNT(*) AS "m" FROM "%s" GROUP BY "g"' % AGG_SOURCE,
        sources=(AGG_SOURCE,),
    ),
    Case(
        "n_distinct_plus_math",
        "%s %%>%% group_by(g) %%>%% summarise(u = n_distinct(x) + 1)" % AGG_SOURCE,
        'SELECT "g", COUNT(DISTINCT "x") + CASE WHEN COUNT(*) > COUNT("x") '
        'THEN 1 ELSE 0 END + 1 AS "u" FROM "%s" GROUP BY "g"' % AGG_SOURCE,
        sources=(AGG_SOURCE,),
    ),
    Case(
        "compound_summarise_with_grouping_and_filter",
        "%s %%>%% group_by(g) %%>%% summarise(m = sum(x) / n(), u = n_distinct(x) + 1) "
        "%%>%% filter(m >= 5) %%>%% arrange(g)" % AGG_SOURCE,
        'SELECT "g", SUM("x") / COUNT(*) AS "m", COUNT(DISTINCT "x") + '
        'CASE WHEN COUNT(*) > COUNT("x") THEN 1 ELSE 0 END + 1 AS "u" FROM "%s" '
        'GROUP BY "g" HAVING SUM("x") / COUNT(*) >= 5 ORDER BY "g"' % AGG_SOURCE,
        ordered=True,
        sources=(AGG_SOURCE,),
    ),
    Case(
        "sum_of_a_product_after_a_filter",
        "%s %%>%% filter(y > 0) %%>%% group_by(g) %%>%% "
        "summarise(p = sum(x * y))" % AGG_SOURCE,
        'SELECT "g", SUM("x" * "y") AS "p" FROM "%s" WHERE "y" > 0 GROUP BY "g"'
        % AGG_SOURCE,
        sources=(AGG_SOURCE,),
    ),
    # Truncation checks: the dividend is not divisible by the divisor, so an
    # integer division renders the wrong answer.
    Case(
        "global_sum_over_count_is_fractional",
        "%s %%>%% summarise(m = sum(v) / n())" % RATIO_SOURCE,
        # SUM(v)=3 over 2 rows is 1.5, which SQLite only returns when the
        # division is forced to floating point.
        'SELECT SUM("v") * 1.0 / COUNT(*) AS "m" FROM "%s"' % RATIO_SOURCE,
        sources=(RATIO_SOURCE,),
    ),
    Case(
        "grouped_sum_over_count_is_fractional",
        "%s %%>%% group_by(g) %%>%% summarise(m = sum(v) / n())" % ODD_RATIO_SOURCE,
        'SELECT "g", SUM("v") * 1.0 / COUNT(*) AS "m" FROM "%s" GROUP BY "g"'
        % ODD_RATIO_SOURCE,
        sources=(ODD_RATIO_SOURCE,),
    ),
    Case(
        "sum_over_a_literal_divisor_is_fractional",
        "%s %%>%% summarise(m = sum(v) / 4)" % RATIO_SOURCE,
        # 3 / 4 is 0.75; integer division would silently yield 0.
        'SELECT SUM("v") * 1.0 / 4 AS "m" FROM "%s"' % RATIO_SOURCE,
        sources=(RATIO_SOURCE,),
    ),
    Case(
        "scalar_ratio_mutate_is_fractional",
        "%s %%>%% mutate(half = v / 2)" % RATIO_SOURCE,
        'SELECT "v" AS "v", "v" * 1.0 / 2 AS "half" FROM "%s"' % RATIO_SOURCE,
        sources=(RATIO_SOURCE,),
    ),
    Case(
        "scalar_ratio_mutate_after_filter",
        "%s %%>%% filter(v > 1) %%>%% mutate(half = v / 2)" % RATIO_SOURCE,
        'SELECT "v" AS "v", "v" * 1.0 / 2 AS "half" FROM "%s" WHERE "v" > 1'
        % RATIO_SOURCE,
        sources=(RATIO_SOURCE,),
    ),
]


def normalize(rows, ordered):
    return rows if ordered else sorted(rows, key=repr)


def compare_columns(name, expected, actual, failures):
    if expected != actual:
        failures.append(
            "%s: output column names differ\n  dplyr: %s\n  expected: %r\n  actual: %r"
            % (name, name, expected, actual)
        )
        return False
    return True


def run_case(case, binary, schema_path, conn, failures):
    try:
        sql = transpile(binary, schema_path, case.code)
    except RuntimeError as err:
        failures.append(str(err))
        return

    if case.expected_sql is None:
        failures.append(
            "%s: expected the compiler to reject %r, got:\n%s" % (case.name, case.code, sql)
        )
        return

    try:
        expected_columns, expected_rows = reference(case.expected_sql)
        cursor = conn.execute(sql)
        actual_columns = [description[0] for description in cursor.description]
        actual_rows = cursor.fetchall()
    except sqlite3.Error as err:
        failures.append(
            "%s: SQL execution failed (%s)\n  dplyr: %s\n  sql: %s"
            % (case.name, err, case.code, sql)
        )
        return

    if not compare_columns(case.name, expected_columns, actual_columns, failures):
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

    single_schema = write_schema_file("data", "schema_relational_exec.json")
    empty_schema = write_schema_file(EMPTY_SOURCE, "schema_relational_exec_empty.json")
    # Multi-source cases pass the array form of --schema, which is the same
    # metadata the CLI receives for joins and set operations.
    multi_schema = write_schema_file(
        ("data", OTHER_SOURCE), "schema_relational_exec_multi.json"
    )
    conn = make_connection()
    failures = []
    skipped = []

    def schema_for(case):
        if case.sources == ("data",):
            return single_schema
        if case.sources == (EMPTY_SOURCE,):
            return empty_schema
        if case.sources == ("data", OTHER_SOURCE):
            return multi_schema
        return write_schema_file(
            case.sources, "schema_relational_exec_%s.json" % "_".join(case.sources)
        )

    try:
        for case in CASES:
            if case.expected_sql is None:
                run_rejection(case, binary, single_schema, failures)
            else:
                run_case(case, binary, single_schema, conn, failures)
        for case in EMPTY_CASES:
            run_case(case, binary, empty_schema, conn, failures)
        for group in (JOIN_CASES, SET_CASES, AGG_CASES):
            for case in group:
                if case.min_sqlite and sqlite3.sqlite_version_info < case.min_sqlite:
                    skipped.append(
                        "%s: SQLite %s predates %d.%d"
                        % (
                            case.name,
                            sqlite3.sqlite_version,
                            case.min_sqlite[0],
                            case.min_sqlite[1],
                        )
                    )
                    continue
                path = schema_for(case)
                if case.expected_sql is None:
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
    print("\nall relational execution cases passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
