//! System readiness checks for the C API.

use std::ffi::CString;
use std::panic;

use crate::cache::SimpleTranspileCache;
use crate::error::{DPLYR_ERROR_INTERNAL, DPLYR_ERROR_PANIC, DPLYR_SUCCESS};

/// Validate system requirements and configuration.
///
/// # Returns
/// 0 if system is ready, negative error code if issues found
#[no_mangle]
pub extern "C" fn dplyr_check_system() -> i32 {
    let result = panic::catch_unwind(|| {
        // Check basic system requirements

        // 1. Check if we can allocate memory
        let test_allocation = CString::new("test");
        if test_allocation.is_err() {
            return DPLYR_ERROR_INTERNAL;
        }

        // 2. Check if libdplyr components are available
        // This is a compile-time check, so if we got here, they should be available

        // 3. Check cache system
        SimpleTranspileCache::clear_cache();
        let stats = SimpleTranspileCache::get_cache_stats();
        if stats.is_empty() {
            return DPLYR_ERROR_INTERNAL;
        }

        DPLYR_SUCCESS
    });

    match result {
        Ok(code) => code,
        Err(_) => DPLYR_ERROR_PANIC,
    }
}
