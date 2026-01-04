//! FFI boundary helpers.
//!
//! This module collects small, reusable helpers used by exported `extern "C"`
//! entrypoints. It is intentionally minimal so code can be moved into it
//! incrementally (“Tidy First”).

use std::ffi::CString;
use std::os::raw::c_char;

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
