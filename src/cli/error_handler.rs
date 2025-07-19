//! Enhanced error handling and exit code management module
//!
//! Provides comprehensive error handling with appropriate exit codes and
//! detailed error messages with hints for resolution.

use crate::TranspileError;
use crate::cli::validator::ValidationErrorInfo;
use std::fmt;
use std::io::{self, Write};

/// Standard exit codes for the CLI application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCode;

impl ExitCode {
    /// Success - operation completed successfully
    pub const SUCCESS: i32 = 0;
    
    /// General error - unspecified error occurred
    pub const GENERAL_ERROR: i32 = 1;
    
    /// Invalid arguments - command line arguments are invalid
    pub const INVALID_ARGUMENTS: i32 = 2;
    
    /// Input/Output error - file or stdin/stdout operations failed
    pub const IO_ERROR: i32 = 3;
    
    /// Validation error - dplyr syntax validation failed
    pub const VALIDATION_ERROR: i32 = 4;
    
    /// Transpilation error - SQL generation failed
    pub const TRANSPILATION_ERROR: i32 = 5;
    
    /// Configuration error - invalid configuration or settings
    pub const CONFIG_ERROR: i32 = 6;
    
    /// Permission error - insufficient permissions
    pub const PERMISSION_ERROR: i32 = 7;
    
    /// System error - system-level operations failed (signals, pipes, etc.)
    pub const SYSTEM_ERROR: i32 = 8;
    
    /// Network error - network-related operations failed
    pub const NETWORK_ERROR: i32 = 9;
    
    /// Timeout error - operation timed out
    pub const TIMEOUT_ERROR: i32 = 10;
    
    /// Internal error - unexpected internal error
    pub const INTERNAL_ERROR: i32 = 11;
}

/// Categories of errors for better organization
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    /// User input related errors
    UserInput,
    /// System/IO related errors
    System,
    /// Application logic errors
    Application,
    /// Configuration errors
    Configuration,
    /// Network related errors
    Network,
    /// Internal/unexpected errors
    Internal,
}

/// Comprehensive error information
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    /// Error category
    pub category: ErrorCategory,
    
    /// Exit code to use
    pub exit_code: i32,
    
    /// Primary error message
    pub message: String,
    
    /// Detailed description (optional)
    pub description: Option<String>,
    
    /// Context information (optional)
    pub context: Option<String>,
    
    /// Suggestions for fixing the error
    pub suggestions: Vec<String>,
    
    /// Whether this error should be reported to stderr
    pub use_stderr: bool,
    
    /// Whether to show help information
    pub show_help: bool,
}

impl ErrorInfo {
    /// Creates a new ErrorInfo with basic information
    pub fn new(category: ErrorCategory, exit_code: i32, message: String) -> Self {
        Self {
            category,
            exit_code,
            message,
            description: None,
            context: None,
            suggestions: Vec::new(),
            use_stderr: true,
            show_help: false,
        }
    }
    
    /// Adds a detailed description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
    
    /// Adds context information
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }
    
    /// Adds suggestions for fixing the error
    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }
    
    /// Sets whether to show help information
    pub fn with_help(mut self, show_help: bool) -> Self {
        self.show_help = show_help;
        self
    }
    
    /// Sets whether to use stderr for output
    pub fn with_stderr(mut self, use_stderr: bool) -> Self {
        self.use_stderr = use_stderr;
        self
    }
}

impl fmt::Display for ErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Enhanced error handler for CLI operations
#[derive(Debug)]
pub struct ErrorHandler {
    /// Whether to use Korean messages
    pub use_korean: bool,
    
    /// Whether to show verbose error information
    pub verbose: bool,
    
    /// Whether to use colored output (if supported)
    pub use_colors: bool,
}

impl ErrorHandler {
    /// Creates a new error handler with default settings
    pub fn new() -> Self {
        Self {
            use_korean: false,  // Use English by default
            verbose: false,
            use_colors: false,
        }
    }
    
    /// Creates a new error handler with custom settings
    pub fn with_settings(use_korean: bool, verbose: bool, use_colors: bool) -> Self {
        Self {
            use_korean,
            verbose,
            use_colors,
        }
    }
    
    /// Handles a transpilation error and returns appropriate exit code
    pub fn handle_transpile_error(&self, error: &TranspileError) -> i32 {
        let error_info = self.convert_transpile_error(error);
        self.print_error(&error_info);
        error_info.exit_code
    }
    
