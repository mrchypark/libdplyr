//! CLI performance benchmarks
//!
//! Measures the performance of CLI-specific operations including:
//! - JSON serialization performance
//! - Output formatting performance
//! - Validation performance
//! - Memory usage patterns

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use libdplyr::cli::{
    DplyrValidator, InputInfo, JsonOutputFormatter, MetadataBuilder, OutputFormat, OutputFormatter,
    ProcessingStats,
};

/// Benchmark JSON serialization performance
fn benchmark_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    let formatter = JsonOutputFormatter::new();
    let pretty_formatter = JsonOutputFormatter::pretty();

    // Test different SQL output sizes
    let simple_sql = "SELECT \"name\" FROM \"data\"";
    let medium_sql =
        "SELECT \"name\", \"age\", \"city\" FROM \"data\" WHERE \"age\" > 18 ORDER BY \"age\" DESC";
    let complex_sql = format!(
        "SELECT {} FROM \"data\" WHERE {} ORDER BY {}",
        (0..20)
            .map(|i| format!("\"col_{}\"", i))
            .collect::<Vec<_>>()
            .join(", "),
        (0..5)
            .map(|i| format!("\"col_{}\" > {}", i, i * 10))
            .collect::<Vec<_>>()
            .join(" AND "),
        (0..3)
            .map(|i| format!("\"col_{}\"", i))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let test_outputs = vec![
        ("simple", simple_sql),
        ("medium", medium_sql),
        ("complex", complex_sql.as_str()),
    ];

    for (size_name, sql) in &test_outputs {
        let metadata = MetadataBuilder::new("postgresql")
            .with_stats(ProcessingStats::with_timing(100, 200, 300))
            .with_input_info(InputInfo::from_stdin("test input"))
            .build();

        group.bench_with_input(
            BenchmarkId::new("compact_json", size_name),
            sql,
            |b, sql| {
                b.iter(|| formatter.format_success(black_box(sql), black_box(metadata.clone())))
            },
        );

        group.bench_with_input(BenchmarkId::new("pretty_json", size_name), sql, |b, sql| {
            b.iter(|| pretty_formatter.format_success(black_box(sql), black_box(metadata.clone())))
        });
    }

    group.finish();
}

/// Benchmark output formatting performance
fn benchmark_output_formatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("output_formatting");

    let formatters = vec![
        (
            "default",
            OutputFormatter::with_format(OutputFormat::Default),
        ),
        ("pretty", OutputFormatter::with_format(OutputFormat::Pretty)),
        (
            "compact",
            OutputFormatter::with_format(OutputFormat::Compact),
        ),
    ];

    let simple_sql = "SELECT \"name\" FROM \"data\"";
    let medium_sql =
        "SELECT \"name\", \"age\" FROM \"data\" WHERE \"age\" > 18 ORDER BY \"age\" DESC";
    let complex_sql = format!(
        "SELECT {} FROM \"data\" WHERE {} GROUP BY {} ORDER BY {}",
        (0..15)
            .map(|i| format!("\"col_{}\"", i))
            .collect::<Vec<_>>()
            .join(", "),
        (0..3)
            .map(|i| format!("\"col_{}\" > {}", i, i * 10))
            .collect::<Vec<_>>()
            .join(" AND "),
        (0..2)
            .map(|i| format!("\"col_{}\"", i))
            .collect::<Vec<_>>()
            .join(", "),
        (0..2)
            .map(|i| format!("\"col_{}\"", i))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let test_sqls = vec![
        ("simple", simple_sql),
        ("medium", medium_sql),
        ("complex", complex_sql.as_str()),
    ];

    for (format_name, formatter) in &formatters {
        for (sql_name, sql) in &test_sqls {
            group.bench_with_input(
                BenchmarkId::new(format!("{}_{}", format_name, sql_name), sql.len()),
                sql,
                |b, sql| b.iter(|| formatter.format(black_box(sql))),
            );
        }
    }

    group.finish();
}

