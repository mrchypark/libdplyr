use crate::{DuckDbDialect, TranspileError, Transpiler};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Result structure for C API
#[repr(C)]
pub struct DplyrTranspileResult {
    /// Pointer to the transpiled SQL string on success (NULL on failure)
    pub output_sql: *mut c_char,
    /// Pointer to the error message on failure (NULL on success)
    pub error_msg: *mut c_char,
}

/// Transpiles dplyr code to DuckDB SQL via C API
///
/// # Arguments
/// * `dplyr_src` - Null-terminated C string containing dplyr code
/// * `dialect` - Null-terminated C string specifying the SQL dialect (currently "duckdb" only)
///
/// # Returns
/// A DplyrTranspileResult containing either the transpiled SQL or an error message.
/// The caller is responsible for freeing both pointers using `dplyr_free`.
///
/// # Safety
/// This function is unsafe because it deals with raw C pointers.
/// The caller must ensure that both input pointers are valid null-terminated C strings.
/// The returned pointers must be freed with `dplyr_free` to avoid memory leaks.
#[no_mangle]
pub unsafe extern "C" fn dplyr_to_sql(
    dplyr_src: *const c_char,
    dialect: *const c_char,
) -> DplyrTranspileResult {
    // Convert C strings to Rust strings
    let dplyr_str = match CStr::from_ptr(dplyr_src).to_str() {
        Ok(s) => s,
        Err(_) => {
            let error_msg = CString::new("Invalid UTF-8 in dplyr source").unwrap();
            return DplyrTranspileResult {
                output_sql: std::ptr::null_mut(),
                error_msg: error_msg.into_raw(),
            };
        }
    };

    let dialect_str = match CStr::from_ptr(dialect).to_str() {
        Ok(s) => s,
        Err(_) => {
            let error_msg = CString::new("Invalid UTF-8 in dialect").unwrap();
            return DplyrTranspileResult {
                output_sql: std::ptr::null_mut(),
                error_msg: error_msg.into_raw(),
            };
        }
    };

    // Only support DuckDB dialect for now
    if dialect_str != "duckdb" {
        let error_msg = CString::new(format!("Unsupported dialect: {}", dialect_str)).unwrap();
        return DplyrTranspileResult {
            output_sql: std::ptr::null_mut(),
            error_msg: error_msg.into_raw(),
        };
    }

    // Create transpiler with DuckDB dialect
    let transpiler = Transpiler::new(Box::new(DuckDbDialect::new()));

    // Perform transpilation
    match transpiler.transpile(dplyr_str) {
        Ok(sql) => match CString::new(sql) {
            Ok(cstr) => DplyrTranspileResult {
                output_sql: cstr.into_raw(),
                error_msg: std::ptr::null_mut(),
            },
            Err(_) => {
                let error_msg =
                    CString::new("Failed to create null-terminated SQL string").unwrap();
                DplyrTranspileResult {
                    output_sql: std::ptr::null_mut(),
                    error_msg: error_msg.into_raw(),
                }
            }
        },
        Err(err) => {
            let error_msg = match err {
                TranspileError::LexError(e) => format!("DPLYR-TRANSPILE: Lexical error: {}", e),
                TranspileError::ParseError(e) => format!("DPLYR-TRANSPILE: Parse error: {}", e),
                TranspileError::GenerationError(e) => {
                    format!("DPLYR-TRANSPILE: Generation error: {}", e)
                }
                TranspileError::IoError(e) => format!("DPLYR-TRANSPILE: I/O error: {}", e),
                TranspileError::ValidationError(e) => {
                    format!("DPLYR-TRANSPILE: Validation error: {}", e)
                }
                TranspileError::ConfigurationError(e) => {
                    format!("DPLYR-TRANSPILE: Configuration error: {}", e)
                }
                TranspileError::SystemError(e) => format!("DPLYR-TRANSPILE: System error: {}", e),
            };

            match CString::new(error_msg) {
                Ok(cstr) => DplyrTranspileResult {
                    output_sql: std::ptr::null_mut(),
                    error_msg: cstr.into_raw(),
                },
                Err(_) => {
                    let fallback_msg =
                        CString::new("DPLYR-TRANSPILE: Unknown error occurred").unwrap();
                    DplyrTranspileResult {
                        output_sql: std::ptr::null_mut(),
                        error_msg: fallback_msg.into_raw(),
                    }
                }
            }
        }
    }
}

/// Frees memory allocated by the C API
///
/// # Arguments
/// * `ptr` - Pointer to the memory to free (can be NULL)
///
/// # Safety
/// This function is unsafe because it deals with raw C pointers.
/// The pointer must have been allocated by `dplyr_to_sql` or be NULL.
/// After calling this function, the pointer becomes invalid.
#[no_mangle]
pub unsafe extern "C" fn dplyr_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        // Convert back to CString to reclaim ownership and drop
        let _ = CString::from_raw(ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_dplyr_to_sql_success() {
        let dplyr_code = "select(name, age) %>% filter(age > 18)";
        let dialect = "duckdb";

        let dplyr_cstr = CString::new(dplyr_code).unwrap();
        let dialect_cstr = CString::new(dialect).unwrap();

        unsafe {
            let result = dplyr_to_sql(dplyr_cstr.as_ptr(), dialect_cstr.as_ptr());

            assert!(!result.output_sql.is_null());
            assert!(result.error_msg.is_null());

            // Convert back to check content
            let sql = CStr::from_ptr(result.output_sql).to_str().unwrap();
            assert!(sql.contains("SELECT"));
            assert!(sql.contains("WHERE"));

            // Clean up
            dplyr_free(result.output_sql);
        }
    }

    #[test]
    fn test_dplyr_to_sql_error() {
        let invalid_code = "@#$%invalid";
        let dialect = "duckdb";

        let code_cstr = CString::new(invalid_code).unwrap();
        let dialect_cstr = CString::new(dialect).unwrap();

        unsafe {
            let result = dplyr_to_sql(code_cstr.as_ptr(), dialect_cstr.as_ptr());

            assert!(result.output_sql.is_null());
            assert!(!result.error_msg.is_null());

            // Convert back to check content
            let error = CStr::from_ptr(result.error_msg).to_str().unwrap();
            assert!(error.contains("DPLYR-TRANSPILE:"));

            // Clean up
            dplyr_free(result.error_msg);
        }
    }

    #[test]
    fn test_dplyr_to_sql_unsupported_dialect() {
        let dplyr_code = "select(name)";
        let dialect = "unsupported";

        let code_cstr = CString::new(dplyr_code).unwrap();
        let dialect_cstr = CString::new(dialect).unwrap();

        unsafe {
            let result = dplyr_to_sql(code_cstr.as_ptr(), dialect_cstr.as_ptr());

            assert!(result.output_sql.is_null());
            assert!(!result.error_msg.is_null());

            // Convert back to check content
            let error = CStr::from_ptr(result.error_msg).to_str().unwrap();
            assert!(error.contains("Unsupported dialect"));

            // Clean up
            dplyr_free(result.error_msg);
        }
    }

    #[test]
    fn test_dplyr_free_null() {
        unsafe {
            // Should not panic with NULL pointer
            dplyr_free(std::ptr::null_mut());
        }
    }
}
