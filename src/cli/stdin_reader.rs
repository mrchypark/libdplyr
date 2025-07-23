//! Stdin input processing module
//!
//! Provides functionality for reading input from stdin with pipe detection,
//! signal handling, and proper handling of empty input and EOF conditions.

use crate::cli::signal_handler::{ProcessingError, SignalError, SignalHandler};
use std::io::{self, BufRead, BufReader, IsTerminal, Read};
use std::time::Duration;

/// Result type for stdin reading operations
pub type StdinResult<T> = Result<T, StdinError>;

/// Errors that can occur during stdin reading
#[derive(Debug, thiserror::Error)]
pub enum StdinError {
    #[error("Failed to read from stdin: {0}")]
    ReadError(#[from] io::Error),

    #[error("No input provided and stdin is not piped")]
    NoInputProvided,

    #[error("Input is empty")]
    EmptyInput,

    #[error("Stdin read timeout")]
    Timeout,

    #[error("Reading interrupted by signal")]
    Interrupted,

    #[error("Pipe was closed")]
    PipeClosed,

    #[error("Signal handling error: {0}")]
    SignalError(#[from] SignalError),

    #[error("Processing error: {0}")]
    ProcessingError(#[from] ProcessingError),
}

/// Configuration for stdin reading behavior
#[derive(Debug, Clone)]
pub struct StdinConfig {
    /// Maximum time to wait for input when stdin is not piped
    pub read_timeout: Option<Duration>,
    /// Whether to trim whitespace from input
    pub trim_input: bool,
    /// Maximum input size in bytes (None for unlimited)
    pub max_input_size: Option<usize>,
}

impl Default for StdinConfig {
    fn default() -> Self {
        Self {
            read_timeout: Some(Duration::from_secs(1)),
            trim_input: true,
            max_input_size: Some(10 * 1024 * 1024), // 10MB default limit
        }
    }
}

/// Stdin reader with pipe detection, signal handling, and configuration options
#[derive(Debug)]
pub struct StdinReader {
    config: StdinConfig,
    signal_handler: Option<SignalHandler>,
}

impl StdinReader {
    /// Creates a new StdinReader with default configuration
    pub fn new() -> Self {
        Self {
            config: StdinConfig::default(),
            signal_handler: None,
        }
    }

    /// Creates a new StdinReader with custom configuration
    pub fn with_config(config: StdinConfig) -> Self {
        Self {
            config,
            signal_handler: None,
        }
    }

    /// Creates a new StdinReader with signal handling enabled
    pub fn with_signal_handling() -> StdinResult<Self> {
        Ok(Self {
            config: StdinConfig::default(),
            signal_handler: Some(SignalHandler::new()?),
        })
    }

    /// Creates a new StdinReader with custom configuration and signal handling
    pub fn with_config_and_signals(config: StdinConfig) -> StdinResult<Self> {
        Ok(Self {
            config,
            signal_handler: Some(SignalHandler::new()?),
        })
    }

    /// Checks if stdin is connected to a pipe (not a terminal)
    pub fn is_piped() -> bool {
        !io::stdin().is_terminal()
    }

    /// Reads all available input from stdin
    pub fn read_all(&self) -> StdinResult<String> {
        let mut input = String::new();

        // Check if stdin is piped
        if Self::is_piped() {
            // Read from pipe - this should be fast and reliable
            self.read_from_pipe(&mut input)?;
        } else {
            // Interactive mode - check if there's any input available
            return Err(StdinError::NoInputProvided);
        }

        // Apply input processing
        self.process_input(input)
    }

    /// Reads input from a pipe with signal handling and memory efficiency
    fn read_from_pipe(&self, buffer: &mut String) -> StdinResult<()> {
        if let Some(ref signal_handler) = self.signal_handler {
            self.read_from_pipe_with_signals(buffer, signal_handler)
        } else {
            self.read_from_pipe_simple(buffer)
        }
    }

    /// Simple pipe reading without signal handling
    fn read_from_pipe_simple(&self, buffer: &mut String) -> StdinResult<()> {
        let mut stdin = io::stdin();
        let mut temp_buffer = Vec::new();

        // Read all input at once
        stdin.read_to_end(&mut temp_buffer)?;

        // Check size limit
        self.check_size_limit(&temp_buffer)?;

        // Convert to string
        *buffer = String::from_utf8(temp_buffer).map_err(|e| {
            StdinError::ReadError(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 input: {}", e),
            ))
        })?;

        Ok(())
    }

    /// Pipe reading with signal handling and chunked processing
    fn read_from_pipe_with_signals(
        &self,
        buffer: &mut String,
        signal_handler: &SignalHandler,
    ) -> StdinResult<()> {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut temp_buffer = Vec::new();
        let chunk_size = 8192; // 8KB chunks
        let mut total_size = 0;

        loop {
            // Check for signals before reading each chunk
            if signal_handler.should_shutdown() {
                if signal_handler.pipe_closed() {
                    return Err(StdinError::PipeClosed);
                } else {
                    return Err(StdinError::Interrupted);
                }
            }

            // Read a chunk
            let mut chunk = vec![0u8; chunk_size];
            match reader.read(&mut chunk)? {
                0 => break, // EOF reached
                n => {
                    chunk.truncate(n);
                    total_size += n;

                    // Check size limit incrementally
                    if let Some(max_size) = self.config.max_input_size {
                        if total_size > max_size {
                            return Err(StdinError::ReadError(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "Input size ({} bytes) exceeds maximum allowed size ({} bytes)",
                                    total_size, max_size
                                ),
                            )));
                        }
                    }

                    temp_buffer.extend_from_slice(&chunk);
                }
            }
        }

        // Convert to string
        *buffer = String::from_utf8(temp_buffer).map_err(|e| {
            StdinError::ReadError(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 input: {}", e),
            ))
        })?;

