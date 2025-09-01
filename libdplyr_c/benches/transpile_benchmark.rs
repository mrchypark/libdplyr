//! Performance benchmarks for libdplyr_c
//!
//! This benchmark suite validates the performance requirements from R6-AC1:
//! - Simple queries: P95 < 2ms
//! - Complex queries: P95 < 15ms
//! - Extension loading: < 50ms
//!
//! Requirements fulfilled:
//! - R6-AC1: Performance target validation
//! - R6-AC2: Caching effectiveness measurement

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use libdplyr_c::{dplyr_compile, dplyr_free_string, DplyrOptions};
use std::ffi::{CStr, CString};
use std::ptr;
use std::time::{Duration, Instant};

// Test data for benchmarks - R6-AC1: Performance target validation
const SIMPLE_QUERIES: &[&str] = &[
    "select(mpg)",
    "filter(mpg > 20)",
    "mutate(efficiency = mpg / cyl)",
    "arrange(mpg)",
    "group_by(cyl)",
    "summarise(avg_mpg = mean(mpg))",
    "select(name, age)",
    "filter(status == 'active')",
    "mutate(total = price * quantity)",
    "arrange(desc(date))",
];

const COMPLEX_QUERIES: &[&str] = &[
    "mtcars %>% select(mpg, cyl, hp) %>% filter(mpg > 20) %>% arrange(desc(hp))",
    "data %>% group_by(category, region) %>% summarise(total = sum(value), avg = mean(value), count = n()) %>% arrange(desc(total))",
    "sales %>% select(date, product, amount, region) %>% filter(amount > 1000) %>% mutate(quarter = quarter(date), profit_margin = amount * 0.15) %>% group_by(quarter, region) %>% summarise(total_sales = sum(amount), avg_margin = mean(profit_margin), product_count = n_distinct(product)) %>% arrange(quarter, desc(total_sales))",
    "employees %>% select(id, name, department, salary, hire_date) %>% filter(salary > 50000 & department %in% c('Engineering', 'Sales')) %>% mutate(years_employed = year(today()) - year(hire_date), salary_grade = case_when(salary < 75000 ~ 'Junior', salary < 100000 ~ 'Mid', TRUE ~ 'Senior')) %>% group_by(department, salary_grade) %>% summarise(count = n(), avg_salary = mean(salary), avg_years = mean(years_employed)) %>% arrange(department, desc(avg_salary))",
    "orders %>% select(order_id, customer_id, product_id, quantity, price, order_date) %>% filter(order_date >= '2023-01-01' & quantity > 0) %>% mutate(total_amount = quantity * price, month = month(order_date), year = year(order_date)) %>% group_by(year, month, customer_id) %>% summarise(order_count = n(), total_spent = sum(total_amount), avg_order_value = mean(total_amount), unique_products = n_distinct(product_id)) %>% filter(total_spent > 1000) %>% arrange(year, month, desc(total_spent))",
];

const EDGE_CASE_QUERIES: &[&str] = &[
    "", // Empty query
    "select()", // Empty select
    "filter()", // Empty filter
    "invalid_function(test)", // Invalid function
    "select(col1 col2)", // Syntax error
];

// Performance targets from R6-AC1
const SIMPLE_QUERY_TARGET_MS: f64 = 2.0;   // P95 < 2ms for simple queries
const COMPLEX_QUERY_TARGET_MS: f64 = 15.0; // P95 < 15ms for complex queries

// Helper function to create default options
fn create_default_options() -> DplyrOptions {
    DplyrOptions {
        strict_mode: false,
        preserve_comments: false,
        debug_mode: false,
        max_input_length: 10000,
        max_processing_time_ms: 5000,
    }
}

// Helper function to create strict options
fn create_strict_options() -> DplyrOptions {
    DplyrOptions {
        strict_mode: true,
        preserve_comments: false,
        debug_mode: false,
        max_input_length: 10000,
        max_processing_time_ms: 5000,
    }
}

// Helper function to create debug options
fn create_debug_options() -> DplyrOptions {
    DplyrOptions {
        strict_mode: false,
        preserve_comments: true,
        debug_mode: true,
        max_input_length: 50000,
        max_processing_time_ms: 10000,
    }
}

