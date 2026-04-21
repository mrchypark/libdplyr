//! FFI boundary helpers.
//!
//! This module collects small, reusable helpers used by exported `extern "C"`
//! entrypoints. It is intentionally minimal so code can be moved into it
//! incrementally (“Tidy First”).

use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;

use crate::error::{DPLYR_ERROR_NULL_POINTER, DPLYR_SUCCESS};
use crate::memory::free_owned_string;

/// Set SQL output pointer safely
pub fn set_sql_output(out_sql: *mut *mut c_char, sql: &str) {
    if !out_sql.is_null() {
        if let Ok(c_string) = CString::new(sql) {
            unsafe {
                *out_sql = c_string.into_raw();
            }
        }
    }
}

/// Set error output pointer safely
pub fn set_error_output(out_error: *mut *mut c_char, error: &str) {
    if !out_error.is_null() {
        if let Ok(c_string) = CString::new(error) {
            unsafe {
                *out_error = c_string.into_raw();
            }
        }
    }
}

/// Clear an owned output string, freeing the previous libdplyr allocation when present.
///
/// The caller must only pass null or pointers previously allocated by libdplyr.
/// Pointer provenance cannot be validated safely at runtime across the FFI
/// boundary, so callers must initialize output slots to null before the first
/// call and only reuse values originally returned by libdplyr.
pub fn clear_output_string(out: *mut *mut c_char) {
    if out.is_null() {
        return;
    }

    unsafe {
        if !(*out).is_null() {
            free_owned_string(*out);
        }
        *out = ptr::null_mut();
    }
}

/// Initialize an output slot to null before first use from C callers.
#[no_mangle]
pub unsafe extern "C" fn dplyr_init_output_string(out: *mut *mut c_char) -> i32 {
    if out.is_null() {
        return DPLYR_ERROR_NULL_POINTER;
    }

    unsafe {
        *out = ptr::null_mut();
    }
    DPLYR_SUCCESS
}

/// Replace an existing owned output string, freeing the previous allocation first.
pub fn replace_output_string(out: *mut *mut c_char, value: &str) {
    if out.is_null() {
        return;
    }

    clear_output_string(out);

    unsafe {
        if let Ok(c_string) = CString::new(value) {
            *out = c_string.into_raw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn replace_output_string_overwrites_existing_allocation() {
        let mut out = CString::new("stale error").unwrap().into_raw();

        replace_output_string(&mut out, "fresh error");

        let message = unsafe {
            let c_str = CStr::from_ptr(out);
            let message = c_str.to_string_lossy().into_owned();
            let _ = CString::from_raw(out);
            message
        };

        assert_eq!(message, "fresh error");
    }

    #[test]
    fn clear_output_string_frees_existing_allocation_and_nulls_slot() {
        let mut out = CString::new("stale error").unwrap().into_raw();

        clear_output_string(&mut out);

        assert!(out.is_null());
    }

    #[test]
    fn init_output_string_sets_slot_to_null() {
        let mut out: *mut c_char = 1usize as *mut c_char;

        let result = unsafe { dplyr_init_output_string(&mut out) };

        assert_eq!(result, DPLYR_SUCCESS);
        assert!(out.is_null());
    }
}
