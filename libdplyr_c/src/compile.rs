//! Compile/transpile entrypoints.

#[cfg(test)]
use std::cell::Cell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use libdplyr::{
    DuckDbDialect, MySqlDialect, PipeSyntax, PostgreSqlDialect, SqlDialect, SqliteDialect,
    Transpiler,
};

use crate::cache;
use crate::cache::SimpleTranspileCache;
use crate::error::{create_error_message_with_context, TranspileError};
use crate::ffi::{clear_output_string, set_error_output, set_sql_output};
use crate::options::{
    DplyrDialect, DplyrOptions, DplyrPipeSyntax, MAX_OUTPUT_LENGTH, MAX_PROCESSING_TIME_MS,
};
use crate::validation::{
    validate_input_encoding, validate_input_security, validate_input_structure,
};

use crate::error::{
    DPLYR_ERROR_INPUT_TOO_LARGE, DPLYR_ERROR_INTERNAL, DPLYR_ERROR_INVALID_UTF8,
    DPLYR_ERROR_NULL_POINTER, DPLYR_ERROR_PANIC, DPLYR_QUERY_NOT_HANDLED, DPLYR_SUCCESS,
};

#[cfg(test)]
static FORCE_FFI_PANIC: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
static FFI_TEST_GATE: Mutex<()> = Mutex::new(());

#[cfg(test)]
thread_local! {
    static HOLDS_FFI_TEST_GATE: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
struct FfiTestGateGuard {
    _guard: Option<MutexGuard<'static, ()>>,
    reentrant: bool,
}

#[cfg(test)]
impl FfiTestGateGuard {
    fn acquire() -> Self {
        let reentrant = HOLDS_FFI_TEST_GATE.with(|holds| {
            let already_holding = holds.get();
            if !already_holding {
                holds.set(true);
            }
            already_holding
        });

        if reentrant {
            Self {
                _guard: None,
                reentrant: true,
            }
        } else {
            Self {
                _guard: Some(
                    FFI_TEST_GATE
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()),
                ),
                reentrant: false,
            }
        }
    }
}

#[cfg(test)]
impl Drop for FfiTestGateGuard {
    fn drop(&mut self) {
        if !self.reentrant {
            HOLDS_FFI_TEST_GATE.with(|holds| holds.set(false));
        }
    }
}

#[cfg(test)]
pub(crate) struct ForcedFfiPanicGuard {
    _gate: FfiTestGateGuard,
}

#[cfg(test)]
pub(crate) fn force_ffi_panic_for_test() -> ForcedFfiPanicGuard {
    let gate = FfiTestGateGuard::acquire();
    FORCE_FFI_PANIC.store(true, Ordering::SeqCst);
    ForcedFfiPanicGuard { _gate: gate }
}

#[cfg(test)]
impl Drop for ForcedFfiPanicGuard {
    fn drop(&mut self) {
        FORCE_FFI_PANIC.store(false, Ordering::SeqCst);
    }
}

fn maybe_force_test_panic() {
    #[cfg(test)]
    if FORCE_FFI_PANIC.load(Ordering::SeqCst) {
        panic!("forced ffi panic for testing");
    }
}

fn create_dialect(dialect: DplyrDialect) -> Box<dyn SqlDialect> {
    match dialect {
        DplyrDialect::DuckDb => Box::new(DuckDbDialect::new()),
        DplyrDialect::PostgreSql => Box::new(PostgreSqlDialect::new()),
        DplyrDialect::MySql => Box::new(MySqlDialect::new()),
        DplyrDialect::Sqlite => Box::new(SqliteDialect::new()),
    }
}

fn validated_dialect(raw_dialect: u32) -> Result<DplyrDialect, TranspileError> {
    DplyrDialect::try_from(raw_dialect)
}

fn validated_pipe_syntax(raw_pipe_syntax: u32) -> Result<PipeSyntax, TranspileError> {
    Ok(PipeSyntax::from(DplyrPipeSyntax::try_from(
        raw_pipe_syntax,
    )?))
}

fn pipe_syntax_from_env_or_default() -> Result<PipeSyntax, TranspileError> {
    static DEFAULT_PIPE_SYNTAX: OnceLock<Result<PipeSyntax, String>> = OnceLock::new();

    DEFAULT_PIPE_SYNTAX
        .get_or_init(PipeSyntax::from_env_or_default)
        .clone()
        .map_err(|message| TranspileError::internal_error(&message))
}

#[derive(Debug)]
enum CompileInputError {
    InputTooLarge(String),
    Transpile(TranspileError),
}

fn set_compile_error_output(out_error: *mut *mut c_char, error: CompileInputError) -> i32 {
    match error {
        CompileInputError::InputTooLarge(message) => {
            if set_error_output(out_error, &message) {
                DPLYR_ERROR_INPUT_TOO_LARGE
            } else {
                DPLYR_ERROR_INTERNAL
            }
        }
        CompileInputError::Transpile(error) => {
            let error_msg = error.to_c_string();
            if set_error_output(out_error, &error_msg.to_string_lossy()) {
                error.to_c_error_code()
            } else {
                DPLYR_ERROR_INTERNAL
            }
        }
    }
}

fn publish_error_or_internal(error_code: i32, out_error: *mut *mut c_char, message: &str) -> i32 {
    if set_error_output(out_error, message) {
        error_code
    } else {
        DPLYR_ERROR_INTERNAL
    }
}