    /// Handles a validation error and returns appropriate exit code
    pub fn handle_validation_error(&self, error: &ValidationErrorInfo) -> i32 {
        let error_info = self.convert_validation_error(error);
        self.print_error(&error_info);
        error_info.exit_code
    }
    
    /// Handles an IO error and returns appropriate exit code
    pub fn handle_io_error(&self, error: &std::io::Error) -> i32 {
        let error_info = self.convert_io_error(error);
        self.print_error(&error_info);
        error_info.exit_code
    }
    
    /// Handles a general error and returns appropriate exit code
    pub fn handle_general_error(&self, message: &str, category: ErrorCategory) -> i32 {
        let error_info = self.create_general_error(message, category);
        self.print_error(&error_info);
        error_info.exit_code
    }
    
    /// Converts a TranspileError to ErrorInfo
    fn convert_transpile_error(&self, error: &TranspileError) -> ErrorInfo {
        match error {
            TranspileError::LexError(e) => {
                if self.use_korean {
                    ErrorInfo::new(
                        ErrorCategory::UserInput,
                        ExitCode::VALIDATION_ERROR,
                        format!("Tokenization error: {}", e),
                    )
                    .with_description("There is an error in the syntax of the input code.".to_string())
                    .with_suggestions(vec![
                        "Check if string quotes are closed correctly".to_string(),
                        "Check for special characters or escape sequences".to_string(),
                        "Check if any unsupported characters are included".to_string(),
                    ])
                } else {
                    ErrorInfo::new(
                        ErrorCategory::UserInput,
                        ExitCode::VALIDATION_ERROR,
                        format!("Lexical error: {}", e),
                    )
                    .with_description("There is a syntax error in the input code.".to_string())
                    .with_suggestions(vec![
                        "Check if string quotes are properly closed".to_string(),
                        "Verify special characters and escape sequences".to_string(),
                        "Ensure no unsupported characters are included".to_string(),
                    ])
                }
            }
            TranspileError::ParseError(e) => {
                if self.use_korean {
                    ErrorInfo::new(
                        ErrorCategory::UserInput,
                        ExitCode::VALIDATION_ERROR,
                        format!("Parsing error: {}", e),
                    )
                    .with_description("The usage of the dplyr function is incorrect.".to_string())
                    .with_suggestions(vec![
                        "Check if the dplyr function name is correct".to_string(),
                        "Check if the function arguments are provided correctly".to_string(),
                        "Check if the pipe operator (%>%) is used correctly".to_string(),
                        "Check the position of parentheses and commas".to_string(),
                    ])
                    .with_help(true)
                } else {
                    ErrorInfo::new(
                        ErrorCategory::UserInput,
                        ExitCode::VALIDATION_ERROR,
                        format!("Parse error: {}", e),
                    )
                    .with_description("The dplyr function usage is incorrect.".to_string())
                    .with_suggestions(vec![
                        "Check if dplyr function names are correct".to_string(),
                        "Verify function arguments are properly provided".to_string(),
                        "Ensure pipe operator (%>%) is used correctly".to_string(),
                        "Check parentheses and comma placement".to_string(),
                    ])
                    .with_help(true)
                }
            }
            TranspileError::GenerationError(e) => {
                if self.use_korean {
                    ErrorInfo::new(
                        ErrorCategory::Application,
                        ExitCode::TRANSPILATION_ERROR,
                        format!("SQL generation error: {}", e),
                    )
                    .with_description("Unsupported feature or complex expression in the selected SQL dialect.".to_string())
                    .with_suggestions(vec![
                        "Try a different SQL dialect (use the -d option)".to_string(),
                        "Try breaking it down into simpler expressions".to_string(),
                        "Use only supported functions and operators".to_string(),
                    ])
                } else {
                    ErrorInfo::new(
                        ErrorCategory::Application,
                        ExitCode::TRANSPILATION_ERROR,
                        format!("SQL generation error: {}", e),
                    )
                    .with_description("The feature is not supported in the selected SQL dialect or the expression is too complex.".to_string())
                    .with_suggestions(vec![
                        "Try a different SQL dialect (use -d option)".to_string(),
                        "Break down into simpler expressions".to_string(),
                        "Use only supported functions and operators".to_string(),
                    ])
                }
            }
            TranspileError::IoError(e) => {
                if self.use_korean {
                    ErrorInfo::new(
                        ErrorCategory::System,
                        ExitCode::IO_ERROR,
                        format!("I/O error: {}", e),
                    )
                    .with_description("An error occurred during file or I/O operations.".to_string())
                    .with_suggestions(vec![
                        "Check the file path and permissions".to_string(),
                        "Check the disk space".to_string(),
                    ])
                } else {
                    ErrorInfo::new(
                        ErrorCategory::System,
                        ExitCode::IO_ERROR,
                        format!("I/O error: {}", e),
                    )
                    .with_description("An error occurred during file or I/O operations.".to_string())
                    .with_suggestions(vec![
                        "Check file paths and permissions".to_string(),
                        "Verify disk space availability".to_string(),
                    ])
                }
            }
            TranspileError::ValidationError(e) => {
                if self.use_korean {
                    ErrorInfo::new(
                        ErrorCategory::UserInput,
                        ExitCode::VALIDATION_ERROR,
                        format!("Validation error: {}", e),
                    )
                    .with_description("dplyr code validation failed.".to_string())
                    .with_suggestions(vec![
                        "Check the dplyr syntax".to_string(),
                        "Check the function usage".to_string(),
                    ])
                } else {
                    ErrorInfo::new(
                        ErrorCategory::UserInput,
                        ExitCode::VALIDATION_ERROR,
                        format!("Validation error: {}", e),
                    )
                    .with_description("dplyr code validation failed.".to_string())
                    .with_suggestions(vec![
                        "Check dplyr syntax".to_string(),
                        "Verify function usage".to_string(),
                    ])
                }
            }
            TranspileError::ConfigurationError(e) => {
                if self.use_korean {
                    ErrorInfo::new(
                        ErrorCategory::Configuration,
                        ExitCode::CONFIG_ERROR,
                        format!("Configuration error: {}", e),
                    )
                    .with_description("There is a problem with the settings or configuration.".to_string())
                    .with_suggestions(vec![
                        "Check the configuration options".to_string(),
                        "Check if all required parameters are provided".to_string(),
                    ])
                } else {
                    ErrorInfo::new(
                        ErrorCategory::Configuration,
                        ExitCode::CONFIG_ERROR,
                        format!("Configuration error: {}", e),
                    )
                    .with_description("There is a problem with configuration or settings.".to_string())
                    .with_suggestions(vec![
                        "Check configuration options".to_string(),
                        "Verify all required parameters are provided".to_string(),
                    ])
                }
            }
            TranspileError::SystemError(e) => {
                if self.use_korean {
                    ErrorInfo::new(
                        ErrorCategory::System,
                        ExitCode::SYSTEM_ERROR,
                        format!("System error: {}", e),
                    )
                    .with_description("A system-level error occurred.".to_string())
                    .with_suggestions(vec![
                        "Check the system permissions".to_string(),
                        "Check the signal handling or pipeline settings".to_string(),
                    ])
                } else {
                    ErrorInfo::new(
                        ErrorCategory::System,
                        ExitCode::SYSTEM_ERROR,
                        format!("System error: {}", e),
                    )
                    .with_description("A system-level error occurred.".to_string())
                    .with_suggestions(vec![
                        "Check system permissions".to_string(),
                        "Verify signal handling or pipeline configuration".to_string(),
                    ])
                }
            }
        }
    }
    
