//! Compile/transpile entrypoints.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic;
use std::ptr;
use std::time::{Duration, Instant};

use libdplyr::{
    DuckDbDialect, MySqlDialect, PostgreSqlDialect, SqlDialect, SqliteDialect, Transpiler,
};

use crate::cache;
use crate::cache::SimpleTranspileCache;
use crate::error::{create_error_message_with_context, TranspileError};
use crate::ffi::{set_error_output, set_sql_output};
use crate::options::{DplyrDialect, DplyrOptions, MAX_OUTPUT_LENGTH, MAX_PROCESSING_TIME_MS};
use crate::validation::{
    validate_input_encoding, validate_input_security, validate_input_structure,
};

use crate::error::{
    DPLYR_ERROR_INPUT_TOO_LARGE, DPLYR_ERROR_INVALID_UTF8, DPLYR_ERROR_NULL_POINTER,
    DPLYR_ERROR_PANIC, DPLYR_QUERY_NOT_HANDLED, DPLYR_SUCCESS,
};

fn create_dialect(dialect: DplyrDialect) -> Box<dyn SqlDialect> {
    match dialect {
        DplyrDialect::DuckDb => Box::new(DuckDbDialect::new()),
        DplyrDialect::PostgreSql => Box::new(PostgreSqlDialect::new()),
        DplyrDialect::MySql => Box::new(MySqlDialect::new()),
        DplyrDialect::Sqlite => Box::new(SqliteDialect::new()),
    }
}

enum CompileInputError {
    InputTooLarge(String),
    Transpile(TranspileError),
}

fn validate_compile_input(code_str: &str, opts: &DplyrOptions) -> Result<(), CompileInputError> {
    if code_str.len() > opts.max_input_length as usize {
        return Err(CompileInputError::InputTooLarge(format!(
            "E-INPUT-TOO-LARGE: Input size {} exceeds maximum {}",
            code_str.len(),
            opts.max_input_length
        )));
    }

    validate_input_encoding(code_str).map_err(CompileInputError::Transpile)?;
    validate_input_structure(code_str).map_err(CompileInputError::Transpile)?;
    opts.validate().map_err(CompileInputError::Transpile)?;
    Ok(())
}

fn compile_to_sql(code_str: &str, opts: &DplyrOptions) -> Result<String, TranspileError> {
    let processing_start = Instant::now();
    let timeout_ms = if opts.max_processing_time_ms == 0 {
        MAX_PROCESSING_TIME_MS
    } else {
        opts.max_processing_time_ms
    };
    let max_processing_time = Duration::from_millis(timeout_ms);

    SimpleTranspileCache::get_or_transpile(code_str, opts, |dplyr_code, options| {
        if processing_start.elapsed() > max_processing_time {
            return Err(TranspileError::internal_error_with_hint(
                &format!(
                    "Processing timeout: exceeded {}ms limit",
                    max_processing_time.as_millis()
                ),
                Some("Reduce input complexity or increase timeout limit".to_string()),
            ));
        }

        validate_input_security(dplyr_code)?;

        let transpiler = Transpiler::new(create_dialect(options.dialect));
        let transpile_start = Instant::now();
        let transpile_result = transpiler.transpile(dplyr_code);

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
            Err(libdplyr_error) => Err(convert_libdplyr_error(libdplyr_error)),
        }
    })
}

fn strip_trailing_semicolon(input: &str) -> String {
    let mut output = input.trim().to_string();
    while output.ends_with(';') {
        output.pop();
        output = output.trim_end().to_string();
    }
    output
}

fn extract_leading_table_name(dplyr_code: &str) -> Option<&str> {
    let pipe_pos = dplyr_code.find("%>%");
    let prefix = match pipe_pos {
        Some(pos) => &dplyr_code[..pos],
        None => dplyr_code,
    }
    .trim();

    if prefix.is_empty() {
        return None;
    }

    if prefix
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        Some(prefix)
    } else {
        None
    }
}

fn require_pipeline_table_name(dplyr_code: &str) -> Result<(), TranspileError> {
    if extract_leading_table_name(dplyr_code).is_none() {
        return Err(TranspileError::syntax_error_with_suggestion(
            "DPLYR pipeline must start with a table name",
            0,
            None,
            Some("Start the pipeline with a source table before %>%".to_string()),
        ));
    }
    Ok(())
}

fn find_embedded_start_marker(query: &str, from: usize) -> Option<(usize, usize)> {
    let bytes = query.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] != b'(' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'|' {
            return Some((i, j + 1));
        }
        i += 1;
    }
    None
}

fn find_embedded_end_marker(query: &str, from: usize) -> Option<(usize, usize)> {
    let bytes = query.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] != b'|' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b')' {
            return Some((i, j));
        }
        i += 1;
    }
    None
}