/// Benchmark validation performance
fn benchmark_validation_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_performance");

    let validator = DplyrValidator::new();

    let test_inputs = vec![
        ("valid_simple", "data %>% select(name)"),
        ("valid_medium", "data %>% select(name, age) %>% filter(age > 18) %>% arrange(desc(age))"),
        ("valid_complex", "data %>% select(name, age, city, country) %>% filter(age > 18) %>% group_by(country) %>% summarise(count = n(), avg_age = mean(age)) %>% arrange(desc(count))"),
        ("invalid_syntax", "invalid_function(test, broken"),
        ("invalid_empty", ""),
        ("invalid_incomplete", "data %>%"),
    ];

    for (test_name, input) in test_inputs {
        group.bench_with_input(
            BenchmarkId::new("validate", test_name),
            &input,
            |b, input| b.iter(|| validator.validate(black_box(input))),
        );
    }

    group.finish();
}

/// Benchmark memory usage patterns with repeated operations
fn benchmark_memory_usage_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage_patterns");

    // Test memory efficiency with repeated operations
    let base_query = "data %>% select(name, age) %>% filter(age > 18)";

    group.bench_function("repeated_small_operations", |b| {
        b.iter(|| {
            let validator = DplyrValidator::new();
            let formatter = OutputFormatter::with_format(OutputFormat::Default);

            for _ in 0..100 {
                let _ = black_box(validator.validate(black_box(base_query)));
                let _ = black_box(formatter.format(black_box("SELECT * FROM data")));
            }
        })
    });

    // Test memory usage with large single operations
    let large_columns: Vec<String> = (0..200).map(|i| format!("col_{}", i)).collect();
    let large_query = format!("data %>% select({})", large_columns.join(", "));

    group.bench_function("single_large_operation", |b| {
        b.iter(|| {
            let validator = DplyrValidator::new();
            let _ = black_box(validator.validate(black_box(large_query.as_str())));
        })
    });

    // Test memory usage with many different small operations
    let small_queries: Vec<String> = (0..100)
        .map(|i| format!("data{} %>% select(col_{})", i % 10, i % 50))
        .collect();

    group.bench_function("many_different_small_operations", |b| {
        b.iter(|| {
            let validator = DplyrValidator::new();
            for query in &small_queries {
                let _ = black_box(validator.validate(black_box(query.as_str())));
            }
        })
    });

    group.finish();
}

/// Benchmark error handling performance impact
fn benchmark_error_handling_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling_impact");

    let validator = DplyrValidator::new();
    let json_formatter = JsonOutputFormatter::new();

    // Valid input baseline
    let valid_input = "data %>% select(name, age) %>% filter(age > 18)";
    group.bench_function("valid_input_baseline", |b| {
        b.iter(|| {
            let result = validator.validate(black_box(valid_input));
            black_box(result.is_ok())
        })
    });

    // Various error scenarios
    let error_cases = vec![
        ("empty_input", ""),
        ("syntax_error", "invalid_function("),
        ("incomplete_pipe", "data %>%"),
        ("unknown_function", "data %>% unknown_func()"),
        ("malformed_expression", "data %>% select(name, )"),
    ];

    for (error_name, input) in error_cases {
        group.bench_function(&format!("error_{}", error_name), |b| {
            b.iter(|| {
                let result = validator.validate(black_box(input));
                // Simulate error handling
                if let Err(_) = result {
                    let metadata = MetadataBuilder::new("postgresql").build();
                    let error_info = libdplyr::cli::json_output::ErrorInfo {
                        error_type: "validation".to_string(),
                        message: "Validation failed".to_string(),
                        position: None,
                        suggestions: vec!["Check syntax".to_string()],
                    };
                    let _ = json_formatter.format_error(error_info, metadata);
                }
                black_box(result.is_err())
            })
        });
    }

    group.finish();
}

criterion_group!(
    cli_benchmarks,
    benchmark_json_serialization,
    benchmark_output_formatting,
    benchmark_validation_performance,
    benchmark_memory_usage_patterns,
    benchmark_error_handling_impact
);
criterion_main!(cli_benchmarks);
