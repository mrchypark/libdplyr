//! C-compatible API for libdplyr DuckDB extension
//!
//! This crate provides a C-compatible interface for the libdplyr transpiler
//! to be used in DuckDB extensions. It handles FFI safety, memory management,
//! and error handling across the C/Rust boundary.
//!
//! # Requirements Fulfilled
//! - R3-AC1: C ABI compatibility with static/dynamic library support
//! - R3-AC2: Structured result format (success/error/message)
//! - R3-AC3: Dedicated memory management with clear ownership rules
//! - R9-AC1: Panic safety across FFI boundaries
//! - R9-AC2: Input validation and DoS prevention

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic;
use std::time::{Duration, Instant};

// Import libdplyr components
use libdplyr::sql_generator::DuckDbDialect;
use libdplyr::Transpiler;

pub mod cache;
pub mod error;
pub mod performance_tests;

// Re-export cache FFI functions for C header generation
pub use cache::{
    dplyr_cache_clear, dplyr_cache_get_capacity, dplyr_cache_get_evictions,
    dplyr_cache_get_hit_rate, dplyr_cache_get_hits, dplyr_cache_get_misses, dplyr_cache_get_size,
    dplyr_cache_get_stats, dplyr_cache_is_effective, dplyr_cache_log_performance_warning,
    dplyr_cache_log_stats, dplyr_cache_log_stats_detailed, dplyr_cache_should_clear,
};

// Import cache components for internal use
use cache::SimpleTranspileCache;

// Import error components for internal use
use error::{create_error_message_with_context, TranspileError};
// Re-export error handling functions for C header generation
pub use error::{dplyr_error_code_name, dplyr_is_recoverable_error, dplyr_is_success};
use error::{
    DPLYR_ERROR_INPUT_TOO_LARGE, DPLYR_ERROR_INTERNAL, DPLYR_ERROR_INVALID_UTF8,
    DPLYR_ERROR_NULL_POINTER, DPLYR_ERROR_PANIC, DPLYR_SUCCESS,
};
pub use error::{DPLYR_ERROR_SYNTAX, DPLYR_ERROR_UNSUPPORTED};

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

// R3-AC2, R9-AC1: FFI function declarations with panic safety
// These functions will be fully implemented in subsequent tasks

/// Compile dplyr code to SQL
///
/// # Safety
/// Caller must ensure that:
/// - `code` is a valid null-terminated C string.
/// - `options` is a valid pointer to a `DplyrOptions` struct, or `std::ptr::null()` if default options are desired.
/// - `out_sql` and `out_error` are valid mutable pointers to `*mut c_char` where results can be stored.
/// - Any `*mut c_char` returned must be freed using `dplyr_free_string`.
///
/// # Returns
/// - 0 on success
/// - Negative error codes on failure
#[no_mangle]
pub unsafe extern "C" fn dplyr_compile(
    code: *const c_char,
    options: *const DplyrOptions,
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    // R9-AC1: Panic safety - catch all panics at FFI boundary
    let result = panic::catch_unwind(|| {
        // R9-AC2: Input validation - check for null pointers
        if code.is_null() {
            set_error_output(out_error, "E-NULL-POINTER: code parameter is null");
            return DPLYR_ERROR_NULL_POINTER;
        }

        if out_sql.is_null() || out_error.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        // Convert C string to Rust string with UTF-8 validation
        let code_str = match unsafe { CStr::from_ptr(code) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                set_error_output(
                    out_error,
                    "E-INVALID-UTF8: Input code contains invalid UTF-8",
                );
                return DPLYR_ERROR_INVALID_UTF8;
            }
        };

        // Get options (use default if null)
        let opts = if options.is_null() {
            DplyrOptions::default()
        } else {
            unsafe { (*options).clone() }
        };

        // R9-AC2: Input size validation
        if code_str.len() > opts.max_input_length as usize {
            let error_msg = format!(
                "E-INPUT-TOO-LARGE: Input size {} exceeds maximum {}",
                code_str.len(),
                opts.max_input_length
            );
            set_error_output(out_error, &error_msg);
            return DPLYR_ERROR_INPUT_TOO_LARGE;
        }

        // R9-AC2: Additional input validation for security
        if let Err(encoding_error) = validate_input_encoding(code_str) {
            let error_msg = encoding_error.to_c_string();
            set_error_output(out_error, &error_msg.to_string_lossy());
            return encoding_error.to_c_error_code();
        }

        if let Err(structure_error) = validate_input_structure(code_str) {
            let error_msg = structure_error.to_c_string();
            set_error_output(out_error, &error_msg.to_string_lossy());
            return structure_error.to_c_error_code();
        }

        // Validate options
        if let Err(validation_error) = opts.validate() {
            let error_msg = validation_error.to_c_string();
            set_error_output(out_error, &error_msg.to_string_lossy());
            return validation_error.to_c_error_code();
        }

        // R9-AC2: DoS prevention - processing time limit
        let processing_start = Instant::now();
        let timeout_ms = if opts.max_processing_time_ms == 0 {
            MAX_PROCESSING_TIME_MS
        } else {
            opts.max_processing_time_ms
        };
        let max_processing_time = Duration::from_millis(timeout_ms);

        // R6-AC1: Use caching system for performance
        let transpile_result =
            SimpleTranspileCache::get_or_transpile(code_str, &opts, |dplyr_code, _options| {
                // R9-AC2: Check processing time limit before expensive operations
                if processing_start.elapsed() > max_processing_time {
                    return Err(TranspileError::internal_error_with_hint(
                        &format!(
                            "Processing timeout: exceeded {}ms limit",
                            max_processing_time.as_millis()
                        ),
                        Some("Reduce input complexity or increase timeout limit".to_string()),
                    ));
                }

                // R9-AC2: Additional input validation for malicious patterns
                validate_input_security(dplyr_code)?;

                // Create transpiler with DuckDB dialect
                let dialect = Box::new(DuckDbDialect::new());
                let transpiler = Transpiler::new(dialect);

                // Perform actual transpilation with timeout monitoring
                let transpile_start = Instant::now();
                let transpile_result = transpiler.transpile(dplyr_code);

                // R9-AC2: Check if transpilation took too long
                if transpile_start.elapsed() > max_processing_time {
                    return Err(TranspileError::internal_error_with_hint(
                        &format!(
                            "Transpilation timeout: exceeded {}ms limit",
                            max_processing_time.as_millis()
                        ),
                        Some("Input may be too complex for processing".to_string()),
                    ));
                }

                match transpile_result {
                    Ok(sql) => {
                        // R9-AC2: Validate output size to prevent memory exhaustion
                        if sql.len() > MAX_OUTPUT_LENGTH {
                            return Err(TranspileError::internal_error_with_hint(
                                &format!(
                                    "Output too large: {} bytes exceeds maximum {}",
                                    sql.len(),
                                    MAX_OUTPUT_LENGTH
                                ),
                                Some("Input generates excessive SQL output".to_string()),
                            ));
                        }
                        Ok(sql)
                    }
                    Err(libdplyr_error) => {
                        // Convert libdplyr error to our error type
                        Err(convert_libdplyr_error(libdplyr_error))
                    }
                }
            });

        match transpile_result {
            Ok(sql) => {
                // R10-AC1: Debug mode logging
                if opts.debug_mode {
                    eprintln!(
                        "DEBUG: Successfully transpiled {} chars to {} chars",
                        code_str.len(),
                        sql.len()
                    );

                    // R10-AC2: Cache statistics logging in debug mode
                    cache::dplyr_cache_log_stats_detailed(
                        c"DEBUG_TRANSPILE".as_ptr(),
                        true,
                    );

                    // R10-AC2: Log performance warning if cache is underperforming
                    cache::dplyr_cache_log_performance_warning();
                }

                set_sql_output(out_sql, &sql);
                DPLYR_SUCCESS
            }
            Err(error) => {
                let error_msg = if opts.debug_mode {
                    create_error_message_with_context(&error, Some(code_str))
                } else {
                    error.to_c_string()
                };

                set_error_output(out_error, &error_msg.to_string_lossy());
                error.to_c_error_code()
            }
        }
    });

    match result {
        Ok(code) => code,
        Err(_) => {
            // Panic occurred - set error message if possible
            unsafe {
                if !out_error.is_null() {
                    let panic_msg = CString::new("E-INTERNAL: Internal panic occurred").unwrap();
                    *out_error = panic_msg.into_raw();
                }
            }
            -4 // Panic error code
        }
    }
}

