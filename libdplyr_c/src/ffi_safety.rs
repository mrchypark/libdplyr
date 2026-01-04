//! FFI pointer validation and safety helpers.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic;

/// Check if a pointer looks like a valid C string.
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
            let c_str = CStr::from_ptr(s);

            // Check if the string is valid UTF-8
            c_str.to_str().is_ok()
        }
    });

    result.unwrap_or(false)
}
