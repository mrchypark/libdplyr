//! Compile/transpile entrypoints.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic;
use std::time::{Duration, Instant};

use libdplyr::sql_generator::DuckDbDialect;
use libdplyr::Transpiler;

use crate::cache;
use crate::cache::SimpleTranspileCache;
use crate::error::{create_error_message_with_context, TranspileError};
use crate::ffi::{set_error_output, set_sql_output};
use crate::options::{DplyrOptions, MAX_OUTPUT_LENGTH, MAX_PROCESSING_TIME_MS};
use crate::validation::{
    validate_input_encoding, validate_input_security, validate_input_structure,
};

use crate::error::{
    DPLYR_ERROR_INPUT_TOO_LARGE, DPLYR_ERROR_INVALID_UTF8, DPLYR_ERROR_NULL_POINTER, DPLYR_SUCCESS,
};

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
                    cache::dplyr_cache_log_stats_detailed(c"DEBUG_TRANSPILE".as_ptr(), true);

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

    result.unwrap_or_else(|_| {
        // Panic occurred - set error message if possible
        unsafe {
            if !out_error.is_null() {
                let panic_msg = CString::new("E-INTERNAL: Internal panic occurred").unwrap();
                *out_error = panic_msg.into_raw();
            }
        }
        -4 // Panic error code
    })
}

/// Convert libdplyr error to our TranspileError type
pub fn convert_libdplyr_error(libdplyr_error: libdplyr::TranspileError) -> TranspileError {
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