fn publish_sql_or_internal_error(
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
    sql: &str,
) -> i32 {
    if set_sql_output(out_sql, sql) {
        DPLYR_SUCCESS
    } else {
        let _ = set_error_output(
            out_error,
            "E-INTERNAL: Failed to publish generated SQL across the FFI boundary",
        );
        DPLYR_ERROR_INTERNAL
    }
}

fn validate_compile_options(opts: &DplyrOptions) -> Result<(), CompileInputError> {
    opts.validate().map_err(CompileInputError::Transpile)
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
    validate_compile_options(opts)?;
    Ok(())
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

fn timeout_error(max_processing_time: Duration, phase: &str, hint: &str) -> TranspileError {
    TranspileError::internal_error_with_hint(
        &format!(
            "{} timeout: exceeded {}ms limit",
            phase,
            max_processing_time.as_millis()
        ),
        Some(hint.to_string()),
    )
}

fn ensure_before_deadline(
    deadline: Instant,
    max_processing_time: Duration,
    phase: &str,
    hint: &str,
) -> Result<(), TranspileError> {
    if Instant::now() > deadline {
        return Err(timeout_error(max_processing_time, phase, hint));
    }

    Ok(())
}

fn compile_to_sql_with_deadline(
    code_str: &str,
    opts: &DplyrOptions,
    pipe_syntax: PipeSyntax,
    deadline: Instant,
) -> Result<String, TranspileError> {
    let max_processing_time = processing_timeout(opts);

    ensure_before_deadline(
        deadline,
        max_processing_time,
        "Processing",
        "Reduce input complexity or increase timeout limit",
    )?;

    let cache_discriminator = format!("pipe-syntax:{}", pipe_syntax.config_value());
    let sql = SimpleTranspileCache::get_or_transpile_with_discriminator(
        code_str,
        opts,
        &cache_discriminator,
        |source_code, options| {
            ensure_before_deadline(
                deadline,
                max_processing_time,
                "Processing",
                "Reduce input complexity or increase timeout limit",
            )?;

            validate_input_security(source_code)?;

            let transpiler = Transpiler::with_pipe_syntax(
                create_dialect(validated_dialect(options.dialect)?),
                pipe_syntax,
            );
            let transpile_result = transpiler.transpile(source_code);

            ensure_before_deadline(
                deadline,
                max_processing_time,
                "Transpilation",
                "Input may be too complex for processing",
            )?;

            match transpile_result {
                Ok(sql) => Ok(sql),
                Err(libdplyr_error) => Err(convert_libdplyr_error(libdplyr_error)),
            }
        },
    )?;

    ensure_before_deadline(
        deadline,
        max_processing_time,
        "Processing",
        "Reduce input complexity or increase timeout limit",
    )?;
    validate_output_length(&sql)?;
    Ok(sql)
}

fn compile_to_sql(
    code_str: &str,
    opts: &DplyrOptions,
    pipe_syntax: PipeSyntax,
) -> Result<String, TranspileError> {
    compile_to_sql_with_deadline(code_str, opts, pipe_syntax, processing_deadline(opts))
}

fn finish_compile_code(
    code_str: &str,
    opts: &DplyrOptions,
    pipe_syntax: PipeSyntax,
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    if let Err(error) = validate_compile_input(code_str, opts) {
        return set_compile_error_output(out_error, error);
    }

    let transpile_result = compile_to_sql(code_str, opts, pipe_syntax);

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
                unsafe {
                    cache::dplyr_cache_log_stats_detailed(c"DEBUG_TRANSPILE".as_ptr(), true);
                }

                // R10-AC2: Log performance warning if cache is underperforming
                cache::dplyr_cache_log_performance_warning();
            }

            publish_sql_or_internal_error(out_sql, out_error, &sql)
        }
        Err(error) => {
            let error_msg = if opts.debug_mode {
                create_error_message_with_context(&error, Some(code_str))
            } else {
                error.to_c_string()
            };

            publish_error_or_internal(
                error.to_c_error_code(),
                out_error,
                &error_msg.to_string_lossy(),
            )
        }
    }
}

fn validate_output_length(sql: &str) -> Result<(), TranspileError> {
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

    Ok(())
}

fn strip_trailing_semicolon(input: &str) -> String {
    input
        .trim()
        .trim_end_matches(|c: char| c == ';' || c.is_whitespace())
        .to_string()
}

#[derive(Clone, Copy, Default)]
struct SqlScanConfig {
    hash_line_comments: bool,
    dollar_quoted_strings: bool,
    pipe_syntax: PipeSyntax,
}

fn scan_config_for_options(opts: &DplyrOptions, pipe_syntax: PipeSyntax) -> SqlScanConfig {
    SqlScanConfig {
        hash_line_comments: opts.dialect == DplyrDialect::MySql as u32,
        dollar_quoted_strings: opts.dialect == DplyrDialect::PostgreSql as u32,
        pipe_syntax,
    }
}

#[derive(Clone, Copy)]
enum SqlScanState {
    Normal,
    SingleQuoted,
    DoubleQuoted,
    BacktickQuoted,
    BracketQuoted(usize),
    DollarQuoted {
        delimiter_start: usize,
        delimiter_len: usize,
    },
    LineComment,
    BlockComment(usize),
}

