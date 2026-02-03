// NOTE: This file is compiled only for `cargo test`.

use super::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::cache::SimpleTranspileCache;
use crate::compile::convert_libdplyr_error;
use crate::error::{
    DPLYR_ERROR_INPUT_TOO_LARGE, DPLYR_ERROR_INTERNAL, DPLYR_ERROR_INVALID_UTF8,
    DPLYR_ERROR_NULL_POINTER, DPLYR_ERROR_PANIC, DPLYR_SUCCESS,
};
use crate::validation::{
    calculate_nesting_depth, contains_suspicious_patterns, count_function_calls,
    has_excessive_repetition, validate_input_encoding, validate_input_security,
    validate_input_structure,
};

#[cfg(test)]
mod ffi_tests {
    use super::*;

    #[test]
    fn test_dplyr_options_default() {
        let options = DplyrOptions::default();
        assert!(!options.strict_mode);
        assert!(!options.preserve_comments);
        assert!(!options.debug_mode);
        assert_eq!(options.max_input_length, 1024 * 1024);
        assert_eq!(options.max_processing_time_ms, MAX_PROCESSING_TIME_MS);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_INPUT_LENGTH, 1024 * 1024);
        assert_eq!(MAX_PROCESSING_TIME_MS, 30000);
    }

    #[test]
    fn test_dplyr_options_creation() {
        let options = DplyrOptions::new();
        assert_eq!(options, DplyrOptions::default());

        let custom_options = DplyrOptions::with_settings(true, true, true, 512);
        assert!(custom_options.strict_mode);
        assert!(custom_options.preserve_comments);
        assert!(custom_options.debug_mode);
        assert_eq!(custom_options.max_input_length, 512);
        assert_eq!(
            custom_options.max_processing_time_ms,
            MAX_PROCESSING_TIME_MS
        );

        let full_options = DplyrOptions::with_all_settings(true, false, true, 1024, 5000);
        assert!(full_options.strict_mode);
        assert!(!full_options.preserve_comments);
        assert!(full_options.debug_mode);
        assert_eq!(full_options.max_input_length, 1024);
        assert_eq!(full_options.max_processing_time_ms, 5000);
    }

    #[test]
    fn test_dplyr_options_validation() {
        let valid_options = DplyrOptions::default();
        assert!(valid_options.validate().is_ok());

        let invalid_options = DplyrOptions::with_settings(false, false, false, 0);
        assert!(invalid_options.validate().is_err());

        // Test with manually created oversized options (bypassing with_settings clamping)
        let oversized_options = DplyrOptions {
            strict_mode: false,
            preserve_comments: false,
            debug_mode: false,
            max_input_length: (MAX_INPUT_LENGTH + 1) as u32,
            max_processing_time_ms: MAX_PROCESSING_TIME_MS,
        };
        assert!(oversized_options.validate().is_err());

        // Test timeout validation - zero timeout is now allowed (means use default)
        let zero_timeout_options = DplyrOptions {
            strict_mode: false,
            preserve_comments: false,
            debug_mode: false,
            max_input_length: 1024,
            max_processing_time_ms: 0, // Zero means use default
        };
        assert!(zero_timeout_options.validate().is_ok());

        let oversized_timeout_options = DplyrOptions {
            strict_mode: false,
            preserve_comments: false,
            debug_mode: false,
            max_input_length: 1024,
            max_processing_time_ms: MAX_PROCESSING_TIME_MS + 1000, // Too large
        };
        assert!(oversized_timeout_options.validate().is_err());
    }

    #[test]
    fn test_dplyr_options_size_limit() {
        let options =
            DplyrOptions::with_settings(false, false, false, (MAX_INPUT_LENGTH + 1000) as u32);
        // Should be clamped to MAX_INPUT_LENGTH
        assert_eq!(options.max_input_length, MAX_INPUT_LENGTH as u32);
    }

    #[test]
    fn test_ffi_options_functions() {
        // Test default options creation
        let default_opts = dplyr_options_default();
        assert_eq!(default_opts, DplyrOptions::default());

        // Test custom options creation
        let custom_opts = dplyr_options_create(true, false, true, 2048);
        assert!(custom_opts.strict_mode);
        assert!(!custom_opts.preserve_comments);
        assert!(custom_opts.debug_mode);
        assert_eq!(custom_opts.max_input_length, 2048);

        // Test validation
        let valid_result = unsafe { dplyr_options_validate(&default_opts as *const DplyrOptions) };
        assert_eq!(valid_result, 0);

        // Test null pointer validation
        let null_result = unsafe { dplyr_options_validate(std::ptr::null()) };
        assert_eq!(null_result, -1);
    }

    #[test]
    fn test_dplyr_compile_null_pointers() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test null code pointer
        let result = unsafe {
            dplyr_compile(
                std::ptr::null(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };

        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);
        assert!(!out_error.is_null());

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_invalid_utf8() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Create invalid UTF-8 sequence
        let invalid_utf8 = b"select(col1)\xFF\xFE\0";

        let result = unsafe {
            dplyr_compile(
                invalid_utf8.as_ptr() as *const c_char,
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };

        assert_eq!(result, DPLYR_ERROR_INVALID_UTF8);
        assert!(!out_error.is_null());

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_input_too_large() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Create options with small limit
        let options = DplyrOptions::with_settings(false, false, false, 10);

        // Create input larger than limit
        let large_input = CString::new("select(very_long_column_name_that_exceeds_limit)").unwrap();

        let result = unsafe {
            dplyr_compile(
                large_input.as_ptr(),
                &options as *const DplyrOptions,
                &mut out_sql,
                &mut out_error,
            )
        };

        assert_eq!(result, DPLYR_ERROR_INPUT_TOO_LARGE);
        assert!(!out_error.is_null());

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_basic_success() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Simple dplyr code that should work
        let input = CString::new("select(col1)").unwrap();

        let result = unsafe {
            dplyr_compile(
                input.as_ptr(),
                std::ptr::null(), // Use default options
                &mut out_sql,
                &mut out_error,
            )
        };

        // Note: This might fail if libdplyr doesn't support the exact syntax
        // but the FFI layer should handle it gracefully
        if result == DPLYR_SUCCESS {
            assert!(!out_sql.is_null());
            assert!(out_error.is_null());

            // Clean up
            assert_eq!(unsafe { dplyr_free_string(out_sql) }, DPLYR_SUCCESS);
        } else {
            // If it fails, should have error message
            assert!(!out_error.is_null());

            // Clean up
            if !out_error.is_null() {
                assert_eq!(unsafe { dplyr_free_string(out_error) }, DPLYR_SUCCESS);
            }
        }
    }

    #[test]
    fn test_dplyr_free_string_safety() {
        // Test freeing null pointer (should be safe)
        let result = unsafe { dplyr_free_string(std::ptr::null_mut()) };
        assert_eq!(result, DPLYR_SUCCESS);

        // Test freeing valid string
        let test_string = CString::new("test string").unwrap();
        let raw_ptr = test_string.into_raw();

        // Verify pointer looks valid
        assert!(unsafe { dplyr_is_valid_string_pointer(raw_ptr) });

        // Free it
        let result = unsafe { dplyr_free_string(raw_ptr) };
        assert_eq!(result, DPLYR_SUCCESS);

        // Note: We can't test double-free safely as it would be undefined behavior
    }

    #[test]
    fn test_dplyr_free_strings_batch() {
        // Create multiple test strings
        let string1 = CString::new("string1").unwrap().into_raw();
        let string2 = CString::new("string2").unwrap().into_raw();
        let string3 = CString::new("string3").unwrap().into_raw();

        // Create array of pointers
        let mut strings = vec![string1, string2, string3, std::ptr::null_mut()];

        // Free all strings
        let freed_count = unsafe { dplyr_free_strings(strings.as_mut_ptr(), strings.len()) };
        assert_eq!(freed_count, 3); // Should free 3 strings (null pointer is skipped)

        // Test with null array
        let result = unsafe { dplyr_free_strings(std::ptr::null_mut(), 0) };
        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);
    }

    #[test]
    fn test_dplyr_is_valid_string_pointer() {
        // Test null pointer
        assert!(unsafe { !dplyr_is_valid_string_pointer(std::ptr::null()) });

        // Test valid string
        let test_string = CString::new("valid string").unwrap();
        assert!(unsafe { dplyr_is_valid_string_pointer(test_string.as_ptr()) });

        // Test static string
        let static_str = b"static string\0";
        assert!(unsafe { dplyr_is_valid_string_pointer(static_str.as_ptr() as *const c_char) });
    }

    #[test]
    fn test_utility_functions() {
        // Test version functions
        let version = unsafe { CStr::from_ptr(libdplyr_c_version_simple()) };
        assert_eq!(version.to_string_lossy(), "0.2.0");

        let detailed_version = unsafe { CStr::from_ptr(dplyr_version_detailed()) };
        assert!(detailed_version.to_string_lossy().contains("0.2.0"));
        assert!(detailed_version.to_string_lossy().contains("libdplyr_c"));

        // Test supported dialects
        let dialects = unsafe { CStr::from_ptr(dplyr_supported_dialects()) };
        assert!(dialects.to_string_lossy().contains("DuckDB"));

        // Test build timestamp (should not be empty)
        let timestamp = unsafe { CStr::from_ptr(dplyr_build_timestamp()) };
        assert!(!timestamp.to_string_lossy().is_empty());

        // Test debug support check
        let _has_debug = dplyr_has_debug_support();
        // Should be true in debug builds, may be false in release builds

        // Test limits
        assert_eq!(dplyr_max_input_length(), MAX_INPUT_LENGTH as u32);
        assert_eq!(dplyr_max_processing_time_ms(), MAX_PROCESSING_TIME_MS);

        // Test system check
        let system_status = dplyr_check_system();
        assert_eq!(system_status, DPLYR_SUCCESS);
    }

    #[test]
    fn test_security_validation_functions() {
        // Test nesting depth calculation
        assert_eq!(calculate_nesting_depth("select(col1)"), 1);
        assert_eq!(calculate_nesting_depth("select(filter(col1, col2 > 0))"), 2);
        assert_eq!(calculate_nesting_depth("(((())))"), 4);

        // Test function call counting
        assert_eq!(count_function_calls("select(col1)"), 1);
        assert_eq!(count_function_calls("select(col1) %>% filter(col2 > 0)"), 2);
        assert_eq!(count_function_calls("func1() + func2() * func3()"), 3);

        // Test suspicious pattern detection
        assert!(!contains_suspicious_patterns(
            "select(col1) %>% filter(col2 > 0)"
        ));
        assert!(contains_suspicious_patterns("'; DROP TABLE users; --"));
        assert!(contains_suspicious_patterns(
            "UNION SELECT * FROM passwords"
        ));
        assert!(contains_suspicious_patterns(
            "<script>alert('xss')</script>"
        ));

        // Test repetition detection
        assert!(!has_excessive_repetition("select(col1, col2, col3)"));
        assert!(has_excessive_repetition(&"a".repeat(101))); // Too many consecutive chars
        assert!(has_excessive_repetition(&"ab".repeat(25))); // Too many repeated patterns
    }

    #[test]
    fn test_input_encoding_validation() {
        // Valid inputs
        assert!(validate_input_encoding("select(col1)").is_ok());
        assert!(validate_input_encoding("select(名前)").is_ok()); // Non-ASCII but valid

        // Invalid inputs with control characters
        assert!(validate_input_encoding("select\u{0001}(col1)").is_err());
        assert!(validate_input_encoding("select\u{0000}(col1)").is_err());

        // Confusing Unicode characters
        assert!(validate_input_encoding("select\u{200B}(col1)").is_err()); // Zero-width space
        assert!(validate_input_encoding("select\u{202E}(col1)").is_err()); // RTL override
    }

    #[test]
    fn test_input_structure_validation() {
        // Valid structures
        assert!(validate_input_structure("select(col1)").is_ok());
        assert!(validate_input_structure("select(col1, col2)").is_ok());
        assert!(validate_input_structure("filter(col1 > 'test')").is_ok());

        // Invalid structures - unmatched delimiters
        assert!(validate_input_structure("select(col1").is_err()); // Missing )
        assert!(validate_input_structure("select)col1(").is_err()); // Wrong order
        assert!(validate_input_structure("select[col1").is_err()); // Missing ]
        assert!(validate_input_structure("select{col1").is_err()); // Missing }
        assert!(validate_input_structure("select(col1 'unclosed").is_err()); // Unclosed string
    }

    #[test]
    fn test_validate_input_security() {
        // Valid inputs
        assert!(validate_input_security("select(col1) %>% filter(col2 > 0)").is_ok());

        // Excessive nesting
        let deep_nesting = "(((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((";
        assert!(validate_input_security(deep_nesting).is_err());

        // Too many function calls
        let many_functions = (0..1001)
            .map(|i| format!("func{}()", i))
            .collect::<Vec<_>>()
            .join(" + ");
        assert!(validate_input_security(&many_functions).is_err());

        // Suspicious patterns
        assert!(validate_input_security("'; DROP TABLE users; --").is_err());

        // Excessive repetition
        assert!(validate_input_security(&"select".repeat(50)).is_err());
    }

    #[test]
    fn test_ffi_options_with_timeout() {
        // Test timeout creation
        let timeout_opts = dplyr_options_create_with_timeout(true, false, true, 1024, 5000);
        assert!(timeout_opts.strict_mode);
        assert!(!timeout_opts.preserve_comments);
        assert!(timeout_opts.debug_mode);
        assert_eq!(timeout_opts.max_input_length, 1024);
        assert_eq!(timeout_opts.max_processing_time_ms, 5000);

        // Test default timeout (0 = use default)
        let default_timeout_opts = dplyr_options_create_with_timeout(false, false, false, 1024, 0);
        assert_eq!(
            default_timeout_opts.max_processing_time_ms,
            MAX_PROCESSING_TIME_MS
        );
    }

    #[test]
    fn test_dplyr_compile_with_security_validation() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test with suspicious input (properly quoted to pass structure validation)
        let malicious_input =
            CString::new("select(col1) %>% filter(col2 = '; DROP TABLE users; --')").unwrap();

        let result = unsafe {
            dplyr_compile(
                malicious_input.as_ptr(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };

        // Should fail with security error
        assert_ne!(result, DPLYR_SUCCESS);
        assert!(!out_error.is_null());

        // Check error message contains security-related information
        let error_msg = unsafe { std::ffi::CStr::from_ptr(out_error).to_string_lossy() };

        // The error should be related to security validation
        assert!(
            error_msg.contains("malicious")
                || error_msg.contains("suspicious")
                || error_msg.contains("DROP")
                || error_msg.contains("potentially")
        );

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    #[test]
    fn test_dplyr_compile_with_structure_validation() {
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test with unbalanced parentheses
        let unbalanced_input = CString::new("select(col1, col2").unwrap();

        let result = unsafe {
            dplyr_compile(
                unbalanced_input.as_ptr(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };

        // Should fail with syntax error
        assert_ne!(result, DPLYR_SUCCESS);
        assert!(!out_error.is_null());

        // Check error message mentions parentheses
        let error_msg = unsafe { std::ffi::CStr::from_ptr(out_error).to_string_lossy() };
        assert!(error_msg.contains("parenthes") || error_msg.contains("unclosed"));

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    // R9-AC3: Thread safety tests
    #[test]
    fn test_thread_safety_basic() {
        use std::thread;

        SimpleTranspileCache::clear_cache();

        let handles = [0, 1, 2, 3].map(|thread_id| {
            thread::spawn(move || {
                let options = DplyrOptions::default();
                let code = format!("select(col{})", thread_id);

                // Each thread should be able to call functions safely
                let result =
                    SimpleTranspileCache::get_or_transpile(&code, &options, |_code, _opts| {
                        Ok(format!("SELECT col{} FROM table", thread_id))
                    });

                assert!(result.is_ok());
                result.unwrap()
            })
        });
        let results: Vec<String> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(results.len(), 4);

        // Each thread should have gotten its own result
        for (i, result) in results.iter().enumerate() {
            assert!(result.contains(&format!("col{}", i)));
        }
    }

    #[test]
    fn test_ffi_thread_safety() {
        use std::ffi::CString;
        use std::thread;

        let handles = [0, 1, 2, 3].map(|thread_id| {
            thread::spawn(move || {
                let mut out_sql: *mut c_char = std::ptr::null_mut();
                let mut out_error: *mut c_char = std::ptr::null_mut();

                let input = CString::new(format!("select(thread_col_{})", thread_id)).unwrap();

                let result = unsafe {
                    dplyr_compile(
                        input.as_ptr(),
                        std::ptr::null(), // Use default options
                        &mut out_sql,
                        &mut out_error,
                    )
                };

                // Clean up regardless of result
                if !out_sql.is_null() {
                    unsafe { dplyr_free_string(out_sql) };
                }
                if !out_error.is_null() {
                    unsafe { dplyr_free_string(out_error) };
                }

                // Return the result code
                result
            })
        });
        let results: Vec<i32> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        // All threads should complete without panicking
        assert_eq!(results.len(), 4);

        // Results may vary (success or error) but should not crash
        for result in results {
            // Should be a valid error code (not some random value from memory corruption)
            assert!((-10..=0).contains(&result));
        }
    }

    #[test]
    fn test_cache_thread_isolation() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        SimpleTranspileCache::clear_cache();

        let barrier = Arc::new(Barrier::new(3));
        let handles = [0, 1, 2].map(|thread_id| {
            let barrier = barrier.clone();
            thread::spawn(move || {
                let options = DplyrOptions::default();

                // Each thread adds its own entries
                for i in 0..5 {
                    let code = format!("select(thread_{}_col_{})", thread_id, i);
                    let _ =
                        SimpleTranspileCache::get_or_transpile(&code, &options, |_code, _opts| {
                            Ok(format!("SELECT thread_{}_col_{} FROM table", thread_id, i))
                        });
                }

                // Wait for all threads to finish adding entries
                barrier.wait();

                // Each thread should see its own cache (thread_local)
                // The cache size should be 5 for each thread
                let cache_size = dplyr_cache_get_size();
                assert_eq!(cache_size, 5);

                thread_id
            })
        });

        let mut thread_count = 0;
        for handle in handles {
            let _ = handle.join().unwrap();
            thread_count += 1;
        }
        assert_eq!(thread_count, 3);
    }

    #[test]
    fn test_memory_management_thread_safety() {
        use std::ffi::CString;
        use std::thread;

        // Test that memory management functions are thread-safe
        let handles = [0, 1, 2, 3].map(|thread_id| {
            thread::spawn(move || {
                // Create and free strings in each thread
                let raw_pointers: Vec<*mut c_char> = (0..10)
                    .map(|i| CString::new(format!("thread_{}_string_{}", thread_id, i)).unwrap())
                    .map(|s| s.into_raw())
                    .collect();

                // Free all strings
                for ptr in raw_pointers {
                    let result = unsafe { dplyr_free_string(ptr) };
                    assert_eq!(result, DPLYR_SUCCESS);
                }

                // Test batch free
                let mut batch_pointers: Vec<*mut c_char> = (0..5)
                    .map(|i| {
                        CString::new(format!("batch_thread_{}_string_{}", thread_id, i)).unwrap()
                    })
                    .map(|s| s.into_raw())
                    .collect();

                let freed_count = unsafe {
                    dplyr_free_strings(batch_pointers.as_mut_ptr(), batch_pointers.len())
                };
                assert_eq!(freed_count, 5);

                thread_id
            })
        });

        let mut thread_count = 0;
        for handle in handles {
            let _ = handle.join().unwrap();
            thread_count += 1;
        }
        assert_eq!(thread_count, 4);
    }

    #[test]
    fn test_options_thread_safety() {
        use std::thread;

        // Test that options creation and validation are thread-safe
        let handles = [0, 1, 2, 3].map(|thread_id| {
            thread::spawn(move || {
                // Create options with different settings in each thread
                let options = dplyr_options_create_with_timeout(
                    thread_id % 2 == 0,               // strict_mode
                    thread_id % 2 == 1,               // preserve_comments
                    true,                             // debug_mode
                    1024 * (thread_id as u32 + 1),    // max_input_length
                    5000 + (thread_id as u64 * 1000), // max_processing_time_ms
                );

                // Validate options
                let validation_result =
                    unsafe { dplyr_options_validate(&options as *const DplyrOptions) };
                assert_eq!(validation_result, 0);

                // Test default options
                let default_options = dplyr_options_default();
                let default_validation =
                    unsafe { dplyr_options_validate(&default_options as *const DplyrOptions) };
                assert_eq!(default_validation, 0);

                thread_id
            })
        });

        let mut thread_count = 0;
        for handle in handles {
            let _ = handle.join().unwrap();
            thread_count += 1;
        }
        assert_eq!(thread_count, 4);
    }

    #[test]
    fn test_utility_functions_thread_safety() {
        use std::thread;

        // Test that utility functions are thread-safe
        let handles = [0, 1, 2, 3].map(|thread_id| {
            thread::spawn(move || unsafe {
                // These functions should be safe to call from multiple threads
                let version_str = { std::ffi::CStr::from_ptr(libdplyr_c_version_simple()) };
                assert!(!version_str.to_string_lossy().is_empty());

                let detailed_version = { std::ffi::CStr::from_ptr(dplyr_version_detailed()) };
                assert!(!detailed_version.to_string_lossy().is_empty());

                let dialects = { std::ffi::CStr::from_ptr(dplyr_supported_dialects()) };
                assert!(!dialects.to_string_lossy().is_empty());

                let _has_debug = dplyr_has_debug_support();

                let max_input = dplyr_max_input_length();
                assert_eq!(max_input, MAX_INPUT_LENGTH as u32);

                let max_time = dplyr_max_processing_time_ms();
                assert_eq!(max_time, MAX_PROCESSING_TIME_MS);

                let system_check = dplyr_check_system();
                assert_eq!(system_check, DPLYR_SUCCESS);

                thread_id
            })
        });

        let mut thread_count = 0;
        for handle in handles {
            let _ = handle.join().unwrap();
            thread_count += 1;
        }
        assert_eq!(thread_count, 4);
    }

    #[test]
    fn test_error_functions_thread_safety() {
        use std::thread;

        // Test that error handling functions are thread-safe
        let handles = [0, 1, 2, 3].map(|thread_id| {
            thread::spawn(move || unsafe {
                // Test error code functions
                let error_codes = [
                    DPLYR_SUCCESS,
                    DPLYR_ERROR_SYNTAX,
                    DPLYR_ERROR_UNSUPPORTED,
                    DPLYR_ERROR_INTERNAL,
                    DPLYR_ERROR_PANIC,
                ];

                for &error_code in &error_codes {
                    let error_name = {
                        std::ffi::CStr::from_ptr(dplyr_error_code_name(error_code))
                            .to_string_lossy()
                    };
                    assert!(!error_name.is_empty());

                    let is_success = dplyr_is_success(error_code);
                    assert_eq!(is_success, error_code == DPLYR_SUCCESS);

                    let _is_recoverable = dplyr_is_recoverable_error(error_code);
                    // Just test it doesn't crash
                }

                thread_id
            })
        });

        let mut thread_count = 0;
        for handle in handles {
            let _ = handle.join().unwrap();
            thread_count += 1;
        }
        assert_eq!(thread_count, 4);
    }

    // R9-AC1: Panic safety tests
    #[test]
    fn test_panic_safety_in_ffi_functions() {
        // Test that panics in FFI functions are caught and handled properly
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();

        // Test with null pointers - should not panic
        let result = unsafe {
            dplyr_compile(
                std::ptr::null(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };
        assert_eq!(result, DPLYR_ERROR_NULL_POINTER);

        // Clean up
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }

    // R9-AC2: Input validation tests
    #[test]
    fn test_input_validation_comprehensive() {
        // Test encoding validation
        assert!(validate_input_encoding("valid input").is_ok());
        assert!(validate_input_encoding("input with\nnewline").is_ok());
        assert!(validate_input_encoding("input with\ttab").is_ok());
        assert!(validate_input_encoding("input with\0null").is_err());
        assert!(validate_input_encoding("input with\x01control").is_err());

        // Test structure validation
        assert!(validate_input_structure("select(col1)").is_ok());
        assert!(validate_input_structure("select(col1, col2)").is_ok());
        assert!(validate_input_structure("select(col1").is_err()); // Unmatched paren
        assert!(validate_input_structure("select)col1(").is_err()); // Wrong order
        assert!(validate_input_structure("select[col1").is_err()); // Unmatched bracket
        assert!(validate_input_structure("select{col1").is_err()); // Unmatched brace

        // Test security validation
        assert!(validate_input_security("select(col1) %>% filter(age > 18)").is_ok());
        assert!(validate_input_security("'; DROP TABLE users;").is_err());
        assert!(validate_input_security("UNION SELECT * FROM passwords").is_err());
        assert!(validate_input_security("<script>alert('xss')</script>").is_err());
        assert!(validate_input_security("../../../etc/passwd").is_err());

        // Test excessive nesting
        let deep_nesting = "(".repeat(60) + &")".repeat(60);
        assert!(validate_input_security(&deep_nesting).is_err());

        // Test excessive function calls
        let many_functions = (0..1100)
            .map(|i| format!("func{}()", i))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(validate_input_security(&many_functions).is_err());

        // Test excessive repetition
        let repeated_chars = "a".repeat(150);
        assert!(validate_input_security(&repeated_chars).is_err());

        let repeated_pattern = "abc".repeat(25);
        assert!(validate_input_security(&repeated_pattern).is_err());
    }

    // R6-AC1: Caching integration tests
    #[test]
    fn test_caching_integration() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        SimpleTranspileCache::clear_cache();

        let options = DplyrOptions::default();
        let call_count = Arc::new(AtomicUsize::new(0));

        // First call should execute function
        let call_count_clone = call_count.clone();
        let result1 =
            SimpleTranspileCache::get_or_transpile("select(col1)", &options, |_code, _opts| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok("SELECT col1 FROM table".to_string())
            });

        assert!(result1.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second call with same input should use cache
        let call_count_clone = call_count.clone();
        let result2 =
            SimpleTranspileCache::get_or_transpile("select(col1)", &options, |_code, _opts| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok("SELECT col1 FROM table".to_string())
            });

        assert!(result2.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Should not increment due to cache hit
        assert_eq!(result1.unwrap(), result2.unwrap());

        // Verify cache metrics
        let metrics = SimpleTranspileCache::get_cache_metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
        assert_eq!(SimpleTranspileCache::get_hit_rate(), 0.5);

        // Test with `dplyr_compile` - using explicit unsafe blocks
        let mut out_sql: *mut c_char = std::ptr::null_mut();
        let mut out_error: *mut c_char = std::ptr::null_mut();
        let dplyr_code = CString::new("select(caching_test)").unwrap();

        let result = unsafe {
            dplyr_compile(
                dplyr_code.as_ptr(),
                std::ptr::null(),
                &mut out_sql,
                &mut out_error,
            )
        };
        assert_eq!(result, DPLYR_SUCCESS);
        assert!(!out_sql.is_null());
        unsafe { dplyr_free_string(out_sql) };
        if !out_error.is_null() {
            unsafe { dplyr_free_string(out_error) };
        }
    }
}

// R3-AC3: Memory management tests
#[test]
fn test_memory_management() {
    // Test string allocation and deallocation
    let test_str = "test string for memory management";
    let c_string = CString::new(test_str).unwrap();
    let raw_ptr = c_string.into_raw();

    // Verify the string is valid
    let recovered = unsafe { CStr::from_ptr(raw_ptr) };
    assert_eq!(recovered.to_str().unwrap(), test_str);

    // Free it safely
    assert_eq!(unsafe { dplyr_free_string(raw_ptr) }, DPLYR_SUCCESS);

    // Test null pointer handling
    assert_eq!(
        unsafe { dplyr_free_string(std::ptr::null_mut()) },
        DPLYR_SUCCESS
    );

    // Test multiple string freeing
    let str1 = CString::new("test1").unwrap().into_raw();
    let str2 = CString::new("test2").unwrap().into_raw();
    let mut string_array = [str1, str2];

    let freed_count = unsafe { dplyr_free_strings(string_array.as_mut_ptr(), 2) };
    assert_eq!(freed_count, 2);

    // Test null array handling
    assert_eq!(
        unsafe { dplyr_free_strings(std::ptr::null_mut(), 0) },
        DPLYR_ERROR_NULL_POINTER
    );
}

// R8-AC1: Version and capability tests
#[test]
fn test_version_and_capabilities() {
    // Test version functions
    let version = unsafe { CStr::from_ptr(libdplyr_c_version_simple()) };
    assert_eq!(version.to_str().unwrap(), "0.2.0");

    let detailed = unsafe { CStr::from_ptr(dplyr_version_detailed()) };
    let detailed_str = detailed.to_str().unwrap();
    assert!(detailed_str.contains("libdplyr_c v0.2.0"));

    let dialects = unsafe { CStr::from_ptr(dplyr_supported_dialects()) };
    assert_eq!(dialects.to_str().unwrap(), "DuckDB");

    let timestamp = unsafe { CStr::from_ptr(dplyr_build_timestamp()) };
    let timestamp_str = timestamp.to_str().unwrap();
    assert!(!timestamp_str.is_empty());

    // Test capability functions
    let _has_debug = dplyr_has_debug_support();

    assert_eq!(dplyr_max_input_length(), MAX_INPUT_LENGTH as u32);
    assert_eq!(dplyr_max_processing_time_ms(), MAX_PROCESSING_TIME_MS);

    // Test system check
    assert_eq!(dplyr_check_system(), DPLYR_SUCCESS);
}

// Helper function tests
#[test]
fn test_helper_functions() {
    // Test nesting depth calculation
    assert_eq!(calculate_nesting_depth("()"), 1);
    assert_eq!(calculate_nesting_depth("(())"), 2);
    assert_eq!(calculate_nesting_depth("()()"), 1);
    assert_eq!(calculate_nesting_depth("((()))"), 3);
    assert_eq!(calculate_nesting_depth("select(filter(col1))"), 2);

    // Test function call counting
    assert_eq!(count_function_calls("func()"), 1);
    assert_eq!(count_function_calls("func1() func2()"), 2);
    assert_eq!(count_function_calls("select(col1) %>% filter(age > 18)"), 2);
    assert_eq!(count_function_calls("no functions here"), 0);
    assert_eq!(count_function_calls("func ( )"), 1); // With spaces

    // Test suspicious pattern detection
    assert!(contains_suspicious_patterns("'; DROP TABLE"));
    assert!(contains_suspicious_patterns("union select"));
    assert!(contains_suspicious_patterns("UNION SELECT"));
    assert!(contains_suspicious_patterns("<script>"));
    assert!(contains_suspicious_patterns("../"));
    assert!(!contains_suspicious_patterns("select(col1)"));

    // Test excessive repetition detection
    assert!(has_excessive_repetition(&"a".repeat(150)));
    assert!(has_excessive_repetition(&"abc".repeat(25)));
    assert!(!has_excessive_repetition("normal input"));
    assert!(!has_excessive_repetition(&"a".repeat(10))); // Well under threshold
}

// Error conversion tests
#[test]
fn test_error_conversion() {
    // Test libdplyr error conversion
    let lex_error =
        libdplyr::TranspileError::LexError(libdplyr::LexError::UnexpectedCharacter('x', 5));
    let converted = convert_libdplyr_error(lex_error);
    assert_eq!(converted.to_c_error_code(), DPLYR_ERROR_SYNTAX);

    let parse_error = libdplyr::TranspileError::ParseError(libdplyr::ParseError::UnexpectedToken {
        expected: "identifier".to_string(),
        found: "number".to_string(),
        position: 10,
    });
    let converted = convert_libdplyr_error(parse_error);
    assert_eq!(converted.to_c_error_code(), DPLYR_ERROR_SYNTAX);

    let gen_error = libdplyr::TranspileError::GenerationError(
        libdplyr::GenerationError::UnsupportedOperation {
            operation: "complex_join".to_string(),
            dialect: "simple_query".to_string(),
        },
    );
    let converted = convert_libdplyr_error(gen_error);
    assert_eq!(converted.to_c_error_code(), DPLYR_ERROR_UNSUPPORTED);
}

// Constants validation tests
#[test]
fn test_constants_validation() {
    // Verify constants are reasonable

    // Verify relationships

    // Test that constants match expected values
    assert_eq!(MAX_INPUT_LENGTH, 1024 * 1024); // 1MB
    assert_eq!(MAX_PROCESSING_TIME_MS, 30000); // 30 seconds
    assert_eq!(MAX_OUTPUT_LENGTH, 10 * 1024 * 1024); // 10MB
    assert_eq!(MAX_NESTING_DEPTH, 50);
    assert_eq!(MAX_FUNCTION_CALLS, 1000);
}

// String pointer validation tests
#[test]
fn test_string_pointer_validation() {
    // Test null pointer
    assert!(!unsafe { dplyr_is_valid_string_pointer(std::ptr::null()) });

    // Test valid string
    let valid_string = CString::new("test").unwrap();
    assert!(unsafe { dplyr_is_valid_string_pointer(valid_string.as_ptr()) });

    // Test empty string
    let empty_string = CString::new("").unwrap();
    assert!(unsafe { dplyr_is_valid_string_pointer(empty_string.as_ptr()) });
}

// Integration test with actual transpilation (if libdplyr is available)
#[test]
#[ignore] // Ignore by default since it requires libdplyr to be fully functional
fn test_full_transpilation_integration() {
    let mut out_sql: *mut c_char = std::ptr::null_mut();
    let mut out_error: *mut c_char = std::ptr::null_mut();

    let dplyr_code = CString::new("select(name, age)").unwrap();
    let options = DplyrOptions::default();

    let result =
        unsafe { dplyr_compile(dplyr_code.as_ptr(), &options, &mut out_sql, &mut out_error) };

    if result == DPLYR_SUCCESS {
        assert!(!out_sql.is_null());
        let sql_result = unsafe { CStr::from_ptr(out_sql) };
        let sql_str = sql_result.to_str().unwrap();
        assert!(!sql_str.is_empty());

        // Clean up
        unsafe { dplyr_free_string(out_sql) };
    } else {
        // If transpilation fails, we should have an error message
        assert!(!out_error.is_null());
        let error_result = unsafe { CStr::from_ptr(out_error) };
        let error_str = error_result.to_str().unwrap();
        assert!(!error_str.is_empty());

        // Clean up
        unsafe { dplyr_free_string(out_error) };
    }
}

// R6-AC1, R6-AC2: Performance validation tests
#[test]
fn test_simple_query_performance_target() {
    use std::time::Instant;

    let options = DplyrOptions::default();
    let query = "select(mpg, cyl)";

    // Warm up
    for _ in 0..10 {
        let _ = safe_dplyr_compile_test(query, &options);
    }

    // Measure performance over multiple runs
    let mut durations = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let result = safe_dplyr_compile_test(query, &options);
        durations.push(start.elapsed());

        // Verify the query actually works
        assert!(result.is_ok(), "Query should succeed: {:?}", result);
    }

    // Calculate P95
    durations.sort();
    let p95_index = (durations.len() as f64 * 0.95) as usize;
    let p95_duration = durations[p95_index];

    println!("Simple query P95: {:?}", p95_duration);

    // R6-AC1: Simple queries should be under 2ms P95
    const SIMPLE_QUERY_TARGET_MS: f64 = 2.0;
    assert!(
        p95_duration.as_millis() as f64 <= SIMPLE_QUERY_TARGET_MS,
        "Simple query P95 ({:?}) exceeds target ({}ms)",
        p95_duration,
        SIMPLE_QUERY_TARGET_MS
    );
}

#[test]
fn test_complex_query_performance_target() {
    use std::time::Instant;

    let options = DplyrOptions::default();
    let query = "mtcars %>% select(mpg, cyl, hp) %>% filter(mpg > 20) %>% group_by(cyl) %>% summarise(avg_hp = mean(hp)) %>% arrange(desc(avg_hp))";

    // Warm up
    for _ in 0..5 {
        let _ = safe_dplyr_compile_test(query, &options);
    }

    // Measure performance over multiple runs
    let mut durations = Vec::new();
    for _ in 0..50 {
        let start = Instant::now();
        let result = safe_dplyr_compile_test(query, &options);
        durations.push(start.elapsed());

        // Verify the query actually works
        assert!(result.is_ok(), "Query should succeed: {:?}", result);
    }

    // Calculate P95
    durations.sort();
    let p95_index = (durations.len() as f64 * 0.95) as usize;
    let p95_duration = durations[p95_index];

    println!("Complex query P95: {:?}", p95_duration);

    // R6-AC1: Complex queries should be under 15ms P95
    const COMPLEX_QUERY_TARGET_MS: f64 = 15.0;
    assert!(
        p95_duration.as_millis() as f64 <= COMPLEX_QUERY_TARGET_MS,
        "Complex query P95 ({:?}) exceeds target ({}ms)",
        p95_duration,
        COMPLEX_QUERY_TARGET_MS
    );
}

#[test]
fn test_cache_effectiveness() {
    use std::time::Instant;

    let options = DplyrOptions::default();
    let query = "select(mpg, cyl) %>% filter(mpg > 20)";

    // First call (cache miss)
    let start = Instant::now();
    let result1 = safe_dplyr_compile_test(query, &options);
    let cache_miss_duration = start.elapsed();

    assert!(result1.is_ok(), "First query should succeed");

    // Second call (cache hit)
    let start = Instant::now();
    let result2 = safe_dplyr_compile_test(query, &options);
    let cache_hit_duration = start.elapsed();

    assert!(result2.is_ok(), "Second query should succeed");
    assert_eq!(
        result1.unwrap(),
        result2.unwrap(),
        "Results should be identical"
    );

    println!(
        "Cache miss: {:?}, Cache hit: {:?}",
        cache_miss_duration, cache_hit_duration
    );

    // R6-AC2: Cache should provide significant speedup
    // Cache hit should be measurably faster than cache miss (>=20% faster)
    assert!(
        cache_hit_duration.as_nanos() * 5 < cache_miss_duration.as_nanos() * 4,
        "Cache not effective enough: miss={:?}, hit={:?}",
        cache_miss_duration,
        cache_hit_duration
    );
}

// Helper function for performance tests
#[allow(dead_code)]
fn safe_dplyr_compile_test(query: &str, options: &DplyrOptions) -> Result<String, String> {
    use std::ffi::{CStr, CString};

    let c_query = CString::new(query).unwrap();
    let mut out_sql: *mut c_char = std::ptr::null_mut();
    let mut out_error: *mut c_char = std::ptr::null_mut();

    let result = unsafe {
        dplyr_compile(
            c_query.as_ptr(),
            options as *const DplyrOptions,
            &mut out_sql,
            &mut out_error,
        )
    };

    if result == 0 {
        // Success
        let sql = unsafe {
            let c_str = CStr::from_ptr(out_sql);
            let rust_str = c_str.to_string_lossy().into_owned();
            dplyr_free_string(out_sql);
            rust_str
        };
        Ok(sql)
    } else {
        // Error
        let error = unsafe {
            let c_str = CStr::from_ptr(out_error);
            let rust_str = c_str.to_string_lossy().into_owned();
            dplyr_free_string(out_error);
            rust_str
        };
        Err(error)
    }
}

// DuckDB C Extension API init function
// This function is required for C API-based DuckDB extensions
// (Removed dplyr_extension_init_c_api to avoid conflict with C++ extension init)
// Initialization for C API extension
// (Removed dangling code)
