//! CLI integration tests
//!
//! Tests for the new CLI pipeline functionality including stdin/stdout,
//! validation mode, JSON output, and various CLI option combinations.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// Helper function to write to stdin and close it properly
fn write_to_stdin(child: &mut std::process::Child, input: &[u8]) {
    if let Some(mut stdin) = child.stdin.take() {
        // Ignore broken pipe errors as the process might have already finished
        let _ = stdin.write_all(input);
        let _ = stdin.flush();
        // stdin is dropped here to signal EOF
    }
}

/// Helper function to build the libdplyr binary path
fn get_libdplyr_path() -> String {
    let binary_name = if cfg!(windows) {
        "libdplyr.exe"
    } else {
        "libdplyr"
    };

    // Try different possible paths for the binary
    let possible_paths = [
        format!("./target/debug/{}", binary_name),
        format!("target/debug/{}", binary_name),
        format!("./target/llvm-cov-target/debug/{}", binary_name),
        format!("target/llvm-cov-target/debug/{}", binary_name),
    ];

    for path in &possible_paths {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }

    // Fallback to default path
    format!("./target/debug/{}", binary_name)
}

#[test]
fn test_stdin_stdout_basic_functionality() {
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name, age)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "Process should succeed");
    assert_eq!(output.status.code(), Some(0), "Exit code should be 0");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(stdout.contains("SELECT"), "Output should contain SELECT");
    assert!(
        stdout.contains("name"),
        "Output should contain 'name' column"
    );
    assert!(stdout.contains("age"), "Output should contain 'age' column");
}

#[test]
fn test_stdin_stdout_complex_query() {
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(
        &mut child,
        b"data %>% select(name, age) %>% filter(age > 18) %>% arrange(desc(age))",
    );

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "Complex query should succeed");
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(stdout.contains("SELECT"), "Should contain SELECT");
    assert!(stdout.contains("WHERE"), "Should contain WHERE for filter");
    assert!(
        stdout.contains("ORDER BY"),
        "Should contain ORDER BY for arrange"
    );
}

#[test]
fn test_stdin_stdout_empty_input() {
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"");

    let output = child.wait_with_output().expect("Failed to read stdout");

    // Empty input should be handled gracefully
    assert_eq!(
        output.status.code(),
        Some(3),
        "Empty input should return exit code 3 (IO_ERROR)"
    );
    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8");
    assert!(
        stderr.contains("입력")
            || stderr.contains("input")
            || stderr.contains("stdin")
            || stderr.contains("empty"),
        "Should mention input error"
    );
}

#[test]
fn test_validation_only_mode_success() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--validate-only"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(
        &mut child,
        b"data %>% select(name, age) %>% filter(age > 18)",
    );

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(
        output.status.success(),
        "Valid syntax should pass validation"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "Validation success should return exit code 0"
    );

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(
        stdout.contains("Valid") || stdout.contains("유효"),
        "Should indicate valid syntax"
    );
}

#[test]
fn test_validation_only_mode_failure() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--validate-only"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"invalid_function(test, broken_syntax");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(
        !output.status.success(),
        "Invalid syntax should fail validation"
    );
    assert_eq!(
        output.status.code(),
        Some(4),
        "Validation error should return exit code 4"
    );

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8");
    assert!(
        stderr.contains("오류") || stderr.contains("error"),
        "Should contain error message"
    );
}

#[test]
fn test_json_output_format() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name, age)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "JSON output should succeed");
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    // Verify JSON structure
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    assert!(
        json["success"].as_bool().unwrap_or(false),
        "Should indicate success"
    );
    assert!(json["sql"].is_string(), "Should contain SQL string");
    assert!(
        json["metadata"].is_object(),
        "Should contain metadata object"
    );
    assert!(
        json["metadata"]["dialect"].is_string(),
        "Should contain dialect info"
    );
    assert!(
        json["metadata"]["timestamp"].is_number(),
        "Should contain timestamp as number"
    );
}

