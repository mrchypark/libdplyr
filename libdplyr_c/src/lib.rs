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
mod schema_compile;
mod system;
mod validation;

pub use compile::{
    dplyr_compile, dplyr_compile_query, dplyr_compile_query_with_pipe_syntax,
    dplyr_compile_with_pipe_syntax, dplyr_prepare_query_with_pipe_syntax,
};
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
pub use schema_compile::{
    dplyr_compile_with_schema, dplyr_compile_with_schema_and_pipe_syntax,
    dplyr_required_source_bindings, dplyr_required_sources,
};

// Re-export error handling functions for C header generation
pub use error::DPLYR_QUERY_NOT_HANDLED;
pub use error::{
    dplyr_error_code_name, dplyr_is_recoverable_error, dplyr_is_success, dplyr_result_has_output,
};
pub use error::{DPLYR_ERROR_SYNTAX, DPLYR_ERROR_UNSUPPORTED};

pub use options::{
    dplyr_options_create, dplyr_options_create_with_timeout, dplyr_options_default,
    dplyr_options_validate, DplyrDialect, DplyrOptions, DplyrPipeSyntax, MAX_FUNCTION_CALLS,
    MAX_INPUT_LENGTH, MAX_NESTING_DEPTH, MAX_OUTPUT_LENGTH, MAX_PROCESSING_TIME_MS,
};

#[cfg(test)]
mod required_source_bindings_tests {
    use super::*;
    use std::ffi::CStr;
    use std::os::raw::c_char;

    fn call(code: &[u8], pipe_syntax: u32) -> (i32, String, String) {
        let mut output = std::ptr::null_mut();
        let mut error = std::ptr::null_mut();
        let status = unsafe {
            dplyr_required_source_bindings(
                code.as_ptr().cast(),
                pipe_syntax,
                &mut output,
                &mut error,
            )
        };
        let take = |value: *mut c_char| {
            if value.is_null() {
                String::new()
            } else {
                let raw = value;
                let value = unsafe { CStr::from_ptr(raw).to_string_lossy().into_owned() };
                let _ = unsafe { dplyr_free_string(raw) };
                value
            }
        };

        (status, take(output), take(error))
    }

    #[test]
    fn required_source_bindings_preserve_source_ref_identity() {
        let code = b"sales.orders %>% inner_join(in_schema(\"sales\", \"orders\"), by = \"id\") %>% union_all(in_catalog(\"memory\", \"sales\", \"items\")) %>% inner_join(\"quoted\\\".table\", by = \"id\")\0";
        let (status, output, error) = call(code, DplyrPipeSyntax::Magrittr as u32);

        assert_eq!(status, error::DPLYR_SUCCESS, "{error}");
        let bindings: serde_json::Value = serde_json::from_str(&output).expect("bindings JSON");
        assert_eq!(bindings["0"]["source_json"], r#""sales.orders""#);
        assert_eq!(bindings["0"]["catalog"], "");
        assert_eq!(bindings["0"]["schema"], "");
        assert_eq!(bindings["0"]["table"], "sales.orders");
        assert_eq!(bindings["1"]["source_json"], r#"["sales","orders"]"#);
        assert_eq!(bindings["1"]["catalog"], "");
        assert_eq!(bindings["1"]["schema"], "sales");
        assert_eq!(bindings["1"]["table"], "orders");
        assert_eq!(
            bindings["2"]["source_json"],
            r#"["memory","sales","items"]"#
        );
        assert_eq!(bindings["2"]["catalog"], "memory");
        assert_eq!(bindings["2"]["schema"], "sales");
        assert_eq!(bindings["2"]["table"], "items");
        assert_eq!(bindings["3"]["source_json"], r#""quoted\".table""#);
        assert_eq!(bindings["3"]["table"], "quoted\".table");
    }

    #[test]
    fn required_source_bindings_use_existing_input_contract() {
        let mut output = std::ptr::null_mut();
        let mut error = std::ptr::null_mut();
        let null_status = unsafe {
            dplyr_required_source_bindings(
                std::ptr::null(),
                DplyrPipeSyntax::Magrittr as u32,
                &mut output,
                &mut error,
            )
        };
        assert_eq!(null_status, error::DPLYR_ERROR_NULL_POINTER);
        assert!(output.is_null());
        assert!(!error.is_null());
        let _ = unsafe { dplyr_free_string(error) };

        let (status, output, error) = call(
            &[b'u', b's', b'e', b'r', b's', 0xff, 0],
            DplyrPipeSyntax::Magrittr as u32,
        );
        assert_eq!(status, error::DPLYR_ERROR_INVALID_UTF8);
        assert!(output.is_empty());
        assert!(error.contains("E-INVALID-UTF8"), "{error}");

        let (status, output, error) = call(b"users %>% select(id)\0", 99);
        assert_eq!(status, error::DPLYR_ERROR_SYNTAX);
        assert!(output.is_empty());
        assert!(error.contains("Invalid pipe syntax value '99'"), "{error}");

        let (status, output, error) = call(
            b"users %>% select(id)\0 this suffix is outside the C string",
            DplyrPipeSyntax::Magrittr as u32,
        );
        assert_eq!(status, error::DPLYR_SUCCESS, "{error}");
        let bindings: serde_json::Value = serde_json::from_str(&output).expect("bindings JSON");
        assert_eq!(bindings["0"]["table"], "users");
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
