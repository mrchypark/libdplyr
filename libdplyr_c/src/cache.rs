//! Simple caching system for transpilation results
//!
//! Implements request-scoped caching to meet performance requirements
//! R6-AC1 (P95 < 2ms for simple pipelines, P95 < 15ms for complex pipelines)

use crate::error::TranspileError;
use crate::DplyrOptions;
use lru::LruCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct CachedResult {
    pub sql: String,
    pub timestamp: Instant,
    pub processing_time_us: u64, // R10-AC2: Metadata for diagnostics
    pub access_count: u64,       // R10-AC2: Access frequency tracking
    pub last_access: Instant,    // R6-AC1: LRU tracking
}

// R10-AC2: Cache performance metrics
#[derive(Clone, Default)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub total_processing_time_us: u64,
    pub cache_processing_time_us: u64, // Time spent on cache operations
}

// R6-AC1: LRU cache for performance optimization with proper eviction policy
thread_local! {
    static REQUEST_CACHE: std::cell::RefCell<LruCache<String, CachedResult>> =
        std::cell::RefCell::new(LruCache::new(NonZeroUsize::new(100).unwrap()));

    static CACHE_METRICS: std::cell::RefCell<CacheMetrics> =
        std::cell::RefCell::new(CacheMetrics::default());
}

pub struct SimpleTranspileCache;