#[cfg(test)]
fn find_pipe_operator(sql: &str, from: usize) -> Option<usize> {
    find_pipe_operator_with_config(sql, from, SqlScanConfig::default())
}

fn find_pipe_operator_with_config(sql: &str, from: usize, config: SqlScanConfig) -> Option<usize> {
    find_unquoted_sequence(sql, from, config.pipe_syntax.operator().as_bytes(), config)
}

fn find_disabled_pipe_operator_with_config(
    sql: &str,
    from: usize,
    config: SqlScanConfig,
) -> Option<usize> {
    find_unquoted_sequence(
        sql,
        from,
        config.pipe_syntax.opposite().operator().as_bytes(),
        config,
    )
}

fn advance_sql_scan(
    bytes: &[u8],
    i: usize,
    state: &mut SqlScanState,
    config: SqlScanConfig,
) -> usize {
    match *state {
        SqlScanState::Normal => match bytes[i] {
            b'\'' => {
                *state = SqlScanState::SingleQuoted;
                i + 1
            }
            b'"' => {
                *state = SqlScanState::DoubleQuoted;
                i + 1
            }
            b'`' => {
                *state = SqlScanState::BacktickQuoted;
                i + 1
            }
            b'[' => {
                *state = SqlScanState::BracketQuoted(1);
                i + 1
            }
            b'$' if config.dollar_quoted_strings => {
                if let Some(delimiter_len) = parse_dollar_quote_delimiter(bytes, i) {
                    *state = SqlScanState::DollarQuoted {
                        delimiter_start: i,
                        delimiter_len,
                    };
                    i + delimiter_len
                } else {
                    i + 1
                }
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                *state = SqlScanState::LineComment;
                i + 2
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                *state = SqlScanState::BlockComment(1);
                i + 2
            }
            b'#' if config.hash_line_comments => {
                *state = SqlScanState::LineComment;
                i + 1
            }
            _ => i + 1,
        },
        SqlScanState::SingleQuoted => {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i + 2
            } else if bytes[i] == b'\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i + 2
                } else {
                    *state = SqlScanState::Normal;
                    i + 1
                }
            } else {
                i + 1
            }
        }
        SqlScanState::DoubleQuoted => {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i + 2
            } else if bytes[i] == b'"' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                    i + 2
                } else {
                    *state = SqlScanState::Normal;
                    i + 1
                }
            } else {
                i + 1
            }
        }
        SqlScanState::BacktickQuoted => {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i + 2
            } else if bytes[i] == b'`' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'`' {
                    i + 2
                } else {
                    *state = SqlScanState::Normal;
                    i + 1
                }
            } else {
                i + 1
            }
        }
        SqlScanState::BracketQuoted(depth) => {
            if bytes[i] == b'[' {
                *state = SqlScanState::BracketQuoted(depth + 1);
                i + 1
            } else if bytes[i] == b']' {
                if depth == 1 && i + 1 < bytes.len() && bytes[i + 1] == b']' {
                    i + 2
                } else if depth == 1 {
                    *state = SqlScanState::Normal;
                    i + 1
                } else {
                    *state = SqlScanState::BracketQuoted(depth - 1);
                    i + 1
                }
            } else {
                i + 1
            }
        }
        SqlScanState::DollarQuoted {
            delimiter_start,
            delimiter_len,
        } => {
            let delimiter_end = delimiter_start + delimiter_len;
            if i + delimiter_len <= bytes.len()
                && bytes[i..].starts_with(&bytes[delimiter_start..delimiter_end])
            {
                *state = SqlScanState::Normal;
                i + delimiter_len
            } else {
                i + 1
            }
        }
        SqlScanState::LineComment => {
            if bytes[i] == b'\n' {
                *state = SqlScanState::Normal;
            }
            i + 1
        }
        SqlScanState::BlockComment(depth) => {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                *state = SqlScanState::BlockComment(depth + 1);
                i + 2
            } else if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                if depth == 1 {
                    *state = SqlScanState::Normal;
                } else {
                    *state = SqlScanState::BlockComment(depth - 1);
                }
                i + 2
            } else {
                i + 1
            }
        }
    }
}

fn parse_dollar_quote_delimiter(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start)? != &b'$' {
        return None;
    }

    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'$' => return Some(i + 1 - start),
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' => i += 1,
            _ => return None,
        }
    }

    None
}

fn find_unquoted_match<T, F>(
    sql: &str,
    from: usize,
    config: SqlScanConfig,
    mut matcher: F,
) -> Option<T>
where
    F: FnMut(&[u8], usize) -> Option<T>,
{
    if from >= sql.len() {
        return None;
    }

    let bytes = sql.as_bytes();
    let mut state = SqlScanState::Normal;
    let mut i = from;

    while i < bytes.len() {
        if matches!(state, SqlScanState::Normal) {
            if let Some(found) = matcher(bytes, i) {
                return Some(found);
            }
        }

        i = advance_sql_scan(bytes, i, &mut state, config);
    }

    None
}

fn find_unquoted_sequence(
    sql: &str,
    from: usize,
    needle: &[u8],
    config: SqlScanConfig,
) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    find_unquoted_match(sql, from, config, |bytes, i| {
        bytes[i..].starts_with(needle).then_some(i)
    })
}

