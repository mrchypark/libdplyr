//! Schema-aware compile entrypoint.
//!
//! Additive: the existing dplyr_compile entrypoints are untouched. This module binds
//! dplyr code to caller-supplied source metadata before SQL generation.
//!
//! The schema JSON is untrusted caller metadata, so it is bounded and UTF-8 checked,
//! parsed, and run through SourceSchema::validate. It is deliberately NOT scanned by the
//! R-code suspicious-pattern filters: legitimate metadata (a column named "../", a
//! "UNION SELECT" literal, or plain JSON punctuation density) trips those filters.
//!
//! Compiled SQL is never cached here. The schema is an input, so identical code with a
//! different schema must produce fresh SQL rather than a cached answer for the old one.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic;
use std::time::{Duration, Instant};

use libdplyr::relational::{SchemaInput, SourceSchema};
use libdplyr::{
    DuckDbDialect, MySqlDialect, PipeSyntax, PostgreSqlDialect, SqlDialect, SqliteDialect,
    Transpiler,
};

use crate::compile::convert_libdplyr_error;
use crate::error::{
    TranspileError, DPLYR_ERROR_INPUT_TOO_LARGE, DPLYR_ERROR_INTERNAL, DPLYR_ERROR_INVALID_UTF8,
    DPLYR_ERROR_NULL_POINTER, DPLYR_ERROR_PANIC, DPLYR_SUCCESS,
};
use crate::ffi::{clear_output_string, set_error_output, set_sql_output};
use crate::options::{
    DplyrDialect, DplyrOptions, DplyrPipeSyntax, MAX_INPUT_LENGTH, MAX_OUTPUT_LENGTH,
    MAX_PROCESSING_TIME_MS,
};
use crate::validation::{
    validate_input_encoding, validate_input_security, validate_input_structure,
};

fn create_dialect(dialect: DplyrDialect) -> Box<dyn SqlDialect> {
    match dialect {
        DplyrDialect::DuckDb => Box::new(DuckDbDialect::new()),
        DplyrDialect::PostgreSql => Box::new(PostgreSqlDialect::new()),
        DplyrDialect::MySql => Box::new(MySqlDialect::new()),
        DplyrDialect::Sqlite => Box::new(SqliteDialect::new()),
    }
}

fn publish_error(out_error: *mut *mut c_char, code: i32, message: &str) -> i32 {
    if set_error_output(out_error, message) {
        code
    } else {
        DPLYR_ERROR_INTERNAL
    }
}

fn publish_transpile_error(out_error: *mut *mut c_char, error: &TranspileError) -> i32 {
    let message = error.to_c_string().to_string_lossy().into_owned();
    publish_error(out_error, error.to_c_error_code(), &message)
}

fn processing_timeout(opts: &DplyrOptions) -> Duration {
    let timeout_ms = if opts.max_processing_time_ms == 0 {
        MAX_PROCESSING_TIME_MS
    } else {
        opts.max_processing_time_ms
    };
    Duration::from_millis(timeout_ms)
}

fn processing_deadline(opts: &DplyrOptions) -> Instant {
    Instant::now() + processing_timeout(opts)
}

fn ensure_before_deadline(opts: &DplyrOptions, deadline: Instant) -> Result<(), TranspileError> {
    if Instant::now() > deadline {
        return Err(TranspileError::internal_error_with_hint(
            &format!(
                "Processing timeout: exceeded {}ms limit",
                processing_timeout(opts).as_millis()
            ),
            Some("Reduce input complexity or increase timeout limit".to_string()),
        ));
    }

    Ok(())
}

/// Parse and validate untrusted schema JSON metadata.
///
/// Accepts one source object or an ordered array of them.
fn parse_schemas(schema_json: &str) -> Result<Vec<SourceSchema>, TranspileError> {
    if schema_json.len() > MAX_INPUT_LENGTH {
        return Err(TranspileError::input_too_large_error(
            schema_json.len(),
            MAX_INPUT_LENGTH,
        ));
    }

    if let Err(error) = validate_input_encoding(schema_json) {
        return Err(TranspileError::invalid_utf8_error(&format!(
            "Schema JSON {}",
            error.to_c_string().to_string_lossy()
        )));
    }

    let schemas: SchemaInput = serde_json::from_str(schema_json).map_err(|error| {
        TranspileError::syntax_error_with_suggestion(
            &format!("Schema JSON is malformed: {}", error),
            0,
            None,
            Some(
                "Expected {\"source\":\"tbl\",\"columns\":[{\"name\":\"col\"}]} or an array of those"
                    .to_string(),
            ),
        )
    })?;

    schemas.validate().map_err(|error| {
        TranspileError::unsupported_operation_with_alternative(
            &format!("Invalid schema: {}", error),
            "schema metadata",
            Some(
                "Provide distinct sources, each with a non-empty name and unique, non-empty column names"
                    .to_string(),
            ),
        )
    })?;

    Ok(schemas.as_slice().to_vec())
}