impl SimpleTranspileCache {
    // R6-AC1: Cache-enabled transpilation to achieve P95 performance targets
    pub fn get_or_transpile<F>(
        dplyr_code: &str,
        options: &DplyrOptions,
        transpile_fn: F,
    ) -> Result<String, TranspileError>
    where
        F: FnOnce(&str, &DplyrOptions) -> Result<String, TranspileError>,
    {
        let cache_start = Instant::now();
        let cache_key = Self::create_cache_key(dplyr_code, options);

        // Cache lookup with LRU update
        let cached_result = REQUEST_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(mut cached) = cache.get(&cache_key).cloned() {
                // Update access tracking for LRU
                cached.access_count += 1;
                cached.last_access = Instant::now();

                // Re-insert to update LRU position
                cache.put(cache_key.clone(), cached.clone());
                Some(cached)
            } else {
                None
            }
        });

        if let Some(cached) = cached_result {
            // Cache expiration check (5 minutes)
            if cached.timestamp.elapsed() < Duration::from_secs(300) {
                // Record cache hit
                CACHE_METRICS.with(|metrics| {
                    let mut metrics = metrics.borrow_mut();
                    metrics.hits += 1;
                    metrics.cache_processing_time_us += cache_start.elapsed().as_micros() as u64;
                });

                return Ok(cached.sql);
            } else {
                // Expired entry - remove it
                REQUEST_CACHE.with(|cache| {
                    cache.borrow_mut().pop(&cache_key);
                });
            }
        }

        // Cache miss - record and perform actual transpilation
        CACHE_METRICS.with(|metrics| {
            metrics.borrow_mut().misses += 1;
        });

        let start_time = Instant::now();
        let sql = transpile_fn(dplyr_code, options)?;
        let processing_time = start_time.elapsed().as_micros() as u64;

        // Cache update with LRU eviction
        let evicted = REQUEST_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();

            let evicted = if cache.len() >= cache.cap().get() {
                cache.peek_lru().is_some()
            } else {
                false
            };

            // R6-AC1: LRU eviction policy - oldest entry automatically evicted
            cache.put(
                cache_key,
                CachedResult {
                    sql: sql.clone(),
                    timestamp: Instant::now(),
                    processing_time_us: processing_time,
                    access_count: 1,
                    last_access: Instant::now(),
                },
            );

            evicted
        });

        // Update metrics
        CACHE_METRICS.with(|metrics| {
            let mut metrics = metrics.borrow_mut();
            metrics.total_processing_time_us += processing_time;
            metrics.cache_processing_time_us += cache_start.elapsed().as_micros() as u64;
            if evicted {
                metrics.evictions += 1;
            }
        });

        Ok(sql)
    }

    // Generate cache key from dplyr_code + dialect + options
    fn create_cache_key(dplyr_code: &str, options: &DplyrOptions) -> String {
        let mut hasher = DefaultHasher::new();
        dplyr_code.hash(&mut hasher);
        "duckdb".hash(&mut hasher); // Fixed dialect for DuckDB extension
        options.strict_mode.hash(&mut hasher);
        options.preserve_comments.hash(&mut hasher);
        options.debug_mode.hash(&mut hasher);

        format!("{}_{}", hasher.finish(), dplyr_code.len())
    }

    // R10-AC2: Cache metadata exposure for diagnostics
    pub fn get_cache_stats() -> String {
        let (cache_info, metrics) = REQUEST_CACHE.with(|cache| {
            let cache = cache.borrow();
            let total_processing_time: u64 = cache
                .iter()
                .map(|(_, result)| result.processing_time_us)
                .sum();

            let total_access_count: u64 = cache.iter().map(|(_, result)| result.access_count).sum();

            let avg_processing_time = if cache.is_empty() {
                0
            } else {
                total_processing_time / cache.len() as u64
            };

            let avg_access_count = if cache.is_empty() {
                0.0
            } else {
                total_access_count as f64 / cache.len() as f64
            };

            let cache_info = (
                cache.len(),
                cache.cap().get(),
                avg_processing_time,
                avg_access_count,
            );

            let metrics = CACHE_METRICS.with(|m| m.borrow().clone());
            (cache_info, metrics)
        });

        let (size, capacity, avg_processing_time, avg_access_count) = cache_info;
        let hit_rate = if metrics.hits + metrics.misses > 0 {
            metrics.hits as f64 / (metrics.hits + metrics.misses) as f64 * 100.0
        } else {
            0.0
        };

        format!(
            r#"{{
            "cache_size": {},
            "max_cache_size": {},
            "hit_rate_percent": {:.2},
            "hits": {},
            "misses": {},
            "evictions": {},
            "avg_processing_time_us": {},
            "avg_access_count": {:.2},
            "total_processing_time_us": {},
            "cache_overhead_us": {}
        }}"#,
            size,
            capacity,
            hit_rate,
            metrics.hits,
            metrics.misses,
            metrics.evictions,
            avg_processing_time,
            avg_access_count,
            metrics.total_processing_time_us,
            metrics.cache_processing_time_us
        )
    }

    // R10-AC2: Get detailed cache performance metrics
    pub fn get_cache_metrics() -> CacheMetrics {
        CACHE_METRICS.with(|metrics| metrics.borrow().clone())
    }

    // Clear cache and reset metrics (useful for testing)
    pub fn clear_cache() {
        REQUEST_CACHE.with(|cache| {
            cache.borrow_mut().clear();
        });
        CACHE_METRICS.with(|metrics| {
            *metrics.borrow_mut() = CacheMetrics::default();
        });
    }

    // R6-AC1: Get cache hit rate for performance monitoring
    pub fn get_hit_rate() -> f64 {
        CACHE_METRICS.with(|metrics| {
            let metrics = metrics.borrow();
            if metrics.hits + metrics.misses > 0 {
                metrics.hits as f64 / (metrics.hits + metrics.misses) as f64
            } else {
                0.0
            }
        })
    }

    // R6-AC1: Check if cache is performing well (hit rate > 50%)
    pub fn is_cache_effective() -> bool {
        Self::get_hit_rate() > 0.5
    }

    // R10-AC2: Get most frequently accessed entries for analysis
    pub fn get_top_entries(limit: usize) -> Vec<(String, u64, u64)> {
        REQUEST_CACHE.with(|cache| {
            let cache = cache.borrow();
            let mut entries: Vec<_> = cache
                .iter()
                .map(|(key, result)| (key.clone(), result.access_count, result.processing_time_us))
                .collect();

            // Sort by access count descending
            entries.sort_by(|a, b| b.1.cmp(&a.1));
            entries.truncate(limit);
            entries
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_generation() {
        let options = DplyrOptions::default();
        let key1 = SimpleTranspileCache::create_cache_key("select(col1)", &options);
        let key2 = SimpleTranspileCache::create_cache_key("select(col2)", &options);

        // Different code should generate different keys
        assert_ne!(key1, key2);

        // Same code should generate same key
        let key3 = SimpleTranspileCache::create_cache_key("select(col1)", &options);
        assert_eq!(key1, key3);
    }

    #[test]
    fn test_cache_stats_format() {
        SimpleTranspileCache::clear_cache();
        let stats = SimpleTranspileCache::get_cache_stats();

        // Should be valid JSON-like format
        assert!(stats.contains("cache_size"));
        assert!(stats.contains("max_cache_size"));
        assert!(stats.contains("100")); // max cache size
    }

    #[test]
    fn test_cache_functionality() {
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

        // Verify cache miss was recorded
        let metrics = SimpleTranspileCache::get_cache_metrics();
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.hits, 0);

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

        // Verify cache hit was recorded
        let metrics = SimpleTranspileCache::get_cache_metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);

        // Verify hit rate
        assert_eq!(SimpleTranspileCache::get_hit_rate(), 0.5); // 1 hit out of 2 total
    }

    #[test]
    fn test_lru_eviction() {
        SimpleTranspileCache::clear_cache();
        let options = DplyrOptions::default();

        // Fill cache to capacity (100 entries)
        for i in 0..100 {
            let code = format!("select(col{})", i);
            let _ = SimpleTranspileCache::get_or_transpile(&code, &options, |_code, _opts| {
                Ok(format!("SELECT col{} FROM table", i))
            });
        }

        assert_eq!(dplyr_cache_get_size(), 100);
        assert_eq!(dplyr_cache_get_evictions(), 0);

        // Add one more entry - should trigger eviction
        let _ =
            SimpleTranspileCache::get_or_transpile("select(new_col)", &options, |_code, _opts| {
                Ok("SELECT new_col FROM table".to_string())
            });

        assert_eq!(dplyr_cache_get_size(), 100); // Size should remain at capacity
        assert_eq!(dplyr_cache_get_evictions(), 1); // One eviction should have occurred
    }

    #[test]
    fn test_cache_access_tracking() {
        SimpleTranspileCache::clear_cache();
        let options = DplyrOptions::default();

        // Add entry and access it multiple times
        for _ in 0..5 {
            let _ = SimpleTranspileCache::get_or_transpile(
                "select(popular_col)",
                &options,
                |_code, _opts| Ok("SELECT popular_col FROM table".to_string()),
            );
        }

        // Check top entries
        let top_entries = SimpleTranspileCache::get_top_entries(1);
        assert_eq!(top_entries.len(), 1);
        assert_eq!(top_entries[0].1, 5); // Should have 5 accesses (1 miss + 4 hits)
    }

    #[test]
    fn test_ffi_cache_functions() {
        SimpleTranspileCache::clear_cache();

        // Test initial state
        assert_eq!(dplyr_cache_get_size(), 0);
        assert_eq!(dplyr_cache_get_capacity(), 100);
        assert_eq!(dplyr_cache_get_hits(), 0);
        assert_eq!(dplyr_cache_get_misses(), 0);
        assert_eq!(dplyr_cache_get_hit_rate(), 0.0);
        assert!(!dplyr_cache_is_effective());

        // Add some entries
        let options = DplyrOptions::default();
        for i in 0..5 {
            let code = format!("select(col{})", i);
            let _ = SimpleTranspileCache::get_or_transpile(&code, &options, |_code, _opts| {
                Ok(format!("SELECT col{} FROM table", i))
            });
        }

        assert_eq!(dplyr_cache_get_size(), 5);
        assert_eq!(dplyr_cache_get_misses(), 5);

        // Access first entry again (cache hit)
        let _ = SimpleTranspileCache::get_or_transpile("select(col0)", &options, |_code, _opts| {
            Ok("SELECT col0 FROM table".to_string())
        });

        assert_eq!(dplyr_cache_get_hits(), 1);
        assert!(dplyr_cache_get_hit_rate() > 0.0);

        // Test cache stats JSON
        let stats_ptr = dplyr_cache_get_stats();
        assert!(!stats_ptr.is_null());

        let stats_str = unsafe { std::ffi::CStr::from_ptr(stats_ptr).to_string_lossy() };
        assert!(stats_str.contains("cache_size"));
        assert!(stats_str.contains("hit_rate_percent"));

        // Clean up
        unsafe {
            let _ = CString::from_raw(stats_ptr);
        }

        // Test clear function
        assert_eq!(dplyr_cache_clear(), 0);
        assert_eq!(dplyr_cache_get_size(), 0);
        assert_eq!(dplyr_cache_get_hits(), 0);
        assert_eq!(dplyr_cache_get_misses(), 0);
    }

    #[test]
    fn test_debug_logging_functions() {
        SimpleTranspileCache::clear_cache();

        // Test basic logging (should not panic)
        dplyr_cache_log_stats(std::ptr::null());
        dplyr_cache_log_stats(b"TEST_PREFIX\0".as_ptr() as *const c_char);

        // Test detailed logging
        dplyr_cache_log_stats_detailed(std::ptr::null(), false);
        dplyr_cache_log_stats_detailed(b"DETAILED_TEST\0".as_ptr() as *const c_char, true);

        // Add some cache entries to test with data
        let options = DplyrOptions::default();
        for i in 0..3 {
            let code = format!("select(test_col{})", i);
            let _ = SimpleTranspileCache::get_or_transpile(&code, &options, |_code, _opts| {
                Ok(format!("SELECT test_col{} FROM table", i))
            });
        }

        // Test logging with data
        dplyr_cache_log_stats_detailed(b"WITH_DATA\0".as_ptr() as *const c_char, true);

        // Test performance warning (should not warn with good performance)
        let warned = dplyr_cache_log_performance_warning();
        assert!(!warned); // Should not warn with only 3 entries

        // Test should_clear function
        let should_clear = dplyr_cache_should_clear();
        assert!(!should_clear); // Should not clear with good performance
    }

    #[test]
    fn test_cache_performance_warning() {
        SimpleTranspileCache::clear_cache();

        // Create scenario with poor hit rate
        let options = DplyrOptions::default();

        // Add many different entries (all misses)
        for i in 0..25 {
            let code = format!("select(unique_col{})", i);
            let _ = SimpleTranspileCache::get_or_transpile(&code, &options, |_code, _opts| {
                Ok(format!("SELECT unique_col{} FROM table", i))
            });
        }

        // Now we have 25 misses, 0 hits - very poor hit rate
        let hit_rate = SimpleTranspileCache::get_hit_rate();
        assert_eq!(hit_rate, 0.0);

        // Should trigger performance warning
        let warned = dplyr_cache_log_performance_warning();
        assert!(warned);

        // Should recommend clearing
        let should_clear = dplyr_cache_should_clear();
        assert!(should_clear);
    }

    #[test]
    fn test_cache_metrics_detailed() {
        SimpleTranspileCache::clear_cache();

        let options = DplyrOptions::default();

        // Add entry and access multiple times
        for _i in 0..5 {
            let _ = SimpleTranspileCache::get_or_transpile(
                "select(popular_column)",
                &options,
                |_code, _opts| {
                    // Simulate some processing time
                    std::thread::sleep(std::time::Duration::from_micros(100));
                    Ok("SELECT popular_column FROM table".to_string())
                },
            );
        }

        let metrics = SimpleTranspileCache::get_cache_metrics();
        assert_eq!(metrics.hits, 4); // 1 miss + 4 hits
        assert_eq!(metrics.misses, 1);
        assert!(metrics.total_processing_time_us > 0);
        assert!(metrics.cache_processing_time_us > 0);

        // Test detailed stats
        let stats = SimpleTranspileCache::get_cache_stats();
        assert!(stats.contains("hit_rate_percent"));
        assert!(stats.contains("total_processing_time_us"));
        assert!(stats.contains("cache_overhead_us"));

        // Test top entries
        let top_entries = SimpleTranspileCache::get_top_entries(5);
        assert_eq!(top_entries.len(), 1);
        assert_eq!(top_entries[0].1, 5); // 5 total accesses
    }
}