// Helper function to safely call dplyr_compile
fn safe_dplyr_compile(query: &str, options: &DplyrOptions) -> Result<String, String> {
    let c_query = CString::new(query).unwrap();
    let mut out_sql: *mut i8 = ptr::null_mut();
    let mut out_error: *mut i8 = ptr::null_mut();
    
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

// R6-AC1: Benchmark simple queries (target <2ms P95)
fn bench_simple_queries(c: &mut Criterion) {
    let options = create_default_options();
    
    let mut group = c.benchmark_group("simple_transpile");
    group.significance_level(0.1).sample_size(1000);
    
    for (i, query) in SIMPLE_QUERIES.iter().enumerate() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("simple", i),
            query,
            |b, query| {
                b.iter(|| {
                    let result = safe_dplyr_compile(black_box(query), black_box(&options));
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

// R6-AC1: Benchmark complex queries (target <15ms P95)
fn bench_complex_queries(c: &mut Criterion) {
    let options = create_default_options();
    
    let mut group = c.benchmark_group("complex_transpile");
    group.significance_level(0.1).sample_size(500);
    group.measurement_time(Duration::from_secs(10));
    
    for (i, query) in COMPLEX_QUERIES.iter().enumerate() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("complex", i),
            query,
            |b, query| {
                b.iter(|| {
                    let result = safe_dplyr_compile(black_box(query), black_box(&options));
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

// Benchmark error handling performance
fn bench_error_handling(c: &mut Criterion) {
    let options = create_strict_options();
    
    let mut group = c.benchmark_group("error_handling");
    group.significance_level(0.1).sample_size(500);
    
    for (i, query) in EDGE_CASE_QUERIES.iter().enumerate() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("error", i),
            query,
            |b, query| {
                b.iter(|| {
                    let result = safe_dplyr_compile(black_box(query), black_box(&options));
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

// R6-AC2: Benchmark caching performance
fn bench_caching_performance(c: &mut Criterion) {
    let options = create_default_options();
    let query = "mtcars %>% select(mpg, cyl) %>% filter(mpg > 20)";
    
    let mut group = c.benchmark_group("caching");
    group.significance_level(0.1).sample_size(1000);
    
    // First call (cache miss)
    group.bench_function("cache_miss", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);
            for i in 0..iters {
                // Create unique query to avoid cache hits
                let unique_query = format!("{} -- iteration {}", query, i);
                let start = Instant::now();
                let result = safe_dplyr_compile(black_box(&unique_query), black_box(&options));
                total_duration += start.elapsed();
                black_box(result);
            }
            total_duration
        });
    });
    
    // Repeated calls (cache hit)
    group.bench_function("cache_hit", |b| {
        // Prime the cache
        let _ = safe_dplyr_compile(query, &options);
        
        b.iter(|| {
            let result = safe_dplyr_compile(black_box(query), black_box(&options));
            black_box(result)
        });
    });
    
    group.finish();
}

// Benchmark different options configurations
fn bench_options_impact(c: &mut Criterion) {
    let query = "data %>% select(col1, col2) %>% filter(col1 > 100) %>% arrange(col2)";
    
    let mut group = c.benchmark_group("options_impact");
    group.significance_level(0.1).sample_size(500);
    
    let configs = [
        ("default", create_default_options()),
        ("strict", create_strict_options()),
        ("debug", create_debug_options()),
    ];
    
    for (name, options) in &configs {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("options", name),
            options,
            |b, options| {
                b.iter(|| {
                    let result = safe_dplyr_compile(black_box(query), black_box(options));
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

// Benchmark memory allocation patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let options = create_default_options();
    
    let mut group = c.benchmark_group("memory_patterns");
    group.significance_level(0.1).sample_size(500);
    
    // Small queries
    let small_query = "select(col1)";
    group.throughput(Throughput::Bytes(small_query.len() as u64));
    group.bench_function("small_query", |b| {
        b.iter(|| {
            let result = safe_dplyr_compile(black_box(small_query), black_box(&options));
            black_box(result)
        });
    });
    
    // Medium queries
    let medium_query = "data %>% select(col1, col2, col3) %>% filter(col1 > 0) %>% group_by(col2) %>% summarise(total = sum(col3))";
    group.throughput(Throughput::Bytes(medium_query.len() as u64));
    group.bench_function("medium_query", |b| {
        b.iter(|| {
            let result = safe_dplyr_compile(black_box(medium_query), black_box(&options));
            black_box(result)
        });
    });
    
    // Large queries
    let large_query = format!(
        "data %>% select({}) %>% filter(col1 > 0) %>% group_by(col2) %>% summarise({})",
        (1..=50).map(|i| format!("col{}", i)).collect::<Vec<_>>().join(", "),
        (1..=20).map(|i| format!("sum{} = sum(col{})", i, i)).collect::<Vec<_>>().join(", ")
    );
    group.throughput(Throughput::Bytes(large_query.len() as u64));
    group.bench_function("large_query", |b| {
        b.iter(|| {
            let result = safe_dplyr_compile(black_box(&large_query), black_box(&options));
            black_box(result)
        });
    });
    
    group.finish();
}

// Benchmark concurrent access patterns (simulated)
fn bench_concurrent_patterns(c: &mut Criterion) {
    let options = create_default_options();
    
    let queries = [
        "select(mpg)",
        "filter(cyl == 4)",
        "mutate(efficiency = mpg / cyl)",
        "arrange(hp)",
        "group_by(gear)",
        "summarise(avg_mpg = mean(mpg))",
    ];
    
    let mut group = c.benchmark_group("concurrent_simulation");
    group.significance_level(0.1).sample_size(500);
    
    // Simulate concurrent access by rapidly switching between different queries
    group.bench_function("rapid_switching", |b| {
        let mut counter = 0;
        b.iter(|| {
            let query = queries[counter % queries.len()];
            counter += 1;
            let result = safe_dplyr_compile(black_box(query), black_box(&options));
            black_box(result)
        });
    });
    
    // Simulate mixed workload
    group.bench_function("mixed_workload", |b| {
        let mut counter = 0;
        b.iter(|| {
            let query = if counter % 3 == 0 {
                // Complex query every 3rd iteration
                COMPLEX_QUERIES[counter % COMPLEX_QUERIES.len()]
            } else {
                // Simple queries otherwise
                SIMPLE_QUERIES[counter % SIMPLE_QUERIES.len()]
            };
            counter += 1;
            let result = safe_dplyr_compile(black_box(query), black_box(&options));
            black_box(result)
        });
    });
    
    group.finish();
}

// Benchmark input size scaling
fn bench_input_scaling(c: &mut Criterion) {
    let options = create_default_options();
    
    let mut group = c.benchmark_group("input_scaling");
    group.significance_level(0.1).sample_size(200);
    
    // Generate queries of different sizes
    let sizes = [10, 50, 100, 500, 1000];
    
    for size in sizes {
        let columns: Vec<String> = (1..=size).map(|i| format!("col{}", i)).collect();
        let query = format!("select({})", columns.join(", "));
        
        group.throughput(Throughput::Bytes(query.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("columns", size),
            &query,
            |b, query| {
                b.iter(|| {
                    let result = safe_dplyr_compile(black_box(query), black_box(&options));
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

// Benchmark pipeline depth scaling
fn bench_pipeline_depth(c: &mut Criterion) {
    let options = create_default_options();
    
    let mut group = c.benchmark_group("pipeline_depth");
    group.significance_level(0.1).sample_size(200);
    
    let depths = [1, 3, 5, 10, 20];
    
    for depth in depths {
        let mut query = "data".to_string();
        for i in 0..depth {
            query.push_str(&format!(" %>% mutate(col{} = col{} + 1)", i, i));
        }
        
        group.throughput(Throughput::Elements(depth as u64));
        group.bench_with_input(
            BenchmarkId::new("depth", depth),
            &query,
            |b, query| {
                b.iter(|| {
                    let result = safe_dplyr_compile(black_box(query), black_box(&options));
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

criterion_group! {
    benches,
    bench_simple_queries,
    bench_complex_queries,
    bench_error_handling,
    bench_caching_performance,
    bench_options_impact,
    bench_memory_patterns,
    bench_concurrent_patterns,
    bench_input_scaling,
    bench_pipeline_depth
}

criterion_main!(benches);

// Performance validation tests
#[cfg(test)]
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_simple_query_performance_target() {
        let options = create_default_options();
        let query = "select(mpg, cyl)";
        
        // Warm up
        for _ in 0..10 {
            let _ = safe_dplyr_compile(query, &options);
        }
        
        // Measure performance over multiple runs
        let mut durations = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            let _ = safe_dplyr_compile(query, &options);
            durations.push(start.elapsed());
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
        let options = create_default_options();
        let query = "mtcars %>% select(mpg, cyl, hp) %>% filter(mpg > 20) %>% group_by(cyl) %>% summarise(avg_hp = mean(hp)) %>% arrange(desc(avg_hp))";
        
        // Warm up
        for _ in 0..5 {
            let _ = safe_dplyr_compile(query, &options);
        }
        
        // Measure performance over multiple runs
        let mut durations = Vec::new();
        for _ in 0..50 {
            let start = Instant::now();
            let _ = safe_dplyr_compile(query, &options);
            durations.push(start.elapsed());
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
        let options = create_default_options();
        let query = "select(mpg, cyl) %>% filter(mpg > 20)";
        
        // First call (cache miss)
        let start = Instant::now();
        let _ = safe_dplyr_compile(query, &options);
        let cache_miss_duration = start.elapsed();
        
        // Second call (cache hit)
        let start = Instant::now();
        let _ = safe_dplyr_compile(query, &options);
        let cache_hit_duration = start.elapsed();
        
        println!("Cache miss: {:?}, Cache hit: {:?}", cache_miss_duration, cache_hit_duration);
        
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
    fn test_benchmark_queries_dont_panic() {
        let options = create_default_options();
        
        // Test that our benchmark queries don't panic
        for query in SIMPLE_QUERIES {
            let result = safe_dplyr_compile(query, &options);
            assert!(result.is_ok(), "Simple query failed: {}", query);
        }
        
        for query in COMPLEX_QUERIES {
            let result = safe_dplyr_compile(query, &options);
            assert!(result.is_ok(), "Complex query failed: {}", query);
        }
        
        // Edge cases should return errors, not panic
        for query in EDGE_CASE_QUERIES {
            let result = safe_dplyr_compile(query, &options);
            // Should either succeed or return an error, but not panic
            match result {
                Ok(_) => {}, // Some edge cases might actually be valid
                Err(_) => {}, // Expected for most edge cases
            }
        }
    }
}