    /// Converts a ValidationErrorInfo to ErrorInfo
    fn convert_validation_error(&self, error: &ValidationErrorInfo) -> ErrorInfo {
        let (message, description, suggestions) = if self.use_korean {
            match error.error_type.as_str() {
                "input" => (
                    format!("Input error: {}", error.message),
                    Some("Please provide valid dplyr code.".to_string()),
                    vec!["Example: data %>% select(name, age)".to_string()],
                ),
                "lex" => (
                    format!("Tokenization error: {}", error.message),
                    Some("Please check the syntax of the input code.".to_string()),
                    vec![
                        "Check the string quotes".to_string(),
                        "Check for special characters".to_string(),
                    ],
                ),
                "parse" => (
                    format!("Parsing error: {}", error.message),
                    Some("Please check the dplyr function usage.".to_string()),
                    vec![
                        "Check the function names".to_string(),
                        "Check the pipe operator usage".to_string(),
                    ],
                ),
                "complexity" => (
                    format!("Complexity error: {}", error.message),
                    Some("The query is too complex.".to_string()),
                    vec![
                        "Break the query into simpler parts".to_string(),
                        "Remove unnecessary operations".to_string(),
                    ],
                ),
                "semantic" => (
                    format!("Semantic error: {}", error.message),
                    Some("Please check the logical structure of the query.".to_string()),
                    vec![
                        "Consider using group_by() with aggregate functions".to_string(),
                        "Check the order of operations".to_string(),
                    ],
                ),
                _ => (
                    format!("Validation error: {}", error.message),
                    None,
                    vec!["Please check the syntax again".to_string()],
                ),
            }
        } else {
            match error.error_type.as_str() {
                "input" => (
                    format!("Input error: {}", error.message),
                    Some("Please provide valid dplyr code.".to_string()),
                    vec!["Example: data %>% select(name, age)".to_string()],
                ),
                "lex" => (
                    format!("Lexical error: {}", error.message),
                    Some("Please check the syntax of your input code.".to_string()),
                    vec![
                        "Check string quotes".to_string(),
                        "Verify special characters".to_string(),
                    ],
                ),
                "parse" => (
                    format!("Parse error: {}", error.message),
                    Some("Please check dplyr function usage.".to_string()),
                    vec![
                        "Check function names".to_string(),
                        "Verify pipe operator usage".to_string(),
                    ],
                ),
                "complexity" => (
                    format!("Complexity error: {}", error.message),
                    Some("The query is too complex.".to_string()),
                    vec![
                        "Break the query into simpler parts".to_string(),
                        "Remove unnecessary operations".to_string(),
                    ],
                ),
                "semantic" => (
                    format!("Semantic error: {}", error.message),
                    Some("Please check the logical structure of the query.".to_string()),
                    vec![
                        "Consider using group_by() with aggregation functions".to_string(),
                        "Check the order of operations".to_string(),
                    ],
                ),
                _ => (
                    format!("Validation error: {}", error.message),
                    None,
                    vec!["Please check the syntax again".to_string()],
                ),
            }
        };
        
        let mut error_info = ErrorInfo::new(ErrorCategory::UserInput, ExitCode::VALIDATION_ERROR, message)
            .with_description(description.unwrap_or_default())
            .with_suggestions(suggestions);
        
        if let Some(context) = &error.context {
            error_info = error_info.with_context(context.clone());
        }
        
        error_info
    }
    