// FFI functions for cache management

use std::ffi::{c_char, CString};
use std::os::raw::c_int;

/// Get cache statistics as JSON string
///
/// # Returns
/// JSON string with cache statistics (must be freed with dplyr_free_string)
#[no_mangle]
pub extern "C" fn dplyr_cache_get_stats() -> *mut c_char {
    let stats = SimpleTranspileCache::get_cache_stats();
    match CString::new(stats) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get cache hit rate as percentage
///
/// # Returns
/// Hit rate as percentage (0.0 to 100.0)
#[no_mangle]
pub extern "C" fn dplyr_cache_get_hit_rate() -> f64 {
    SimpleTranspileCache::get_hit_rate() * 100.0
}

/// Check if cache is performing effectively
///
/// # Returns
/// true if hit rate > 50%, false otherwise
#[no_mangle]
pub extern "C" fn dplyr_cache_is_effective() -> bool {
    SimpleTranspileCache::is_cache_effective()
}

/// Clear cache and reset all metrics
///
/// # Returns
/// 0 on success, negative error code on failure
#[no_mangle]
pub extern "C" fn dplyr_cache_clear() -> c_int {
    let result = std::panic::catch_unwind(|| {
        SimpleTranspileCache::clear_cache();
        0
    });

    match result {
        Ok(code) => code,
        Err(_) => -1, // Panic error
    }
}

/// Get current cache size
///
/// # Returns
/// Number of entries currently in cache
#[no_mangle]
pub extern "C" fn dplyr_cache_get_size() -> usize {
    REQUEST_CACHE.with(|cache| cache.borrow().len())
}

/// Get maximum cache capacity
///
/// # Returns
/// Maximum number of entries cache can hold
#[no_mangle]
pub extern "C" fn dplyr_cache_get_capacity() -> usize {
    REQUEST_CACHE.with(|cache| cache.borrow().cap().get())
}

/// Get total number of cache hits
///
/// # Returns
/// Total cache hits since last clear
#[no_mangle]
pub extern "C" fn dplyr_cache_get_hits() -> u64 {
    CACHE_METRICS.with(|metrics| metrics.borrow().hits)
}

/// Get total number of cache misses
///
/// # Returns
/// Total cache misses since last clear
#[no_mangle]
pub extern "C" fn dplyr_cache_get_misses() -> u64 {
    CACHE_METRICS.with(|metrics| metrics.borrow().misses)
}

/// Get total number of cache evictions
///
/// # Returns
/// Total evictions since last clear
#[no_mangle]
pub extern "C" fn dplyr_cache_get_evictions() -> u64 {
    CACHE_METRICS.with(|metrics| metrics.borrow().evictions)
}

/// Log cache statistics to stderr (for debugging)
///
/// # Arguments
/// * `prefix` - Optional prefix for log message (can be null)
#[no_mangle]
pub extern "C" fn dplyr_cache_log_stats(prefix: *const c_char) {
    let prefix_str = if prefix.is_null() {
        "CACHE_STATS"
    } else {
        unsafe {
            match std::ffi::CStr::from_ptr(prefix).to_str() {
                Ok(s) => s,
                Err(_) => "CACHE_STATS",
            }
        }
    };

    let stats = SimpleTranspileCache::get_cache_stats();
    eprintln!("{}: {}", prefix_str, stats);
}

/// Log cache statistics with timestamp (R10-AC2: Debug mode logging)
///
/// # Arguments
/// * `prefix` - Optional prefix for log message (can be null)
/// * `include_timestamp` - Whether to include timestamp in log
#[no_mangle]
pub extern "C" fn dplyr_cache_log_stats_detailed(prefix: *const c_char, include_timestamp: bool) {
    let prefix_str = if prefix.is_null() {
        "CACHE_STATS"
    } else {
        unsafe {
            match std::ffi::CStr::from_ptr(prefix).to_str() {
                Ok(s) => s,
                Err(_) => "CACHE_STATS",
            }
        }
    };

    let timestamp_str = if include_timestamp {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("[{}] ", timestamp)
    } else {
        String::new()
    };

    let stats = SimpleTranspileCache::get_cache_stats();
    let metrics = SimpleTranspileCache::get_cache_metrics();
    let hit_rate = SimpleTranspileCache::get_hit_rate();

    eprintln!(
        "{}{}: {} (hit_rate: {:.2}%, effective: {})",
        timestamp_str,
        prefix_str,
        stats,
        hit_rate * 100.0,
        SimpleTranspileCache::is_cache_effective()
    );

    // R10-AC2: Additional debug information in detailed mode
    if metrics.hits + metrics.misses > 0 {
        eprintln!(
            "{}CACHE_PERFORMANCE: avg_processing_time: {}μs, cache_overhead: {}μs",
            timestamp_str,
            if metrics.misses > 0 {
                metrics.total_processing_time_us / metrics.misses
            } else {
                0
            },
            if metrics.hits + metrics.misses > 0 {
                metrics.cache_processing_time_us / (metrics.hits + metrics.misses)
            } else {
                0
            }
        );
    }
}

/// Log cache performance warning if performance is poor (R10-AC2)
///
/// # Returns
/// true if warning was logged, false if performance is acceptable
#[no_mangle]
pub extern "C" fn dplyr_cache_log_performance_warning() -> bool {
    let metrics = SimpleTranspileCache::get_cache_metrics();
    let hit_rate = SimpleTranspileCache::get_hit_rate();

    // Only warn if we have enough data points
    if metrics.hits + metrics.misses < 10 {
        return false;
    }

    let mut warnings = Vec::new();

    // Check hit rate
    if hit_rate < 0.3 {
        warnings.push(format!("Low hit rate: {:.1}%", hit_rate * 100.0));
    }

    // Check cache overhead
    if metrics.hits + metrics.misses > 0 {
        let avg_cache_overhead = metrics.cache_processing_time_us / (metrics.hits + metrics.misses);
        let avg_processing_time = if metrics.misses > 0 {
            metrics.total_processing_time_us / metrics.misses
        } else {
            0
        };

        // Warn if cache overhead is more than 10% of processing time
        if avg_cache_overhead > avg_processing_time / 10 {
            warnings.push(format!(
                "High cache overhead: {}μs vs {}μs processing",
                avg_cache_overhead, avg_processing_time
            ));
        }
    }

    // Check excessive evictions
    if metrics.evictions > metrics.hits / 2 {
        warnings.push(format!(
            "Excessive evictions: {} ({}% of hits)",
            metrics.evictions,
            if metrics.hits > 0 {
                metrics.evictions * 100 / metrics.hits
            } else {
                0
            }
        ));
    }

    if !warnings.is_empty() {
        eprintln!("CACHE_WARNING: Performance issues detected:");
        for warning in warnings {
            eprintln!("  - {}", warning);
        }
        eprintln!("  Consider clearing cache or adjusting cache size");
        return true;
    }

    false
}

/// Check if cache should be cleared based on performance
///
/// # Returns
/// true if cache performance is poor and should be cleared
#[no_mangle]
pub extern "C" fn dplyr_cache_should_clear() -> bool {
    let metrics = SimpleTranspileCache::get_cache_metrics();

    // Clear if hit rate is very low (< 10%) and we have enough data points
    if metrics.hits + metrics.misses >= 20 {
        let hit_rate = metrics.hits as f64 / (metrics.hits + metrics.misses) as f64;
        hit_rate < 0.1
    } else {
        false
    }
}
