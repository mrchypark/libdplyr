//! Cross-platform compatibility tests
//!
//! This module contains tests to verify that libdplyr works correctly
//! across different platforms (Unix, Windows, etc.)

use libdplyr::cli::signal_handler::{SignalHandler, utils};
use libdplyr::cli::stdin_reader::StdinReader;
use std::time::Duration;

#[test]
fn test_signal_handler_creation_cross_platform() {
    // Signal handler should be creatable on all platforms
    let handler = SignalHandler::new();
    assert!(handler.is_ok(), "Signal handler creation should work on all platforms");
    
    let handler = handler.unwrap();
    assert!(!handler.should_shutdown(), "Handler should not indicate shutdown initially");
    assert!(!handler.pipe_closed(), "Handler should not indicate pipe closed initially");
}

#[test]
fn test_signal_handler_reset_cross_platform() {
    let handler = SignalHandler::new().unwrap();
    handler.reset();
    assert!(!handler.should_shutdown());
    assert!(!handler.pipe_closed());
}

#[test]
fn test_signal_wait_timeout_cross_platform() {
    let handler = SignalHandler::new().unwrap();
    let result = handler.wait_for_signal(Duration::from_millis(50));
    
    // Should timeout on all platforms when no signal is sent
    assert_eq!(result, libdplyr::cli::signal_handler::SignalWaitResult::Timeout);
}

#[test]
fn test_ignore_sigpipe_cross_platform() {
    // This should work on all platforms (no-op on Windows)
    let result = utils::ignore_sigpipe();
    assert!(result.is_ok(), "ignore_sigpipe should work on all platforms");
}

#[test]
fn test_is_unix_like_detection() {
    let is_unix = utils::is_unix_like();
    
    // Verify the detection matches the actual platform
    #[cfg(unix)]
    assert!(is_unix, "Should detect Unix-like systems correctly");
    
    #[cfg(not(unix))]
    assert!(!is_unix, "Should detect non-Unix systems correctly");
}

#[test]
fn test_process_id_retrieval() {
    let pid = utils::get_pid();
    assert!(pid > 0, "Process ID should be positive");
}

#[test]
fn test_pipeline_detection_cross_platform() {
    // This test verifies that pipeline detection doesn't panic
    // The actual result depends on how tests are run
    let _is_in_pipeline = utils::is_in_pipeline();
    // We don't assert the result since it depends on test environment
}

#[test]
fn test_stdin_pipe_detection_cross_platform() {
    // This should work on all platforms
    let _is_piped = StdinReader::is_piped();
    // We don't assert the result since it depends on test environment
}

#[test]
fn test_stdin_reader_creation_cross_platform() {
    let reader = StdinReader::new();
    assert!(reader.config().trim_input);
    
    // Test with signal handling
    let reader_with_signals = StdinReader::with_signal_handling();
    assert!(reader_with_signals.is_ok(), "StdinReader with signals should work on all platforms");
}

#[cfg(test)]
mod platform_specific_tests {
    use super::*;
    
    #[cfg(unix)]
    mod unix_tests {
        use super::*;
        
        #[test]
        fn test_unix_signal_handling() {
            // Test Unix-specific signal handling
            let handler = SignalHandler::new().unwrap();
            
            // These should work on Unix systems
            assert!(!handler.should_shutdown());
            assert!(!handler.pipe_closed());
            
            // Test SIGPIPE handling
            let result = utils::ignore_sigpipe();
            assert!(result.is_ok(), "SIGPIPE handling should work on Unix");
        }
        
        #[test]
        fn test_unix_pipeline_detection() {
            // On Unix, we can test more specific pipeline behavior
            let is_unix = utils::is_unix_like();
            assert!(is_unix, "Should be Unix-like system");
        }
    }
    
    #[cfg(windows)]
    mod windows_tests {
        use super::*;
        
        #[test]
        fn test_windows_signal_handling() {
            // Test Windows-specific signal handling
            let handler = SignalHandler::new().unwrap();
            
            // These should work on Windows systems
            assert!(!handler.should_shutdown());
            // SIGPIPE doesn't exist on Windows, so this should always be false
            assert!(!handler.pipe_closed());
            
            // Test SIGPIPE handling (no-op on Windows)
            let result = utils::ignore_sigpipe();
            assert!(result.is_ok(), "SIGPIPE handling should be no-op on Windows");
        }
        
        #[test]
        fn test_windows_pipeline_detection() {
            // On Windows, we can test more specific pipeline behavior
            let is_unix = utils::is_unix_like();
            assert!(!is_unix, "Should not be Unix-like system on Windows");
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::io::Write;
    
    #[test]
    fn test_cross_platform_cli_execution() {
        // Test that the CLI can be executed on the current platform
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "--", "--help"]);
        
        let output = cmd.output();
        assert!(output.is_ok(), "CLI should be executable on current platform");
        
        let output = output.unwrap();
        assert!(output.status.success(), "CLI help should execute successfully");
    }
    
    #[test]
    fn test_cross_platform_pipe_handling() {
        // Test basic pipe handling across platforms
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "--", "--validate-only"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        let mut child = cmd.spawn();
        if let Ok(mut child) = child {
            // Write some test input
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(b"select(name, age)");
            }
            
            // Wait for completion
            let output = child.wait_with_output();
            if let Ok(output) = output {
                // The command should handle piped input without crashing
                // We don't assert success since the input might not be valid dplyr
                println!("Exit status: {}", output.status);
                println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_signal_handler_performance() {
        // Test that signal handler creation is reasonably fast
        let start = Instant::now();
        
        for _ in 0..10 {
            let _handler = SignalHandler::new().unwrap();
        }
        
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 1000, "Signal handler creation should be fast");
    }
    
    #[test]
    fn test_pipeline_detection_performance() {
        // Test that pipeline detection is fast
        let start = Instant::now();
        
        for _ in 0..1000 {
            let _is_piped = StdinReader::is_piped();
            let _is_in_pipeline = utils::is_in_pipeline();
        }
        
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 100, "Pipeline detection should be fast");
    }
}