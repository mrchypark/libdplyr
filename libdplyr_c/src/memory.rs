//! Memory management for FFI-owned strings.

use std::collections::HashSet;
use std::ffi::CString;
use std::os::raw::c_char;
use std::panic;
use std::sync::{Mutex, OnceLock};

use crate::error::{DPLYR_ERROR_NULL_POINTER, DPLYR_ERROR_PANIC, DPLYR_SUCCESS};

fn owned_strings() -> &'static Mutex<HashSet<usize>> {
    static OWNED_STRINGS: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    OWNED_STRINGS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn alloc_owned_string(value: &str) -> Option<*mut c_char> {
    let c_string = CString::new(value).ok()?;
    let ptr = c_string.into_raw();
    let registry = owned_strings();
    let Ok(mut owned) = registry.lock() else {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
        return None;
    };

    owned.insert(ptr as usize);
    Some(ptr)
}

#[cfg(test)]
pub(crate) fn live_owned_string_count() -> usize {
    let registry = owned_strings();
    let Ok(owned) = registry.lock() else {
        return 0;
    };
    owned.len()
}

/// Reclaim a single string owned by libdplyr.
///
/// The caller must only pass a pointer previously returned by libdplyr, or null.
pub(crate) unsafe fn free_owned_string(s: *mut c_char) -> bool {
    if s.is_null() {
        return true;
    }

    let registry = owned_strings();
    let Ok(mut owned) = registry.lock() else {
        return false;
    };

    if !owned.remove(&(s as usize)) {
        return false;
    }

    unsafe {
        let _ = CString::from_raw(s);
    }
    true
}

/// Free string allocated by `libdplyr_c` functions.
///
/// # Safety
/// Caller must ensure that:
/// - `s` is a valid `*mut c_char` that was previously allocated by a `libdplyr_c` function.
/// - `s` must not be freed concurrently with any other operation on the same pointer.
/// - `s` can be `std::ptr::null_mut()`, in which case it's a no-op.
///
/// # Returns
/// - `DPLYR_SUCCESS` when the pointer was null or reclaimed successfully
/// - `DPLYR_ERROR_PANIC` when the pointer is not a currently owned libdplyr allocation
///   or a panic occurred while handling the request
///
/// Passing a pointer after it has already been released is a caller bug. The
/// implementation only rejects that pattern on a best-effort basis.
#[no_mangle]
pub unsafe extern "C" fn dplyr_free_string(s: *mut c_char) -> i32 {
    // R3-AC3: Safe memory management with null pointer check
    if s.is_null() {
        return DPLYR_SUCCESS; // Null pointer is OK, no-op
    }

    // R9-AC1: Panic safety for memory operations
    let result = panic::catch_unwind(|| {
        if unsafe { free_owned_string(s) } {
            DPLYR_SUCCESS
        } else {
            DPLYR_ERROR_PANIC
        }
    });

    result.unwrap_or(DPLYR_ERROR_PANIC)
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
/// Number of strings successfully reclaimed. Unknown pointers are skipped and
/// do not contribute to the count.
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
                if !string_ptr.is_null() && dplyr_free_string(string_ptr) == DPLYR_SUCCESS {
                    freed_count += 1;
                }
            }
        }

        freed_count
    });

    result.map_or(DPLYR_ERROR_PANIC, |count| count)
}
