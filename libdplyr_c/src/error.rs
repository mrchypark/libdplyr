//! Error handling for C-API bridge
//!
//! Implements the error code system defined in requirements R1-AC3 and R2-AC3
//! with structured error information including position, token, and suggestions.

use std::ffi::CString;
use std::os::raw::c_char;
use thiserror::Error;

// R2-AC3: C-compatible error codes from Appendix C
pub const DPLYR_SUCCESS: i32 = 0;
pub const DPLYR_ERROR_NULL_POINTER: i32 = -1;
pub const DPLYR_ERROR_INVALID_UTF8: i32 = -2;
pub const DPLYR_ERROR_INPUT_TOO_LARGE: i32 = -3;
pub const DPLYR_ERROR_TIMEOUT: i32 = -4;
pub const DPLYR_ERROR_SYNTAX: i32 = -5;
pub const DPLYR_ERROR_UNSUPPORTED: i32 = -6;
pub const DPLYR_ERROR_INTERNAL: i32 = -7;
pub const DPLYR_ERROR_PANIC: i32 = -8;

// R1-AC3, R2-AC3: Error code system from Appendix C
#[derive(Debug, Error, Clone)]
pub enum TranspileError {
    #[error("E-SYNTAX: {message} at position {position}")]
    Syntax {
        code: String,
        message: String,
        position: usize,
        token: Option<String>,      // R1-AC3: Cause token
        suggestion: Option<String>, // R1-AC3: Simple alternative
    },

    #[error("E-UNSUPPORTED: Operation '{operation}' not supported in context '{context}'")]
    Unsupported {
        code: String,
        operation: String,
        alternative: Option<String>, // R1-AC3: Alternative suggestion
        context: String,
    },

    #[error("E-INTERNAL: {details}")]
    Internal {
        code: String,
        details: String,
        recovery_hint: Option<String>,
    },

    #[error("E-FFI: Error at FFI boundary '{boundary}'")]
    Ffi {
        code: String,
        boundary: String,
        safety_info: String,
    },
}

impl TranspileError {
    // R1-AC3: Format error with code, position, token, and suggestion
    pub fn to_c_string(&self) -> CString {
        let formatted = match self {
            Self::Syntax {
                code,
                message,
                position,
                token,
                suggestion,
            } => {
                let token_info = token
                    .as_ref()
                    .map(|t| format!(" (token: '{}')", t))
                    .unwrap_or_default();
                let suggestion_info = suggestion
                    .as_ref()
                    .map(|s| format!(". Try: {}", s))
                    .unwrap_or_default();
                format!(
                    "{}: {} at position {}{}{}",
                    code, message, position, token_info, suggestion_info
                )
            }
            Self::Unsupported {
                code,
                operation,
                alternative,
                context,
            } => {
                let alt_info = alternative
                    .as_ref()
                    .map(|a| format!(". Alternative: {}", a))
                    .unwrap_or_default();
                format!(
                    "{}: Operation '{}' not supported in context '{}'{}",
                    code, operation, context, alt_info
                )
            }
            Self::Internal {
                code,
                details,
                recovery_hint,
            } => {
                let hint_info = recovery_hint
                    .as_ref()
                    .map(|h| format!(". Recovery: {}", h))
                    .unwrap_or_default();
                format!("{}: {}{}", code, details, hint_info)
            }
            Self::Ffi {
                code,
                boundary,
                safety_info,
            } => {
                format!(
                    "{}: Error at FFI boundary '{}'. Safety: {}",
                    code, boundary, safety_info
                )
            }
        };

        CString::new(formatted)
            .unwrap_or_else(|_| CString::new("E-INTERNAL: Error message encoding failed").unwrap())
    }

    // R2-AC3: Explicit error code return
    pub fn get_error_code(&self) -> &str {
        match self {
            Self::Syntax { code, .. } => code,
            Self::Unsupported { code, .. } => code,
            Self::Internal { code, .. } => code,
            Self::Ffi { code, .. } => code,
        }
    }

    // R2-AC3: Convert to C-compatible error code
    pub const fn to_c_error_code(&self) -> i32 {
        match self {
            Self::Syntax { .. } => DPLYR_ERROR_SYNTAX,
            Self::Unsupported { .. } => DPLYR_ERROR_UNSUPPORTED,
            Self::Internal { .. } => DPLYR_ERROR_INTERNAL,
            Self::Ffi { .. } => DPLYR_ERROR_PANIC,
        }
    }

    // Helper constructors for common error cases
    pub fn syntax_error(message: &str, position: usize, token: Option<String>) -> Self {
        Self::Syntax {
            code: "E-SYNTAX".to_string(),
            message: message.to_string(),
            position,
            token,
            suggestion: None,
        }
    }

    pub fn syntax_error_with_suggestion(
        message: &str,
        position: usize,
        token: Option<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self::Syntax {
            code: "E-SYNTAX".to_string(),
            message: message.to_string(),
            position,
            token,
            suggestion,
        }
    }

