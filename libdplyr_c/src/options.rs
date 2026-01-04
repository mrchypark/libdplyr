//! Options and limits for the libdplyr C API.
//!
//! Kept separate so FFI entrypoints can stay focused on boundary safety while
//! options/limits evolve independently.

use std::panic;

use crate::error::TranspileError;

// R3-AC1: C-compatible options structure
#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct DplyrOptions {
    pub strict_mode: bool,
    pub preserve_comments: bool,
    pub debug_mode: bool,            // R10-AC1: Debug mode support
    pub max_input_length: u32,       // R9-AC2: DoS prevention
    pub max_processing_time_ms: u64, // R9-AC2: Processing time limit (0 = use default)
}

impl Default for DplyrOptions {
    fn default() -> Self {
        Self {
            strict_mode: false,
            preserve_comments: false,
            debug_mode: false,
            max_input_length: 1024 * 1024, // 1MB default limit
            max_processing_time_ms: MAX_PROCESSING_TIME_MS, // R9-AC2: Default timeout
        }
    }
}

impl DplyrOptions {
    /// Create new DplyrOptions with default values
    ///
    /// # Safety
    /// This function is safe to call from C code
    pub fn new() -> Self {
        Self::default()
    }

    /// Create DplyrOptions with custom settings
    ///
    /// # Arguments
    /// * `strict_mode` - Enable strict parsing mode
    /// * `preserve_comments` - Keep comments in output
    /// * `debug_mode` - Enable debug information (R10-AC1)
    /// * `max_input_length` - Maximum input size in bytes (R9-AC2)
    ///
    /// # Returns
    /// Validated DplyrOptions instance
    pub fn with_settings(
        strict_mode: bool,
        preserve_comments: bool,
        debug_mode: bool,
        max_input_length: u32,
    ) -> Self {
        Self {
            strict_mode,
            preserve_comments,
            debug_mode,
            max_input_length: max_input_length.min(MAX_INPUT_LENGTH as u32),
            max_processing_time_ms: MAX_PROCESSING_TIME_MS,
        }
    }

    /// Create DplyrOptions with custom settings including timeout
    ///
    /// # Arguments
    /// * `strict_mode` - Enable strict parsing mode
    /// * `preserve_comments` - Keep comments in output
    /// * `debug_mode` - Enable debug information (R10-AC1)
    /// * `max_input_length` - Maximum input size in bytes (R9-AC2)
    /// * `max_processing_time_ms` - Maximum processing time in milliseconds (0 = use default) (R9-AC2)
    ///
    /// # Returns
    /// Validated DplyrOptions instance
    pub fn with_all_settings(
        strict_mode: bool,
        preserve_comments: bool,
        debug_mode: bool,
        max_input_length: u32,
        max_processing_time_ms: u64,
    ) -> Self {
        let timeout = if max_processing_time_ms == 0 {
            MAX_PROCESSING_TIME_MS
        } else {
            max_processing_time_ms.min(MAX_PROCESSING_TIME_MS)
        };
        Self {
            strict_mode,
            preserve_comments,
            debug_mode,
            max_input_length: max_input_length.min(MAX_INPUT_LENGTH as u32),
            max_processing_time_ms: timeout,
        }
    }

    /// Validate options for security and correctness
    ///
    /// # Returns
    /// Result indicating if options are valid
    pub fn validate(&self) -> Result<(), TranspileError> {
        // R9-AC2: Validate input length limit
        if self.max_input_length == 0 {
            return Err(TranspileError::internal_error(
                "max_input_length cannot be zero",
            ));
        }

        if self.max_input_length > MAX_INPUT_LENGTH as u32 {
            return Err(TranspileError::internal_error(&format!(
                "max_input_length {} exceeds maximum {}",
                self.max_input_length, MAX_INPUT_LENGTH
            )));
        }

        // R9-AC2: Validate processing time limit
        if self.max_processing_time_ms > MAX_PROCESSING_TIME_MS {
            return Err(TranspileError::internal_error(&format!(
                "max_processing_time_ms {} exceeds maximum {}",
                self.max_processing_time_ms, MAX_PROCESSING_TIME_MS
            )));
        }

        Ok(())
    }
}

// R9-AC2: DoS prevention constants
pub const MAX_INPUT_LENGTH: usize = 1024 * 1024; // 1MB
pub const MAX_PROCESSING_TIME_MS: u64 = 30000; // 30 seconds
pub const MAX_OUTPUT_LENGTH: usize = 10 * 1024 * 1024; // 10MB max SQL output
pub const MAX_NESTING_DEPTH: usize = 50; // Maximum nesting depth
pub const MAX_FUNCTION_CALLS: usize = 1000; // Maximum function calls per input

/// Create default DplyrOptions
///
/// # Returns
/// DplyrOptions with default settings
#[no_mangle]
pub extern "C" fn dplyr_options_default() -> DplyrOptions {
    DplyrOptions::default()
}

/// Create DplyrOptions with custom settings
///
/// # Arguments
/// * `strict_mode` - Enable strict parsing mode
/// * `preserve_comments` - Keep comments in output
/// * `debug_mode` - Enable debug information
/// * `max_input_length` - Maximum input size in bytes
///
/// # Returns
/// DplyrOptions with specified settings
#[no_mangle]
pub extern "C" fn dplyr_options_create(
    strict_mode: bool,
    preserve_comments: bool,
    debug_mode: bool,
    max_input_length: u32,
) -> DplyrOptions {
    DplyrOptions::with_settings(strict_mode, preserve_comments, debug_mode, max_input_length)
}

/// Create DplyrOptions with all settings including timeout
///
/// # Arguments
/// * `strict_mode` - Enable strict parsing mode
/// * `preserve_comments` - Keep comments in output
/// * `debug_mode` - Enable debug information
/// * `max_input_length` - Maximum input size in bytes
/// * `max_processing_time_ms` - Maximum processing time in milliseconds (0 = use default)
///
/// # Returns
/// DplyrOptions with specified settings
#[no_mangle]
pub extern "C" fn dplyr_options_create_with_timeout(
    strict_mode: bool,
    preserve_comments: bool,
    debug_mode: bool,
    max_input_length: u32,
    max_processing_time_ms: u64,
) -> DplyrOptions {
    DplyrOptions::with_all_settings(
        strict_mode,
        preserve_comments,
        debug_mode,
        max_input_length,
        max_processing_time_ms,
    )
}

/// Validate DplyrOptions
///
/// # Safety
/// Caller must ensure that:
/// - `options` is a valid `*const DplyrOptions` or `std::ptr::null()`.
/// - Passing an invalid pointer to a `DplyrOptions` struct will result in undefined behavior.
///
/// # Returns
/// 0 if valid, negative error code if invalid
#[no_mangle]
pub unsafe extern "C" fn dplyr_options_validate(options: *const DplyrOptions) -> i32 {
    if options.is_null() {
        return -1; // Null pointer error
    }

    let result = panic::catch_unwind(|| {
        let opts = unsafe { &*options };
        match opts.validate() {
            Ok(()) => 0,
            Err(_) => -2, // Validation error
        }
    });

    result.unwrap_or(-4)
}
