//! Extension loading performance benchmarks
//!
//! This benchmark measures the DuckDB extension loading time to validate
//! the R6-AC1 requirement of <50ms extension loading time.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::process::Command;
use std::time::{Duration, Instant};

use std::env;
use std::path::Path;

// R6-AC1: Extension loading target <50ms
#[allow(dead_code)]
const EXTENSION_LOADING_TARGET_MS: f64 = 50.0;

// Helper function to find the extension binary
fn find_extension_binary() -> Option<String> {
    if let Ok(path) = env::var("DPLYR_EXTENSION_FILE") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    // Common build paths
    let possible_paths = [
        "build/dplyr.duckdb_extension",
        "build/Release/dplyr.duckdb_extension",
        "build/Debug/dplyr.duckdb_extension",
        "build/release/extension/dplyr/dplyr.duckdb_extension",
        "build/debug/extension/dplyr/dplyr.duckdb_extension",
        "../build/dplyr.duckdb_extension",
        "../build/Release/dplyr.duckdb_extension",
        "../build/Debug/dplyr.duckdb_extension",
        "../build/release/extension/dplyr/dplyr.duckdb_extension",
        "../build/debug/extension/dplyr/dplyr.duckdb_extension",
        "target/debug/build/libdplyr_c-*/out/dplyr.duckdb_extension",
        "target/release/build/libdplyr_c-*/out/dplyr.duckdb_extension",
    ];

    for path in &possible_paths {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // Try to find using glob pattern
    if let Ok(entries) = glob::glob("target/*/build/libdplyr_c-*/out/dplyr.duckdb_extension") {
        for path in entries.flatten() {
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }

    for pattern in [
        "../build/**/dplyr.duckdb_extension",
        "build/**/dplyr.duckdb_extension",
    ] {
        if let Ok(entries) = glob::glob(pattern) {
            for path in entries.flatten() {
                if path.exists() {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
    }

    None
}

// Helper function to check if DuckDB is available
fn is_duckdb_available() -> bool {
    Command::new("duckdb").arg("--version").output().is_ok()
}

// Benchmark extension loading time
fn bench_extension_loading(c: &mut Criterion) {
    // Skip if DuckDB is not available
    if !is_duckdb_available() {
        eprintln!("Warning: DuckDB not found in PATH, skipping extension loading benchmark");
        return;
    }

    // Find extension binary
    let extension_path = match find_extension_binary() {
        Some(path) => path,
        None => {
            eprintln!("Warning: Extension binary not found, skipping extension loading benchmark");
            eprintln!("Build the extension first with: cmake --build build --config Release");
            return;
        }
    };

    println!("Using extension: {}", extension_path);

    let mut group = c.benchmark_group("extension_loading");
    group.significance_level(0.1).sample_size(100);
    group.measurement_time(Duration::from_secs(30));

    // Benchmark cold loading (new DuckDB instance each time)
    group.bench_function("cold_loading", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);

            for _ in 0..iters {
                let start = Instant::now();

                let output = Command::new("duckdb")
                    .arg(":memory:")
                    .arg("-c")
                    .arg(format!(
                        "LOAD '{}'; SELECT 'loaded' as status;",
                        extension_path
                    ))
                    .output();

                let duration = start.elapsed();
                total_duration += duration;

                // Verify the command succeeded
                if let Ok(output) = output {
                    if !output.status.success() {
                        eprintln!(
                            "Extension loading failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    black_box(output);
                } else {
                    eprintln!("Failed to execute DuckDB command");
                }
            }

            total_duration
        });
    });

    // Benchmark warm loading (reuse connection, multiple loads)
    group.bench_function("warm_loading", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);

            for _ in 0..iters {
                let start = Instant::now();

                let output = Command::new("duckdb")
                    .arg(":memory:")
                    .arg("-c")
                    .arg(format!(
                        "LOAD '{}'; SELECT 'loaded' as status; LOAD '{}'; SELECT 'reloaded' as status;",
                        extension_path, extension_path
                    ))
                    .output();

                let duration = start.elapsed();
                total_duration += duration;

                if let Ok(output) = output {
                    black_box(output);
                }
            }

            total_duration
        });
    });

    // Benchmark loading with immediate usage
    group.bench_function("loading_with_usage", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);

            for _ in 0..iters {
                let start = Instant::now();

                let output = Command::new("duckdb")
                    .arg(":memory:")
                    .arg("-c")
                    .arg(format!(
                        "LOAD '{}'; CREATE TABLE __dplyr_bench(x INTEGER); INSERT INTO __dplyr_bench VALUES (1); SELECT * FROM dplyr('__dplyr_bench %>% select(x)');",
                        extension_path
                    ))
                    .output();

                let duration = start.elapsed();
                total_duration += duration;

                if let Ok(output) = output {
                    black_box(output);
                }
            }

            total_duration
        });
    });

    group.finish();
}