    /// Converts an IO error to ErrorInfo
    fn convert_io_error(&self, error: &std::io::Error) -> ErrorInfo {
        let (message, description, suggestions) = if self.use_korean {
            match error.kind() {
                io::ErrorKind::NotFound => (
                    "File not found".to_string(),
                    Some("The specified file does not exist.".to_string()),
                    vec![
                        "Check if the file path is correct".to_string(),
                        "Check if the file exists".to_string(),
                    ],
                ),
                io::ErrorKind::PermissionDenied => (
                    "Permission denied".to_string(),
                    Some("You do not have read/write permissions for the file.".to_string()),
                    vec![
                        "Check the file permissions".to_string(),
                        "Try running with administrator privileges".to_string(),
                    ],
                ),
                io::ErrorKind::InvalidInput => (
                    "Invalid input".to_string(),
                    Some("The input data is incorrect.".to_string()),
                    vec![
                        "Check the input format".to_string(),
                        "Check the UTF-8 encoding".to_string(),
                    ],
                ),
                _ => (
                    format!("I/O error: {}", error),
                    None,
                    vec!["Check the system status".to_string()],
                ),
            }
        } else {
            match error.kind() {
                io::ErrorKind::NotFound => (
                    "File not found".to_string(),
                    Some("The specified file does not exist.".to_string()),
                    vec![
                        "Check if the file path is correct".to_string(),
                        "Verify the file exists".to_string(),
                    ],
                ),
                io::ErrorKind::PermissionDenied => (
                    "Permission denied".to_string(),
                    Some("No read/write permission for the file.".to_string()),
                    vec![
                        "Check file permissions".to_string(),
                        "Try running with administrator privileges".to_string(),
                    ],
                ),
                io::ErrorKind::InvalidInput => (
                    "Invalid input".to_string(),
                    Some("The input data is not valid.".to_string()),
                    vec![
                        "Check input format".to_string(),
                        "Verify UTF-8 encoding".to_string(),
                    ],
                ),
                _ => (
                    format!("I/O error: {}", error),
                    None,
                    vec!["Check system status".to_string()],
                ),
            }
        };
        
        let exit_code = match error.kind() {
            io::ErrorKind::PermissionDenied => ExitCode::PERMISSION_ERROR,
            _ => ExitCode::IO_ERROR,
        };
        
        ErrorInfo::new(ErrorCategory::System, exit_code, message)
            .with_description(description.unwrap_or_default())
            .with_suggestions(suggestions)
    }
    
