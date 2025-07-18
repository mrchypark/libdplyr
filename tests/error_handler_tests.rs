//! Unit tests for ErrorHandler module

use libdplyr::cli::error_handler::{
    ErrorHandler, ErrorCategory, ExitCode, ErrorInfo
};
use libdplyr::cli::validator::ValidationErrorInfo;
use libdplyr::TranspileError;
use std::io;

#[test]
fn test_exit_code_constants() {
    // Verify all exit codes are unique and in expected ranges
    let codes = vec![
        ExitCode::SUCCESS,
        ExitCode::GENERAL_ERROR,
        ExitCode::INVALID_ARGUMENTS,
        ExitCode::IO_ERROR,
        ExitCode::VALIDATION_ERROR,
        ExitCode::TRANSPILATION_ERROR,
        ExitCode::CONFIG_ERROR,
        ExitCode::PERMISSION_ERROR,
        ExitCode::SYSTEM_ERROR,
        ExitCode::NETWORK_ERROR,
        ExitCode::TIMEOUT_ERROR,
        ExitCode::INTERNAL_ERROR,
    ];
    
    // Check all codes are unique
    for (i, &code1) in codes.iter().enumerate() {
        for (j, &code2) in codes.iter().enumerate() {
            if i != j {
                assert_ne!(code1, code2, "Exit codes should be unique");
            }
        }
    }
    
    // Check codes are in reasonable range
    for &code in &codes {
        assert!(code >= 0 && code <= 255, "Exit codes should be in range 0-255");
    }
    
    // Check specific values
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
fn test_error_categories() {
    let categories = vec![
        ErrorCategory::UserInput,
        ErrorCategory::System,
        ErrorCategory::Application,
        ErrorCategory::Configuration,
        ErrorCategory::Network,
        ErrorCategory::Internal,
    ];
    
    // Test that categories can be compared
    for (i, cat1) in categories.iter().enumerate() {
        for (j, cat2) in categories.iter().enumerate() {
            if i == j {
                assert_eq!(cat1, cat2);
            } else {
                assert_ne!(cat1, cat2);
            }
        }
    }
}

#[test]
fn test_error_categories_clone() {
    let category1 = ErrorCategory::UserInput;
    let category2 = category1.clone();
    assert_eq!(category1, category2);
}

#[test]
fn test_error_categories_debug() {
    let category = ErrorCategory::System;
    let debug_str = format!("{:?}", category);
    assert!(debug_str.contains("System"));
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
fn test_error_info_display() {
    let error_info = ErrorInfo::new(
        ErrorCategory::Application,
        ExitCode::GENERAL_ERROR,
        "Application error occurred".to_string(),
    );
    
    assert_eq!(error_info.to_string(), "Application error occurred");
}

#[test]
fn test_error_info_clone() {
    let error_info1 = ErrorInfo::new(
        ErrorCategory::Configuration,
        ExitCode::CONFIG_ERROR,
        "Config error".to_string(),
    )
    .with_description("Invalid configuration".to_string())
    .with_suggestions(vec!["Check config file".to_string()]);
    
    let error_info2 = error_info1.clone();
    
    assert_eq!(error_info1.category, error_info2.category);
    assert_eq!(error_info1.exit_code, error_info2.exit_code);
    assert_eq!(error_info1.message, error_info2.message);
    assert_eq!(error_info1.description, error_info2.description);
    assert_eq!(error_info1.suggestions, error_info2.suggestions);
}

#[test]
fn test_error_info_debug() {
    let error_info = ErrorInfo::new(
        ErrorCategory::Network,
        ExitCode::NETWORK_ERROR,
        "Network error".to_string(),
    );
    
    let debug_str = format!("{:?}", error_info);
    assert!(debug_str.contains("ErrorInfo"));
    assert!(debug_str.contains("Network"));
    assert!(debug_str.contains("Network error"));
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
fn test_error_handler_default() {
    let handler1 = ErrorHandler::new();
    let handler2 = ErrorHandler::default();
    
    assert_eq!(handler1.use_korean, handler2.use_korean);
    assert_eq!(handler1.verbose, handler2.verbose);
    assert_eq!(handler1.use_colors, handler2.use_colors);
}

#[test]
fn test_error_handler_debug() {
    let handler = ErrorHandler::new();
    let debug_str = format!("{:?}", handler);
    
    assert!(debug_str.contains("ErrorHandler"));
    assert!(debug_str.contains("use_korean"));
    assert!(debug_str.contains("verbose"));
}

#[test]
fn test_error_handler_transpile_errors() {
    let handler = ErrorHandler::new();
    
    // Test lexical error
    let lex_error = TranspileError::LexError(
        libdplyr::LexError::UnexpectedCharacter('@', 5)
    );
    let exit_code = handler.handle_transpile_error(&lex_error);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
    
    // Test parse error
    let parse_error = TranspileError::ParseError(
        libdplyr::ParseError::UnexpectedToken {
            expected: "identifier".to_string(),
            found: "number".to_string(),
            position: 10,
        }
    );
    let exit_code = handler.handle_transpile_error(&parse_error);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
    
    // Test generation error
    let gen_error = TranspileError::GenerationError(
        libdplyr::GenerationError::UnsupportedOperation {
            operation: "complex_join".to_string(),
            dialect: "sqlite".to_string(),
        }
    );
    let exit_code = handler.handle_transpile_error(&gen_error);
    assert_eq!(exit_code, ExitCode::TRANSPILATION_ERROR);
}

#[test]
fn test_error_handler_validation_errors() {
    let handler = ErrorHandler::new();
    
    let test_cases = vec![
        (
            ValidationErrorInfo {
                error_type: "input".to_string(),
                message: "Empty input".to_string(),
                position: None,
                context: None,
            },
            ExitCode::VALIDATION_ERROR,
        ),
        (
            ValidationErrorInfo {
                error_type: "parse".to_string(),
                message: "Unexpected token".to_string(),
                position: Some(5),
                context: Some("at function call".to_string()),
            },
            ExitCode::VALIDATION_ERROR,
        ),
        (
            ValidationErrorInfo {
                error_type: "complexity".to_string(),
                message: "Query too complex".to_string(),
                position: None,
                context: None,
            },
            ExitCode::VALIDATION_ERROR,
        ),
        (
            ValidationErrorInfo {
                error_type: "semantic".to_string(),
                message: "Aggregation without grouping".to_string(),
                position: None,
                context: Some("in summarise operation".to_string()),
            },
            ExitCode::VALIDATION_ERROR,
        ),
        (
            ValidationErrorInfo {
                error_type: "lex".to_string(),
                message: "Invalid character".to_string(),
                position: Some(15),
                context: None,
            },
            ExitCode::VALIDATION_ERROR,
        ),
    ];
    
    for (error, expected_exit_code) in test_cases {
        let exit_code = handler.handle_validation_error(&error);
        assert_eq!(exit_code, expected_exit_code);
    }
}

#[test]
fn test_error_handler_io_errors() {
    let handler = ErrorHandler::new();
    
    let test_cases = vec![
        (
            io::Error::new(io::ErrorKind::NotFound, "File not found"),
            ExitCode::IO_ERROR,
        ),
        (
            io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied"),
            ExitCode::PERMISSION_ERROR,
        ),
        (
            io::Error::new(io::ErrorKind::InvalidInput, "Invalid input"),
            ExitCode::IO_ERROR,
        ),
        (
            io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF"),
            ExitCode::IO_ERROR,
        ),
        (
            io::Error::new(io::ErrorKind::BrokenPipe, "Broken pipe"),
            ExitCode::IO_ERROR,
        ),
        (
            io::Error::new(io::ErrorKind::ConnectionRefused, "Connection refused"),
            ExitCode::IO_ERROR,
        ),
    ];
    
    for (error, expected_exit_code) in test_cases {
        let exit_code = handler.handle_io_error(&error);
        assert_eq!(exit_code, expected_exit_code);
    }
}

#[test]
fn test_error_handler_general_errors() {
    let handler = ErrorHandler::new();
    
    let test_cases = vec![
        ("Invalid argument", ErrorCategory::UserInput, ExitCode::INVALID_ARGUMENTS),
        ("System failure", ErrorCategory::System, ExitCode::IO_ERROR),
        ("Application error", ErrorCategory::Application, ExitCode::GENERAL_ERROR),
        ("Config error", ErrorCategory::Configuration, ExitCode::CONFIG_ERROR),
        ("Network error", ErrorCategory::Network, ExitCode::NETWORK_ERROR),
        ("Internal error", ErrorCategory::Internal, ExitCode::INTERNAL_ERROR),
    ];
    
    for (message, category, expected_exit_code) in test_cases {
        let exit_code = handler.handle_general_error(message, category);
        assert_eq!(exit_code, expected_exit_code);
    }
}

#[test]
fn test_error_handler_korean_vs_english() {
    // Test Korean messages
    let korean_handler = ErrorHandler::with_settings(true, false, false);
    let validation_error = ValidationErrorInfo {
        error_type: "parse".to_string(),
        message: "Unexpected token".to_string(),
        position: None,
        context: None,
    };
    
    let exit_code = korean_handler.handle_validation_error(&validation_error);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
    
    // Test English messages
    let english_handler = ErrorHandler::with_settings(false, false, false);
    let exit_code = english_handler.handle_validation_error(&validation_error);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
}

#[test]
fn test_error_handler_verbose_mode() {
    let verbose_handler = ErrorHandler::with_settings(true, true, false);
    let normal_handler = ErrorHandler::with_settings(true, false, false);
    
    let error = ValidationErrorInfo {
        error_type: "complexity".to_string(),
        message: "Query is too complex".to_string(),
        position: None,
        context: Some("in pipeline analysis".to_string()),
    };
    
    // Both should return the same exit code
    let verbose_exit = verbose_handler.handle_validation_error(&error);
    let normal_exit = normal_handler.handle_validation_error(&error);
    
    assert_eq!(verbose_exit, normal_exit);
    assert_eq!(verbose_exit, ExitCode::VALIDATION_ERROR);
}

#[test]
fn test_error_handler_colors_mode() {
    let color_handler = ErrorHandler::with_settings(true, false, true);
    let normal_handler = ErrorHandler::with_settings(true, false, false);
    
    assert!(color_handler.use_colors);
    assert!(!normal_handler.use_colors);
    
    // Both should handle errors the same way
    let error = ValidationErrorInfo {
        error_type: "input".to_string(),
        message: "Empty input".to_string(),
        position: None,
        context: None,
    };
    
    let color_exit = color_handler.handle_validation_error(&error);
    let normal_exit = normal_handler.handle_validation_error(&error);
    
    assert_eq!(color_exit, normal_exit);
}

#[test]
fn test_error_handler_message_methods() {
    let handler = ErrorHandler::new();
    
    // These methods should not panic
    handler.print_success("Operation completed");
    handler.print_warning("This is a warning");
    handler.print_info("This is information");
    
    // Test with English handler
    let english_handler = ErrorHandler::with_settings(false, false, false);
    english_handler.print_success("Operation completed");
    english_handler.print_warning("This is a warning");
    english_handler.print_info("This is information");
}

#[test]
fn test_error_handler_transpile_error_types() {
    let handler = ErrorHandler::new();
    
    // Test IO error
    let io_error = TranspileError::IoError("File read failed".to_string());
    let exit_code = handler.handle_transpile_error(&io_error);
    assert_eq!(exit_code, ExitCode::IO_ERROR);
    
    // Test validation error
    let validation_error = TranspileError::ValidationError("Invalid syntax".to_string());
    let exit_code = handler.handle_transpile_error(&validation_error);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
    
    // Test configuration error
    let config_error = TranspileError::ConfigurationError("Invalid config".to_string());
    let exit_code = handler.handle_transpile_error(&config_error);
    assert_eq!(exit_code, ExitCode::CONFIG_ERROR);
    
    // Test system error
    let system_error = TranspileError::SystemError("Signal handling failed".to_string());
    let exit_code = handler.handle_transpile_error(&system_error);
    assert_eq!(exit_code, ExitCode::SYSTEM_ERROR);
}

#[test]
fn test_error_handler_handle_error_method() {
    let handler = ErrorHandler::new();
    
    // Test different error types through the generic handle_error method
    let lex_error = TranspileError::LexError(
        libdplyr::LexError::UnexpectedCharacter('$', 3)
    );
    let exit_code = handler.handle_error(&lex_error);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
    
    let gen_error = TranspileError::GenerationError(
        libdplyr::GenerationError::UnsupportedOperation {
            operation: "custom_func".to_string(),
            dialect: "mysql".to_string(),
        }
    );
    let exit_code = handler.handle_error(&gen_error);
    assert_eq!(exit_code, ExitCode::TRANSPILATION_ERROR);
}

#[test]
fn test_validation_error_info_with_context() {
    let handler = ErrorHandler::new();
    
    let error_with_context = ValidationErrorInfo {
        error_type: "parse".to_string(),
        message: "Missing closing parenthesis".to_string(),
        position: Some(25),
        context: Some("in function call at line 2".to_string()),
    };
    
    let exit_code = handler.handle_validation_error(&error_with_context);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
}

#[test]
fn test_validation_error_info_without_context() {
    let handler = ErrorHandler::new();
    
    let error_without_context = ValidationErrorInfo {
        error_type: "lex".to_string(),
        message: "Invalid string literal".to_string(),
        position: Some(10),
        context: None,
    };
    
    let exit_code = handler.handle_validation_error(&error_without_context);
    assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
}

#[test]
fn test_error_handler_settings_combinations() {
    // Test all combinations of settings
    let combinations = vec![
        (true, true, true),
        (true, true, false),
        (true, false, true),
        (true, false, false),
        (false, true, true),
        (false, true, false),
        (false, false, true),
        (false, false, false),
    ];
    
    for (korean, verbose, colors) in combinations {
        let handler = ErrorHandler::with_settings(korean, verbose, colors);
        assert_eq!(handler.use_korean, korean);
        assert_eq!(handler.verbose, verbose);
        assert_eq!(handler.use_colors, colors);
        
        // Test that handler works with these settings
        let error = ValidationErrorInfo {
            error_type: "test".to_string(),
            message: "Test error".to_string(),
            position: None,
            context: None,
        };
        
        let exit_code = handler.handle_validation_error(&error);
        assert_eq!(exit_code, ExitCode::VALIDATION_ERROR);
    }
}