fn compile_with_schema_to_sql(
    code_str: &str,
    schema_json: &str,
    opts: &DplyrOptions,
    pipe_syntax: PipeSyntax,
) -> Result<String, TranspileError> {
    // The deadline starts before parsing so that untrusted schema validation is
    // charged to the caller's processing budget.
    let deadline = processing_deadline(opts);
    let schemas = parse_schemas(schema_json)?;
    ensure_before_deadline(opts, deadline)?;

    // Only the dplyr code goes through the R-code security filters; the schema has its
    // own validator above.
    validate_input_security(code_str)?;
    ensure_before_deadline(opts, deadline)?;

    let transpiler = Transpiler::with_pipe_syntax(
        create_dialect(DplyrDialect::try_from(opts.dialect)?),
        pipe_syntax,
    );

    let compiled = transpiler
        .transpile_with_schemas(code_str, &schemas)
        .map_err(convert_libdplyr_error)?;

    ensure_before_deadline(opts, deadline)?;

    if compiled.sql.len() > MAX_OUTPUT_LENGTH {
        return Err(TranspileError::internal_error_with_hint(
            &format!(
                "Output too large: {} bytes exceeds maximum {}",
                compiled.sql.len(),
                MAX_OUTPUT_LENGTH
            ),
            Some("Input generates excessive SQL output".to_string()),
        ));
    }

    Ok(compiled.sql)
}

/// Mirrors compile.rs: oversized input is reported as E-INPUT-TOO-LARGE, other
/// validation failures map through the transpile error code.
fn validate_code_input(
    code_str: &str,
    opts: &DplyrOptions,
    out_error: *mut *mut c_char,
) -> Result<(), i32> {
    if code_str.len() > opts.max_input_length as usize {
        return Err(publish_error(
            out_error,
            DPLYR_ERROR_INPUT_TOO_LARGE,
            &format!(
                "E-INPUT-TOO-LARGE: Input size {} exceeds maximum {}",
                code_str.len(),
                opts.max_input_length
            ),
        ));
    }

    validate_input_encoding(code_str)
        .and_then(|()| validate_input_structure(code_str))
        .and_then(|()| opts.validate())
        .map_err(|error| publish_transpile_error(out_error, &error))
}

/// Same resolution dplyr_compile uses: env override, otherwise magrittr.
fn pipe_syntax_from_env_or_default() -> Result<PipeSyntax, TranspileError> {
    PipeSyntax::from_env_or_default().map_err(|message| {
        TranspileError::syntax_error_with_suggestion(
            &message,
            0,
            Some("DPLYR_PIPE_SYNTAX".to_string()),
            Some("Set DPLYR_PIPE_SYNTAX=magrittr or DPLYR_PIPE_SYNTAX=native".to_string()),
        )
    })
}

/// Compile dplyr code against caller-supplied schema metadata.
///
/// Pipe syntax comes from the environment (DPLYR_PIPE_SYNTAX), defaulting to magrittr.
/// See dplyr_compile_with_schema_and_pipe_syntax for an explicit override.
///
/// # Safety
/// See compile_with_schema_boundary.
#[no_mangle]
pub unsafe extern "C" fn dplyr_compile_with_schema(
    dplyr_code: *const c_char,
    schema_json: *const c_char,
    options: *const DplyrOptions,
    sql_output: *mut *mut c_char,
    error_output: *mut *mut c_char,
) -> i32 {
    unsafe {
        compile_with_schema_boundary(
            dplyr_code,
            schema_json,
            options,
            None,
            sql_output,
            error_output,
        )
    }
}

/// Compile dplyr code against caller-supplied schema metadata with an explicit
/// pipe syntax mode, independent of the DPLYR_PIPE_SYNTAX environment variable.
///
/// # Safety
/// See compile_with_schema_boundary.
#[no_mangle]
pub unsafe extern "C" fn dplyr_compile_with_schema_and_pipe_syntax(
    dplyr_code: *const c_char,
    schema_json: *const c_char,
    options: *const DplyrOptions,
    pipe_syntax: u32,
    sql_output: *mut *mut c_char,
    error_output: *mut *mut c_char,
) -> i32 {
    unsafe {
        compile_with_schema_boundary(
            dplyr_code,
            schema_json,
            options,
            Some(pipe_syntax),
            sql_output,
            error_output,
        )
    }
}