    pub fn unsupported_operation(operation: &str, context: &str) -> Self {
        Self::Unsupported {
            code: "E-UNSUPPORTED".to_string(),
            operation: operation.to_string(),
            alternative: None,
            context: context.to_string(),
        }
    }

    pub fn unsupported_operation_with_alternative(
        operation: &str,
        context: &str,
        alternative: Option<String>,
    ) -> Self {
        Self::Unsupported {
            code: "E-UNSUPPORTED".to_string(),
            operation: operation.to_string(),
            alternative,
            context: context.to_string(),
        }
    }

    pub fn internal_error(details: &str) -> Self {
        Self::Internal {
            code: "E-INTERNAL".to_string(),
            details: details.to_string(),
            recovery_hint: None,
        }
    }

    pub fn internal_error_with_hint(details: &str, recovery_hint: Option<String>) -> Self {
        Self::Internal {
            code: "E-INTERNAL".to_string(),
            details: details.to_string(),
            recovery_hint,
        }
    }

    pub fn ffi_error(boundary: &str, safety_info: &str) -> Self {
        Self::Ffi {
            code: "E-FFI".to_string(),
            boundary: boundary.to_string(),
            safety_info: safety_info.to_string(),
        }
    }

    // Common error constructors for FFI boundary
    pub fn null_pointer_error(parameter: &str) -> Self {
        Self::ffi_error(
            "parameter_validation",
            &format!("Parameter '{}' is null", parameter),
        )
    }

    pub fn invalid_utf8_error(details: &str) -> Self {
        Self::ffi_error(
            "string_encoding",
            &format!("Invalid UTF-8 encoding: {}", details),
        )
    }

    pub fn input_too_large_error(size: usize, max_size: usize) -> Self {
        Self::internal_error_with_hint(
            &format!("Input size {} exceeds maximum {}", size, max_size),
            Some("Reduce input size or increase max_input_length".to_string()),
        )
    }
}

// FFI helper functions for error handling

/// Get error code name as C string
///
/// # Arguments
/// * `error_code` - C error code
///
/// # Returns
/// Static string pointer (no need to free)
#[no_mangle]
pub const extern "C" fn dplyr_error_code_name(error_code: i32) -> *const c_char {
    match error_code {
        DPLYR_SUCCESS => c"SUCCESS".as_ptr(),
        DPLYR_ERROR_NULL_POINTER => c"E-NULL-POINTER".as_ptr(),
        DPLYR_ERROR_INVALID_UTF8 => c"E-INVALID-UTF8".as_ptr(),
        DPLYR_ERROR_INPUT_TOO_LARGE => c"E-INPUT-TOO-LARGE".as_ptr(),
        DPLYR_ERROR_TIMEOUT => c"E-TIMEOUT".as_ptr(),
        DPLYR_ERROR_SYNTAX => c"E-SYNTAX".as_ptr(),
        DPLYR_ERROR_UNSUPPORTED => c"E-UNSUPPORTED".as_ptr(),
        DPLYR_ERROR_INTERNAL => c"E-INTERNAL".as_ptr(),
        DPLYR_ERROR_PANIC => c"E-PANIC".as_ptr(),
        _ => c"E-UNKNOWN".as_ptr(),
    }
}

/// Check if error code indicates success
///
/// # Arguments
/// * `error_code` - C error code
///
/// # Returns
/// true if success, false if error
#[no_mangle]
pub const extern "C" fn dplyr_is_success(error_code: i32) -> bool {
    error_code == DPLYR_SUCCESS
}

/// Check if error code indicates a recoverable error
///
/// # Arguments
/// * `error_code` - C error code
///
/// # Returns
/// true if recoverable, false if fatal
#[no_mangle]
pub const extern "C" fn dplyr_is_recoverable_error(error_code: i32) -> bool {
    match error_code {
        DPLYR_ERROR_SYNTAX
        | DPLYR_ERROR_UNSUPPORTED
        | DPLYR_ERROR_INPUT_TOO_LARGE
        | DPLYR_ERROR_TIMEOUT => true,
        DPLYR_ERROR_INTERNAL
        | DPLYR_ERROR_PANIC
        | DPLYR_ERROR_NULL_POINTER
        | DPLYR_ERROR_INVALID_UTF8 => false,
        _ => false,
    }
}