        Ok(())
    }

    /// Check if the buffer size exceeds the configured limit
    fn check_size_limit(&self, buffer: &[u8]) -> StdinResult<()> {
        if let Some(max_size) = self.config.max_input_size {
            if buffer.len() > max_size {
                return Err(StdinError::ReadError(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Input size ({} bytes) exceeds maximum allowed size ({} bytes)",
                        buffer.len(),
                        max_size
                    ),
                )));
            }
        }
        Ok(())
    }

    /// Reads input line by line for streaming processing (future extension)
    pub fn read_lines(&self) -> StdinResult<Vec<String>> {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = Vec::new();

        for line_result in reader.lines() {
            // Check for signals if handler is available
            if let Some(ref signal_handler) = self.signal_handler {
                if signal_handler.should_shutdown() {
                    if signal_handler.pipe_closed() {
                        return Err(StdinError::PipeClosed);
                    } else {
                        return Err(StdinError::Interrupted);
                    }
                }
            }

            let line = line_result?;
            lines.push(line);

            // Check size limit
            let total_size: usize = lines.iter().map(|l| l.len()).sum();
            if let Some(max_size) = self.config.max_input_size {
                if total_size > max_size {
                    return Err(StdinError::ReadError(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Input size ({} bytes) exceeds maximum allowed size ({} bytes)",
                            total_size, max_size
                        ),
                    )));
                }
            }
        }

        Ok(lines)
    }

    /// Reads input with streaming support for large inputs
    pub fn read_streaming<F>(&self, mut processor: F) -> StdinResult<()>
    where
        F: FnMut(&str) -> StdinResult<()>,
    {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);

        for line_result in reader.lines() {
            // Check for signals if handler is available
            if let Some(ref signal_handler) = self.signal_handler {
                if signal_handler.should_shutdown() {
                    if signal_handler.pipe_closed() {
                        return Err(StdinError::PipeClosed);
                    } else {
                        return Err(StdinError::Interrupted);
                    }
                }
            }

            let line = line_result?;
            processor(&line)?;
        }

        Ok(())
    }

    /// Processes the input according to configuration
    fn process_input(&self, mut input: String) -> StdinResult<String> {
        // Trim whitespace if configured
        if self.config.trim_input {
            input = input.trim().to_string();
        }

        // Check for empty input
        if input.is_empty() {
            return Err(StdinError::EmptyInput);
        }

        Ok(input)
    }

    /// Attempts to read input with fallback behavior
    pub fn read_with_fallback(&self) -> StdinResult<String> {
        match self.read_all() {
            Ok(input) => Ok(input),
            Err(StdinError::NoInputProvided) => {
                // Provide helpful error message for interactive use
                Err(StdinError::NoInputProvided)
            }
            Err(e) => Err(e),
        }
    }

    /// Gets the current stdin configuration
    pub fn config(&self) -> &StdinConfig {
        &self.config
    }

    /// Updates the stdin configuration
    pub fn set_config(&mut self, config: StdinConfig) {
        self.config = config;
    }
}

impl Default for StdinReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdin_config_default() {
        let config = StdinConfig::default();
        assert!(config.read_timeout.is_some());
        assert!(config.trim_input);
        assert_eq!(config.max_input_size, Some(10 * 1024 * 1024));
    }

    #[test]
    fn test_stdin_reader_creation() {
        let reader = StdinReader::new();
        assert!(reader.config.trim_input);

        let custom_config = StdinConfig {
            read_timeout: None,
            trim_input: false,
            max_input_size: Some(1024),
        };
        let custom_reader = StdinReader::with_config(custom_config);
        assert!(!custom_reader.config.trim_input);
        assert_eq!(custom_reader.config.max_input_size, Some(1024));
    }

    #[test]
    fn test_process_input_trimming() {
        let reader = StdinReader::new();

        // Test trimming
        let result = reader.process_input("  hello world  ".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");

        // Test empty after trimming
        let result = reader.process_input("   ".to_string());
        assert!(matches!(result, Err(StdinError::EmptyInput)));
    }

    #[test]
    fn test_process_input_no_trimming() {
        let config = StdinConfig {
            trim_input: false,
            ..Default::default()
        };
        let reader = StdinReader::with_config(config);

        let result = reader.process_input("  hello world  ".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "  hello world  ");
    }

    #[test]
    fn test_empty_input_handling() {
        let reader = StdinReader::new();

        let result = reader.process_input("".to_string());
        assert!(matches!(result, Err(StdinError::EmptyInput)));
    }

    #[test]
    fn test_stdin_error_display() {
        let error = StdinError::NoInputProvided;
        assert_eq!(
            error.to_string(),
            "No input provided and stdin is not piped"
        );

        let error = StdinError::EmptyInput;
        assert_eq!(error.to_string(), "Input is empty");

        let error = StdinError::Timeout;
        assert_eq!(error.to_string(), "Stdin read timeout");
    }

    // Note: Testing actual stdin reading is difficult in unit tests
    // as it requires actual pipe input. These would be better tested
    // in integration tests with actual command execution.

    #[test]
    fn test_is_piped_in_test_environment() {
        // In test environment, this will typically return true or false
        // depending on how tests are run. We just verify it doesn't panic.
        let _is_piped = StdinReader::is_piped();
    }
}
