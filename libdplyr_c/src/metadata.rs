//! Version/build/capabilities metadata for the C API.

use std::os::raw::c_char;

use crate::options::{MAX_INPUT_LENGTH, MAX_PROCESSING_TIME_MS};

/// WASM entrypoint stub (required for some WASM toolchains).
#[cfg(target_family = "wasm")]
#[no_mangle]
pub extern "C" fn main() {}

/// Get libdplyr_c version string (simple).
///
/// # Returns
/// Static version string (no need to free)
#[no_mangle]
pub extern "C" fn libdplyr_c_version_simple() -> *const c_char {
    // R8-AC1: Version information - static string management
    c"0.1.0".as_ptr()
}

/// Get basic version string.
///
/// # Returns
/// Static version string (no need to free)
#[no_mangle]
pub extern "C" fn dplyr_version() -> *const c_char {
    // R8-AC1: Version information - static string management
    c"0.1.0".as_ptr()
}

/// Get detailed version information including build info.
///
/// # Returns
/// Static version string with build details (no need to free)
#[no_mangle]
pub extern "C" fn dplyr_version_detailed() -> *const c_char {
    // R8-AC1: Extended version information
    concat!(
        "libdplyr_c v0.1.0 (built with rustc ",
        env!("RUSTC_VERSION", "unknown"),
        ")\0"
    )
    .as_ptr() as *const c_char
}

/// Get supported SQL dialects as a comma-separated string.
///
/// # Returns
/// Static string listing supported dialects (no need to free)
#[no_mangle]
pub extern "C" fn dplyr_supported_dialects() -> *const c_char {
    // R8-AC1: Capability information
    c"DuckDB".as_ptr()
}

/// Get build timestamp.
///
/// # Returns
/// Static build timestamp string (no need to free)
#[no_mangle]
pub extern "C" fn dplyr_build_timestamp() -> *const c_char {
    // R8-AC1: Build information
    concat!(env!("BUILD_TIMESTAMP", "unknown"), "\0").as_ptr() as *const c_char
}

/// Check if debug mode is available in this build.
///
/// # Returns
/// true if debug features are available, false otherwise
#[no_mangle]
pub extern "C" fn dplyr_has_debug_support() -> bool {
    // R10-AC1: Debug capability check
    cfg!(debug_assertions)
}

/// Get maximum supported input length.
///
/// # Returns
/// Maximum input length in bytes
#[no_mangle]
pub extern "C" fn dplyr_max_input_length() -> u32 {
    // R9-AC2: DoS prevention information
    MAX_INPUT_LENGTH as u32
}

/// Get maximum processing time limit.
///
/// # Returns
/// Maximum processing time in milliseconds
#[no_mangle]
pub extern "C" fn dplyr_max_processing_time_ms() -> u64 {
    // R9-AC2: DoS prevention information
    MAX_PROCESSING_TIME_MS
}