    /// Creates a general error
    fn create_general_error(&self, message: &str, category: ErrorCategory) -> ErrorInfo {
        let exit_code = match category {
            ErrorCategory::UserInput => ExitCode::INVALID_ARGUMENTS,
            ErrorCategory::System => ExitCode::IO_ERROR,
            ErrorCategory::Application => ExitCode::GENERAL_ERROR,
            ErrorCategory::Configuration => ExitCode::CONFIG_ERROR,
            ErrorCategory::Network => ExitCode::NETWORK_ERROR,
            ErrorCategory::Internal => ExitCode::INTERNAL_ERROR,
        };
        
        ErrorInfo::new(category, exit_code, message.to_string())
    }
    
    /// Prints error information to stderr
    pub fn print_error(&self, error_info: &ErrorInfo) {
        let mut stderr = io::stderr();
        
        // Print main error message
        if self.use_korean {
            let _ = writeln!(stderr, "Error: {}", error_info.message);
        } else {
            let _ = writeln!(stderr, "Error: {}", error_info.message);
        }
        
        // Print description if available
        if let Some(description) = &error_info.description {
            let _ = writeln!(stderr, "{}", description);
        }
        
        // Print context if available
        if let Some(context) = &error_info.context {
            if self.use_korean {
                let _ = writeln!(stderr, "Context: {}", context);
            } else {
                let _ = writeln!(stderr, "Context: {}", context);
            }
        }
        
        // Print suggestions
        if !error_info.suggestions.is_empty() {
            let _ = writeln!(stderr);
            if self.use_korean {
                let _ = writeln!(stderr, "Solution:");
            } else {
                let _ = writeln!(stderr, "Suggestions:");
            }
            
            for suggestion in &error_info.suggestions {
                let _ = writeln!(stderr, "  • {}", suggestion);
            }
        }
        
        // Print help information if requested
        if error_info.show_help {
            let _ = writeln!(stderr);
            if self.use_korean {
                let _ = writeln!(stderr, "To see help, run the following command:");
                let _ = writeln!(stderr, "  libdplyr --help");
            } else {
                let _ = writeln!(stderr, "For help, run:");
                let _ = writeln!(stderr, "  libdplyr --help");
            }
        }
        
        let _ = stderr.flush();
    }
    
    /// Prints a success message
    pub fn print_success(&self, message: &str) {
        if self.use_korean {
            println!("Success: {}", message);
        } else {
            println!("Success: {}", message);
        }
    }
    
    /// Prints a warning message
    pub fn print_warning(&self, message: &str) {
        let mut stderr = io::stderr();
        if self.use_korean {
            let _ = writeln!(stderr, "Warning: {}", message);
        } else {
            let _ = writeln!(stderr, "Warning: {}", message);
        }
        let _ = stderr.flush();
    }
    
    /// Prints an info message
    pub fn print_info(&self, message: &str) {
        let mut stderr = io::stderr();
        if self.use_korean {
            let _ = writeln!(stderr, "Info: {}", message);
        } else {
            let _ = writeln!(stderr, "Info: {}", message);
        }
        let _ = stderr.flush();
    }
    
