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
mod system;
mod validation;

pub use compile::{dplyr_compile, dplyr_compile_query};
pub use ffi::dplyr_init_output_string;
pub use ffi_safety::dplyr_is_valid_string_pointer;
pub use memory::{dplyr_free_string, dplyr_free_strings};
#[cfg(target_family = "wasm")]
pub use metadata::main;
pub use metadata::{
    dplyr_build_timestamp, dplyr_has_debug_support, dplyr_max_input_length,
    dplyr_max_processing_time_ms, dplyr_supported_dialects, dplyr_version, dplyr_version_detailed,
    libdplyr_c_version_simple,
};

// Re-export error handling functions for C header generation
pub use error::DPLYR_QUERY_NOT_HANDLED;
pub use error::{
    dplyr_error_code_name, dplyr_is_recoverable_error, dplyr_is_success, dplyr_result_has_output,
};
pub use error::{DPLYR_ERROR_SYNTAX, DPLYR_ERROR_UNSUPPORTED};

pub use options::{
    dplyr_options_create, dplyr_options_create_with_timeout, dplyr_options_default,
    dplyr_options_validate, DplyrDialect, DplyrOptions, MAX_FUNCTION_CALLS, MAX_INPUT_LENGTH,
    MAX_NESTING_DEPTH, MAX_OUTPUT_LENGTH, MAX_PROCESSING_TIME_MS,
};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