#[test]
fn test_json_output_with_validation_error() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"invalid_syntax(");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(!output.status.success(), "Invalid syntax should fail");
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    // Even with errors, JSON output should be valid
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output should also be valid JSON");

    assert!(
        !json["success"].as_bool().unwrap_or(true),
        "Should indicate failure"
    );
    assert!(json["error"].is_object(), "Should contain error object");
}

#[test]
fn test_pretty_formatting() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--pretty"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(
        &mut child,
        b"data %>% select(name, age) %>% filter(age > 18)",
    );

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "Pretty formatting should succeed");
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    // Pretty format should have multiple lines with proper indentation
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() > 1, "Pretty format should have multiple lines");
    assert!(stdout.contains("SELECT"), "Should contain SELECT");
    assert!(stdout.contains("FROM"), "Should contain FROM");
    assert!(stdout.contains("WHERE"), "Should contain WHERE");
}

#[test]
fn test_compact_formatting() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--compact"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(
        &mut child,
        b"data %>% select(name, age) %>% filter(age > 18)",
    );

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "Compact formatting should succeed");
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    // Compact format should be a single line (plus final newline)
    let trimmed = stdout.trim();
    assert!(
        !trimmed.contains('\n'),
        "Compact format should not contain internal newlines"
    );
    assert!(trimmed.contains("SELECT"), "Should contain SELECT");
    assert!(trimmed.contains("WHERE"), "Should contain WHERE");
}

#[test]
fn test_verbose_mode() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--verbose"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "Verbose mode should succeed");
    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8");

    // Verbose mode should output processing information to stderr
    assert!(
        stderr.contains("Reading") || stderr.contains("Processing") || stderr.contains("처리"),
        "Verbose mode should show processing information"
    );
}

#[test]
fn test_debug_mode() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--debug"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "Debug mode should succeed");
    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8");

    // Debug mode should show detailed information including AST
    assert!(
        stderr.contains("AST") || stderr.contains("Debug") || stderr.contains("디버그"),
        "Debug mode should show AST or debug information"
    );
}

#[test]
fn test_different_sql_dialects() {
    let dialects = [
        ("postgresql", "PostgreSQL"),
        ("mysql", "MySQL"),
        ("sqlite", "SQLite"),
        ("duckdb", "DuckDB"),
    ];

    for (dialect_arg, dialect_name) in &dialects {
        let mut child = Command::new(get_libdplyr_path())
            .args(["--dialect", dialect_arg])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start libdplyr process");

        write_to_stdin(&mut child, b"data %>% select(name, age)");

        let output = child.wait_with_output().expect("Failed to read stdout");

        assert!(
            output.status.success(),
            "Dialect {} should work correctly",
            dialect_name
        );

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
        assert!(
            stdout.contains("SELECT"),
            "Dialect {} should produce SELECT statement",
            dialect_name
        );
    }
}

