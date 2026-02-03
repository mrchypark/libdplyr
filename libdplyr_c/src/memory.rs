//! Memory management for FFI-owned strings.

use std::ffi::CString;
use std::os::raw::c_char;
use std::panic;

use crate::error::{DPLYR_ERROR_NULL_POINTER, DPLYR_ERROR_PANIC, DPLYR_SUCCESS};

/// Free string allocated by `libdplyr_c` functions.
///
/// # Safety
/// Caller must ensure that:
/// - `s` is a valid `*mut c_char` that was previously allocated by a `libdplyr_c` function.
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

    result.unwrap_or_else(|_| {
        // Panic occurred during deallocation - this is serious
        eprintln!("CRITICAL: Panic occurred during string deallocation");
        DPLYR_ERROR_PANIC
    })
}

/// Free multiple strings at once.
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

    result.map_or(DPLYR_ERROR_PANIC, |count| count)
}
