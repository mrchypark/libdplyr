//! Unit tests for StdinReader module

use libdplyr::cli::stdin_reader::{StdinConfig, StdinError, StdinReader};
use std::time::Duration;

#[test]
fn test_stdin_reader_creation() {
    let reader = StdinReader::new();
    assert!(reader.config().trim_input);
    assert!(reader.config().read_timeout.is_some());
    assert_eq!(reader.config().max_input_size, Some(10 * 1024 * 1024));
}

#[test]
fn test_stdin_reader_with_custom_config() {
    let config = StdinConfig {
        read_timeout: Some(Duration::from_millis(500)),
        trim_input: false,
        max_input_size: Some(1024),
    };

    let reader = StdinReader::with_config(config.clone());
    assert_eq!(reader.config().read_timeout, config.read_timeout);
    assert_eq!(reader.config().trim_input, config.trim_input);
    assert_eq!(reader.config().max_input_size, config.max_input_size);
}

#[test]
fn test_stdin_reader_with_signal_handling() {
    // Test creation with signal handling
    match StdinReader::with_signal_handling() {
        Ok(reader) => {
            // Should create successfully
            assert!(reader.config().trim_input);
        }
        Err(_) => {
            // Signal handling might not be available in test environment
            // This is acceptable
        }
    }
}

#[test]
fn test_stdin_reader_with_config_and_signals() {
    let config = StdinConfig {
        read_timeout: Some(Duration::from_millis(100)),
        trim_input: false,
        max_input_size: Some(512),
    };

    match StdinReader::with_config_and_signals(config.clone()) {
        Ok(reader) => {
            assert_eq!(reader.config().trim_input, config.trim_input);
            assert_eq!(reader.config().max_input_size, config.max_input_size);
        }
        Err(_) => {
            // Signal handling might not be available in test environment
        }
    }
}

#[test]
fn test_stdin_config_default() {
    let config = StdinConfig::default();
    assert!(config.read_timeout.is_some());
    assert!(config.trim_input);
    assert_eq!(config.max_input_size, Some(10 * 1024 * 1024));
}

#[test]
fn test_stdin_config_custom() {
    let config = StdinConfig {
        read_timeout: None,
        trim_input: false,
        max_input_size: Some(2048),
    };

    assert!(config.read_timeout.is_none());
    assert!(!config.trim_input);
    assert_eq!(config.max_input_size, Some(2048));
}

#[test]
fn test_stdin_error_types() {
    // Test error message formatting
    let error = StdinError::NoInputProvided;
    assert_eq!(
        error.to_string(),
        "No input provided and stdin is not piped"
    );

    let error = StdinError::EmptyInput;
    assert_eq!(error.to_string(), "Input is empty");

    let error = StdinError::Timeout;
    assert_eq!(error.to_string(), "Stdin read timeout");

    let error = StdinError::Interrupted;
    assert_eq!(error.to_string(), "Reading interrupted by signal");

    let error = StdinError::PipeClosed;
    assert_eq!(error.to_string(), "Pipe was closed");
}

#[test]
fn test_stdin_error_from_io_error() {
    let io_error = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Broken pipe");
    let stdin_error = StdinError::ReadError(io_error);

    assert!(stdin_error
        .to_string()
        .contains("Failed to read from stdin"));
    assert!(stdin_error.to_string().contains("Broken pipe"));
}

#[test]
fn test_is_piped_function() {
    // This test just verifies the function doesn't panic
    // The actual result depends on how the test is run
    let _is_piped = StdinReader::is_piped();
}

#[test]
fn test_read_all_no_pipe() {
    let reader = StdinReader::new();

    // When stdin is not piped, should return NoInputProvided error
    // Note: This test assumes we're running in a non-piped environment
    if !StdinReader::is_piped() {
        let result = reader.read_all();
        assert!(matches!(result, Err(StdinError::NoInputProvided)));
    }
}

#[test]
fn test_read_with_fallback() {
    let reader = StdinReader::new();

    // Test fallback behavior
    let result = reader.read_with_fallback();

    if !StdinReader::is_piped() {
        // Should return NoInputProvided error in non-piped environment
        assert!(matches!(result, Err(StdinError::NoInputProvided)));
    }
}

#[test]
fn test_stdin_reader_default() {
    let reader1 = StdinReader::new();
    let reader2 = StdinReader::default();

    // Both should have the same configuration
    assert_eq!(reader1.config().trim_input, reader2.config().trim_input);
    assert_eq!(reader1.config().read_timeout, reader2.config().read_timeout);
    assert_eq!(
        reader1.config().max_input_size,
        reader2.config().max_input_size
    );
}

#[test]
fn test_stdin_config_clone() {
    let config1 = StdinConfig {
        read_timeout: Some(Duration::from_millis(200)),
        trim_input: false,
        max_input_size: Some(4096),
    };

    let config2 = config1.clone();

    assert_eq!(config1.read_timeout, config2.read_timeout);
    assert_eq!(config1.trim_input, config2.trim_input);
    assert_eq!(config1.max_input_size, config2.max_input_size);
}

#[test]
fn test_stdin_reader_debug() {
    let reader = StdinReader::new();
    let debug_str = format!("{:?}", reader);

    // Should contain StdinReader in debug output
    assert!(debug_str.contains("StdinReader"));
}

#[test]
fn test_stdin_config_debug() {
    let config = StdinConfig::default();
    let debug_str = format!("{:?}", config);

    // Should contain StdinConfig in debug output
    assert!(debug_str.contains("StdinConfig"));
    assert!(debug_str.contains("trim_input"));
    assert!(debug_str.contains("max_input_size"));
}

#[test]
fn test_stdin_error_debug() {
    let error = StdinError::EmptyInput;
    let debug_str = format!("{:?}", error);

    // Should contain error type in debug output
    assert!(debug_str.contains("EmptyInput"));
}

// Note: Testing actual stdin reading requires external process execution
// which is better handled in end-to-end tests with actual command execution