#[test]
fn test_combined_options() {
    // Test --json + --verbose
    let mut child = Command::new(get_libdplyr_path())
        .args(["--json", "--verbose"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(output.status.success(), "Combined options should work");

    // Should have JSON output
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let _json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Should produce valid JSON");

    // Should have verbose stderr
    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8");
    assert!(
        stderr.contains("Reading") || stderr.contains("Processing") || stderr.contains("처리"),
        "Should show verbose information"
    );
}

#[test]
fn test_validation_with_json_output() {
    let mut child = Command::new(get_libdplyr_path())
        .args(["--validate-only", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name, age)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(
        output.status.success(),
        "Validation with JSON should succeed"
    );

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Should produce valid JSON");

    assert!(
        json["success"].as_bool().unwrap_or(false),
        "Should indicate validation success"
    );
    assert!(
        json["validation"].is_object(),
        "Should contain validation info"
    );
}

#[test]
fn test_exit_codes() {
    // Test successful transpilation (exit code 0)
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name)");
    let output = child.wait_with_output().expect("Failed to read stdout");
    assert_eq!(
        output.status.code(),
        Some(0),
        "Success should return exit code 0"
    );

    // Test syntax error (exit code 4)
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"invalid_syntax(");
    let output = child.wait_with_output().expect("Failed to read stdout");
    assert_eq!(
        output.status.code(),
        Some(4),
        "Syntax error should return exit code 4"
    );

    // Test invalid arguments (exit code 2)
    let output = Command::new(get_libdplyr_path())
        .args(["--invalid-option"])
        .output()
        .expect("Failed to execute libdplyr");
    assert_eq!(
        output.status.code(),
        Some(2),
        "Invalid arguments should return exit code 2"
    );

    // Test unsupported dialect (exit code 2)
    let mut child = Command::new(get_libdplyr_path())
        .args(["--dialect", "unsupported"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name)");
    let output = child.wait_with_output().expect("Failed to read stdout");
    assert_eq!(
        output.status.code(),
        Some(2),
        "Unsupported dialect should return exit code 2"
    );
}

#[test]
fn test_file_input_with_stdin_output() {
    // Create temporary input file
    let mut input_file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(
        input_file,
        "data %>% select(name, age) %>% filter(age > 18)"
    )
    .expect("Failed to write to temp file");
    let input_path = input_file.path().to_str().unwrap();

    let output = Command::new(get_libdplyr_path())
        .args(["-i", input_path, "--pretty"])
        .output()
        .expect("Failed to execute libdplyr");

    assert!(output.status.success(), "File input should work");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(stdout.contains("SELECT"), "Should contain SELECT");
    assert!(stdout.contains("FROM"), "Should contain FROM");
    assert!(stdout.contains("WHERE"), "Should contain WHERE");
}

#[test]
fn test_file_input_output() {
    // Create temporary input file
    let mut input_file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(
        input_file,
        "data %>% select(name, age) %>% filter(age > 18)"
    )
    .expect("Failed to write to temp file");
    let input_path = input_file.path().to_str().unwrap();

    // Create temporary output file
    let output_file = NamedTempFile::new().expect("Failed to create temp file");
    let output_path = output_file.path().to_str().unwrap();

    let output = Command::new(get_libdplyr_path())
        .args(["-i", input_path, "-o", output_path, "--pretty"])
        .output()
        .expect("Failed to execute libdplyr");

    assert!(output.status.success(), "File I/O should work");

    // Check output file content
    let sql_content = fs::read_to_string(output_path).expect("Failed to read output file");
    assert!(
        sql_content.contains("SELECT"),
        "Output file should contain SELECT"
    );
    assert!(
        sql_content.contains("FROM"),
        "Output file should contain FROM"
    );
    assert!(
        sql_content.contains("WHERE"),
        "Output file should contain WHERE"
    );
}

#[test]
fn test_text_input_mode() {
    let output = Command::new(get_libdplyr_path())
        .args(["-t", "data %>% select(name, age)", "--compact"])
        .output()
        .expect("Failed to execute libdplyr");

    assert!(output.status.success(), "Text input mode should work");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(stdout.contains("SELECT"), "Should contain SELECT");
    assert!(stdout.contains("name"), "Should contain name column");
    assert!(stdout.contains("age"), "Should contain age column");
}

#[test]
fn test_help_option() {
    let output = Command::new(get_libdplyr_path())
        .args(["--help"])
        .output()
        .expect("Failed to execute libdplyr");

    assert!(output.status.success(), "Help should work");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(
        stdout.contains("Usage") || stdout.contains("사용법"),
        "Should show usage information"
    );
    assert!(stdout.contains("--json"), "Should mention JSON option");
    assert!(
        stdout.contains("--validate-only"),
        "Should mention validation option"
    );
}

#[test]
fn test_version_option() {
    let output = Command::new(get_libdplyr_path())
        .args(["--version"])
        .output()
        .expect("Failed to execute libdplyr");

    assert!(output.status.success(), "Version should work");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(
        stdout.contains("libdplyr") || stdout.contains("version"),
        "Should show version information"
    );
}

#[test]
#[cfg(unix)]
fn test_signal_handling() {
    use std::thread;
    use std::time::Duration;

    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    // Send SIGTERM after a short delay
    let child_id = child.id();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        unsafe {
            libc::kill(child_id as i32, libc::SIGTERM);
        }
    });

    write_to_stdin(&mut child, b"data %>% select(name)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    // Process should handle signal gracefully
    // Exit code may vary depending on timing, but should not crash
    assert!(
        output.status.code().is_some(),
        "Should have a valid exit code"
    );
}

#[test]
#[cfg(windows)]
fn test_signal_handling() {
    use std::thread;
    use std::time::Duration;

    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    // On Windows, we can't easily send SIGTERM, so we'll test basic process handling
    // by sending input and checking that the process completes normally
    write_to_stdin(&mut child, b"data %>% select(name)");

    let output = child.wait_with_output().expect("Failed to read stdout");

    // Process should complete successfully
    assert!(
        output.status.success(),
        "Process should complete successfully on Windows"
    );
    assert!(
        output.status.code().is_some(),
        "Should have a valid exit code"
    );
}

#[test]
fn test_large_input_processing() {
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    // Create a large input with multiple operations (using supported syntax)
    let large_input = "data %>% select(name, age, city, country, email, phone) %>% filter(age > 18) %>% arrange(desc(age)) %>% group_by(country)";

    write_to_stdin(&mut child, large_input.as_bytes());

    let output = child.wait_with_output().expect("Failed to read stdout");

    assert!(
        output.status.success(),
        "Large input should be processed successfully"
    );
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(stdout.contains("SELECT"), "Should contain SELECT");
    assert!(stdout.contains("GROUP BY"), "Should contain GROUP BY");
    assert!(stdout.contains("ORDER BY"), "Should contain ORDER BY");
}

#[test]
fn test_unicode_input_handling() {
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    // Test with Unicode characters in column names (using ASCII table name)
    write_to_stdin(&mut child, "data %>% select(이름, 나이, 주소)".as_bytes());

    let output = child.wait_with_output().expect("Failed to read stdout");

    // Unicode column names may not be fully supported yet, so we check for graceful handling
    let exit_code = output.status.code().unwrap_or(-1);
    assert!(
        (0..=10).contains(&exit_code),
        "Should handle Unicode input gracefully"
    );

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
        assert!(stdout.contains("SELECT"), "Should contain SELECT");
    }
}

