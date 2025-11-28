//! Performance validation tests for libdplyr_c
//!
//! This module contains performance tests that validate the requirements
//! from R6-AC1 and R6-AC2 regarding performance targets and caching effectiveness.

#[cfg(test)]
mod tests {
    use crate::{dplyr_compile, dplyr_free_string, DplyrOptions};
    use std::ffi::{CStr, CString};
    use std::ptr;
    use std::time::Instant;

    // Performance targets from R6-AC1
    const SIMPLE_QUERY_TARGET_MS: f64 = 2.0; // P95 < 2ms for simple queries
    const COMPLEX_QUERY_TARGET_MS: f64 = 15.0; // P95 < 15ms for complex queries
    const ERROR_PERFORMANCE_SAMPLES: usize = 60; // Enough samples so P95 isn't dominated by one cold run
    const ERROR_PERFORMANCE_WARMUP: usize = 5; // Warm-up runs to stabilize cache and allocator

    // Helper function to safely call dplyr_compile
    fn safe_dplyr_compile_test(query: &str, options: &DplyrOptions) -> Result<String, String> {
        let c_query = CString::new(query).unwrap();
        let mut out_sql: *mut i8 = ptr::null_mut();
        let mut out_error: *mut i8 = ptr::null_mut();

        let result = unsafe { dplyr_compile(
            c_query.as_ptr(),
            options as *const DplyrOptions,
            &mut out_sql,
            &mut out_error,
        ) };

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

    #[test]
    fn test_simple_query_performance_target() {
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
        assert!(
            p95_duration.as_millis() as f64 <= SIMPLE_QUERY_TARGET_MS,
            "Simple query P95 ({:?}) exceeds target ({}ms)",
            p95_duration,
            SIMPLE_QUERY_TARGET_MS
        );
    }

    #[test]
    fn test_complex_query_performance_target() {
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
        assert!(
            p95_duration.as_millis() as f64 <= COMPLEX_QUERY_TARGET_MS,
            "Complex query P95 ({:?}) exceeds target ({}ms)",
            p95_duration,
            COMPLEX_QUERY_TARGET_MS
        );
    }

    #[test]
    fn test_cache_effectiveness() {
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
        // Cache hit should be at least 2x faster than cache miss
        assert!(
            cache_hit_duration.as_nanos() * 2 < cache_miss_duration.as_nanos(),
            "Cache not effective: miss={:?}, hit={:?}",
            cache_miss_duration,
            cache_hit_duration
        );
    }

    #[test]
    fn test_performance_consistency() {
        let options = DplyrOptions::default();
        let queries = [
            "select(mpg)",
            "filter(cyl == 4)",
            "mutate(efficiency = mpg / cyl)",
            "arrange(hp)",
            "group_by(gear)",
            "summarise(avg_mpg = mean(mpg))",
        ];

        for query in &queries {
            // Warm up
            for _ in 0..5 {
                let _ = safe_dplyr_compile_test(query, &options);
            }

            // Measure performance
            let mut durations = Vec::new();
            for _ in 0..20 {
                let start = Instant::now();
                let result = safe_dplyr_compile_test(query, &options);
                durations.push(start.elapsed());

                assert!(result.is_ok(), "Query '{}' should succeed", query);
            }

            // Calculate statistics
            durations.sort();
            let p95_index = (durations.len() as f64 * 0.95) as usize;
            let p95_duration = durations[p95_index];

            println!("Query '{}' P95: {:?}", query, p95_duration);

            // All simple queries should meet the performance target
            assert!(
                p95_duration.as_millis() as f64 <= SIMPLE_QUERY_TARGET_MS,
                "Query '{}' P95 ({:?}) exceeds target ({}ms)",
                query,
                p95_duration,
                SIMPLE_QUERY_TARGET_MS
            );
        }
    }

    #[test]
    fn test_error_handling_performance() {
        let options = DplyrOptions::default();
        let invalid_queries = [
            "",                       // Empty query
            "invalid_function(test)", // Invalid function
            "select(col1 col2)",      // Syntax error
            "filter()",               // Empty filter
        ];

        for query in &invalid_queries {
            // Warm-up to avoid counting cold-start penalties
            for _ in 0..ERROR_PERFORMANCE_WARMUP {
                let _ = safe_dplyr_compile_test(query, &options);
            }

            // Measure error handling performance
            let mut durations = Vec::new();
            for _ in 0..ERROR_PERFORMANCE_SAMPLES {
                let start = Instant::now();
                let result = safe_dplyr_compile_test(query, &options);
                durations.push(start.elapsed());

                // Should either succeed or fail gracefully, but not panic
                if result.is_ok() {}
            }

            // Calculate P95
            durations.sort();
            let p95_index = (durations.len() as f64 * 0.95) as usize;
            let p95_duration = durations[p95_index];

            println!("Error handling for '{}' P95: {:?}", query, p95_duration);

            // Error handling should also be fast
            assert!(
                p95_duration.as_millis() as f64 <= SIMPLE_QUERY_TARGET_MS,
                "Error handling for '{}' P95 ({:?}) exceeds target ({}ms)",
                query,
                p95_duration,
                SIMPLE_QUERY_TARGET_MS
            );
        }
    }

    #[test]
    fn test_memory_stability() {
        let options = DplyrOptions::default();
        let query = "select(mpg, cyl) %>% filter(mpg > 20)";

        // Run many iterations to check for memory leaks
        for i in 0..1000 {
            let result = safe_dplyr_compile_test(query, &options);
            assert!(result.is_ok(), "Iteration {} should succeed", i);

            // Occasionally check that performance doesn't degrade
            if i % 100 == 0 {
                let start = Instant::now();
                let _ = safe_dplyr_compile_test(query, &options);
                let duration = start.elapsed();

                // Performance should remain consistent
                assert!(
                    duration.as_millis() as f64 <= SIMPLE_QUERY_TARGET_MS * 2.0,
                    "Performance degraded at iteration {}: {:?}",
                    i,
                    duration
                );
            }
        }
    }
}