fn replace_embedded_pipelines(query: &str, opts: &DplyrOptions) -> Result<String, TranspileError> {
    let mut output = String::with_capacity(query.len());
    let mut cursor = 0;

    while cursor < query.len() {
        let Some((marker_start, content_start)) = find_embedded_start_marker(query, cursor) else {
            output.push_str(&query[cursor..]);
            break;
        };

        output.push_str(&query[cursor..marker_start]);
        let Some((content_end, marker_end)) = find_embedded_end_marker(query, content_start) else {
            return Err(TranspileError::syntax_error_with_suggestion(
                "Unterminated embedded dplyr segment",
                marker_start,
                None,
                Some("Close embedded pipelines with '|)'.".to_string()),
            ));
        };

        let embedded = strip_trailing_semicolon(&query[content_start..content_end]);
        if embedded.is_empty() {
            return Err(TranspileError::syntax_error_with_suggestion(
                "Embedded dplyr segment cannot be empty",
                content_start,
                None,
                None,
            ));
        }
        if !embedded.contains("%>%") {
            return Err(TranspileError::syntax_error_with_suggestion(
                "Embedded dplyr segment must contain a %>% pipeline",
                content_start,
                None,
                None,
            ));
        }

        require_pipeline_table_name(&embedded)?;
        let sql = compile_to_sql(&embedded, opts)?;
        output.push('(');
        output.push_str(&sql);
        output.push(')');

        cursor = marker_end + 1;
    }

    Ok(output)
}

fn compile_query_string(
    query: &str,
    opts: &DplyrOptions,
) -> Result<Option<String>, TranspileError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || !trimmed.contains("%>%") {
        return Ok(None);
    }

    let sql = if find_embedded_start_marker(trimmed, 0).is_some() {
        let rewritten = replace_embedded_pipelines(trimmed, opts)?;
        if rewritten.contains("%>%") {
            return Err(TranspileError::syntax_error_with_suggestion(
                "Unprocessed %>% pipeline remains",
                0,
                None,
                Some(
                    "Wrap pipelines with (| ... |) or provide a pure pipeline statement."
                        .to_string(),
                ),
            ));
        }
        rewritten
    } else {
        let dplyr_code = strip_trailing_semicolon(trimmed);
        require_pipeline_table_name(&dplyr_code)?;
        compile_to_sql(&dplyr_code, opts)?
    };

    let normalized = sql.trim_start().to_ascii_uppercase();
    if !(normalized.starts_with("SELECT") || normalized.starts_with("WITH")) {
        return Err(TranspileError::unsupported_operation_with_alternative(
            "generated a non-SELECT statement",
            "query compilation",
            Some("Only SELECT/WITH statements are supported for parser rewrite".to_string()),
        ));
    }

    Ok(Some(sql))
}

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

        unsafe {
            *out_sql = ptr::null_mut();
            *out_error = ptr::null_mut();
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

        if let Err(error) = validate_compile_input(code_str, &opts) {
            match error {
                CompileInputError::InputTooLarge(message) => {
                    set_error_output(out_error, &message);
                    return DPLYR_ERROR_INPUT_TOO_LARGE;
                }
                CompileInputError::Transpile(error) => {
                    let error_msg = error.to_c_string();
                    set_error_output(out_error, &error_msg.to_string_lossy());
                    return error.to_c_error_code();
                }
            }
        }

        let transpile_result = compile_to_sql(code_str, &opts);

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
        DPLYR_ERROR_PANIC
    })
}

#[no_mangle]
/// Compile a DuckDB query string, rewriting dplyr pipelines when present.
///
/// # Safety
/// Caller must ensure that:
/// - `query` is a valid null-terminated C string.
/// - `options` is a valid pointer to a `DplyrOptions` struct, or `std::ptr::null()`.
/// - `out_sql` and `out_error` are valid mutable pointers to `*mut c_char`.
/// - Any returned string pointer is freed with `dplyr_free_string`.
pub unsafe extern "C" fn dplyr_compile_query(
    query: *const c_char,
    options: *const DplyrOptions,
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if query.is_null() {
            set_error_output(out_error, "E-NULL-POINTER: query parameter is null");
            return DPLYR_ERROR_NULL_POINTER;
        }

        if out_sql.is_null() || out_error.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        unsafe {
            *out_sql = ptr::null_mut();
            *out_error = ptr::null_mut();
        }

        let query_str = match unsafe { CStr::from_ptr(query) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                set_error_output(
                    out_error,
                    "E-INVALID-UTF8: Input query contains invalid UTF-8",
                );
                return DPLYR_ERROR_INVALID_UTF8;
            }
        };

        let opts = if options.is_null() {
            DplyrOptions::default()
        } else {
            unsafe { (*options).clone() }
        };

        if let Err(error) = validate_compile_input(query_str, &opts) {
            match error {
                CompileInputError::InputTooLarge(message) => {
                    set_error_output(out_error, &message);
                    return DPLYR_ERROR_INPUT_TOO_LARGE;
                }
                CompileInputError::Transpile(error) => {
                    let error_msg = error.to_c_string();
                    set_error_output(out_error, &error_msg.to_string_lossy());
                    return error.to_c_error_code();
                }
            }
        }

        match compile_query_string(query_str, &opts) {
            Ok(Some(sql)) => {
                set_sql_output(out_sql, &sql);
                DPLYR_SUCCESS
            }
            Ok(None) => DPLYR_QUERY_NOT_HANDLED,
            Err(error) => {
                let error_msg = if opts.debug_mode {
                    create_error_message_with_context(&error, Some(query_str))
                } else {
                    error.to_c_string()
                };
                set_error_output(out_error, &error_msg.to_string_lossy());
                error.to_c_error_code()
            }
        }
    });

    result.unwrap_or_else(|_| {
        unsafe {
            if !out_error.is_null() {
                let panic_msg = CString::new("E-INTERNAL: Internal panic occurred").unwrap();
                *out_error = panic_msg.into_raw();
            }
        }
        DPLYR_ERROR_PANIC
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