// Benchmark extension initialization overhead
fn bench_extension_initialization(c: &mut Criterion) {
    if !is_duckdb_available() {
        return;
    }

    let extension_path = match find_extension_binary() {
        Some(path) => path,
        None => return,
    };

    let mut group = c.benchmark_group("extension_initialization");
    group.significance_level(0.1).sample_size(50);

    // Compare DuckDB startup time with and without extension
    group.bench_function("without_extension", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);

            for _ in 0..iters {
                let start = Instant::now();

                let output = Command::new("duckdb")
                    .arg(":memory:")
                    .arg("-c")
                    .arg("SELECT 'no extension' as status;")
                    .output();

                let duration = start.elapsed();
                total_duration += duration;

                if let Ok(output) = output {
                    black_box(output);
                }
            }

            total_duration
        });
    });

    group.bench_function("with_extension", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);

            for _ in 0..iters {
                let start = Instant::now();

                let output = Command::new("duckdb")
                    .arg(":memory:")
                    .arg("-c")
                    .arg(format!(
                        "LOAD '{}'; SELECT 'with extension' as status;",
                        extension_path
                    ))
                    .output();

                let duration = start.elapsed();
                total_duration += duration;

                if let Ok(output) = output {
                    black_box(output);
                }
            }

            total_duration
        });
    });

    group.finish();
}

criterion_group! {
    benches,
    bench_extension_loading,
    bench_extension_initialization
}

criterion_main!(benches);

// Extension loading performance tests

mod extension_loading_tests {

    #[test]
    fn test_extension_loading_performance_target() {
        if !is_duckdb_available() {
            println!("Skipping extension loading test: DuckDB not available");
            return;
        }

        let extension_path = match find_extension_binary() {
            Some(path) => path,
            None => {
                println!("Skipping extension loading test: Extension binary not found");
                return;
            }
        };

        // Measure extension loading time over multiple runs
        let mut durations = Vec::new();

        for _ in 0..20 {
            let start = Instant::now();

            let output = Command::new("duckdb")
                .arg(":memory:")
                .arg("-c")
                .arg(format!(
                    "LOAD '{}'; SELECT 'loaded' as status;",
                    extension_path
                ))
                .output();

            let duration = start.elapsed();
            durations.push(duration);

            // Verify the command succeeded
            if let Ok(output) = output {
                assert!(
                    output.status.success(),
                    "Extension loading failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            } else {
                panic!("Failed to execute DuckDB command");
            }
        }

        // Calculate P95
        durations.sort();
        let p95_index = (durations.len() as f64 * 0.95) as usize;
        let p95_duration = durations[p95_index];

        println!("Extension loading P95: {:?}", p95_duration);

        // R6-AC1: Extension loading should be under 50ms P95
        assert!(
            p95_duration.as_millis() as f64 <= EXTENSION_LOADING_TARGET_MS,
            "Extension loading P95 ({:?}) exceeds target ({}ms)",
            p95_duration,
            EXTENSION_LOADING_TARGET_MS
        );
    }

    #[test]
    fn test_extension_functionality_after_loading() {
        if !is_duckdb_available() {
            println!("Skipping extension functionality test: DuckDB not available");
            return;
        }

        let extension_path = match find_extension_binary() {
            Some(path) => path,
            None => {
                println!("Skipping extension functionality test: Extension binary not found");
                return;
            }
        };

        // Test that extension works immediately after loading
        let output = Command::new("duckdb")
            .arg(":memory:")
            .arg("-c")
            .arg(format!(
                "LOAD '{}'; CREATE TABLE __dplyr_test(test_col INTEGER); INSERT INTO __dplyr_test VALUES (1); SELECT * FROM dplyr('__dplyr_test %>% select(test_col)');",
                extension_path
            ))
            .output()
            .expect("Failed to execute DuckDB command");

        assert!(
            output.status.success(),
            "Extension functionality test failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("test_col"),
            "Extension output doesn't contain expected result: {}",
            stdout
        );
    }
}
