//! C-compatible API for libdplyr DuckDB extension
//!
//! This crate provides a C-compatible interface for the libdplyr transpiler
//! to be used in DuckDB extensions. It handles FFI safety, memory management,
//! and error handling across the C/Rust boundary.
//!
//! # Requirements Fulfilled
//! - R3-AC1: C ABI compatibility with static/dynamic library support
//! - R3-AC2: Structured result format (success/error/message)
//! - R3-AC3: Dedicated memory management with clear ownership rules
//! - R9-AC1: Panic safety across FFI boundaries
//! - R9-AC2: Input validation and DoS prevention

pub mod cache;
mod compile;
pub mod error;
mod ffi;
mod ffi_safety;
mod memory;
mod metadata;
pub mod options;
pub mod performance_tests;
mod system;
mod validation;

pub use compile::dplyr_compile;
pub use ffi_safety::dplyr_is_valid_string_pointer;
pub use memory::{dplyr_free_string, dplyr_free_strings};
#[cfg(target_family = "wasm")]
pub use metadata::main;
pub use metadata::{
    dplyr_build_timestamp, dplyr_has_debug_support, dplyr_max_input_length,
    dplyr_max_processing_time_ms, dplyr_supported_dialects, dplyr_version, dplyr_version_detailed,
    libdplyr_c_version_simple,
};
pub use system::dplyr_check_system;

// Re-export cache FFI functions for C header generation
pub use cache::{
    dplyr_cache_clear, dplyr_cache_get_capacity, dplyr_cache_get_evictions,
    dplyr_cache_get_hit_rate, dplyr_cache_get_hits, dplyr_cache_get_misses, dplyr_cache_get_size,
    dplyr_cache_get_stats, dplyr_cache_is_effective, dplyr_cache_log_performance_warning,
    dplyr_cache_log_stats, dplyr_cache_log_stats_detailed, dplyr_cache_should_clear,
};

// Re-export error handling functions for C header generation
pub use error::{dplyr_error_code_name, dplyr_is_recoverable_error, dplyr_is_success};
pub use error::{DPLYR_ERROR_SYNTAX, DPLYR_ERROR_UNSUPPORTED};

pub use options::{
    dplyr_options_create, dplyr_options_create_with_timeout, dplyr_options_default,
    dplyr_options_validate, DplyrOptions, MAX_FUNCTION_CALLS, MAX_INPUT_LENGTH, MAX_NESTING_DEPTH,
    MAX_OUTPUT_LENGTH, MAX_PROCESSING_TIME_MS,
};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

// DuckDB C Extension API init function
// This function is required for C API-based DuckDB extensions
// (Removed dplyr_extension_init_c_api to avoid conflict with C++ extension init)
// Initialization for C API extension
// (Removed dangling code)
