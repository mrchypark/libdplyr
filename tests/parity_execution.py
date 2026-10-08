#!/usr/bin/env python3
"""Execute parity regressions on SQLite and DuckDB; optionally local test containers."""
import argparse
import collections
import json
import pathlib
import sqlite3
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCHEMAS = [
    {"source": "data", "columns": [{"name": n} for n in ["id", "grp", "x", "y", "weight"]]},
    {"source": "other", "columns": [{"name": n} for n in ["other_id", "grp", "value"]]},
    {"source": "keys", "columns": [{"name": n} for n in ["id", "key", "value"]]},
]
FIXTURE = """
DROP TABLE IF EXISTS data;
DROP TABLE IF EXISTS other;
DROP TABLE IF EXISTS "keys";
CREATE TABLE data(id INTEGER, grp VARCHAR(20), x DOUBLE PRECISION, y DOUBLE PRECISION, weight DOUBLE PRECISION);
INSERT INTO data VALUES (1,'a',10,1,1),(2,'a',10,NULL,0),(3,'a',NULL,4,2),(4,'b',5,2,1),(5,'b',0,NULL,1),(6,NULL,-3,7,1);
CREATE TABLE other(other_id INTEGER, grp VARCHAR(20), value DOUBLE PRECISION);
INSERT INTO other VALUES (1,'a',8),(1,'a',8),(4,'b',3),(7,'b',-1);
CREATE TABLE "keys"(id INTEGER, "key" VARCHAR(20), value DOUBLE PRECISION);
INSERT INTO "keys" VALUES (1,'x',2),(1,'y',3),(2,'x',4),(2,'y',NULL);
"""
CASES = [
    ("temporary grouping", 'data %>% mutate(z = mean(x), .by = grp) %>% select(id, z) %>% arrange(id)', [[1,10],[2,10],[3,10],[4,2.5],[5,2.5],[6,-3]]),
    ("used columns", 'data %>% mutate(z = x + 1, .keep = "used")', [[10,11],[10,11],[None,None],[5,6],[0,1],[-3,-2]]),
    ("NULL deletion", 'data %>% mutate(y = NULL) %>% select(id, x)', [[1,10],[2,10],[3,None],[4,5],[5,0],[6,-3]]),
    ("commas and empty filter", 'data %>% filter() %>% filter(x >= 0, y > 0) %>% select(id)', [[1],[4]]),
    ("filter_out keeps missing", 'data %>% filter_out(x > 5) %>% select(id)', [[3],[4],[5],[6]]),
    ("weighted count", 'data %>% count(grp, wt = weight, name = "total", sort = TRUE)', [['a',3],['b',2],[None,1]]),
    ("NULL ifelse", 'data %>% mutate(z = ifelse(x > 5, 1, 0)) %>% select(id, z)', [[1,1],[2,1],[3,None],[4,0],[5,0],[6,0]]),
    ("NULL rank", 'data %>% mutate(z = min_rank(x)) %>% select(id, z)', [[1,4],[2,4],[3,None],[4,3],[5,2],[6,1]]),
    ("ordered lag", 'data %>% group_by(grp) %>% mutate(z = lag(x, order_by = id)) %>% ungroup() %>% select(id, z)', [[1,None],[2,10],[3,10],[4,None],[5,5],[6,None]]),
    ("ntile argument", 'data %>% mutate(z = ntile(x, 2)) %>% select(id, z)', [[1,2],[2,2],[3,None],[4,1],[5,1],[6,1]]),
    ("window distinct includes NULL", 'data %>% group_by(grp) %>% mutate(z = n_distinct(x)) %>% ungroup() %>% select(id, z)', [[1,2],[2,2],[3,2],[4,2],[5,2],[6,1]]),
    ("window distinct removes NULL", 'data %>% group_by(grp) %>% mutate(z = n_distinct(x, na.rm = TRUE)) %>% ungroup() %>% select(id, z)', [[1,1],[2,1],[3,1],[4,2],[5,2],[6,1]]),
    ("ordered cumulative", 'data %>% arrange(id) %>% mutate(z = cumsum(weight)) %>% select(id, z)', [[1,1],[2,1],[3,3],[4,4],[5,5],[6,6]]),
    ("group overwrite", 'data %>% group_by(grp) %>% mutate(grp = coalesce(grp, "missing")) %>% summarise(n = n())', [['a',3],['b',2],['missing',1]]),
    ("set missing columns", 'data %>% select(id, x) %>% union_all(other %>% select(id = other_id, value))', [[1,10,None],[2,10,None],[3,None,None],[4,5,None],[5,0,None],[6,-3,None],[1,None,8],[1,None,8],[4,None,3],[7,None,-1]]),
    ("rolling all ties", 'data %>% left_join(other, by = join_by(grp, closest(x >= value))) %>% select(id, value)', [[1,8],[1,8],[2,8],[2,8],[3,None],[4,3],[5,-1],[6,None]]),
    ("right pipeline", 'data %>% left_join(other %>% filter(value > 5), by = c(id = "other_id")) %>% select(id, value)', [[1,8],[1,8],[2,None],[3,None],[4,None],[5,None],[6,None]]),
    ("positional duplicate", 'data %>% arrange(id) %>% slice(c(3, 1, 3)) %>% select(id)', [[3],[1],[3]]),
    ("positional negative", 'data %>% arrange(id) %>% slice(-c(1, 3)) %>% select(id)', [[2],[4],[5],[6]]),
    ("head", 'data %>% arrange(id) %>% head(2) %>% select(id)', [[1],[2]]),
    ("long pivot preserves rows", 'keys %>% pivot_longer(c(value), names_to = "metric", values_to = "amount")', [[1,'x','value',2],[1,'y','value',3],[2,'x','value',4],[2,'y','value',None]]),
    ("wide pivot", 'keys %>% pivot_wider(names_from = key, values_from = value, keys = c("x", "y"))', [[1,2,3],[2,4,None]]),
    ("replace_na", 'data %>% replace_na(list(y = 0)) %>% select(id, y)', [[1,1],[2,0],[3,4],[4,2],[5,0],[6,7]]),
    ("ordered fill", 'data %>% arrange(id) %>% fill(y) %>% select(id, y)', [[1,1],[2,1],[3,4],[4,2],[5,2],[6,7]]),
    ("negative fractional modulo", 'data %>% mutate(z = x %% 2.5) %>% select(id, z)', [[1,0],[2,0],[3,None],[4,0],[5,0],[6,2]]),
    ("weighted cap", 'data %>% slice_sample(n = 100, weight_by = weight) %>% select(id)', [[1],[3],[4],[5],[6]]),
    ("replacement constant candidate", 'data %>% filter(id == 3) %>% slice_sample(n = 4, replace = TRUE, weight_by = weight) %>% select(id)', [[3],[3],[3],[3]]),
    ("replacement unweighted", 'data %>% filter(id == 1) %>% slice_sample(n = 3, replace = TRUE) %>% select(id)', [[1],[1],[1]]),
    ("ordered tail", 'data %>% arrange(id) %>% slice_tail(n = 2) %>% select(id)', [[5],[6]]),
    ("negative head", 'data %>% arrange(id) %>% slice_head(n = -2) %>% select(id)', [[1],[2],[3],[4]]),
    ("fill up", 'data %>% arrange(id) %>% fill(y, .direction = "up") %>% select(id, y)', [[1,1],[2,4],[3,4],[4,2],[5,7],[6,7]]),
    ("grouped fill", 'data %>% arrange(id) %>% fill(y, .by = grp) %>% select(id, y)', [[1,1],[2,1],[3,4],[4,2],[5,2],[6,7]]),
    ("expand domains", 'keys %>% expand(id, key)', [[1,'x'],[1,'y'],[2,'x'],[2,'y']]),
    ("complete domain", 'keys %>% filter(key == "x") %>% complete(id, key = c("x", "y"), fill = list(value = 0))', [[1,'x',2],[2,'x',4],[1,'y',0],[2,'y',0]]),
    ("multi column long pivot", 'data %>% select(id, x, y) %>% pivot_longer(c(x, y), names_to = "key", values_to = "val")', [[1,'x',10],[2,'x',10],[3,'x',None],[4,'x',5],[5,'x',0],[6,'x',-3],[1,'y',1],[2,'y',None],[3,'y',4],[4,'y',2],[5,'y',None],[6,'y',7]]),
    ("tuple slice", 'data %>% slice_min(tibble(x, id), n = 2) %>% select(id)', [[6],[5]]),
    ("fractional slice", 'data %>% slice_min(x, n = 2.9) %>% select(id)', [[6],[5]]),
    ("median", 'data %>% summarise(z = median(x))', [[5]]),
    ("grouped median", 'data %>% summarise(z = median(x), .by = grp)', [['a',10],['b',2.5],[None,-3]]),
    ("variance", 'keys %>% filter(key == "x") %>% summarise(z = var(value))', [[2]]),
    ("summary missing false", 'data %>% summarise(z = sum(x, na.rm = FALSE))', [[22]]),
    ("rows update", 'data %>% rows_update(keys %>% filter(key == "x") %>% select(id, x = value), by = "id", unmatched = "ignore") %>% select(id, x)', [[1,2],[2,4],[3,None],[4,5],[5,0],[6,-3]]),
    ("rows update NULL", 'data %>% rows_update(keys %>% filter(key == "y") %>% select(id, x = value), by = "id", unmatched = "ignore") %>% select(id, x)', [[1,3],[2,None],[3,None],[4,5],[5,0],[6,-3]]),
    ("rows patch", 'data %>% rows_patch(keys %>% filter(key == "x") %>% select(id, y = value), by = "id", unmatched = "ignore") %>% select(id, y)', [[1,1],[2,4],[3,4],[4,2],[5,None],[6,7]]),
    ("rows delete", 'data %>% rows_delete(keys %>% filter(key == "x") %>% select(id), by = "id", unmatched = "ignore") %>% select(id)', [[3],[4],[5],[6]]),
    ("rows append subset", 'data %>% select(id, x) %>% rows_append(keys %>% filter(key == "x") %>% select(id))', [[1,10],[2,10],[3,None],[4,5],[5,0],[6,-3],[1,None],[2,None]]),
    ("rows insert", 'data %>% select(id, x) %>% rows_insert(other %>% filter(other_id == 7) %>% select(id = other_id, x = value), by = "id", conflict = "ignore")', [[1,10],[2,10],[3,None],[4,5],[5,0],[6,-3],[7,-1]]),
    ("rows upsert", 'data %>% select(id, x) %>% rows_upsert(other %>% filter(other_id >= 4) %>% select(id = other_id, x = value), by = "id")', [[1,10],[2,10],[3,None],[4,3],[5,0],[6,-3],[7,-1]]),
    ("weighted replacement count", 'data %>% slice_sample(n = 128, replace = TRUE, weight_by = weight) %>% summarise(draws = n(), zeros = as.numeric(sum(ifelse(id == 2, 1, 0))))', [[128,0]]),
    ("grouped replacement count", 'data %>% slice_sample(n = 3, replace = TRUE, weight_by = weight, by = grp) %>% count(grp)', [['a',3],['b',3],[None,3]]),
    ("window tuple distinct missing", 'data %>% mutate(z = n_distinct(x, y, na.rm = TRUE)) %>% select(id, z)', [[1,3],[2,3],[3,3],[4,3],[5,3],[6,3]]),
    ("recursion above mysql default", 'data %>% filter(id == 1) %>% dbplyr_uncount(1005) %>% summarise(n = n())', [[1005]]),
    ("positional large descending range", 'data %>% arrange(id) %>% slice(200000:1) %>% select(id)', [[6],[5],[4],[3],[2],[1]]),
    ("positional negative range", 'data %>% arrange(id) %>% slice(-c(1, 3)) %>% select(id)', [[2],[4],[5],[6]]),
    ("window variance", 'keys %>% filter(key == "x") %>% mutate(z = var(value)) %>% select(id, z)', [[1,2],[2,2]]),
    ("window median", 'data %>% mutate(z = median(x)) %>% select(id, z)', [[1,5],[2,5],[3,5],[4,5],[5,5],[6,5]]),
    ("summary tuple distinct", 'data %>% summarise(z = n_distinct(x, y, na.rm = TRUE))', [[3]]),
    ("computed order fill both", 'data %>% arrange(-id) %>% fill(y, .direction = "downup") %>% select(id, y)', [[6,7],[5,7],[4,2],[3,4],[2,4],[1,1]]),
    ("computed order distinct", 'data %>% arrange(-id) %>% distinct(x, .keep_all = TRUE) %>% select(id)', [[6],[5],[4],[3],[2]]),
    ("NA assigns missing column", 'data %>% mutate(x = NA) %>% select(id, x)', [[1,None],[2,None],[3,None],[4,None],[5,None],[6,None]]),
    ("NA across preserves columns", 'data %>% mutate(across(c(x, y), ~ NA)) %>% select(id, x, y)', [[1,None,None],[2,None,None],[3,None,None],[4,None,None],[5,None,None],[6,None,None]]),
    ("uncount", 'data %>% dbplyr_uncount(weight) %>% select(id)', [[1],[3],[3],[4],[5],[6]]),
]

