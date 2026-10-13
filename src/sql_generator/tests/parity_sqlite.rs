use std::io::Write;
use std::process::{Command, Stdio};

/// Executes generated SQL against deterministic data in a real SQLite engine.
pub fn execute(sql: &str, empty: bool) -> serde_json::Value {
    let script = r#"
import json, sqlite3, sys
request = json.load(sys.stdin)
db = sqlite3.connect(":memory:")
db.execute("CREATE TABLE data (id INTEGER, g TEXT, x INTEGER, y INTEGER, flag BOOLEAN, label TEXT, date_value TEXT)")
rows = [
    (1, 'a', 20, 20, 1, '', '2026-01-02 03:04:05'),
    (2, 'a', 10, 99, 0, 'hi', '2026-01-02'),
    (3, 'a', 10, 10, None, None, None),
    (4, 'a', None, None, None, 'x', None),
    (5, 'a', 30, 30, 1, None, '2026-02-03'),
    (6, 'b', None, None, None, None, None),
    (7, 'b', 5, 5, 1, 'z', '2026-01-04'),
    (8, 'b', 5, 5, 0, '', '2026-01-04'),
]
if not request["empty"]:
    db.executemany("INSERT INTO data VALUES (?, ?, ?, ?, ?, ?, ?)", rows)
print(json.dumps(db.execute(request["sql"]).fetchall()))
"#;
    let mut child = Command::new("python3")
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python with SQLite is required for behavior tests");
    let request = serde_json::json!({"sql": sql, "empty": empty});
    child
        .stdin
        .take()
        .expect("SQL input pipe")
        .write_all(request.to_string().as_bytes())
        .expect("write SQL input");
    let output = child.wait_with_output().expect("SQLite result");
    assert!(
        output.status.success(),
        "SQL failed: {sql}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("SQLite JSON rows")
}