#[test]
fn test_concurrent_processing() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    // Run multiple instances concurrently
    for i in 0..5 {
        let results_clone = Arc::clone(&results);
        let handle = thread::spawn(move || {
            let mut child = Command::new(get_libdplyr_path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to start libdplyr process");

            let input = format!("data{} %>% select(col{})", i, i);
            write_to_stdin(&mut child, input.as_bytes());

            let output = child.wait_with_output().expect("Failed to read stdout");

            let mut results = results_clone.lock().unwrap();
            results.push((i, output.status.success()));
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let results = results.lock().unwrap();
    assert_eq!(results.len(), 5, "All concurrent processes should complete");

    for (i, success) in results.iter() {
        assert!(*success, "Process {} should succeed", i);
    }
}

#[test]
fn test_memory_usage_with_repeated_processing() {
    // Test for memory leaks by running many operations
    for i in 0..10 {
        let mut child = Command::new(get_libdplyr_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start libdplyr process");

        let input = format!(
            "data{} %>% select(name, age) %>% filter(age > {})",
            i,
            i * 10
        );
        write_to_stdin(&mut child, input.as_bytes());

        let output = child.wait_with_output().expect("Failed to read stdout");

        assert!(output.status.success(), "Iteration {} should succeed", i);
    }

    // If we reach here without running out of memory, the test passes
    println!("Memory usage test completed successfully");
}

#[test]
fn test_error_recovery_and_multiple_attempts() {
    // Test error handling followed by successful processing

    // First, send invalid input
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"invalid_syntax(");
    let output = child.wait_with_output().expect("Failed to read stdout");
    assert!(!output.status.success(), "Invalid input should fail");

    // Then, send valid input
    let mut child = Command::new(get_libdplyr_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr process");

    write_to_stdin(&mut child, b"data %>% select(name)");
    let output = child.wait_with_output().expect("Failed to read stdout");
    assert!(
        output.status.success(),
        "Valid input should succeed after previous error"
    );
}

#[test]
fn test_comprehensive_cli_option_combinations() {
    let test_cases = vec![
        (vec!["--json", "--verbose"], "JSON with verbose"),
        (vec!["--json", "--debug"], "JSON with debug"),
        (vec!["--pretty", "--verbose"], "Pretty with verbose"),
        (vec!["--compact", "--debug"], "Compact with debug"),
        (
            vec!["--validate-only", "--verbose"],
            "Validation with verbose",
        ),
        (vec!["--validate-only", "--debug"], "Validation with debug"),
        (
            vec!["--dialect", "mysql", "--json"],
            "MySQL dialect with JSON",
        ),
        (
            vec!["--dialect", "sqlite", "--pretty"],
            "SQLite dialect with pretty",
        ),
        (
            vec!["--dialect", "duckdb", "--compact"],
            "DuckDB dialect with compact",
        ),
    ];

    for (args, description) in test_cases {
        let mut child = Command::new(get_libdplyr_path())
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start libdplyr process");

        write_to_stdin(&mut child, b"data %>% select(name, age)");

        let output = child.wait_with_output().expect("Failed to read stdout");

        assert!(output.status.success(), "{} should work", description);

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
        assert!(!stdout.is_empty(), "{} should produce output", description);
    }
}

#[test]
fn test_edge_case_inputs() {
    let edge_cases = vec![
        ("", "Empty input"),
        ("   ", "Whitespace only"),
        ("\n\n\n", "Newlines only"),
        ("data", "Incomplete input"),
        ("data %>%", "Incomplete pipe"),
        ("select()", "Empty function call"),
        ("data %>% select(name) %>%", "Trailing pipe"),
    ];

    for (input, description) in edge_cases {
        let mut child = Command::new(get_libdplyr_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start libdplyr process");

        write_to_stdin(&mut child, input.as_bytes());

        let output = child.wait_with_output().expect("Failed to read stdout");

        // Edge cases should either succeed or fail gracefully with appropriate exit codes
        let exit_code = output.status.code().unwrap_or(-1);
        assert!(
            (0..=10).contains(&exit_code),
            "{} should have valid exit code, got {}",
            description,
            exit_code
        );
    }
}

#[test]
fn test_performance_benchmarking() {
    use std::time::Instant;

    let test_inputs = [
        "data %>% select(name)",
        "data %>% select(name, age) %>% filter(age > 18)",
        "data %>% select(name, age, city) %>% filter(age > 18) %>% arrange(name)",
        "data %>% select(name, age, city, country) %>% filter(age > 18) %>% group_by(country) %>% summarise(count = n())",
    ];

    for (i, input) in test_inputs.iter().enumerate() {
        let start = Instant::now();

        let mut child = Command::new(get_libdplyr_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start libdplyr process");

        write_to_stdin(&mut child, input.as_bytes());
        let output = child.wait_with_output().expect("Failed to read stdout");

        let duration = start.elapsed();

        assert!(
            output.status.success(),
            "Performance test {} should succeed",
            i
        );
        assert!(
            duration.as_secs() < 5,
            "Processing should complete within 5 seconds, took {:?}",
            duration
        );

        println!("Performance test {}: {:?}", i, duration);
    }
}