    /// Handles any error and returns appropriate exit code
    pub fn handle_error(&self, error: &TranspileError) -> i32 {
        match error {
            TranspileError::LexError(_) | 
            TranspileError::ParseError(_) | 
            TranspileError::ValidationError(_) => {
                self.handle_transpile_error(error)
            }
            TranspileError::GenerationError(_) => {
                self.handle_transpile_error(error)
            }
            TranspileError::IoError(msg) => {
                let io_error = std::io::Error::new(std::io::ErrorKind::Other, msg.clone());
                self.handle_io_error(&io_error)
            }
            TranspileError::ConfigurationError(msg) => {
                self.handle_general_error(msg, ErrorCategory::Configuration)
            }
            TranspileError::SystemError(msg) => {
                self.handle_general_error(msg, ErrorCategory::System)
            }
        }
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exit_codes() {
        assert_eq!(ExitCode::SUCCESS, 0);
        assert_eq!(ExitCode::GENERAL_ERROR, 1);
        assert_eq!(ExitCode::INVALID_ARGUMENTS, 2);
        assert_eq!(ExitCode::IO_ERROR, 3);
        assert_eq!(ExitCode::VALIDATION_ERROR, 4);
        assert_eq!(ExitCode::TRANSPILATION_ERROR, 5);
        assert_eq!(ExitCode::CONFIG_ERROR, 6);
        assert_eq!(ExitCode::PERMISSION_ERROR, 7);
        assert_eq!(ExitCode::SYSTEM_ERROR, 8);
        assert_eq!(ExitCode::NETWORK_ERROR, 9);
        assert_eq!(ExitCode::TIMEOUT_ERROR, 10);
        assert_eq!(ExitCode::INTERNAL_ERROR, 11);
    }
    
    #[test]
    fn test_error_info_creation() {
        let error_info = ErrorInfo::new(
            ErrorCategory::UserInput,
            ExitCode::VALIDATION_ERROR,
            "Test error".to_string(),
        );
        
        assert_eq!(error_info.category, ErrorCategory::UserInput);
        assert_eq!(error_info.exit_code, ExitCode::VALIDATION_ERROR);
        assert_eq!(error_info.message, "Test error");
        assert!(error_info.description.is_none());
        assert!(error_info.suggestions.is_empty());
        assert!(error_info.use_stderr);
        assert!(!error_info.show_help);
    }
    
    #[test]
    fn test_error_info_builder() {
        let error_info = ErrorInfo::new(
            ErrorCategory::System,
            ExitCode::IO_ERROR,
            "IO error".to_string(),
        )
        .with_description("File not found".to_string())
        .with_context("Reading input file".to_string())
        .with_suggestions(vec!["Check file path".to_string()])
        .with_help(true)
        .with_stderr(false);
        
        assert_eq!(error_info.description, Some("File not found".to_string()));
        assert_eq!(error_info.context, Some("Reading input file".to_string()));
        assert_eq!(error_info.suggestions, vec!["Check file path".to_string()]);
        assert!(error_info.show_help);
        assert!(!error_info.use_stderr);
    }
    
    #[test]
    fn test_error_handler_creation() {
        let handler = ErrorHandler::new();
        assert!(handler.use_korean);
        assert!(!handler.verbose);
        assert!(!handler.use_colors);
        
        let custom_handler = ErrorHandler::with_settings(false, true, true);
        assert!(!custom_handler.use_korean);
        assert!(custom_handler.verbose);
        assert!(custom_handler.use_colors);
    }
    
    #[test]
    fn test_error_categories() {
        assert_eq!(ErrorCategory::UserInput, ErrorCategory::UserInput);
        assert_ne!(ErrorCategory::UserInput, ErrorCategory::System);
    }
    
    #[test]
    fn test_general_error_handling() {
        let handler = ErrorHandler::new();
        let exit_code = handler.handle_general_error("Test error", ErrorCategory::UserInput);
        assert_eq!(exit_code, ExitCode::INVALID_ARGUMENTS);
        
        let exit_code = handler.handle_general_error("System error", ErrorCategory::System);
        assert_eq!(exit_code, ExitCode::IO_ERROR);
    }
    
    #[test]
    fn test_io_error_conversion() {
        let handler = ErrorHandler::new();
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let error_info = handler.convert_io_error(&io_error);
        
        assert_eq!(error_info.category, ErrorCategory::System);
        assert_eq!(error_info.exit_code, ExitCode::IO_ERROR);
        assert!(error_info.message.contains("File not found"));
    }
    
    #[test]
    fn test_validation_error_conversion() {
        let handler = ErrorHandler::new();
        let validation_error = ValidationErrorInfo {
            error_type: "parse".to_string(),
            message: "Unexpected token".to_string(),
            position: Some(10),
            context: Some("at position 10".to_string()),
        };
        
        let error_info = handler.convert_validation_error(&validation_error);
        assert_eq!(error_info.category, ErrorCategory::UserInput);
        assert_eq!(error_info.exit_code, ExitCode::VALIDATION_ERROR);
        assert!(error_info.message.contains("Parsing error"));
    }
    
    #[test]
    fn test_english_messages() {
        let handler = ErrorHandler::with_settings(false, false, false);
        let validation_error = ValidationErrorInfo {
            error_type: "parse".to_string(),
            message: "Unexpected token".to_string(),
            position: None,
            context: None,
        };
        
        let error_info = handler.convert_validation_error(&validation_error);
        assert!(error_info.message.contains("Parse error"));
        assert!(error_info.description.as_ref().unwrap().contains("dplyr function usage"));
    }
}