/// Shared boundary for the schema-aware entrypoints.
///
/// Same return contract as dplyr_compile: 0 on success with *sql_output set, a negative
/// DPLYR_ERROR_* code otherwise with *error_output set.
///
/// The compiled SQL is not cached, so changing the schema always re-compiles.
///
/// `pipe_syntax` is `Some` only when the caller passes an explicit mode; `None`
/// resolves the environment default like dplyr_compile does.
///
/// # Safety
/// Caller must ensure that:
/// - code and schema_json are valid null-terminated C strings.
/// - options is a valid pointer to a DplyrOptions struct, or null for defaults.
/// - sql_output and error_output are valid mutable pointers to *mut c_char.
/// - On entry, *sql_output and *error_output are null or pointers previously allocated by
///   libdplyr; ownership of any non-null incoming libdplyr pointer transfers back here.
/// - Any returned string pointer is freed with dplyr_free_string.
/// - If this returns DPLYR_ERROR_PANIC, callers must not assume *out_error was populated.
unsafe fn compile_with_schema_boundary(
    dplyr_code: *const c_char,
    schema_json: *const c_char,
    options: *const DplyrOptions,
    pipe_syntax: Option<u32>,
    sql_output: *mut *mut c_char,
    error_output: *mut *mut c_char,
) -> i32 {
    #[cfg(test)]
    let _test_gate = crate::compile::acquire_ffi_test_gate_for_test();

    let result = panic::catch_unwind(|| {
        if sql_output.is_null() || error_output.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        clear_output_string(sql_output);
        clear_output_string(error_output);

        if dplyr_code.is_null() {
            return publish_error(
                error_output,
                DPLYR_ERROR_NULL_POINTER,
                "E-NULL-POINTER: dplyr_code parameter is null",
            );
        }

        if schema_json.is_null() {
            return publish_error(
                error_output,
                DPLYR_ERROR_NULL_POINTER,
                "E-NULL-POINTER: schema_json parameter is null",
            );
        }

        let code_str = match unsafe { CStr::from_ptr(dplyr_code) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                return publish_error(
                    error_output,
                    DPLYR_ERROR_INVALID_UTF8,
                    "E-INVALID-UTF8: dplyr_code contains invalid UTF-8",
                );
            }
        };

        let schema_str = match unsafe { CStr::from_ptr(schema_json) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                return publish_error(
                    error_output,
                    DPLYR_ERROR_INVALID_UTF8,
                    "E-INVALID-UTF8: schema_json contains invalid UTF-8",
                );
            }
        };

        let opts = if options.is_null() {
            DplyrOptions::default()
        } else {
            unsafe { (*options).clone() }
        };

        if let Err(code) = validate_code_input(code_str, &opts, error_output) {
            return code;
        }

        let pipe_syntax = match pipe_syntax {
            Some(raw) => match DplyrPipeSyntax::try_from(raw) {
                Ok(pipe_syntax) => PipeSyntax::from(pipe_syntax),
                Err(error) => return publish_transpile_error(error_output, &error),
            },
            None => match pipe_syntax_from_env_or_default() {
                Ok(pipe_syntax) => pipe_syntax,
                Err(error) => return publish_transpile_error(error_output, &error),
            },
        };

        match compile_with_schema_to_sql(code_str, schema_str, &opts, pipe_syntax) {
            Ok(sql) => {
                if set_sql_output(sql_output, &sql) {
                    DPLYR_SUCCESS
                } else {
                    publish_error(
                        error_output,
                        DPLYR_ERROR_INTERNAL,
                        "E-INTERNAL: Failed to publish generated SQL across the FFI boundary",
                    )
                }
            }
            Err(error) => publish_transpile_error(error_output, &error),
        }
    });

    result.unwrap_or(DPLYR_ERROR_PANIC)
}

