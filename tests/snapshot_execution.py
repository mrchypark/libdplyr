#!/usr/bin/env python3
"""Real SQLite snapshot boundary tests; Rust tests verify executor call order."""
import json
import pathlib
import sqlite3
import sys
import tempfile
from parity_execution import ROOT, SCHEMAS, FIXTURE, run

binary = sys.argv[1] if len(sys.argv) > 1 else str(ROOT / "target/debug/libdplyr")
with tempfile.TemporaryDirectory() as directory:
    schema = pathlib.Path(directory) / "schema.json"
    schema.write_text(json.dumps(SCHEMAS))
    path = str(pathlib.Path(directory) / "snapshot.db")
    reader = sqlite3.connect(path, isolation_level=None)
    writer = sqlite3.connect(path, isolation_level=None)
    try:
        reader.execute("PRAGMA journal_mode=WAL")
        reader.executescript(FIXTURE)
        code = 'data %>% filter(id == 4) %>% left_join(other, by = c(id = "other_id"), relationship = "many-to-one") %>% select(id, value)'
        plan = json.loads(run([binary, "-d", "sqlite", "--schema", str(schema), "--execution-plan", "-t", code]))
        reader.execute("BEGIN")
        assert not reader.execute(plan["checks"][0]["sql"]).fetchall()
        writer.execute("INSERT INTO other VALUES(4, 'concurrent', 42)")
        assert reader.execute(plan["query"]["sql"]).fetchall() == [(4, 3.0)]
        reader.execute("COMMIT")
        assert reader.execute(plan["checks"][0]["sql"]).fetchall(), "fresh snapshot must see the new violation"
        print("PASS relationship check and result share a stable snapshot")
        reader.execute("BEGIN")
        keys = [row[0] for row in reader.execute('SELECT DISTINCT "key" FROM keys ORDER BY "key"')]
        writer.execute("INSERT INTO keys VALUES(1, 'late_key', 99)")
        code = 'keys %%>%% pivot_wider(names_from = key, values_from = value, keys = c(%s))' % ", ".join(json.dumps(v) for v in keys)
        plan = json.loads(run([binary, "-d", "sqlite", "--schema", str(schema), "--execution-plan", "-t", code]))
        assert [c["name"] for c in plan["query"]["columns"]] == ["id", "x", "y"]
        assert reader.execute(plan["query"]["sql"]).fetchall() == [(1, 2.0, 3.0), (2, 4.0, None)]
        reader.execute("COMMIT")
        assert writer.execute('SELECT COUNT(*) FROM keys WHERE "key" = ?', ("late_key",)).fetchone()[0] == 1
        print("PASS pivot discovery and result share a stable snapshot")
    finally:
        if reader.in_transaction:
            reader.rollback()
        reader.close()
        writer.close()
