//! CLI regression tests for the --schema option.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

const VALID_SCHEMA: &str = r#"{
  "source": "users",
  "columns": [
    { "name": "id", "data_type": "integer", "nullable": false },
    { "name": "age", "data_type": "integer" },
    { "name": "name", "data_type": "text" }
  ]
}"#;

const VALID_SCHEMA_ARRAY: &str = r#"[
  {
    "source": "users",
    "columns": [
      { "name": "id" },
      { "name": "name" }
    ]
  },
  {
    "source": "orders",
    "columns": [
      { "name": "user_id" },
      { "name": "total" }
    ]
  }
]"#;

fn get_libdplyr_path() -> String {
    if let Some(path) = option_env!("CARGO_BIN_EXE_libdplyr") {
        return path.to_string();
    }
    let binary_name = format!("libdplyr{}", std::env::consts::EXE_SUFFIX);
    if let Ok(mut path) = std::env::current_exe() {
        path.pop();
        if path
            .file_name()
            .is_some_and(|n| n == std::ffi::OsStr::new("deps"))
        {
            path.pop();
        }
        path.push(&binary_name);
        if path.exists() {
            return path.to_string_lossy().into_owned();
        }
    }
    format!("./target/debug/{binary_name}")
}

fn write_schema(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp schema file");
    file.write_all(contents.as_bytes()).expect("write schema");
    file.flush().expect("flush schema");
    file
}

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(get_libdplyr_path())
        .args(args)
        .output()
        .expect("run libdplyr");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn test_schema_option_appears_in_help_with_example() {
    let (code, stdout, _) = run(&["--help"]);
    assert_eq!(code, 0, "--help should exit 0");
    assert!(stdout.contains("--schema"), "help should document --schema");
    assert!(
        stdout.contains("--schema schema.json"),
        "help should show a --schema usage example"
    );
}