/// Free string allocated by dplyr_compile
///
/// # Safety
/// Caller must ensure that:
/// - `s` is a valid `*mut c_char` that was previously allocated by a `libdplyr_c` function (e.g., `dplyr_compile`).
/// - `s` must not be freed twice.
/// - `s` can be `std::ptr::null_mut()`, in which case it's a no-op.
///
/// # Returns
/// 0 on success, negative error code on failure
#[no_mangle]
pub unsafe extern "C" fn dplyr_free_string(s: *mut c_char) -> i32 {
    // R3-AC3: Safe memory management with null pointer check
    if s.is_null() {
        return DPLYR_SUCCESS; // Null pointer is OK, no-op
    }

    // R9-AC1: Panic safety for memory operations
    let result = panic::catch_unwind(|| {
        unsafe {
            // R3-AC3: Proper memory management with CString::from_raw
            // This will properly deallocate the memory allocated by CString::into_raw
            let _ = CString::from_raw(s);
        }
        DPLYR_SUCCESS
    });

    match result {
        Ok(code) => code,
        Err(_) => {
            // Panic occurred during deallocation - this is serious
            eprintln!("CRITICAL: Panic occurred during string deallocation");
            DPLYR_ERROR_PANIC
        }
    }
}

/// Free multiple strings at once
///
/// # Safety
/// Caller must ensure that:
/// - `strings` is a valid `*mut *mut c_char` pointing to an array of string pointers, or `std::ptr::null_mut()`.
/// - `count` accurately reflects the number of valid `*mut c_char` pointers in the array.
/// - Each `*mut c_char` in the array must have been previously allocated by a `libdplyr_c` function.
/// - No `*mut c_char` in the array is freed twice.
///
/// # Arguments
/// * `strings` - Array of string pointers to free
/// * `count` - Number of strings in the array
///
/// # Returns
/// Number of strings successfully freed, or negative error code
#[no_mangle]
pub unsafe extern "C" fn dplyr_free_strings(strings: *mut *mut c_char, count: usize) -> i32 {
    if strings.is_null() {
        return DPLYR_ERROR_NULL_POINTER;
    }

    let result = panic::catch_unwind(|| {
        let mut freed_count = 0;

        unsafe {
            for i in 0..count {
                let string_ptr = *strings.add(i);
                if !string_ptr.is_null() {
                    match dplyr_free_string(string_ptr) {
                        DPLYR_SUCCESS => freed_count += 1,
                        _ => {
                            // Continue freeing other strings even if one fails
                            eprintln!("Warning: Failed to free string at index {}", i);
                        }
                    }
                }
            }
        }

        freed_count
    });

    match result {
        Ok(count) => count,
        Err(_) => DPLYR_ERROR_PANIC,
    }
}

/// Check if a pointer looks like a valid C string
///
/// # Safety
/// Caller must ensure that:
/// - `s` is either `std::ptr::null()` or a valid, non-dangling `*const c_char` that is part of a null-terminated C string.
/// - Dereferencing an invalid pointer is undefined behavior.
///
/// # Arguments
/// * `s` - Pointer to check
///
/// # Returns
/// true if pointer appears valid, false otherwise
#[no_mangle]
pub unsafe extern "C" fn dplyr_is_valid_string_pointer(s: *const c_char) -> bool {
    if s.is_null() {
        return false;
    }

    let result = panic::catch_unwind(|| {
        unsafe {
            // Try to create a CStr from the pointer
            // This will fail if the pointer is invalid or doesn't contain a null terminator
            CStr::from_ptr(s).to_str().is_ok()
        }
    });

    result.unwrap_or(false)
}

/// Get libdplyr version string
///
/// # Returns
/// Static version string (no need to free)
#[no_mangle]
pub extern "C" fn libdplyr_c_version_simple() -> *const c_char {
    // R8-AC1: Version information - static string management
    c"0.1.0".as_ptr()
}

/// Get detailed version information including build info
///
/// # Returns
/// Static version string with build details (no need to free)
#[no_mangle]
pub extern "C" fn dplyr_version_detailed() -> *const c_char {
    // R8-AC1: Extended version information
    concat!(
        "libdplyr_c v0.1.0 (built with rustc ",
        env!("RUSTC_VERSION", "unknown"),
        ")\0"
    )
    .as_ptr() as *const c_char
}

/// Get supported SQL dialects as a comma-separated string
///
/// # Returns
/// Static string listing supported dialects (no need to free)
#[no_mangle]
pub extern "C" fn dplyr_supported_dialects() -> *const c_char {
    // R8-AC1: Capability information
    c"DuckDB".as_ptr()
}

/// Get build timestamp
///
/// # Returns
/// Static build timestamp string (no need to free)
#[no_mangle]
pub extern "C" fn dplyr_build_timestamp() -> *const c_char {
    // R8-AC1: Build information
    concat!(env!("BUILD_TIMESTAMP", "unknown"), "\0").as_ptr() as *const c_char
}

/// Check if debug mode is available in this build
///
/// # Returns
/// true if debug features are available, false otherwise
#[no_mangle]
pub extern "C" fn dplyr_has_debug_support() -> bool {
    // R10-AC1: Debug capability check
    cfg!(debug_assertions)
}

/// Get maximum supported input length
///
/// # Returns
/// Maximum input length in bytes
#[no_mangle]
pub extern "C" fn dplyr_max_input_length() -> u32 {
    // R9-AC2: DoS prevention information
    MAX_INPUT_LENGTH as u32
}

/// Get maximum processing time limit
///
/// # Returns
/// Maximum processing time in milliseconds
#[no_mangle]
pub extern "C" fn dplyr_max_processing_time_ms() -> u64 {
    // R9-AC2: DoS prevention information
    MAX_PROCESSING_TIME_MS
}

/// Validate system requirements and configuration
///
/// # Returns
/// 0 if system is ready, negative error code if issues found
#[no_mangle]
pub extern "C" fn dplyr_check_system() -> i32 {
    let result = panic::catch_unwind(|| {
        // Check basic system requirements

        // 1. Check if we can allocate memory
        let test_allocation = CString::new("test");
        if test_allocation.is_err() {
            return DPLYR_ERROR_INTERNAL;
        }

        // 2. Check if libdplyr components are available
        // This is a compile-time check, so if we got here, they should be available

        // 3. Check cache system
        SimpleTranspileCache::clear_cache();
        let stats = SimpleTranspileCache::get_cache_stats();
        if stats.is_empty() {
            return DPLYR_ERROR_INTERNAL;
        }

        DPLYR_SUCCESS
    });

    match result {
        Ok(code) => code,
        Err(_) => DPLYR_ERROR_PANIC,
    }
}

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

// Helper functions for FFI implementation (will be used in subsequent tasks)

/// Set SQL output pointer safely
pub(crate) fn set_sql_output(out_sql: *mut *mut c_char, sql: &str) {
    if !out_sql.is_null() {
        if let Ok(c_string) = CString::new(sql) {
            unsafe {
                *out_sql = c_string.into_raw();
            }
        }
    }
}

/// Set error output pointer safely
pub(crate) fn set_error_output(out_error: *mut *mut c_char, error: &str) {
    if !out_error.is_null() {
        if let Ok(c_string) = CString::new(error) {
            unsafe {
                *out_error = c_string.into_raw();
            }
        }
    }
}