fn split_identifier_chain(prefix: &str) -> Option<Vec<&str>> {
    if prefix.is_empty() {
        return None;
    }

    #[derive(Clone, Copy)]
    enum IdentifierQuote {
        Double,
        Backtick,
        Bracket(usize),
    }

    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = prefix.as_bytes();
    let mut quote = None;
    let mut i = 0usize;

    while i < bytes.len() {
        match quote {
            Some(IdentifierQuote::Double) => {
                if bytes[i] == b'"' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                        i += 2;
                    } else {
                        quote = None;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            Some(IdentifierQuote::Backtick) => {
                if bytes[i] == b'`' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'`' {
                        i += 2;
                    } else {
                        quote = None;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            Some(IdentifierQuote::Bracket(depth)) => {
                if bytes[i] == b'[' {
                    quote = Some(IdentifierQuote::Bracket(depth + 1));
                    i += 1;
                } else if bytes[i] == b']' {
                    if depth == 1 && i + 1 < bytes.len() && bytes[i + 1] == b']' {
                        i += 2;
                    } else if depth == 1 {
                        quote = None;
                        i += 1;
                    } else {
                        quote = Some(IdentifierQuote::Bracket(depth - 1));
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            None => match bytes[i] {
                b'"' => {
                    quote = Some(IdentifierQuote::Double);
                    i += 1;
                }
                b'`' => {
                    quote = Some(IdentifierQuote::Backtick);
                    i += 1;
                }
                b'[' => {
                    quote = Some(IdentifierQuote::Bracket(1));
                    i += 1;
                }
                b'.' => {
                    parts.push(&prefix[start..i]);
                    start = i + 1;
                    i += 1;
                }
                _ => i += 1,
            },
        }
    }

    if quote.is_some() {
        return None;
    }

    parts.push(&prefix[start..]);
    Some(parts)
}

fn is_probably_identifier_chain(prefix: &str) -> bool {
    let Some(parts) = split_identifier_chain(prefix) else {
        return false;
    };

    let mut part_count = 0;
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            return false;
        }
        part_count += 1;

        let quoted = (part.starts_with('"') && part.ends_with('"'))
            || (part.starts_with('`') && part.ends_with('`'))
            || (part.starts_with('[') && part.ends_with(']'));

        if quoted {
            if part.len() < 2 {
                return false;
            }
            continue;
        }

        if !part
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return false;
        }
    }

    part_count > 0
}

fn extract_leading_table_name_with_config(dplyr_code: &str, config: SqlScanConfig) -> Option<&str> {
    let pipe_pos = find_pipe_operator_with_config(dplyr_code, 0, config);
    let prefix = match pipe_pos {
        Some(pos) => &dplyr_code[..pos],
        None => dplyr_code,
    }
    .trim();

    if prefix.is_empty() {
        return None;
    }

    if is_probably_identifier_chain(prefix) {
        Some(prefix)
    } else {
        None
    }
}

fn require_pipeline_table_name_with_config(
    dplyr_code: &str,
    config: SqlScanConfig,
) -> Result<(), TranspileError> {
    if extract_leading_table_name_with_config(dplyr_code, config).is_none() {
        return Err(TranspileError::syntax_error_with_suggestion(
            "DPLYR pipeline must start with a table name",
            0,
            None,
            Some(format!(
                "Start the pipeline with a source table before {}",
                config.pipe_syntax.operator()
            )),
        ));
    }
    Ok(())
}

#[cfg(test)]
fn find_embedded_start_marker(query: &str, from: usize) -> Option<(usize, usize)> {
    find_embedded_start_marker_with_config(query, from, SqlScanConfig::default())
}

fn find_embedded_start_marker_with_config(
    query: &str,
    from: usize,
    config: SqlScanConfig,
) -> Option<(usize, usize)> {
    find_unquoted_match(query, from, config, |bytes, i| {
        if bytes[i] != b'(' {
            return None;
        }

        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        (j < bytes.len() && bytes[j] == b'|').then_some((i, j + 1))
    })
}

fn find_embedded_end_marker_with_config(
    query: &str,
    from: usize,
    config: SqlScanConfig,
) -> Option<(usize, usize)> {
    find_unquoted_match(query, from, config, |bytes, i| {
        if bytes[i] != b'|' {
            return None;
        }

        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        (j < bytes.len() && bytes[j] == b')').then_some((i, j))
    })
}

fn replace_embedded_pipelines_with_deadline(
    query: &str,
    opts: &DplyrOptions,
    deadline: Instant,
    scan_config: SqlScanConfig,
) -> Result<String, CompileInputError> {
    let mut output = String::with_capacity(query.len());
    let mut cursor = 0;

    while cursor < query.len() {
        let Some((marker_start, content_start)) =
            find_embedded_start_marker_with_config(query, cursor, scan_config)
        else {
            output.push_str(&query[cursor..]);
            break;
        };

        output.push_str(&query[cursor..marker_start]);
        let Some((content_end, marker_end)) =
            find_embedded_end_marker_with_config(query, content_start, scan_config)
        else {
            return Err(CompileInputError::Transpile(
                TranspileError::syntax_error_with_suggestion(
                    "Unterminated embedded dplyr segment",
                    marker_start,
                    None,
                    Some("Close embedded pipelines with '|)'.".to_string()),
                ),
            ));
        };

        let embedded =
            strip_trailing_semicolon(strip_leading_sql_comments_and_whitespace_with_config(
                &query[content_start..content_end],
                scan_config,
            ));
        if embedded.is_empty() {
            return Err(CompileInputError::Transpile(
                TranspileError::syntax_error_with_suggestion(
                    "Embedded dplyr segment cannot be empty",
                    content_start,
                    None,
                    None,
                ),
            ));
        }
        if find_pipe_operator_with_config(&embedded, 0, scan_config).is_none() {
            return Err(CompileInputError::Transpile(
                TranspileError::syntax_error_with_suggestion(
                    &format!(
                        "Embedded dplyr segment must contain a {} pipeline",
                        scan_config.pipe_syntax.operator()
                    ),
                    content_start,
                    None,
                    None,
                ),
            ));
        }

        validate_compile_input(&embedded, opts)?;
        require_pipeline_table_name_with_config(&embedded, scan_config)
            .map_err(CompileInputError::Transpile)?;
        let sql = compile_to_sql_with_deadline(&embedded, opts, scan_config.pipe_syntax, deadline)
            .map_err(CompileInputError::Transpile)?;
        output.push('(');
        output.push_str(&sql);
        output.push(')');

        cursor = marker_end + 1;
    }

    Ok(output)
}

#[cfg(test)]
fn strip_leading_sql_comments_and_whitespace(sql: &str) -> &str {
    strip_leading_sql_comments_and_whitespace_with_config(sql, SqlScanConfig::default())
}

fn strip_leading_sql_comments_and_whitespace_with_config(
    mut sql: &str,
    config: SqlScanConfig,
) -> &str {
    loop {
        sql = sql.trim_start();

        if let Some(rest) = sql.strip_prefix("--") {
            sql = rest.find('\n').map_or("", |pos| &rest[pos + 1..]);
            continue;
        }

        if config.hash_line_comments {
            if let Some(rest) = sql.strip_prefix('#') {
                sql = rest.find('\n').map_or("", |pos| &rest[pos + 1..]);
                continue;
            }
        }

        if let Some(rest) = sql.strip_prefix("/*") {
            let bytes = rest.as_bytes();
            let mut depth = 1usize;
            let mut i = 0usize;

            while i + 1 < bytes.len() && depth > 0 {
                if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }

            sql = if depth == 0 { &rest[i..] } else { "" };
            continue;
        }

        return sql;
    }
}

fn has_sql_keyword_prefix(sql: &str, keyword: &str) -> bool {
    let Some(head) = sql.get(..keyword.len()) else {
        return false;
    };
    if !head.eq_ignore_ascii_case(keyword) {
        return false;
    }

    sql[keyword.len()..]
        .chars()
        .next()
        .map(|c| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .unwrap_or(true)
}

#[cfg(test)]
fn starts_with_supported_query_prefix(sql: &str) -> bool {
    starts_with_supported_query_prefix_with_config(sql, SqlScanConfig::default())
}

fn starts_with_supported_query_prefix_with_config(sql: &str, config: SqlScanConfig) -> bool {
    let sql = strip_leading_sql_comments_and_whitespace_with_config(sql, config);
    sql.starts_with('(')
        || ["SELECT", "WITH"]
            .iter()
            .any(|prefix| has_sql_keyword_prefix(sql, prefix))
}

fn compile_query_string_with_deadline(
    query: &str,
    opts: &DplyrOptions,
    pipe_syntax: PipeSyntax,
    deadline: Instant,
) -> Result<Option<String>, CompileInputError> {
    let trimmed = query.trim();
    let scan_config = scan_config_for_options(opts, pipe_syntax);

    if trimmed.is_empty() || find_pipe_operator_with_config(trimmed, 0, scan_config).is_none() {
        if find_disabled_pipe_operator_with_config(trimmed, 0, scan_config).is_some() {
            let disabled_syntax = scan_config.pipe_syntax.opposite();
            return Err(CompileInputError::Transpile(
                TranspileError::syntax_error_with_suggestion(
                    disabled_syntax.disabled_message(),
                    0,
                    None,
                    Some(disabled_syntax.disabled_suggestion()),
                ),
            ));
        }
        return Ok(None);
    }

    let sql = if find_embedded_start_marker_with_config(trimmed, 0, scan_config).is_some() {
        let rewritten =
            replace_embedded_pipelines_with_deadline(trimmed, opts, deadline, scan_config)?;
        if find_pipe_operator_with_config(&rewritten, 0, scan_config).is_some() {
            return Err(CompileInputError::Transpile(
                TranspileError::syntax_error_with_suggestion(
                    &format!(
                        "Unprocessed {} pipeline remains",
                        scan_config.pipe_syntax.operator()
                    ),
                    0,
                    None,
                    Some(
                        "Wrap pipelines with (| ... |) or provide a pure pipeline statement."
                            .to_string(),
                    ),
                ),
            ));
        }
        rewritten
    } else {
        let dplyr_code = strip_trailing_semicolon(
            strip_leading_sql_comments_and_whitespace_with_config(trimmed, scan_config),
        );
        validate_compile_input(&dplyr_code, opts)?;
        require_pipeline_table_name_with_config(&dplyr_code, scan_config)
            .map_err(CompileInputError::Transpile)?;
        compile_to_sql_with_deadline(&dplyr_code, opts, pipe_syntax, deadline)
            .map_err(CompileInputError::Transpile)?
    };

    validate_output_length(&sql).map_err(CompileInputError::Transpile)?;

    if !starts_with_supported_query_prefix_with_config(&sql, scan_config) {
        return Err(CompileInputError::Transpile(
            TranspileError::unsupported_operation_with_alternative(
                "generated a non-SELECT statement",
                "query compilation",
                Some("Only SELECT/WITH statements are supported for parser rewrite".to_string()),
            ),
        ));
    }

    Ok(Some(sql))
}

fn finish_compile_query(
    query_str: &str,
    opts: &DplyrOptions,
    pipe_syntax: PipeSyntax,
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    if let Err(error) = validate_compile_options(opts) {
        return set_compile_error_output(out_error, error);
    }

    let scan_config = scan_config_for_options(opts, pipe_syntax);
    let has_pipeline = find_pipe_operator_with_config(query_str, 0, scan_config).is_some();
    let has_disabled_pipeline =
        find_disabled_pipe_operator_with_config(query_str, 0, scan_config).is_some();
    if !has_pipeline && !has_disabled_pipeline {
        return DPLYR_QUERY_NOT_HANDLED;
    }

    if query_str.len() > opts.max_input_length as usize {
        return publish_error_or_internal(
            DPLYR_ERROR_INPUT_TOO_LARGE,
            out_error,
            &format!(
                "E-INPUT-TOO-LARGE: Input size {} exceeds maximum {}",
                query_str.len(),
                opts.max_input_length
            ),
        );
    }

    let trimmed_query = query_str.trim();
    match compile_query_string_with_deadline(
        trimmed_query,
        opts,
        pipe_syntax,
        processing_deadline(opts),
    ) {
        Ok(Some(sql)) => publish_sql_or_internal_error(out_sql, out_error, &sql),
        Ok(None) => DPLYR_QUERY_NOT_HANDLED,
        Err(CompileInputError::InputTooLarge(message)) => {
            publish_error_or_internal(DPLYR_ERROR_INPUT_TOO_LARGE, out_error, &message)
        }
        Err(CompileInputError::Transpile(error)) => {
            let error_msg = if opts.debug_mode {
                create_error_message_with_context(&error, Some(query_str))
            } else {
                error.to_c_string()
            };
            publish_error_or_internal(
                error.to_c_error_code(),
                out_error,
                &error_msg.to_string_lossy(),
            )
        }
    }
}

/// Compile dplyr code to SQL
///
/// # Safety
/// Caller must ensure that:
/// - `code` is a valid null-terminated C string.
/// - `options` is a valid pointer to a `DplyrOptions` struct, or `std::ptr::null()` if default options are desired.
/// - `out_sql` and `out_error` are valid mutable pointers to `*mut c_char` where results can be stored.
/// - On entry, `*out_sql` and `*out_error` must be either null or pointers previously allocated by libdplyr.
///   Ownership of any non-null incoming libdplyr pointer is transferred back to this function.
/// - Any `*mut c_char` returned must be freed using `dplyr_free_string`.
/// - If the function returns `DPLYR_ERROR_PANIC`, callers must not assume `*out_error` was populated.
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
    #[cfg(test)]
    let _test_gate = FfiTestGateGuard::acquire();

    // R9-AC1: Panic safety - catch all panics at FFI boundary
    let result = panic::catch_unwind(|| {
        if out_sql.is_null() || out_error.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        clear_output_string(out_sql);
        clear_output_string(out_error);
        maybe_force_test_panic();

        // R9-AC2: Input validation - check for null pointers
        if code.is_null() {
            return publish_error_or_internal(
                DPLYR_ERROR_NULL_POINTER,
                out_error,
                "E-NULL-POINTER: code parameter is null",
            );
        }

        // Convert C string to Rust string with UTF-8 validation
        let code_str = match unsafe { CStr::from_ptr(code) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                return publish_error_or_internal(
                    DPLYR_ERROR_INVALID_UTF8,
                    out_error,
                    "E-INVALID-UTF8: Input code contains invalid UTF-8",
                );
            }
        };

        // Get options (use default if null)
        let opts = if options.is_null() {
            DplyrOptions::default()
        } else {
            unsafe { (*options).clone() }
        };

        let pipe_syntax = match pipe_syntax_from_env_or_default() {
            Ok(pipe_syntax) => pipe_syntax,
            Err(error) => {
                return set_compile_error_output(out_error, CompileInputError::Transpile(error))
            }
        };

        finish_compile_code(code_str, &opts, pipe_syntax, out_sql, out_error)
    });

    result.unwrap_or(DPLYR_ERROR_PANIC)
}

#[no_mangle]
pub unsafe extern "C" fn dplyr_compile_with_pipe_syntax(
    code: *const c_char,
    options: *const DplyrOptions,
    pipe_syntax: u32,
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    #[cfg(test)]
    let _test_gate = FfiTestGateGuard::acquire();

    let result = panic::catch_unwind(|| {
        if out_sql.is_null() || out_error.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        clear_output_string(out_sql);
        clear_output_string(out_error);
        maybe_force_test_panic();

        if code.is_null() {
            return publish_error_or_internal(
                DPLYR_ERROR_NULL_POINTER,
                out_error,
                "E-NULL-POINTER: code parameter is null",
            );
        }

        let code_str = match unsafe { CStr::from_ptr(code) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                return publish_error_or_internal(
                    DPLYR_ERROR_INVALID_UTF8,
                    out_error,
                    "E-INVALID-UTF8: Input code contains invalid UTF-8",
                );
            }
        };

        let opts = if options.is_null() {
            DplyrOptions::default()
        } else {
            unsafe { (*options).clone() }
        };

        let pipe_syntax = match validated_pipe_syntax(pipe_syntax) {
            Ok(pipe_syntax) => pipe_syntax,
            Err(error) => {
                return set_compile_error_output(out_error, CompileInputError::Transpile(error))
            }
        };

        finish_compile_code(code_str, &opts, pipe_syntax, out_sql, out_error)
    });

    result.unwrap_or(DPLYR_ERROR_PANIC)
}

#[no_mangle]
/// Compile a DuckDB query string, rewriting dplyr pipelines when present.
///
/// # Safety
/// Caller must ensure that:
/// - `query` is a valid null-terminated C string.
/// - `options` is a valid pointer to a `DplyrOptions` struct, or `std::ptr::null()`.
/// - `out_sql` and `out_error` are valid mutable pointers to `*mut c_char`.
/// - On entry, `*out_sql` and `*out_error` must be either null or pointers previously allocated by libdplyr.
///   Ownership of any non-null incoming libdplyr pointer is transferred back to this function.
/// - Any returned string pointer is freed with `dplyr_free_string`.
/// - If the function returns `DPLYR_ERROR_PANIC`, callers must not assume `*out_error` was populated.
pub unsafe extern "C" fn dplyr_compile_query(
    query: *const c_char,
    options: *const DplyrOptions,
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    #[cfg(test)]
    let _test_gate = FfiTestGateGuard::acquire();

    let result = panic::catch_unwind(|| {
        if out_sql.is_null() || out_error.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        clear_output_string(out_sql);
        clear_output_string(out_error);
        maybe_force_test_panic();

        if query.is_null() {
            return publish_error_or_internal(
                DPLYR_ERROR_NULL_POINTER,
                out_error,
                "E-NULL-POINTER: query parameter is null",
            );
        }

        let query_str = match unsafe { CStr::from_ptr(query) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                return publish_error_or_internal(
                    DPLYR_ERROR_INVALID_UTF8,
                    out_error,
                    "E-INVALID-UTF8: Input query contains invalid UTF-8",
                );
            }
        };

        let opts = if options.is_null() {
            DplyrOptions::default()
        } else {
            unsafe { (*options).clone() }
        };

        let pipe_syntax = match pipe_syntax_from_env_or_default() {
            Ok(pipe_syntax) => pipe_syntax,
            Err(error) => {
                return set_compile_error_output(out_error, CompileInputError::Transpile(error))
            }
        };

        finish_compile_query(query_str, &opts, pipe_syntax, out_sql, out_error)
    });

    result.unwrap_or(DPLYR_ERROR_PANIC)
}