#[test]
fn test_schema_compiles_text_pipeline() {
    let schema = write_schema(VALID_SCHEMA);
    let (code, stdout, stderr) = run(&[
        "-t",
        "users %>% select(name, age)",
        "--schema",
        schema.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "schema compile failed: {stderr}");
    assert!(
        stdout.contains("users"),
        "compiled SQL should reference the schema source, got: {stdout}"
    );
    assert!(stdout.contains("name"), "expected column name in SQL");
}

#[test]
fn test_schema_json_output_reports_stages_and_columns() {
    let schema = write_schema(VALID_SCHEMA);
    let (code, stdout, stderr) = run(&[
        "-t",
        "users %>% select(name, age)",
        "--schema",
        schema.path().to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, 0, "schema json compile failed: {stderr}");

    let value: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}: {stdout}"));
    assert_eq!(value["success"], serde_json::json!(true));
    assert!(
        value["stages"].as_u64().is_some(),
        "JSON should report stages, got: {stdout}"
    );
    let columns = value["columns"]
        .as_array()
        .unwrap_or_else(|| panic!("JSON should report columns, got: {stdout}"));
    assert_eq!(columns.len(), 2, "select(name, age) yields 2 columns");
    assert_eq!(columns[0]["name"], serde_json::json!("name"));
    assert_eq!(columns[1]["name"], serde_json::json!("age"));
}

#[test]
fn test_schema_respects_compact_and_pretty_formats() {
    let schema = write_schema(VALID_SCHEMA);
    let (code, compact, stderr) = run(&[
        "-t",
        "users %>% filter(age > 18)",
        "--schema",
        schema.path().to_str().unwrap(),
        "--compact",
    ]);
    assert_eq!(code, 0, "compact failed: {stderr}");
    assert!(
        !compact.trim_end().contains('\n'),
        "compact output should have no interior newlines, got: {compact}"
    );

    let (code, pretty, stderr) = run(&[
        "-t",
        "users %>% filter(age > 18)",
        "--schema",
        schema.path().to_str().unwrap(),
        "--pretty",
    ]);
    assert_eq!(code, 0, "pretty failed: {stderr}");
    assert!(
        !pretty.trim().is_empty(),
        "pretty output should not be empty"
    );
    assert!(
        pretty.trim_end().matches('\n').count() >= 1,
        "pretty output should be multi-line, got: {pretty}"
    );
}

#[test]
fn test_schema_writes_to_output_file() {
    let schema = write_schema(VALID_SCHEMA);
    let out = NamedTempFile::new().expect("temp output file");
    let out_path = out.path().to_path_buf();
    let (code, stdout, stderr) = run(&[
        "-t",
        "users %>% select(name)",
        "--schema",
        schema.path().to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "file output failed: {stderr}");
    assert!(stdout.is_empty(), "file mode should not write to stdout");
    let written = fs::read_to_string(&out_path).expect("read output file");
    assert!(written.contains("name"), "output file should hold SQL");
}

#[test]
fn test_schema_works_with_stdin_input() {
    let schema = write_schema(VALID_SCHEMA);
    let mut child = Command::new(get_libdplyr_path())
        .args(["--schema", schema.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn libdplyr");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"users %>% select(age)")
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "stdin schema mode should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("age"),
        "stdin schema SQL should contain column, got: {stdout}"
    );
}

#[test]
fn test_missing_schema_file_is_an_io_error() {
    let (code, _, stderr) = run(&[
        "-t",
        "users %>% select(name)",
        "--schema",
        "/nonexistent/x.json",
    ]);
    assert_eq!(
        code, 3,
        "missing schema file should exit with IO error code"
    );
    assert!(
        stderr.contains("schema"),
        "stderr should mention the schema file, got: {stderr}"
    );
}

#[test]
fn test_malformed_schema_json_is_rejected() {
    let schema = write_schema("{ not json ");
    let (code, _, stderr) = run(&[
        "-t",
        "users %>% select(name)",
        "--schema",
        schema.path().to_str().unwrap(),
    ]);
    assert_ne!(code, 0, "malformed schema JSON must not succeed");
    assert!(
        stderr.contains("schema"),
        "stderr should mention the schema, got: {stderr}"
    );
}

#[test]
fn test_schema_conflicts_with_validate_only() {
    let schema = write_schema(VALID_SCHEMA);
    let (code, _, stderr) = run(&[
        "-t",
        "users %>% select(name)",
        "--schema",
        schema.path().to_str().unwrap(),
        "--validate-only",
    ]);
    assert_ne!(
        code, 0,
        "--schema with --validate-only must be rejected explicitly"
    );
    assert!(
        stderr.contains("schema"),
        "stderr should explain the conflict, got: {stderr}"
    );
}

#[test]
fn test_without_schema_existing_behavior_is_unchanged() {
    let (code, stdout, stderr) = run(&["-t", "data %>% select(name, age)"]);
    assert_eq!(code, 0, "baseline transpile failed: {stderr}");
    assert!(
        stdout.contains("name") && stdout.contains("age"),
        "baseline SQL should keep working, got: {stdout}"
    );
}

#[test]
fn test_schema_array_compiles_a_join() {
    let schema = write_schema(VALID_SCHEMA_ARRAY);
    let (code, stdout, stderr) = run(&[
        "-t",
        "users %>% inner_join(orders, by = c(\"id\" = \"user_id\"))",
        "--schema",
        schema.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "schema array compile failed: {stderr}");
    assert!(
        stdout.contains("users") && stdout.contains("orders"),
        "join SQL should reference both sources, got: {stdout}"
    );
}

#[test]
fn test_schema_array_json_output_reports_columns() {
    let schema = write_schema(VALID_SCHEMA_ARRAY);
    let (code, stdout, stderr) = run(&[
        "-t",
        "users %>% inner_join(orders, by = c(\"id\" = \"user_id\"))",
        "--schema",
        schema.path().to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, 0, "schema array json compile failed: {stderr}");

    let value: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}: {stdout}"));
    assert_eq!(value["success"], serde_json::json!(true));
    let columns = value["columns"]
        .as_array()
        .unwrap_or_else(|| panic!("JSON should report columns, got: {stdout}"));
    assert!(!columns.is_empty(), "got: {stdout}");
}

#[test]
fn test_schema_array_with_duplicate_sources_is_rejected() {
    let schema = write_schema(
        r#"[
          { "source": "users", "columns": [ { "name": "id" } ] },
          { "source": "users", "columns": [ { "name": "id" } ] }
        ]"#,
    );
    let (code, stdout, stderr) = run(&[
        "-t",
        "users %>% select(id)",
        "--schema",
        schema.path().to_str().unwrap(),
    ]);
    assert_ne!(code, 0, "duplicate schema sources must be rejected");
    assert!(stdout.is_empty(), "no SQL should be emitted, got: {stdout}");
    assert!(stderr.contains("duplicate"), "got: {stderr}");
}

#[test]
fn test_empty_schema_array_is_rejected() {
    let schema = write_schema("[]");
    let (code, stdout, stderr) = run(&[
        "-t",
        "users %>% select(id)",
        "--schema",
        schema.path().to_str().unwrap(),
    ]);
    assert_ne!(code, 0, "an empty schema array must be rejected");
    assert!(stdout.is_empty(), "no SQL should be emitted, got: {stdout}");
    assert!(!stderr.is_empty(), "stderr should explain the failure");
}