/// Convert libdplyr error to our TranspileError type
fn convert_libdplyr_error(libdplyr_error: libdplyr::TranspileError) -> TranspileError {
    match libdplyr_error {
        libdplyr::TranspileError::LexError(lex_error) => {
            TranspileError::syntax_error_with_suggestion(
                &format!("Lexical error: {}", lex_error),
                0, // Position not available from libdplyr
                None,
                Some("Check for invalid characters or syntax".to_string()),
            )
        }
        libdplyr::TranspileError::ParseError(parse_error) => {
            TranspileError::syntax_error_with_suggestion(
                &format!("Parse error: {}", parse_error),
                0, // Position not available from libdplyr
                None,
                Some("Check dplyr function syntax".to_string()),
            )
        }
        libdplyr::TranspileError::GenerationError(gen_error) => {
            TranspileError::unsupported_operation_with_alternative(
                &format!("Generation error: {}", gen_error),
                "DuckDB",
                Some("Try simpler dplyr operations".to_string()),
            )
        }
        libdplyr::TranspileError::IoError(io_error) => TranspileError::internal_error_with_hint(
            &format!("IO error: {}", io_error),
            Some("Check file permissions and disk space".to_string()),
        ),
        libdplyr::TranspileError::ValidationError(validation_error) => {
            TranspileError::syntax_error_with_suggestion(
                &format!("Validation error: {}", validation_error),
                0,
                None,
                Some("Check input format and syntax".to_string()),
            )
        }
        libdplyr::TranspileError::ConfigurationError(config_error) => {
            TranspileError::internal_error_with_hint(
                &format!("Configuration error: {}", config_error),
                Some("Check system configuration".to_string()),
            )
        }
        libdplyr::TranspileError::SystemError(system_error) => {
            TranspileError::internal_error_with_hint(
                &format!("System error: {}", system_error),
                Some("Check system resources and permissions".to_string()),
            )
        }
    }
}

// R9-AC2: Security validation functions for malicious input detection
fn validate_input_security(input: &str) -> Result<(), TranspileError> {
    // Check for excessive nesting depth
    let nesting_depth = calculate_nesting_depth(input);
    if nesting_depth > MAX_NESTING_DEPTH {
        return Err(TranspileError::internal_error_with_hint(
            &format!(
                "Excessive nesting depth: {} exceeds maximum {}",
                nesting_depth, MAX_NESTING_DEPTH
            ),
            Some("Reduce nested function calls or parentheses".to_string()),
        ));
    }

    // Check for excessive function calls
    let function_count = count_function_calls(input);
    if function_count > MAX_FUNCTION_CALLS {
        return Err(TranspileError::internal_error_with_hint(
            &format!(
                "Too many function calls: {} exceeds maximum {}",
                function_count, MAX_FUNCTION_CALLS
            ),
            Some("Simplify the dplyr pipeline".to_string()),
        ));
    }

    // Check for suspicious patterns that might indicate malicious input
    if contains_suspicious_patterns(input) {
        return Err(TranspileError::internal_error_with_hint(
            "Input contains potentially malicious patterns",
            Some("Remove suspicious characters or patterns".to_string()),
        ));
    }

    // Check for excessive repetition (potential DoS pattern)
    if has_excessive_repetition(input) {
        return Err(TranspileError::internal_error_with_hint(
            "Input contains excessive repetition patterns",
            Some("Reduce repetitive patterns in input".to_string()),
        ));
    }

    Ok(())
}

fn calculate_nesting_depth(input: &str) -> usize {
    let mut max_depth = 0;
    let mut current_depth: i32 = 0;

    for ch in input.chars() {
        match ch {
            '(' | '[' | '{' => {
                current_depth += 1;
                max_depth = max_depth.max(current_depth);
            }
            ')' | ']' | '}' => {
                if current_depth > 0 {
                    current_depth -= 1;
                }
            }
            _ => {}
        }
    }

    max_depth.try_into().unwrap()
}

fn count_function_calls(input: &str) -> usize {
    // Count patterns that look like function calls: identifier followed by '('
    let mut count = 0;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_alphabetic() || chars[i] == '_' {
            // Found start of identifier
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }

            // Skip whitespace
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }

            // Check if followed by '('
            if i < chars.len() && chars[i] == '(' {
                count += 1;
            }
        } else {
            i += 1;
        }
    }

    count
}

fn contains_suspicious_patterns(input: &str) -> bool {
    // Check for patterns that might indicate injection attempts or malicious input
    let suspicious_patterns = [
        // SQL injection patterns
        "'; DROP",
        "'; DELETE",
        "'; INSERT",
        "'; UPDATE",
        "UNION SELECT",
        "OR 1=1",
        "AND 1=1",
        // Script injection patterns
        "<script",
        "javascript:",
        "eval(",
        "exec(",
        // Path traversal patterns
        "../",
        "..\\",
        // Null bytes and control characters
        "\0",
        "\x01",
        "\x02",
        "\x03",
        "\x04",
        "\x05",
        "\x06",
        "\x07",
        "\x08",
        "\x0B",
        "\x0C",
        "\x0E",
        "\x0F",
        // Excessive special characters
    ];

    let input_upper = input.to_uppercase();
    for pattern in &suspicious_patterns {
        if input_upper.contains(&pattern.to_uppercase()) {
            return true;
        }
    }

    // Check for excessive special characters (potential obfuscation)
    let special_char_count = input
        .chars()
        .filter(|&c| {
            !c.is_alphanumeric() && !c.is_whitespace() && !"()[]{},.;:_-+*/%><=!&|".contains(c)
        })
        .count();

    if special_char_count > input.len() / 10 {
        return true; // More than 10% special characters
    }

    false
}

fn has_excessive_repetition(input: &str) -> bool {
    // Check for patterns that repeat excessively (potential DoS)
    let chars: Vec<char> = input.chars().collect();

    // Check for repeated characters
    let mut consecutive_count = 1;
    for i in 1..chars.len() {
        if chars[i] == chars[i - 1] {
            consecutive_count += 1;
            if consecutive_count > 100 {
                return true; // More than 100 consecutive identical characters
            }
        } else {
            consecutive_count = 1;
        }
    }

    // Check for repeated substrings
    for pattern_len in 2..=10 {
        if pattern_len * 20 > input.len() {
            break;
        }

        let mut pattern_counts = std::collections::HashMap::new();
        for i in 0..=(chars.len() - pattern_len) {
            let pattern: String = chars[i..i + pattern_len].iter().collect();
            *pattern_counts.entry(pattern).or_insert(0) += 1;
        }

        // If any pattern repeats more than 20 times, consider it excessive
        if pattern_counts.values().any(|&count| count > 20) {
            return true;
        }
    }

    false
}

// R9-AC2: Additional input validation functions
fn validate_input_encoding(input: &str) -> Result<(), TranspileError> {
    // Check for valid UTF-8 (already done by CStr::to_str, but double-check)
    // Check all characters for control characters and confusing Unicode
    for ch in input.chars() {
        // Check for control characters (except common whitespace)
        if ch.is_control() && !matches!(ch, '\t' | '\n' | '\r') {
            return Err(TranspileError::invalid_utf8_error(&format!(
                "Contains control character: U+{:04X}",
                ch as u32
            )));
        }

        // Check for potentially confusing Unicode characters
        if is_confusing_unicode(ch) {
            return Err(TranspileError::invalid_utf8_error(&format!(
                "Contains potentially confusing Unicode character: U+{:04X}",
                ch as u32
            )));
        }
    }

    Ok(())
}

fn is_confusing_unicode(ch: char) -> bool {
    // Check for characters that might be used for visual spoofing
    match ch {
        // Zero-width characters
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => true,
        // Right-to-left override characters
        '\u{202D}' | '\u{202E}' => true,
        // Other potentially confusing characters
        '\u{00A0}' => true, // Non-breaking space
        _ => false,
    }
}