#[no_mangle]
pub unsafe extern "C" fn dplyr_compile_query_with_pipe_syntax(
    query: *const c_char,
    options: *const DplyrOptions,
    pipe_syntax: u32,
    out_sql: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    #[cfg(test)]
    let _test_gate = FfiTestGateGuard::acquire();

    let result = panic::catch_unwind(|| {
        if out_sql.is_null() || out_error.is_null() {
            return DPLYR_ERROR_NULL_POINTER;
        }

        clear_output_string(out_sql);
        clear_output_string(out_error);
        maybe_force_test_panic();

        if query.is_null() {
            return publish_error_or_internal(
                DPLYR_ERROR_NULL_POINTER,
                out_error,
                "E-NULL-POINTER: query parameter is null",
            );
        }

        let query_str = match unsafe { CStr::from_ptr(query) }.to_str() {
            Ok(s) => s,
            Err(_) => {
                return publish_error_or_internal(
                    DPLYR_ERROR_INVALID_UTF8,
                    out_error,
                    "E-INVALID-UTF8: Input query contains invalid UTF-8",
                );
            }
        };

        let opts = if options.is_null() {
            DplyrOptions::default()
        } else {
            unsafe { (*options).clone() }
        };

        let pipe_syntax = match validated_pipe_syntax(pipe_syntax) {
            Ok(pipe_syntax) => pipe_syntax,
            Err(error) => {
                return set_compile_error_output(out_error, CompileInputError::Transpile(error))
            }
        };

        finish_compile_query(query_str, &opts, pipe_syntax, out_sql, out_error)
    });

    result.unwrap_or(DPLYR_ERROR_PANIC)
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

#[cfg(test)]
mod query_rewrite_tests {
    use super::*;

    #[test]
    fn identifier_chain_accepts_quoted_segments_with_embedded_dots() {
        assert!(is_probably_identifier_chain("schema.\"table.name\""));
        assert!(is_probably_identifier_chain("`catalog.with.dot`.orders"));
        assert!(is_probably_identifier_chain(
            "[schema.with.dot].[table.name]"
        ));
    }

    #[test]
    fn identifier_chain_accepts_unicode_identifiers() {
        assert!(is_probably_identifier_chain("데이터_원본"));
        assert!(is_probably_identifier_chain("스키마.테이블"));
    }

    #[test]
    fn identifier_chain_rejects_unterminated_quotes() {
        assert!(!is_probably_identifier_chain("\"broken.table"));
        assert!(!is_probably_identifier_chain("[broken.table"));
    }

    #[test]
    fn identifier_chain_accepts_escaped_quotes_inside_identifiers() {
        assert!(is_probably_identifier_chain("\"table\"\"name\""));
        assert!(is_probably_identifier_chain("`table``name`.col"));
        assert!(is_probably_identifier_chain("[table]]name].[col]"));
    }

    #[test]
    fn identifier_chain_accepts_nested_bracket_segments() {
        assert!(is_probably_identifier_chain("[arr[arr2[1]]].value"));
    }

    #[test]
    fn pipe_operator_detection_ignores_literals_and_comments() {
        assert_eq!(find_pipe_operator("SELECT '%>%'", 0), None);
        assert_eq!(
            find_pipe_operator(r"SELECT 'value with escaped quote \' and %>%' AS marker", 0),
            None
        );
        assert_eq!(find_pipe_operator("SELECT 1 -- %>%\nFROM tbl", 0), None);
        assert_eq!(find_pipe_operator("/* %>% */ SELECT 1", 0), None);
        assert_eq!(find_pipe_operator("SELECT [arr[%>%]] FROM tbl", 0), None);
        assert!(find_pipe_operator("tbl %>% select(col)", 0).is_some());
    }

    #[test]
    fn pipe_operator_detection_ignores_mysql_hash_comments_when_enabled() {
        let config = SqlScanConfig {
            hash_line_comments: true,
            dollar_quoted_strings: false,
            ..Default::default()
        };

        assert_eq!(
            find_pipe_operator_with_config("# %>%\nSELECT 1", 0, config),
            None
        );
        assert!(find_pipe_operator_with_config("tbl %>% select(col)", 0, config).is_some());
    }

    #[test]
    fn embedded_marker_detection_ignores_literals_and_comments() {
        assert_eq!(find_embedded_start_marker("SELECT '(|' AS marker", 0), None);
        assert_eq!(
            find_embedded_start_marker("SELECT 1 /* (| */ FROM tbl", 0),
            None
        );
        assert!(find_embedded_start_marker("SELECT * FROM (| tbl %>% select(col) |)", 0).is_some());
    }

    #[test]
    fn strip_leading_comments_handles_nested_block_comments() {
        let sql = "/* outer /* nested */ still outer */ SELECT 1";
        assert_eq!(strip_leading_sql_comments_and_whitespace(sql), "SELECT 1");
    }

    #[test]
    fn supported_query_prefix_requires_keyword_boundary() {
        assert!(starts_with_supported_query_prefix("SELECT * FROM tbl"));
        assert!(starts_with_supported_query_prefix(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ));
        assert!(!starts_with_supported_query_prefix("SELECTED * FROM tbl"));
        assert!(!starts_with_supported_query_prefix("WITHIN grp AS value"));
    }

    #[test]
    fn supported_query_prefix_ignores_mysql_hash_comments_when_enabled() {
        let config = SqlScanConfig {
            hash_line_comments: true,
            dollar_quoted_strings: false,
            ..Default::default()
        };

        assert!(starts_with_supported_query_prefix_with_config(
            "# rewritten mysql comment\nSELECT * FROM tbl",
            config
        ));
    }

    #[test]
    fn pipe_operator_detection_ignores_postgresql_dollar_quoted_literals() {
        let config = SqlScanConfig {
            hash_line_comments: false,
            dollar_quoted_strings: true,
            ..Default::default()
        };

        assert_eq!(
            find_pipe_operator_with_config("SELECT $$ %>% $$ AS marker", 0, config),
            None
        );
        assert_eq!(
            find_pipe_operator_with_config("SELECT $tag$(| %>% |)$tag$ AS marker", 0, config),
            None
        );
        assert!(find_pipe_operator_with_config("tbl %>% select(col)", 0, config).is_some());
    }

    #[test]
    fn embedded_marker_detection_ignores_postgresql_dollar_quoted_literals() {
        let config = SqlScanConfig {
            hash_line_comments: false,
            dollar_quoted_strings: true,
            ..Default::default()
        };

        assert_eq!(
            find_embedded_start_marker_with_config(
                "SELECT $tag$(| %>% |)$tag$ AS marker",
                0,
                config
            ),
            None
        );
        assert_eq!(
            find_embedded_end_marker_with_config("SELECT $$ |) $$ AS marker", 0, config),
            None
        );
    }

    #[test]
    fn validate_output_length_rejects_excessive_sql() {
        let oversized = "x".repeat(MAX_OUTPUT_LENGTH + 1);
        let error = validate_output_length(&oversized).expect_err("oversized SQL must fail");
        assert!(error.to_string().contains("Output too large"));
    }

    #[test]
    fn embedded_pipeline_rewrite_respects_shared_deadline() {
        let opts = DplyrOptions::default();
        let deadline = Instant::now() - Duration::from_millis(1);
        let result = replace_embedded_pipelines_with_deadline(
            "(| mtcars %>% select(mpg) |) UNION ALL (| mtcars %>% select(cyl) |)",
            &opts,
            deadline,
            scan_config_for_options(&opts, PipeSyntax::Magrittr),
        );

        match result {
            Err(CompileInputError::Transpile(error)) => {
                assert!(error.to_c_string().to_string_lossy().contains("timeout"));
            }
            other => panic!("expected timeout error, got {other:?}"),
        }
    }
}