// Helper function to create error message with context
pub(crate) fn create_error_message_with_context(
    error: &TranspileError,
    input_context: Option<&str>,
) -> CString {
    let base_message = error.to_c_string();
    let base_str = base_message.to_string_lossy();

    let full_message = input_context.map_or_else(
        || base_str.to_string(),
        |context| {
            format!(
                "{}\nInput context: {}",
                base_str,
                if context.len() > 100 {
                    format!("{}...", &context[..100])
                } else {
                    context.to_string()
                }
            )
        },
    );

    CString::new(full_message)
        .unwrap_or_else(|_| CString::new("E-INTERNAL: Error message encoding failed").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_error_formatting() {
        let error =
            TranspileError::syntax_error("Unexpected token", 5, Some("invalid_token".to_string()));

        let formatted = error.to_c_string();
        let formatted_str = formatted.to_string_lossy();

        assert!(formatted_str.contains("E-SYNTAX"));
        assert!(formatted_str.contains("position 5"));
        assert!(formatted_str.contains("token: 'invalid_token'"));
    }

    #[test]
    fn test_error_code_extraction() {
        let error = TranspileError::unsupported_operation("complex_join", "simple_context");
        assert_eq!(error.get_error_code(), "E-UNSUPPORTED");
    }

    #[test]
    fn test_error_message_encoding_safety() {
        // Test with potentially problematic characters
        let error = TranspileError::internal_error("Error with null\0byte");
        let c_string = error.to_c_string();
        // Should not panic and should handle encoding issues gracefully
        assert!(!c_string.to_string_lossy().is_empty());
    }

    #[test]
    fn test_c_error_codes() {
        let syntax_error = TranspileError::syntax_error("test", 0, None);
        assert_eq!(syntax_error.to_c_error_code(), DPLYR_ERROR_SYNTAX);

        let unsupported_error = TranspileError::unsupported_operation("test", "context");
        assert_eq!(unsupported_error.to_c_error_code(), DPLYR_ERROR_UNSUPPORTED);

        let internal_error = TranspileError::internal_error("test");
        assert_eq!(internal_error.to_c_error_code(), DPLYR_ERROR_INTERNAL);

        let ffi_error = TranspileError::ffi_error("boundary", "info");
        assert_eq!(ffi_error.to_c_error_code(), DPLYR_ERROR_PANIC);
    }

    #[test]
    fn test_error_constructors_with_suggestions() {
        let syntax_error = TranspileError::syntax_error_with_suggestion(
            "Invalid token",
            5,
            Some("bad_token".to_string()),
            Some("try: good_token".to_string()),
        );

        let formatted = syntax_error.to_c_string();
        let formatted_str = formatted.to_string_lossy();
        assert!(formatted_str.contains("Try: try: good_token"));

        let unsupported_error = TranspileError::unsupported_operation_with_alternative(
            "complex_join",
            "simple_context",
            Some("use inner_join instead".to_string()),
        );

        let formatted = unsupported_error.to_c_string();
        let formatted_str = formatted.to_string_lossy();
        assert!(formatted_str.contains("Alternative: use inner_join instead"));
    }

    #[test]
    fn test_ffi_error_functions() {
        // Test error code names
        let success_name = unsafe {
            std::ffi::CStr::from_ptr(dplyr_error_code_name(DPLYR_SUCCESS)).to_string_lossy()
        };
        assert_eq!(success_name, "SUCCESS");

        let syntax_name = unsafe {
            std::ffi::CStr::from_ptr(dplyr_error_code_name(DPLYR_ERROR_SYNTAX)).to_string_lossy()
        };
        assert_eq!(syntax_name, "E-SYNTAX");

        // Test success check
        assert!(dplyr_is_success(DPLYR_SUCCESS));
        assert!(!dplyr_is_success(DPLYR_ERROR_SYNTAX));

        // Test recoverability
        assert!(dplyr_is_recoverable_error(DPLYR_ERROR_SYNTAX));
        assert!(dplyr_is_recoverable_error(DPLYR_ERROR_UNSUPPORTED));
        assert!(!dplyr_is_recoverable_error(DPLYR_ERROR_PANIC));
        assert!(!dplyr_is_recoverable_error(DPLYR_ERROR_INTERNAL));
    }

    #[test]
    fn test_specialized_error_constructors() {
        let null_error = TranspileError::null_pointer_error("input_code");
        assert_eq!(null_error.to_c_error_code(), DPLYR_ERROR_PANIC);

        let utf8_error = TranspileError::invalid_utf8_error("invalid sequence");
        assert_eq!(utf8_error.to_c_error_code(), DPLYR_ERROR_PANIC);

        let size_error = TranspileError::input_too_large_error(2000, 1000);
        assert_eq!(size_error.to_c_error_code(), DPLYR_ERROR_INTERNAL);

        let formatted = size_error.to_c_string();
        let formatted_str = formatted.to_string_lossy();
        assert!(formatted_str.contains("2000"));
        assert!(formatted_str.contains("1000"));
        assert!(formatted_str.contains("Reduce input size"));
    }

    #[test]
    fn test_error_message_with_context() {
        let error = TranspileError::syntax_error("test error", 5, None);
        let context = "select(col1, col2) %>% filter(invalid_syntax_here)";

        let message = create_error_message_with_context(&error, Some(context));
        let message_str = message.to_string_lossy();

        assert!(message_str.contains("test error"));
        assert!(message_str.contains("Input context:"));
        assert!(message_str.contains("select(col1, col2)"));
    }

    #[test]
    fn test_error_message_with_long_context() {
        let error = TranspileError::syntax_error("test error", 5, None);
        let long_context = "a".repeat(200); // Very long context

        let message = create_error_message_with_context(&error, Some(&long_context));
        let message_str = message.to_string_lossy();

        assert!(message_str.contains("test error"));
        assert!(message_str.contains("Input context:"));
        assert!(message_str.contains("...")); // Should be truncated
        assert!(message_str.len() < long_context.len() + 100); // Should be shorter than full context
    }
}