fn validate_input_structure(input: &str) -> Result<(), TranspileError> {
    // Check for balanced parentheses, brackets, and braces
    let mut paren_count = 0;
    let mut bracket_count = 0;
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut string_char = '\0';

    for ch in input.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }

        if ch == '\\' {
            escape_next = true;
            continue;
        }

        if in_string {
            if ch == string_char {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                in_string = true;
                string_char = ch;
            }
            '(' => paren_count += 1,
            ')' => {
                paren_count -= 1;
                if paren_count < 0 {
                    return Err(TranspileError::syntax_error_with_suggestion(
                        "Unmatched closing parenthesis",
                        0, // Position tracking would require more complex parsing
                        Some(")".to_string()),
                        Some("Check parentheses balance".to_string()),
                    ));
                }
            }
            '[' => bracket_count += 1,
            ']' => {
                bracket_count -= 1;
                if bracket_count < 0 {
                    return Err(TranspileError::syntax_error_with_suggestion(
                        "Unmatched closing bracket",
                        0,
                        Some("]".to_string()),
                        Some("Check brackets balance".to_string()),
                    ));
                }
            }
            '{' => brace_count += 1,
            '}' => {
                brace_count -= 1;
                if brace_count < 0 {
                    return Err(TranspileError::syntax_error_with_suggestion(
                        "Unmatched closing brace",
                        0,
                        Some("}".to_string()),
                        Some("Check braces balance".to_string()),
                    ));
                }
            }
            _ => {}
        }
    }

    // Check for unclosed delimiters
    if paren_count > 0 {
        return Err(TranspileError::syntax_error_with_suggestion(
            &format!("{} unclosed parentheses", paren_count),
            0,
            Some("(".to_string()),
            Some("Add missing closing parentheses".to_string()),
        ));
    }

    if bracket_count > 0 {
        return Err(TranspileError::syntax_error_with_suggestion(
            &format!("{} unclosed brackets", bracket_count),
            0,
            Some("[".to_string()),
            Some("Add missing closing brackets".to_string()),
        ));
    }

    if brace_count > 0 {
        return Err(TranspileError::syntax_error_with_suggestion(
            &format!("{} unclosed braces", brace_count),
            0,
            Some("{".to_string()),
            Some("Add missing closing braces".to_string()),
        ));
    }

    if in_string {
        return Err(TranspileError::syntax_error_with_suggestion(
            "Unclosed string literal",
            0,
            Some(string_char.to_string()),
            Some("Add missing closing quote".to_string()),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dplyr_options_default() {
        let options = DplyrOptions::default();
        assert!(!options.strict_mode);
        assert!(!options.preserve_comments);
        assert!(!options.debug_mode);
        assert_eq!(options.max_input_length, 1024 * 1024);
        assert_eq!(options.max_processing_time_ms, MAX_PROCESSING_TIME_MS);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_INPUT_LENGTH, 1024 * 1024);
        assert_eq!(MAX_PROCESSING_TIME_MS, 30000);
    }

    #[test]
    fn test_dplyr_options_creation() {
        let options = DplyrOptions::new();
        assert_eq!(options, DplyrOptions::default());

        let custom_options = DplyrOptions::with_settings(true, true, true, 512);
        assert!(custom_options.strict_mode);
        assert!(custom_options.preserve_comments);
        assert!(custom_options.debug_mode);
        assert_eq!(custom_options.max_input_length, 512);
        assert_eq!(
            custom_options.max_processing_time_ms,
            MAX_PROCESSING_TIME_MS
        );

        let full_options = DplyrOptions::with_all_settings(true, false, true, 1024, 5000);
        assert!(full_options.strict_mode);
        assert!(!full_options.preserve_comments);
        assert!(full_options.debug_mode);
        assert_eq!(full_options.max_input_length, 1024);
        assert_eq!(full_options.max_processing_time_ms, 5000);
    }

    #[test]
    fn test_dplyr_options_validation() {
        let valid_options = DplyrOptions::default();
        assert!(valid_options.validate().is_ok());

        let invalid_options = DplyrOptions::with_settings(false, false, false, 0);
        assert!(invalid_options.validate().is_err());

        // Test with manually created oversized options (bypassing with_settings clamping)
        let oversized_options = DplyrOptions {
            strict_mode: false,
            preserve_comments: false,
            debug_mode: false,
            max_input_length: (MAX_INPUT_LENGTH + 1) as u32,
            max_processing_time_ms: MAX_PROCESSING_TIME_MS,
        };
        assert!(oversized_options.validate().is_err());

        // Test timeout validation - zero timeout is now allowed (means use default)
        let zero_timeout_options = DplyrOptions {
            strict_mode: false,
            preserve_comments: false,
            debug_mode: false,
            max_input_length: 1024,
            max_processing_time_ms: 0, // Zero means use default
        };
        assert!(zero_timeout_options.validate().is_ok());

        let oversized_timeout_options = DplyrOptions {
            strict_mode: false,
            preserve_comments: false,
            debug_mode: false,
            max_input_length: 1024,
            max_processing_time_ms: MAX_PROCESSING_TIME_MS + 1000, // Too large
        };
        assert!(oversized_timeout_options.validate().is_err());
    }

    #[test]
    fn test_dplyr_options_size_limit() {
        let options =
            DplyrOptions::with_settings(false, false, false, (MAX_INPUT_LENGTH + 1000) as u32);
        // Should be clamped to MAX_INPUT_LENGTH
        assert_eq!(options.max_input_length, MAX_INPUT_LENGTH as u32);
    }

    #[test]
    fn test_ffi_options_functions() {
        // Test default options creation
        let default_opts = dplyr_options_default();
        assert_eq!(default_opts, DplyrOptions::default());

        // Test custom options creation
        let custom_opts = dplyr_options_create(true, false, true, 2048);
        assert!(custom_opts.strict_mode);
        assert!(!custom_opts.preserve_comments);
        assert!(custom_opts.debug_mode);
        assert_eq!(custom_opts.max_input_length, 2048);

        // Test validation
        let valid_result = unsafe { dplyr_options_validate(&default_opts as *const DplyrOptions) };
        assert_eq!(valid_result, 0);

        // Test null pointer validation
        let null_result = unsafe { dplyr_options_validate(std::ptr::null()) };
        assert_eq!(null_result, -1);
    }

    #[test]
    fn test_dplyr_compile_null_pointers() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test null code pointer
        let result = unsafe { dplyr_compile(
            std::ptr::null(),
            std::ptr::null(),
            &mut out_sql,
            &mut out_error,
        ) };

        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);
        assert!(!out_error.is_null());

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_invalid_utf8() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Create invalid UTF-8 sequence
        let invalid_utf8 = b"select(col1)\xFF\xFE\0";

        let result = unsafe { dplyr_compile(
            invalid_utf8.as_ptr() as *const c_char,
            std::ptr::null(),
            &mut out_sql,
            &mut out_error,
        ) };

        assert_eq!(result, DPLYR_ERROR_INVALID_UTF8);
        assert!(!out_error.is_null());

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_input_too_large() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Create options with small limit
        let options = DplyrOptions::with_settings(false, false, false, 10);

        // Create input larger than limit
        let large_input = CString::new("select(very_long_column_name_that_exceeds_limit)").unwrap();

        let result = unsafe { dplyr_compile(
            large_input.as_ptr(),
            &options as *const DplyrOptions,
            &mut out_sql,
            &mut out_error,
        ) };

        assert_eq!(result, DPLYR_ERROR_INPUT_TOO_LARGE);
        assert!(!out_error.is_null());

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_basic_success() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Simple dplyr code that should work
        let input = CString::new("select(col1)").unwrap();

        let result = unsafe { dplyr_compile(
            input.as_ptr(),
            std::ptr::null(), // Use default options
            &mut out_sql,
            &mut out_error,
        ) };

        // Note: This might fail if libdplyr doesn't support the exact syntax
        // but the FFI layer should handle it gracefully
        if result == DPLYR_SUCCESS {
            assert!(!out_sql.is_null());
            assert!(out_error.is_null());

            // Clean up
            assert_eq!(unsafe { dplyr_free_string(out_sql) }, DPLYR_SUCCESS);
        } else {
            // If it fails, should have error message
            assert!(!out_error.is_null());

            // Clean up
            if !out_error.is_null() {
                assert_eq!(unsafe { dplyr_free_string(out_error) }, DPLYR_SUCCESS);
            }
        }
    }

    #[test]
    fn test_dplyr_free_string_safety() {
        // Test freeing null pointer (should be safe)
        let result = unsafe { dplyr_free_string(std::ptr::null_mut()) };
        assert_eq!(result, DPLYR_SUCCESS);

        // Test freeing valid string
        let test_string = CString::new("test string").unwrap();
        let raw_ptr = test_string.into_raw();

        // Verify pointer looks valid
        assert!(unsafe { dplyr_is_valid_string_pointer(raw_ptr) });

        // Free it
        let result = unsafe { dplyr_free_string(raw_ptr) };
        assert_eq!(result, DPLYR_SUCCESS);

        // Note: We can't test double-free safely as it would be undefined behavior
    }

    #[test]
    fn test_dplyr_free_strings_batch() {
        // Create multiple test strings
        let string1 = CString::new("string1").unwrap().into_raw();
        let string2 = CString::new("string2").unwrap().into_raw();
        let string3 = CString::new("string3").unwrap().into_raw();

        // Create array of pointers
        let mut strings = vec![string1, string2, string3, std::ptr::null_mut()];

        // Free all strings
        let freed_count = unsafe { dplyr_free_strings(strings.as_mut_ptr(), strings.len()) };
        assert_eq!(freed_count, 3); // Should free 3 strings (null pointer is skipped)

        // Test with null array
        let result = unsafe { dplyr_free_strings(std::ptr::null_mut(), 0) };
        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);
    }

    #[test]
    fn test_dplyr_is_valid_string_pointer() {
        // Test null pointer
        assert!(unsafe { !dplyr_is_valid_string_pointer(std::ptr::null()) });

        // Test valid string
        let test_string = CString::new("valid string").unwrap();
        assert!(unsafe { dplyr_is_valid_string_pointer(test_string.as_ptr()) });

        // Test static string
        let static_str = b"static string\0";
        assert!(unsafe { dplyr_is_valid_string_pointer(
            static_str.as_ptr() as *const c_char
        ) });
    }

    #[test]
    fn test_utility_functions() {
        // Test version functions
        let version = unsafe { CStr::from_ptr(libdplyr_c_version_simple()) };
        assert_eq!(version.to_string_lossy(), "0.1.0");

        let detailed_version = unsafe { CStr::from_ptr(dplyr_version_detailed()) };
        assert!(detailed_version.to_string_lossy().contains("0.1.0"));
        assert!(detailed_version.to_string_lossy().contains("libdplyr_c"));

        // Test supported dialects
        let dialects = unsafe { CStr::from_ptr(dplyr_supported_dialects()) };
        assert!(dialects.to_string_lossy().contains("DuckDB"));

        // Test build timestamp (should not be empty)
        let timestamp = unsafe { CStr::from_ptr(dplyr_build_timestamp()) };
        assert!(!timestamp.to_string_lossy().is_empty());

        // Test debug support check
        let _has_debug = dplyr_has_debug_support();
        // Should be true in debug builds, may be false in release builds


        // Test limits
        assert_eq!(dplyr_max_input_length(), MAX_INPUT_LENGTH as u32);
        assert_eq!(dplyr_max_processing_time_ms(), MAX_PROCESSING_TIME_MS);

        // Test system check
        let system_status = dplyr_check_system();
        assert_eq!(system_status, DPLYR_SUCCESS);
    }

    #[test]
    fn test_security_validation_functions() {
        // Test nesting depth calculation
        assert_eq!(calculate_nesting_depth("select(col1)"), 1);
        assert_eq!(calculate_nesting_depth("select(filter(col1, col2 > 0))"), 2);
        assert_eq!(calculate_nesting_depth("(((())))"), 4);

        // Test function call counting
        assert_eq!(count_function_calls("select(col1)"), 1);
        assert_eq!(count_function_calls("select(col1) %>% filter(col2 > 0)"), 2);
        assert_eq!(count_function_calls("func1() + func2() * func3()"), 3);

        // Test suspicious pattern detection
        assert!(!contains_suspicious_patterns(
            "select(col1) %>% filter(col2 > 0)"
        ));
        assert!(contains_suspicious_patterns("'; DROP TABLE users; --"));
        assert!(contains_suspicious_patterns(
            "UNION SELECT * FROM passwords"
        ));
        assert!(contains_suspicious_patterns(
            "<script>alert('xss')</script>"
        ));

        // Test repetition detection
        assert!(!has_excessive_repetition("select(col1, col2, col3)"));
        assert!(has_excessive_repetition(&"a".repeat(101))); // Too many consecutive chars
        assert!(has_excessive_repetition(&"ab".repeat(25))); // Too many repeated patterns
    }

    #[test]
    fn test_input_encoding_validation() {
        // Valid inputs
        assert!(validate_input_encoding("select(col1)").is_ok());
        assert!(validate_input_encoding("select(名前)").is_ok()); // Non-ASCII but valid

        // Invalid inputs with control characters
        assert!(validate_input_encoding("select\u{0001}(col1)").is_err());
        assert!(validate_input_encoding("select\u{0000}(col1)").is_err());

        // Confusing Unicode characters
        assert!(validate_input_encoding("select\u{200B}(col1)").is_err()); // Zero-width space
        assert!(validate_input_encoding("select\u{202E}(col1)").is_err()); // RTL override
    }

    #[test]
    fn test_input_structure_validation() {
        // Valid structures
        assert!(validate_input_structure("select(col1)").is_ok());
        assert!(validate_input_structure("select(col1, col2)").is_ok());
        assert!(validate_input_structure("filter(col1 > 'test')").is_ok());

        // Invalid structures - unmatched delimiters
        assert!(validate_input_structure("select(col1").is_err()); // Missing )
        assert!(validate_input_structure("select)col1(").is_err()); // Wrong order
        assert!(validate_input_structure("select[col1").is_err()); // Missing ]
        assert!(validate_input_structure("select{col1").is_err()); // Missing }
        assert!(validate_input_structure("select(col1 'unclosed").is_err()); // Unclosed string
    }

    #[test]
    fn test_validate_input_security() {
        // Valid inputs
        assert!(validate_input_security("select(col1) %>% filter(col2 > 0)").is_ok());

        // Excessive nesting
        let deep_nesting = "(((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((";
        assert!(validate_input_security(deep_nesting).is_err());

        // Too many function calls
        let many_functions = (0..1001)
            .map(|i| format!("func{}()", i))
            .collect::<Vec<_>>()
            .join(" + ");
        assert!(validate_input_security(&many_functions).is_err());

        // Suspicious patterns
        assert!(validate_input_security("'; DROP TABLE users; --").is_err());

        // Excessive repetition
        assert!(validate_input_security(&"select".repeat(50)).is_err());
    }

    #[test]
    fn test_ffi_options_with_timeout() {
        // Test timeout creation
        let timeout_opts = dplyr_options_create_with_timeout(true, false, true, 1024, 5000);
        assert!(timeout_opts.strict_mode);
        assert!(!timeout_opts.preserve_comments);
        assert!(timeout_opts.debug_mode);
        assert_eq!(timeout_opts.max_input_length, 1024);
        assert_eq!(timeout_opts.max_processing_time_ms, 5000);

        // Test default timeout (0 = use default)
        let default_timeout_opts = dplyr_options_create_with_timeout(false, false, false, 1024, 0);
        assert_eq!(
            default_timeout_opts.max_processing_time_ms,
            MAX_PROCESSING_TIME_MS
        );
    }

    #[test]
    fn test_dplyr_compile_with_security_validation() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test with suspicious input (properly quoted to pass structure validation)
        let malicious_input =
            CString::new("select(col1) %>% filter(col2 = '; DROP TABLE users; --')").unwrap();

        let result = unsafe { dplyr_compile(
            malicious_input.as_ptr(),
            std::ptr::null(),
            &mut out_sql,
            &mut out_error,
        ) };

        // Should fail with security error
        assert_ne!(result, DPLYR_SUCCESS);
        assert!(!out_error.is_null());

        // Check error message contains security-related information
        let error_msg = unsafe { std::ffi::CStr::from_ptr(out_error).to_string_lossy() };

        // The error should be related to security validation
        assert!(
            error_msg.contains("malicious")
                || error_msg.contains("suspicious")
                || error_msg.contains("DROP")
                || error_msg.contains("potentially")
        );

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_with_structure_validation() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test with unbalanced parentheses
        let unbalanced_input = CString::new("select(col1, col2").unwrap();

        let result = unsafe { dplyr_compile(
            unbalanced_input.as_ptr(),
            std::ptr::null(),
            &mut out_sql,
            &mut out_error,
        ) };

        // Should fail with syntax error
        assert_ne!(result, DPLYR_SUCCESS);
        assert!(!out_error.is_null());

        // Check error message mentions parentheses
        let error_msg = unsafe { std::ffi::CStr::from_ptr(out_error).to_string_lossy() };
        assert!(error_msg.contains("parenthes") || error_msg.contains("unclosed"));

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    // R9-AC3: Thread safety tests
    #[test]
    fn test_thread_safety_basic() {
        use std::thread;

        SimpleTranspileCache::clear_cache();

        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                thread::spawn(move || {
                    let options = DplyrOptions::default();
                    let code = format!("select(col{})", thread_id);

                    // Each thread should be able to call functions safely
                    let result =
                        SimpleTranspileCache::get_or_transpile(&code, &options, |_code, _opts| {
                            Ok(format!("SELECT col{} FROM table", thread_id))
                        });

                    assert!(result.is_ok());
                    result.unwrap()
                })
            })
            .collect();

        let results: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.len(), 4);

        // Each thread should have gotten its own result
        for (i, result) in results.iter().enumerate() {
            assert!(result.contains(&format!("col{}", i)));
        }
    }

    #[test]
    fn test_ffi_thread_safety() {
        use std::ffi::CString;
        use std::thread;

        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                thread::spawn(move || {
                    let mut out_sql: *mut c_char = std::ptr::null_mut();
                    let mut out_error: *mut c_char = std::ptr::null_mut();

                    let input = CString::new(format!("select(thread_col_{})", thread_id)).unwrap();

                    let result = unsafe { dplyr_compile(
                        input.as_ptr(),
                        std::ptr::null(), // Use default options
                        &mut out_sql,
                        &mut out_error,
                    ) };

                    // Clean up regardless of result
                    if !out_sql.is_null() {
                        unsafe { dplyr_free_string(out_sql) };
                    }
                    if !out_error.is_null() {
                        unsafe { dplyr_free_string(out_error) };
                    }

                    // Return the result code
                    result
                })
            })
            .collect();

        let results: Vec<i32> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All threads should complete without panicking
        assert_eq!(results.len(), 4);

        // Results may vary (success or error) but should not crash
        for result in results {
            // Should be a valid error code (not some random value from memory corruption)
            assert!((-10..=0).contains(&result));
        }
    }

    #[test]
    fn test_cache_thread_isolation() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        SimpleTranspileCache::clear_cache();

        let barrier = Arc::new(Barrier::new(3));
        let handles: Vec<_> = (0..3)
            .map(|thread_id| {
                let barrier = barrier.clone();
                thread::spawn(move || {
                    let options = DplyrOptions::default();

                    // Each thread adds its own entries
                    for i in 0..5 {
                        let code = format!("select(thread_{}_col_{})", thread_id, i);
                        let _ = SimpleTranspileCache::get_or_transpile(
                            &code,
                            &options,
                            |_code, _opts| {
                                Ok(format!("SELECT thread_{}_col_{} FROM table", thread_id, i))
                            },
                        );
                    }

                    // Wait for all threads to finish adding entries
                    barrier.wait();

                    // Each thread should see its own cache (thread_local)
                    // The cache size should be 5 for each thread
                    let cache_size = dplyr_cache_get_size();
                    assert_eq!(cache_size, 5);

                    thread_id
                })
            })
            .collect();

        let thread_ids: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(thread_ids.len(), 3);
    }

    #[test]
    fn test_memory_management_thread_safety() {
        use std::ffi::CString;
        use std::thread;

        // Test that memory management functions are thread-safe
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                thread::spawn(move || {
                    // Create and free strings in each thread
                    let test_strings: Vec<CString> = (0..10)
                        .map(|i| {
                            CString::new(format!("thread_{}_string_{}", thread_id, i)).unwrap()
                        })
                        .collect();

                    let raw_pointers: Vec<*mut c_char> =
                        test_strings.into_iter().map(|s| s.into_raw()).collect();

                    // Free all strings
                    for ptr in raw_pointers {
                        let result = unsafe { dplyr_free_string(ptr) };
                        assert_eq!(result, DPLYR_SUCCESS);
                    }

                    // Test batch free
                    let batch_strings: Vec<CString> = (0..5)
                        .map(|i| {
                            CString::new(format!("batch_thread_{}_string_{}", thread_id, i))
                                .unwrap()
                        })
                        .collect();

                    let mut batch_pointers: Vec<*mut c_char> =
                        batch_strings.into_iter().map(|s| s.into_raw()).collect();

                    let freed_count = unsafe {
                        dplyr_free_strings(batch_pointers.as_mut_ptr(), batch_pointers.len())
                    };
                    assert_eq!(freed_count, 5);

                    thread_id
                })
            })
            .collect();

        let thread_ids: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(thread_ids.len(), 4);
    }

    #[test]
    fn test_options_thread_safety() {
        use std::thread;

        // Test that options creation and validation are thread-safe
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                thread::spawn(move || {
                    // Create options with different settings in each thread
                    let options = dplyr_options_create_with_timeout(
                        thread_id % 2 == 0,               // strict_mode
                        thread_id % 2 == 1,               // preserve_comments
                        true,                             // debug_mode
                        1024 * (thread_id as u32 + 1),    // max_input_length
                        5000 + (thread_id as u64 * 1000), // max_processing_time_ms
                    );

                    // Validate options
                    let validation_result = unsafe { dplyr_options_validate(&options as *const DplyrOptions) };
                    assert_eq!(validation_result, 0);

                    // Test default options
                    let default_options = dplyr_options_default();
                    let default_validation =
                        unsafe { dplyr_options_validate(&default_options as *const DplyrOptions) };
                    assert_eq!(default_validation, 0);

                    thread_id
                })
            })
            .collect();

        let thread_ids: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(thread_ids.len(), 4);
    }

    #[test]
    fn test_utility_functions_thread_safety() {
        use std::thread;

        // Test that utility functions are thread-safe
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                thread::spawn(move || unsafe {
                    // These functions should be safe to call from multiple threads
                    let version_str =
                        { std::ffi::CStr::from_ptr(libdplyr_c_version_simple()) };
                    assert!(!version_str.to_string_lossy().is_empty());

                    let detailed_version =
                        { std::ffi::CStr::from_ptr(dplyr_version_detailed()) };
                    assert!(!detailed_version.to_string_lossy().is_empty());

                    let dialects = { std::ffi::CStr::from_ptr(dplyr_supported_dialects()) };
                    assert!(!dialects.to_string_lossy().is_empty());

                    let _has_debug = dplyr_has_debug_support();


                    let max_input = dplyr_max_input_length();
                    assert_eq!(max_input, MAX_INPUT_LENGTH as u32);

                    let max_time = dplyr_max_processing_time_ms();
                    assert_eq!(max_time, MAX_PROCESSING_TIME_MS);

                    let system_check = dplyr_check_system();
                    assert_eq!(system_check, DPLYR_SUCCESS);

                    thread_id
                })
            })
            .collect();

        let thread_ids: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(thread_ids.len(), 4);
    }

    #[test]
    fn test_error_functions_thread_safety() {
        use std::thread;

        // Test that error handling functions are thread-safe
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                thread::spawn(move || unsafe {
                    // Test error code functions
                    let error_codes = [
                        DPLYR_SUCCESS,
                        DPLYR_ERROR_SYNTAX,
                        DPLYR_ERROR_UNSUPPORTED,
                        DPLYR_ERROR_INTERNAL,
                        DPLYR_ERROR_PANIC,
                    ];

                    for &error_code in &error_codes {
                        let error_name = {
                            std::ffi::CStr::from_ptr(dplyr_error_code_name(error_code))
                                .to_string_lossy()
                        };
                        assert!(!error_name.is_empty());

                        let is_success = dplyr_is_success(error_code);
                        assert_eq!(is_success, error_code == DPLYR_SUCCESS);

                        let _is_recoverable = dplyr_is_recoverable_error(error_code);
                        // Just test it doesn't crash

                    }

                    thread_id
                })
            })
            .collect();

        let thread_ids: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(thread_ids.len(), 4);
    }

    // R9-AC1: Panic safety tests
    #[test]
    fn test_panic_safety_in_ffi_functions() {
        // Test that panics in FFI functions are caught and handled properly
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test with null pointers - should not panic
        let result = unsafe { dplyr_compile(
            std::ptr::null(),
            std::ptr::null(),
            &mut out_sql,
            &mut out_error,
        ) };
        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    // R9-AC2: Input validation tests
    #[test]
    fn test_input_validation_comprehensive() {
        // Test encoding validation
        assert!(validate_input_encoding("valid input").is_ok());
        assert!(validate_input_encoding("input with\nnewline").is_ok());
        assert!(validate_input_encoding("input with\ttab").is_ok());
        assert!(validate_input_encoding("input with\0null").is_err());
        assert!(validate_input_encoding("input with\x01control").is_err());

        // Test structure validation
        assert!(validate_input_structure("select(col1)").is_ok());
        assert!(validate_input_structure("select(col1, col2)").is_ok());
        assert!(validate_input_structure("select(col1").is_err()); // Unmatched paren
        assert!(validate_input_structure("select)col1(").is_err()); // Wrong order
        assert!(validate_input_structure("select[col1").is_err()); // Unmatched bracket
        assert!(validate_input_structure("select{col1").is_err()); // Unmatched brace

        // Test security validation
        assert!(validate_input_security("select(col1) %>% filter(age > 18)").is_ok());
        assert!(validate_input_security("'; DROP TABLE users;").is_err());
        assert!(validate_input_security("UNION SELECT * FROM passwords").is_err());
        assert!(validate_input_security("<script>alert('xss')</script>").is_err());
        assert!(validate_input_security("../../../etc/passwd").is_err());

        // Test excessive nesting
        let deep_nesting = "(".repeat(60) + &")".repeat(60);
        assert!(validate_input_security(&deep_nesting).is_err());

        // Test excessive function calls
        let many_functions = (0..1100)
            .map(|i| format!("func{}()", i))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(validate_input_security(&many_functions).is_err());

        // Test excessive repetition
        let repeated_chars = "a".repeat(150);
        assert!(validate_input_security(&repeated_chars).is_err());

        let repeated_pattern = "abc".repeat(25);
        assert!(validate_input_security(&repeated_pattern).is_err());
    }

    // R6-AC1: Caching integration tests
    #[test]
    fn test_caching_integration() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        SimpleTranspileCache::clear_cache();

        let options = DplyrOptions::default();
        let call_count = Arc::new(AtomicUsize::new(0));

        // First call should execute function
        let call_count_clone = call_count.clone();
        let result1 =
            SimpleTranspileCache::get_or_transpile("select(col1)", &options, |_code, _opts| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok("SELECT col1 FROM table".to_string())
            });

        assert!(result1.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second call with same input should use cache
        let call_count_clone = call_count.clone();
        let result2 =
            SimpleTranspileCache::get_or_transpile("select(col1)", &options, |_code, _opts| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok("SELECT col1 FROM table".to_string())
            });

        assert!(result2.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Should not increment due to cache hit
        assert_eq!(result1.unwrap(), result2.unwrap());

        // Verify cache metrics
        let metrics = SimpleTranspileCache::get_cache_metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
        assert_eq!(SimpleTranspileCache::get_hit_rate(), 0.5);

        // Test with `dplyr_compile` - using explicit unsafe blocks
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let dplyr_code = CString::new("select(caching_test)").unwrap();

        let result = unsafe { dplyr_compile(
            dplyr_code.as_ptr(),
            std::ptr::null(),
            &mut out_sql,
            &mut out_error,
        ) };
        assert_eq!(result, DPLYR_SUCCESS);
        assert!(!out_sql.is_null());
        unsafe { dplyr_free_string(out_sql) };
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }
    }

    // R3-AC3: Memory management tests
    #[test]
    fn test_memory_management() {
        // Test string allocation and deallocation
        let test_str = "test string for memory management";
        let c_string = CString::new(test_str).unwrap();
        let raw_ptr = c_string.into_raw();

        // Verify the string is valid
        let recovered = unsafe { CStr::from_ptr(raw_ptr) };
        assert_eq!(recovered.to_str().unwrap(), test_str);

        // Free it safely
        assert_eq!(unsafe { dplyr_free_string(raw_ptr) }, DPLYR_SUCCESS);

        // Test null pointer handling
        assert_eq!(unsafe { dplyr_free_string(std::ptr::null_mut()) }, DPLYR_SUCCESS);

        // Test multiple string freeing
        let str1 = CString::new("test1").unwrap().into_raw();
        let str2 = CString::new("test2").unwrap().into_raw();
        let mut string_array = [str1, str2];

        let freed_count = unsafe { dplyr_free_strings(string_array.as_mut_ptr(), 2) };
        assert_eq!(freed_count, 2);

        // Test null array handling
        assert_eq!(
            unsafe { dplyr_free_strings(std::ptr::null_mut(), 0) },
            DPLYR_ERROR_NULL_POINTER
        );
    }

    // R8-AC1: Version and capability tests
    #[test]
    fn test_version_and_capabilities() {
        // Test version functions
        let version = unsafe { CStr::from_ptr(libdplyr_c_version_simple()) };
        assert_eq!(version.to_str().unwrap(), "0.1.0");

        let detailed = unsafe { CStr::from_ptr(dplyr_version_detailed()) };
        let detailed_str = detailed.to_str().unwrap();
        assert!(detailed_str.contains("libdplyr_c v0.1.0"));

        let dialects = unsafe { CStr::from_ptr(dplyr_supported_dialects()) };
        assert_eq!(dialects.to_str().unwrap(), "DuckDB");

        let timestamp = unsafe { CStr::from_ptr(dplyr_build_timestamp()) };
        let timestamp_str = timestamp.to_str().unwrap();
        assert!(!timestamp_str.is_empty());

        // Test capability functions
        let _has_debug = dplyr_has_debug_support();


        assert_eq!(dplyr_max_input_length(), MAX_INPUT_LENGTH as u32);
        assert_eq!(dplyr_max_processing_time_ms(), MAX_PROCESSING_TIME_MS);

        // Test system check
        assert_eq!(dplyr_check_system(), DPLYR_SUCCESS);
    }

    // Helper function tests
    #[test]
    fn test_helper_functions() {
        // Test nesting depth calculation
        assert_eq!(calculate_nesting_depth("()"), 1);
        assert_eq!(calculate_nesting_depth("(())"), 2);
        assert_eq!(calculate_nesting_depth("()()"), 1);
        assert_eq!(calculate_nesting_depth("((()))"), 3);
        assert_eq!(calculate_nesting_depth("select(filter(col1))"), 2);

        // Test function call counting
        assert_eq!(count_function_calls("func()"), 1);
        assert_eq!(count_function_calls("func1() func2()"), 2);
        assert_eq!(count_function_calls("select(col1) %>% filter(age > 18)"), 2);
        assert_eq!(count_function_calls("no functions here"), 0);
        assert_eq!(count_function_calls("func ( )"), 1); // With spaces

        // Test suspicious pattern detection
        assert!(contains_suspicious_patterns("'; DROP TABLE"));
        assert!(contains_suspicious_patterns("union select"));
        assert!(contains_suspicious_patterns("UNION SELECT"));
        assert!(contains_suspicious_patterns("<script>"));
        assert!(contains_suspicious_patterns("../"));
        assert!(!contains_suspicious_patterns("select(col1)"));

        // Test excessive repetition detection
        assert!(has_excessive_repetition(&"a".repeat(150)));
        assert!(has_excessive_repetition(&"abc".repeat(25)));
        assert!(!has_excessive_repetition("normal input"));
        assert!(!has_excessive_repetition(&"a".repeat(10))); // Well under threshold
    }

    // Error conversion tests
    #[test]
    fn test_error_conversion() {
        // Test libdplyr error conversion
        let lex_error =
            libdplyr::TranspileError::LexError(libdplyr::LexError::UnexpectedCharacter('x', 5));
        let converted = convert_libdplyr_error(lex_error);
        assert_eq!(converted.to_c_error_code(), DPLYR_ERROR_SYNTAX);

        let parse_error =
            libdplyr::TranspileError::ParseError(libdplyr::ParseError::UnexpectedToken {
                expected: "identifier".to_string(),
                found: "number".to_string(),
                position: 10,
            });
        let converted = convert_libdplyr_error(parse_error);
        assert_eq!(converted.to_c_error_code(), DPLYR_ERROR_SYNTAX);

        let gen_error = libdplyr::TranspileError::GenerationError(
            libdplyr::GenerationError::UnsupportedOperation {
                operation: "complex_join".to_string(),
                dialect: "simple_query".to_string(),
            },
        );
        let converted = convert_libdplyr_error(gen_error);
        assert_eq!(converted.to_c_error_code(), DPLYR_ERROR_UNSUPPORTED);
    }

    // Constants validation tests
    #[test]
    fn test_constants_validation() {
        // Verify constants are reasonable






        // Verify relationships





        // Test that constants match expected values
        assert_eq!(MAX_INPUT_LENGTH, 1024 * 1024); // 1MB
        assert_eq!(MAX_PROCESSING_TIME_MS, 30000); // 30 seconds
        assert_eq!(MAX_OUTPUT_LENGTH, 10 * 1024 * 1024); // 10MB
        assert_eq!(MAX_NESTING_DEPTH, 50);
        assert_eq!(MAX_FUNCTION_CALLS, 1000);
    }

    // String pointer validation tests
    #[test]
    fn test_string_pointer_validation() {
        // Test null pointer
        assert!(!unsafe { dplyr_is_valid_string_pointer(std::ptr::null()) });

        // Test valid string
        let valid_string = CString::new("test").unwrap();
        assert!(unsafe { dplyr_is_valid_string_pointer(valid_string.as_ptr()) });

        // Test empty string
        let empty_string = CString::new("").unwrap();
        assert!(unsafe { dplyr_is_valid_string_pointer(empty_string.as_ptr()) });
    }

    // Integration test with actual transpilation (if libdplyr is available)
    #[test]
    #[ignore] // Ignore by default since it requires libdplyr to be fully functional
    fn test_full_transpilation_integration() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        let dplyr_code = CString::new("select(name, age)").unwrap();
        let options = DplyrOptions::default();

        let result = unsafe { dplyr_compile(dplyr_code.as_ptr(), &options, &mut out_sql, &mut out_error) };

        if result == DPLYR_SUCCESS {
            assert!(!out_sql.is_null());
            let sql_result = unsafe { CStr::from_ptr(out_sql) };
            let sql_str = sql_result.to_str().unwrap();
            assert!(!sql_str.is_empty());

            // Clean up
            unsafe { dplyr_free_string(out_sql) };
        } else {
            // If transpilation fails, we should have an error message
            assert!(!out_error.is_null());
            let error_result = unsafe { CStr::from_ptr(out_error) };
            let error_str = error_result.to_str().unwrap();
            assert!(!error_str.is_empty());

            // Clean up
            unsafe { dplyr_free_string(out_error) };
        }
    }

    // R6-AC1, R6-AC2: Performance validation tests
    #[test]
    fn test_simple_query_performance_target() {
        use std::time::Instant;

        let options = DplyrOptions::default();
        let query = "select(mpg, cyl)";

        // Warm up
        for _ in 0..10 {
            let _ = safe_dplyr_compile_test(query, &options);
        }

        // Measure performance over multiple runs
        let mut durations = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            let result = safe_dplyr_compile_test(query, &options);
            durations.push(start.elapsed());

            // Verify the query actually works
            assert!(result.is_ok(), "Query should succeed: {:?}", result);
        }

        // Calculate P95
        durations.sort();
        let p95_index = (durations.len() as f64 * 0.95) as usize;
        let p95_duration = durations[p95_index];

        println!("Simple query P95: {:?}", p95_duration);

        // R6-AC1: Simple queries should be under 2ms P95
        const SIMPLE_QUERY_TARGET_MS: f64 = 2.0;
        assert!(
            p95_duration.as_millis() as f64 <= SIMPLE_QUERY_TARGET_MS,
            "Simple query P95 ({:?}) exceeds target ({}ms)",
            p95_duration,
            SIMPLE_QUERY_TARGET_MS
        );
    }

    #[test]
    fn test_complex_query_performance_target() {
        use std::time::Instant;

        let options = DplyrOptions::default();
        let query = "mtcars %>% select(mpg, cyl, hp) %>% filter(mpg > 20) %>% group_by(cyl) %>% summarise(avg_hp = mean(hp)) %>% arrange(desc(avg_hp))";

        // Warm up
        for _ in 0..5 {
            let _ = safe_dplyr_compile_test(query, &options);
        }

        // Measure performance over multiple runs
        let mut durations = Vec::new();
        for _ in 0..50 {
            let start = Instant::now();
            let result = safe_dplyr_compile_test(query, &options);
            durations.push(start.elapsed());

            // Verify the query actually works
            assert!(result.is_ok(), "Query should succeed: {:?}", result);
        }

        // Calculate P95
        durations.sort();
        let p95_index = (durations.len() as f64 * 0.95) as usize;
        let p95_duration = durations[p95_index];

        println!("Complex query P95: {:?}", p95_duration);

        // R6-AC1: Complex queries should be under 15ms P95
        const COMPLEX_QUERY_TARGET_MS: f64 = 15.0;
        assert!(
            p95_duration.as_millis() as f64 <= COMPLEX_QUERY_TARGET_MS,
            "Complex query P95 ({:?}) exceeds target ({}ms)",
            p95_duration,
            COMPLEX_QUERY_TARGET_MS
        );
    }

    #[test]
    fn test_cache_effectiveness() {
        use std::time::Instant;

        let options = DplyrOptions::default();
        let query = "select(mpg, cyl) %>% filter(mpg > 20)";

        // First call (cache miss)
        let start = Instant::now();
        let result1 = safe_dplyr_compile_test(query, &options);
        let cache_miss_duration = start.elapsed();

        assert!(result1.is_ok(), "First query should succeed");

        // Second call (cache hit)
        let start = Instant::now();
        let result2 = safe_dplyr_compile_test(query, &options);
        let cache_hit_duration = start.elapsed();

        assert!(result2.is_ok(), "Second query should succeed");
        assert_eq!(
            result1.unwrap(),
            result2.unwrap(),
            "Results should be identical"
        );

        println!(
            "Cache miss: {:?}, Cache hit: {:?}",
            cache_miss_duration, cache_hit_duration
        );

        // R6-AC2: Cache should provide significant speedup
        // Cache hit should be measurably faster than cache miss (>=20% faster)
        assert!(
            cache_hit_duration.as_nanos() * 5 < cache_miss_duration.as_nanos() * 4,
            "Cache not effective enough: miss={:?}, hit={:?}",
            cache_miss_duration,
            cache_hit_duration
        );
    }

    // Helper function for performance tests
    #[allow(dead_code)]
    fn safe_dplyr_compile_test(query: &str, options: &DplyrOptions) -> Result<String, String> {
        use std::ffi::{CStr, CString};

        let c_query = CString::new(query).unwrap();
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        let result = unsafe { dplyr_compile(
            c_query.as_ptr(),
            options as *const DplyrOptions,
            &mut out_sql,
            &mut out_error,
        ) };

        if result == 0 {
            // Success
            let sql = unsafe {
                let c_str = CStr::from_ptr(out_sql);
                let rust_str = c_str.to_string_lossy().into_owned();
                dplyr_free_string(out_sql);
                rust_str
            };
            Ok(sql)
        } else {
            // Error
            let error = unsafe {
                let c_str = CStr::from_ptr(out_error);
                let rust_str = c_str.to_string_lossy().into_owned();
                dplyr_free_string(out_error);
                rust_str
            };
            Err(error)
        }
    }

// DuckDB C Extension API init function
// This function is required for C API-based DuckDB extensions
// (Removed dplyr_extension_init_c_api to avoid conflict with C++ extension init)
// Initialization for C API extension
// (Removed dangling code)