/// List the distinct source relations a pipeline reads, as a JSON array of names.
///
/// This is the discovery step an embedder runs before it can supply schema
/// metadata: the JSON never comes from the caller, only the source names in the
/// parsed pipeline do, so callers can match them against catalog metadata.
///
/// `pipe_syntax` is the same `DplyrPipeSyntax` value the compile entrypoints take.
/// A pipeline with no explicit source (the schema-less path, such as a bare
/// `filter(a > 1)`) has no relation to report and fails with a syntax error.
///
/// Return codes, ownership, and panic behavior match dplyr_compile_with_schema().
/// Free `*sources_json` with dplyr_free_string.
///
/// # Safety
/// Caller must ensure that:
/// - dplyr_code is a valid null-terminated C string.
/// - sources_json and error_output are valid mutable pointers to *mut c_char.
/// - On entry, both slots are null or pointers previously allocated by libdplyr.
#[no_mangle]
pub unsafe extern "C" fn dplyr_required_sources(
    dplyr_code: *const c_char,
    pipe_syntax: u32,
    sources_json: *mut *mut c_char,
    error_output: *mut *mut c_char,
) -> i32 {
    #[cfg(test)]
    let _test_gate = crate::compile::acquire_ffi_test_gate_for_test();

    let result = panic::catch_unwind(|| {
        if sources_json.is_null() || error_output.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        clear_output_string(sources_json);
        clear_output_string(error_output);

        if dplyr_code.is_null() {
            return publish_error(
                error_output,
                DPLYR_ERROR_NULL_POINTER,
                "E-NULL-POINTER: dplyr_code parameter is null",
            );
        }

        let code_str = match unsafe { CStr::from_ptr(dplyr_code) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                return publish_error(
                    error_output,
                    DPLYR_ERROR_INVALID_UTF8,
                    "E-INVALID-UTF8: dplyr_code contains invalid UTF-8",
                );
            }
        };

        // The pipeline is untrusted caller input, so it goes through the same
        // size and security filters as the compile entrypoints.
        if code_str.len() > MAX_INPUT_LENGTH {
            return publish_error(
                error_output,
                DPLYR_ERROR_INPUT_TOO_LARGE,
                &format!(
                    "E-INPUT-TOO-LARGE: Input size {} exceeds maximum {}",
                    code_str.len(),
                    MAX_INPUT_LENGTH
                ),
            );
        }

        if let Err(error) = validate_input_encoding(code_str)
            .and_then(|()| validate_input_structure(code_str))
            .and_then(|()| validate_input_security(code_str))
        {
            return publish_transpile_error(error_output, &error);
        }

        let pipe_syntax = match DplyrPipeSyntax::try_from(pipe_syntax) {
            Ok(pipe_syntax) => PipeSyntax::from(pipe_syntax),
            Err(error) => return publish_transpile_error(error_output, &error),
        };

        let transpiler =
            Transpiler::with_pipe_syntax(create_dialect(DplyrDialect::DuckDb), pipe_syntax);

        let sources = match transpiler
            .required_sources(code_str)
            .map_err(convert_libdplyr_error)
        {
            Ok(sources) => sources,
            Err(error) => return publish_transpile_error(error_output, &error),
        };

        // serde_json escapes the names, so a source name that would break the
        // JSON cannot be emitted raw.
        let json = match serde_json::to_string(&sources) {
            Ok(json) => json,
            Err(error) => {
                return publish_error(
                    error_output,
                    DPLYR_ERROR_INTERNAL,
                    &format!("E-INTERNAL: Failed to serialize required sources: {error}"),
                )
            }
        };

        if set_sql_output(sources_json, &json) {
            DPLYR_SUCCESS
        } else {
            publish_error(
                error_output,
                DPLYR_ERROR_INTERNAL,
                "E-INTERNAL: Failed to publish required sources across the FFI boundary",
            )
        }
    });

    result.unwrap_or(DPLYR_ERROR_PANIC)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    use crate::error::{DPLYR_ERROR_SYNTAX, DPLYR_ERROR_UNSUPPORTED};
    use crate::memory::dplyr_free_string;

    fn restore_env(previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => std::env::set_var("DPLYR_PIPE_SYNTAX", value),
            None => std::env::remove_var("DPLYR_PIPE_SYNTAX"),
        }
    }

    fn compile(code: &str, schema: &str, options: Option<&DplyrOptions>) -> (i32, String, String) {
        compile_with_pipe(code, schema, options, None)
    }

    fn compile_with_pipe(
        code: &str,
        schema: &str,
        options: Option<&DplyrOptions>,
        pipe_syntax: Option<u32>,
    ) -> (i32, String, String) {
        let code = CString::new(code).expect("code has no NUL");
        let schema = CString::new(schema).expect("schema has no NUL");
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        let options_ptr = options.map_or(std::ptr::null(), |o| o as *const DplyrOptions);

        let result = unsafe {
            match pipe_syntax {
                Some(raw) => dplyr_compile_with_schema_and_pipe_syntax(
                    code.as_ptr(),
                    schema.as_ptr(),
                    options_ptr,
                    raw,
                    &mut out_sql,
                    &mut out_error,
                ),
                None => dplyr_compile_with_schema(
                    code.as_ptr(),
                    schema.as_ptr(),
                    options_ptr,
                    &mut out_sql,
                    &mut out_error,
                ),
            }
        };

        let take = |ptr: *mut c_char| -> String {
            if ptr.is_null() {
                return String::new();
            }
            let value = unsafe {
                let value = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                dplyr_free_string(ptr);
                value
            };
            value
        };

        (result, take(out_sql), take(out_error))
    }

    const VALID_SCHEMA: &str = r#"{"source":"mtcars","columns":[{"name":"mpg"},{"name":"cyl"}]}"#;

    #[test]
    fn valid_schema_produces_sql() {
        let (result, sql, error) = compile(
            "mtcars %>% filter(mpg > 20) %>% select(mpg, cyl)",
            VALID_SCHEMA,
            None,
        );

        assert_eq!(result, DPLYR_SUCCESS, "error: {error}");
        assert!(error.is_empty());
        assert!(!sql.is_empty());
        assert!(sql.contains("mtcars"), "{sql}");
    }

    #[test]
    fn schema_change_is_not_cached() {
        // No explicit source in the pipeline, so the schema drives the relation name
        // and column list. Identical code, different schema, must not reuse SQL.
        let code = "filter(a > 1)";
        let first = compile(
            code,
            r#"{"source":"first","columns":[{"name":"a"},{"name":"b"}]}"#,
            None,
        );
        let second = compile(
            code,
            r#"{"source":"second","columns":[{"name":"a"}]}"#,
            None,
        );
        let third = compile(code, r#"{"source":"first","columns":[{"name":"a"}]}"#, None);

        assert_eq!(first.0, DPLYR_SUCCESS, "error: {}", first.2);
        assert_eq!(second.0, DPLYR_SUCCESS, "error: {}", second.2);
        assert_eq!(third.0, DPLYR_SUCCESS, "error: {}", third.2);
        assert!(first.1.contains("first"), "{}", first.1);
        assert!(second.1.contains("second"), "{}", second.1);
        assert_ne!(first.1, second.1);
        // Same source as the first call but a narrower column list: a cached result
        // would replay the wider projection here.
        assert_ne!(first.1, third.1);
        assert!(first.1.contains("b"), "{}", first.1);
        assert_eq!(
            first.1,
            compile(
                code,
                r#"{"source":"first","columns":[{"name":"a"},{"name":"b"}]}"#,
                None,
            )
            .1
        );
    }

    #[test]
    fn malformed_json_maps_to_syntax_error() {
        let (result, sql, error) = compile("tbl %>% select(a)", "{\"source\": ", None);

        assert_eq!(result, DPLYR_ERROR_SYNTAX);
        assert!(sql.is_empty());
        assert!(error.contains("malformed"), "{error}");
    }

    #[test]
    fn json_null_fields_are_rejected_by_schema_validator() {
        for schema in [
            r#"{"source":null,"columns":[{"name":"a"}]}"#,
            r#"{"source":"tbl","columns":null}"#,
            r#"{"source":"tbl","columns":[{"name":null}]}"#,
        ] {
            let (result, sql, error) = compile("tbl %>% select(a)", schema, None);

            assert_ne!(result, DPLYR_SUCCESS, "schema: {schema}");
            assert!(sql.is_empty(), "schema: {schema}");
            assert!(!error.is_empty(), "schema: {schema}");
        }
    }

    #[test]
    fn invalid_parsed_schema_maps_to_unsupported() {
        for schema in [
            r#"{"source":"","columns":[{"name":"a"}]}"#,
            r#"{"source":"tbl","columns":[]}"#,
            r#"{"source":"tbl","columns":[{"name":"a"},{"name":"a"}]}"#,
            r#"{"source":"tbl","columns":[{"name":""}]}"#,
        ] {
            let (result, _sql, error) = compile("tbl %>% select(a)", schema, None);

            assert_eq!(
                result, DPLYR_ERROR_UNSUPPORTED,
                "schema: {schema} err: {error}"
            );
            assert!(
                error.contains("Invalid schema"),
                "schema: {schema} err: {error}"
            );
        }
    }

    #[test]
    fn schema_metadata_matching_r_code_patterns_is_accepted() {
        let (result, _sql, error) = compile(
            "tbl %>% filter(x == '../')",
            r#"{"source":"tbl","columns":[{"name":"x"},{"name":"UNION SELECT"}]}"#,
            None,
        );

        assert_eq!(result, DPLYR_SUCCESS, "error: {error}");
    }

    #[test]
    fn null_pointers_map_to_null_pointer_error() {
        let schema = CString::new(VALID_SCHEMA).expect("schema has no NUL");
        let code = CString::new("tbl %>% select(a)").expect("code has no NUL");
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        let null_code = unsafe {
            dplyr_compile_with_schema(
                std::ptr::null(),
                schema.as_ptr(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };
        assert_eq!(null_code, DPLYR_ERROR_NULL_POINTER);
        assert!(!out_error.is_null());
        unsafe { dplyr_free_string(out_error) };

        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let null_schema = unsafe {
            dplyr_compile_with_schema(
                code.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };
        assert_eq!(null_schema, DPLYR_ERROR_NULL_POINTER);
        assert!(!out_error.is_null());
        unsafe { dplyr_free_string(out_error) };

        let null_out = unsafe {
            dplyr_compile_with_schema(
                code.as_ptr(),
                schema.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                &mut out_error,
            )
        };
        assert_eq!(null_out, DPLYR_ERROR_NULL_POINTER);
    }

    #[test]
    fn oversized_schema_json_is_rejected() {
        let schema = format!(
            r#"{{"source":"tbl","columns":[{}]}}"#,
            (0..80_000)
                .map(|i| format!(r#"{{"name":"col{}"}}"#, i))
                .collect::<Vec<_>>()
                .join(",")
        );

        let (result, sql, error) = compile("tbl %>% select(a)", &schema, None);

        assert_eq!(result, DPLYR_ERROR_INTERNAL);
        assert!(sql.is_empty());
        assert!(error.contains("exceeds maximum"), "{error}");
    }

    #[test]
    fn invalid_options_map_to_option_error() {
        let options = DplyrOptions {
            max_input_length: u32::MAX,
            ..DplyrOptions::default()
        };

        let (result, sql, error) = compile("tbl %>% select(a)", VALID_SCHEMA, Some(&options));

        assert_eq!(result, DPLYR_ERROR_INTERNAL);
        assert!(sql.is_empty());
        assert!(error.contains("exceeds maximum"), "{error}");
    }

    #[test]
    fn oversized_code_is_rejected_before_schema_use() {
        let options = DplyrOptions {
            max_input_length: 64,
            ..DplyrOptions::default()
        };

        let long_code = format!("mtcars %>% filter(mpg > {})", "1".repeat(200));
        let (result, sql, error) = compile(&long_code, VALID_SCHEMA, Some(&options));

        assert_eq!(result, DPLYR_ERROR_INPUT_TOO_LARGE);
        assert!(sql.is_empty());
        assert!(!error.is_empty());
    }

    #[test]
    fn output_slots_are_cleared_on_reuse() {
        let code = CString::new("tbl %>% select(a)").expect("code has no NUL");
        let schema = CString::new(r#"{"source":"tbl","columns":[{"name":"a"}]}"#).expect("no NUL");
        let mut out_sql: *mut c_char =
            crate::memory::alloc_owned_string("stale sql").expect("allocation succeeds");
        let mut out_error: *mut c_char =
            crate::memory::alloc_owned_string("stale error").expect("allocation succeeds");

        let result = unsafe {
            dplyr_compile_with_schema(
                code.as_ptr(),
                schema.as_ptr(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };

        assert_eq!(result, DPLYR_SUCCESS);
        assert!(out_error.is_null());
        assert!(!out_sql.is_null());
        unsafe {
            dplyr_free_string(out_sql);
        }
    }

    #[test]
    fn explicit_native_pipe_compiles_independent_of_env() {
        let _gate = crate::compile::acquire_ffi_test_gate_for_test();
        let previous = std::env::var_os("DPLYR_PIPE_SYNTAX");
        // A hostile env must not affect the explicit entrypoint.
        std::env::set_var("DPLYR_PIPE_SYNTAX", "invalid-pipe-mode");

        let schema = r#"{"source":"tbl","columns":[{"name":"a"}]}"#;
        let (result, sql, error) = compile_with_pipe(
            "tbl |> filter(a > 1)",
            schema,
            None,
            Some(DplyrPipeSyntax::Native as u32),
        );

        restore_env(previous);

        assert_eq!(result, DPLYR_SUCCESS, "error: {error}");
        assert!(!sql.is_empty());
        assert!(sql.contains("tbl"), "{sql}");
    }

    #[test]
    fn explicit_pipe_overrides_env_contradiction() {
        let _gate = crate::compile::acquire_ffi_test_gate_for_test();
        let previous = std::env::var_os("DPLYR_PIPE_SYNTAX");
        std::env::set_var("DPLYR_PIPE_SYNTAX", "magrittr");

        let schema = r#"{"source":"tbl","columns":[{"name":"a"}]}"#;
        let (result, sql, error) = compile_with_pipe(
            "tbl |> filter(a > 1)",
            schema,
            None,
            Some(DplyrPipeSyntax::Native as u32),
        );

        restore_env(previous);

        // Native code must succeed even while the env asks for magrittr.
        assert_eq!(result, DPLYR_SUCCESS, "error: {error}");
        assert!(!sql.is_empty());
    }

    #[test]
    fn explicit_pipe_rejects_magrittr_code_when_native_is_requested() {
        let _gate = crate::compile::acquire_ffi_test_gate_for_test();
        let previous = std::env::var_os("DPLYR_PIPE_SYNTAX");
        std::env::set_var("DPLYR_PIPE_SYNTAX", "native");

        let schema = r#"{"source":"tbl","columns":[{"name":"a"}]}"#;
        let (result, sql, error) = compile_with_pipe(
            "tbl %>% filter(a > 1)",
            schema,
            None,
            Some(DplyrPipeSyntax::Native as u32),
        );

        restore_env(previous);

        assert_ne!(result, DPLYR_SUCCESS);
        assert!(sql.is_empty());
        assert!(error.contains("Magrittr pipe is not enabled"), "{error}");
    }

    #[test]
    fn invalid_explicit_pipe_mode_is_a_syntax_error() {
        let (result, sql, error) = compile_with_pipe(
            "tbl %>% filter(a > 1)",
            r#"{"source":"tbl","columns":[{"name":"a"}]}"#,
            None,
            Some(99),
        );

        assert_eq!(result, DPLYR_ERROR_SYNTAX);
        assert!(sql.is_empty());
        assert!(error.contains("Invalid pipe syntax value '99'"), "{error}");
    }

    #[test]
    fn explicit_pipe_entrypoint_still_validates_nulls_and_utf8() {
        let schema = CString::new(r#"{"source":"tbl","columns":[{"name":"a"}]}"#).expect("no NUL");
        let code = CString::new("tbl |> filter(a > 1)").expect("no NUL");

        // Invalid UTF-8 code.
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let invalid_utf8 = [b't', b'b', b'l', 0xF0, 0x9F, b' ', 1];
        let result = unsafe {
            dplyr_compile_with_schema_and_pipe_syntax(
                invalid_utf8.as_ptr().cast(),
                schema.as_ptr(),
                std::ptr::null(),
                DplyrPipeSyntax::Native as u32,
                &mut out_sql,
                &mut out_error,
            )
        };
        assert_eq!(result, DPLYR_ERROR_INVALID_UTF8);
        assert!(out_sql.is_null());
        unsafe { dplyr_free_string(out_error) };

        // Null schema.
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let result = unsafe {
            dplyr_compile_with_schema_and_pipe_syntax(
                code.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                DplyrPipeSyntax::Native as u32,
                &mut out_sql,
                &mut out_error,
            )
        };
        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);
        assert!(out_sql.is_null());
        unsafe { dplyr_free_string(out_error) };

        // Null SQL output slot.
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let result = unsafe {
            dplyr_compile_with_schema_and_pipe_syntax(
                code.as_ptr(),
                schema.as_ptr(),
                std::ptr::null(),
                DplyrPipeSyntax::Native as u32,
                std::ptr::null_mut(),
                &mut out_error,
            )
        };
        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);
    }

    #[test]
    fn explicit_pipe_respects_resource_limits() {
        let options = DplyrOptions {
            max_input_length: 64,
            ..DplyrOptions::default()
        };
        let long_code = format!("tbl |> filter(a > {})", "1".repeat(200));

        let (result, sql, error) = compile_with_pipe(
            &long_code,
            r#"{"source":"tbl","columns":[{"name":"a"}]}"#,
            Some(&options),
            Some(DplyrPipeSyntax::Native as u32),
        );

        assert_eq!(result, DPLYR_ERROR_INPUT_TOO_LARGE);
        assert!(sql.is_empty());
        assert!(error.contains("E-INPUT-TOO-LARGE"), "{error}");
    }

    #[test]
    fn explicit_pipe_schema_errors_still_map_to_c_codes() {
        let native = DplyrPipeSyntax::Native as u32;

        let (malformed, _, malformed_error) =
            compile_with_pipe("tbl |> filter(a > 1)", "{\"source\": ", None, Some(native));
        assert_eq!(malformed, DPLYR_ERROR_SYNTAX);
        assert!(malformed_error.contains("malformed"), "{malformed_error}");

        let (invalid, _, invalid_error) = compile_with_pipe(
            "tbl |> filter(a > 1)",
            r#"{"source":"tbl","columns":[]}"#,
            None,
            Some(native),
        );
        assert_eq!(invalid, DPLYR_ERROR_UNSUPPORTED);
        assert!(invalid_error.contains("Invalid schema"), "{invalid_error}");
    }

    #[test]
    fn array_of_schemas_compiles_a_join() {
        let schemas = r#"[
            {"source":"users","columns":[{"name":"id"},{"name":"name"}]},
            {"source":"orders","columns":[{"name":"user_id"},{"name":"total"}]}
        ]"#;

        let (result, sql, error) = compile(
            "users %>% inner_join(orders, by = c(\"id\" = \"user_id\"))",
            schemas,
            None,
        );

        assert_eq!(result, DPLYR_SUCCESS, "error: {error}");
        assert!(sql.contains("users") && sql.contains("orders"), "{sql}");
    }

    #[test]
    fn duplicate_sources_in_an_array_are_rejected() {
        let schemas = r#"[
            {"source":"users","columns":[{"name":"id"}]},
            {"source":"users","columns":[{"name":"id"}]}
        ]"#;

        let (result, sql, error) = compile("users %>% select(id)", schemas, None);

        assert_eq!(result, DPLYR_ERROR_UNSUPPORTED);
        assert!(sql.is_empty());
        assert!(error.contains("duplicate schema source"), "{error}");
    }

    #[test]
    fn empty_schema_array_is_rejected() {
        let (result, sql, error) = compile("users %>% select(id)", "[]", None);

        assert_eq!(result, DPLYR_ERROR_UNSUPPORTED);
        assert!(sql.is_empty());
        assert!(error.contains("at least one source schema"), "{error}");
    }

    fn required_sources(code: &str, pipe_syntax: u32) -> (i32, String, String) {
        let code = CString::new(code).expect("code has no NUL");
        let mut out_sources: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        let result = unsafe {
            dplyr_required_sources(code.as_ptr(), pipe_syntax, &mut out_sources, &mut out_error)
        };

        let take = |ptr: *mut c_char| -> String {
            if ptr.is_null() {
                return String::new();
            }
            let value = unsafe {
                let value = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                dplyr_free_string(ptr);
                value
            };
            value
        };

        (result, take(out_sources), take(out_error))
    }

    #[test]
    fn required_sources_returns_json_array_in_input_order() {
        let (result, sources, error) = required_sources(
            "users %>% inner_join(orders, by = c(\"id\" = \"user_id\")) %>% filter(total > 1)",
            DplyrPipeSyntax::Magrittr as u32,
        );

        assert_eq!(result, DPLYR_SUCCESS, "error: {error}");
        assert_eq!(sources, r#"["users","orders"]"#);
    }

    #[test]
    fn required_sources_deduplicates_repeated_sources() {
        let (result, sources, error) = required_sources(
            "users %>% filter(id > 1) %>% filter(name == 'a')",
            DplyrPipeSyntax::Magrittr as u32,
        );

        assert_eq!(result, DPLYR_SUCCESS, "error: {error}");
        assert_eq!(sources, r#"["users"]"#);
    }

    #[test]
    fn required_sources_without_an_explicit_source_is_an_error() {
        let (result, sources, error) =
            required_sources("filter(a > 1)", DplyrPipeSyntax::Magrittr as u32);

        assert_ne!(result, DPLYR_SUCCESS);
        assert!(sources.is_empty());
        assert!(!error.is_empty());
    }

    #[test]
    fn required_sources_validates_nulls_pipe_mode_and_utf8() {
        let code = CString::new("users %>% select(id)").expect("code has no NUL");

        let mut out_sources: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let null_code = unsafe {
            dplyr_required_sources(
                std::ptr::null(),
                DplyrPipeSyntax::Magrittr as u32,
                &mut out_sources,
                &mut out_error,
            )
        };
        assert_eq!(null_code, DPLYR_ERROR_NULL_POINTER);
        assert!(!out_error.is_null());
        unsafe { dplyr_free_string(out_error) };

        let null_slot = unsafe {
            dplyr_required_sources(
                code.as_ptr(),
                DplyrPipeSyntax::Magrittr as u32,
                std::ptr::null_mut(),
                &mut out_error,
            )
        };
        assert_eq!(null_slot, DPLYR_ERROR_NULL_POINTER);

        let mut out_sources: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let invalid_utf8 = [b'u', b's', b'e', b'r', b's', 0xF0, 0x9F, 1];
        let bad_utf8 = unsafe {
            dplyr_required_sources(
                invalid_utf8.as_ptr().cast(),
                DplyrPipeSyntax::Magrittr as u32,
                &mut out_sources,
                &mut out_error,
            )
        };
        assert_eq!(bad_utf8, DPLYR_ERROR_INVALID_UTF8);
        assert!(out_sources.is_null());
        unsafe { dplyr_free_string(out_error) };

        let (result, sources, error) = required_sources("users %>% select(id)", 99);
        assert_eq!(result, DPLYR_ERROR_SYNTAX);
        assert!(sources.is_empty());
        assert!(error.contains("Invalid pipe syntax value '99'"), "{error}");
    }

    #[test]
    fn required_sources_output_slot_is_cleared_on_reuse() {
        let code = CString::new("users %>% select(id)").expect("code has no NUL");
        let mut out_sources: *mut c_char =
            crate::memory::alloc_owned_string("stale sources").expect("allocation succeeds");
        let mut out_error: *mut c_char = std::ptr::null_mut();

        let result = unsafe {
            dplyr_required_sources(
                code.as_ptr(),
                DplyrPipeSyntax::Magrittr as u32,
                &mut out_sources,
                &mut out_error,
            )
        };

        assert_eq!(result, DPLYR_SUCCESS);
        assert!(out_error.is_null());
        assert!(!out_sources.is_null());
        unsafe {
            dplyr_free_string(out_sources);
        }
    }
}