VALIDATION_CASES = [
    ("negative weights", 'data %>% slice_sample(n = 2, weight_by = -weight)'),
    ("missing weights", 'data %>% slice_sample(n = 2, weight_by = y)'),
    ("zero totals", 'data %>% slice_sample(n = 2, weight_by = weight * 0)'),
    ("fractional uncount", 'data %>% dbplyr_uncount(weight + 0.5)'),
    ("duplicate row update keys", 'data %>% rows_update(keys %>% select(id, x = value), by = "id", unmatched = "ignore")'),
    ("row insert conflicts", 'data %>% rows_insert(keys %>% filter(key == "x") %>% select(id, x = value), by = "id")'),
    ("unmatched right rows", 'data %>% left_join(other, by = c(id = "other_id"), unmatched = "error")'),
]


def normalize(value):
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return value


def bag(rows):
    return collections.Counter(tuple(normalize(v) for v in row) for row in rows)


def run(cmd, text=None):
    return subprocess.run(cmd, input=text, text=True, capture_output=True, check=True, timeout=45).stdout


class Database:
    def __init__(self, dialect, containers):
        self.dialect = dialect
        self.db = sqlite3.connect(":memory:") if dialect == "sqlite" else None
        self.path = tempfile.NamedTemporaryFile(suffix=".duckdb", delete=False).name if dialect == "duckdb" else None
        if self.path:
            pathlib.Path(self.path).unlink()
        if self.db:
            self.db.executescript(FIXTURE)
        elif dialect == "duckdb":
            run(["duckdb", self.path, "-c", FIXTURE])
        elif dialect == "postgresql":
            run(["docker", "exec", "-i", containers[0], "psql", "-U", "postgres", "-v", "ON_ERROR_STOP=1"], FIXTURE)
        else:
            run(["docker", "exec", containers[1], "mysql", "-uroot", "-e", "CREATE DATABASE IF NOT EXISTS parity;"])
            run(["docker", "exec", containers[1], "mysql", "-uroot", "parity", "-e", FIXTURE.replace('"keys"', "`keys`").replace('"key"', "`key`")])

    def rows(self, sql):
        if self.db:
            return [list(row) for row in self.db.execute(sql)]
        if self.dialect == "duckdb":
            output = run(["duckdb", "-json", self.path, "-c", sql])
            return [list(row.values()) for row in json.loads(output or "[]")]
        if self.dialect == "postgresql":
            output = run(["docker", "exec", "-i", "libdplyr-parity-pg", "psql", "-U", "postgres", "-v", "ON_ERROR_STOP=1", "-At", "-F", "\t", "-P", "null=NULL", "-c", sql])
        else:
            output = run(["docker", "exec", "libdplyr-parity-mysql", "mysql", "-uroot", "--batch", "--skip-column-names", "parity", "-e", sql])
        def cell(v):
            if v == "NULL": return None
            try: return normalize(float(v))
            except ValueError: return v
        return [[cell(v) for v in row.split("\t")] for row in output.splitlines()]

    def close(self):
        if self.db: self.db.close()
        if self.path: pathlib.Path(self.path).unlink(missing_ok=True)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("binary", nargs="?", default=str(ROOT/"target/debug/libdplyr"))
    parser.add_argument("--containers", action="store_true")
    opts=parser.parse_args()
    dialects=["sqlite","duckdb"] + (["postgresql","mysql"] if opts.containers else [])
    failures=[]
    with tempfile.NamedTemporaryFile(mode="w",suffix=".json") as schema:
        json.dump(SCHEMAS,schema); schema.flush()
        for dialect in dialects:
            database=Database(dialect,["libdplyr-parity-pg","libdplyr-parity-mysql"])
            try:
                for name,code,expected in CASES:
                    try:
                        plan=json.loads(run([opts.binary,"-d",dialect,"--schema",schema.name,"--execution-plan","-t",code]))
                        for check in plan["checks"]:
                            assert not database.rows(check["sql"]), check["message"]
                        actual=database.rows(plan["query"]["sql"])
                        assert (actual==expected if name.startswith("positional") else bag(actual)==bag(expected)), (actual,expected)
                        print(f"PASS {dialect}: {name}", flush=True)
                    except (subprocess.CalledProcessError,AssertionError,sqlite3.Error) as error:
                        message=error.stderr if isinstance(error,subprocess.CalledProcessError) else str(error)
                        failures.append((dialect,name,message))
                        print(f"FAIL {dialect}: {name}: {message}", flush=True)
                code='data %>% left_join(other, by = c(id = "other_id"), relationship = "many-to-one")'
                plan=json.loads(run([opts.binary,"-d",dialect,"--schema",schema.name,"--execution-plan","-t",code]))
                assert len(plan["checks"])==1
                assert database.rows(plan["checks"][0]["sql"]), "actual duplicate must violate many-to-one"
                print(f"PASS {dialect}: relationship violation query")
                for name, code in VALIDATION_CASES:
                    plan=json.loads(run([opts.binary,"-d",dialect,"--schema",schema.name,"--execution-plan","-t",code]))
                    assert plan["checks"] and any(database.rows(check["sql"]) for check in plan["checks"]), f"{dialect}: missing violation for {name}"
                    print(f"PASS {dialect}: rejects {name}", flush=True)
            finally: database.close()
    print(f"{len(CASES)*len(dialects)-len(failures)}/{len(CASES)*len(dialects)} result cases passed")
    return 1 if failures else 0

if __name__=="__main__":
    raise SystemExit(main())
