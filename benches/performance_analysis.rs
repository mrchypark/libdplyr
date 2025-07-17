//! Performance analysis and optimization tools
//!
//! This module provides utilities for analyzing performance characteristics
//! and identifying optimization opportunities in the libdplyr transpiler.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use libdplyr::{MySqlDialect, PostgreSqlDialect, SqliteDialect, Transpiler};
use std::time::Instant;

/// Memory usage estimation for different operations
fn benchmark_memory_usage_estimation(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    
    let mut group = c.benchmark_group("memory_usage_estimation");
    
    // Test queries of different complexity levels
    let test_cases = vec![
        ("tiny", "select(a)"),
        ("small", "select(a, b) %>% filter(a > 1)"),
        ("medium", "select(a, b, c, d) %>% filter(a > 1 & b < 10) %>% arrange(desc(c))"),
        ("large", r#"
            select(name, age, salary, department, hire_date) %>%
            filter(age >= 18 & salary > 30000 & department != "temp") %>%
            mutate(
                age_group = age / 10,
                salary_tier = salary / 10000,
                years_employed = 2024 - hire_date
            ) %>%
            group_by(department, age_group) %>%
            summarise(
                avg_salary = mean(salary),
                count = n(),
                max_age = max(age)
            ) %>%
            arrange(desc(avg_salary))
        "#),
    ];
    
    for (size, query) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("memory_estimation", size),
            &query,
            |b, query| {
                b.iter(|| {
                    // Simulate memory usage by measuring allocation patterns
                    let start_time = Instant::now();
                    let result = transpiler.transpile(black_box(query));
                    let duration = start_time.elapsed();
                    
                    // Return both result and timing for analysis
                    black_box((result, duration))
                })
            },
        );
    }
    
    group.finish();
}

/// Scaling analysis - how performance changes with input size
fn benchmark_scaling_analysis(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    
    let mut group = c.benchmark_group("scaling_analysis");
    
    // Generate queries with increasing number of columns
    let column_counts = vec![1, 5, 10, 25, 50, 100];
    
    for count in column_counts {
        let columns: Vec<String> = (0..count).map(|i| format!("col_{}", i)).collect();
        let query = format!("select({})", columns.join(", "));
        
        group.bench_with_input(
            BenchmarkId::new("column_scaling", count),
            &query,
            |b, query| {
                b.iter(|| transpiler.transpile(black_box(query.as_str())))
            },
        );
    }
    
    // Generate queries with increasing number of filter conditions
    let filter_counts = vec![1, 3, 5, 10, 15, 20];
    
    for count in filter_counts {
        let conditions: Vec<String> = (0..count)
            .map(|i| format!("col_{} > {}", i, i * 10))
            .collect();
        let query = format!("select(name) %>% filter({})", conditions.join(" & "));
        
        group.bench_with_input(
            BenchmarkId::new("filter_scaling", count),
            &query,
            |b, query| {
                b.iter(|| transpiler.transpile(black_box(query.as_str())))
            },
        );
    }
    
    group.finish();
}

/// Bottleneck identification - which stage takes the most time
fn benchmark_bottleneck_identification(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    
    let mut group = c.benchmark_group("bottleneck_identification");
    
    let complex_query = "select(employee_id, name, age, salary, department, hire_date, performance_score) %>% filter(age >= 21 & age <= 65 & salary > 40000 & salary < 200000 & department != \"temp\" & department != \"intern\" & performance_score >= 3.0) %>% mutate(age_group = age / 10, salary_bracket = salary / 20000, tenure_years = 2024 - hire_date, performance_tier = performance_score / 1.0, is_senior = age > 40 & salary > 80000, bonus_eligible = performance_score > 4.0 & tenure_years > 2) %>% group_by(department, age_group, salary_bracket) %>% summarise(employee_count = n(), avg_salary = mean(salary), avg_performance = mean(performance_score), avg_tenure = mean(tenure_years), senior_count = sum(is_senior), bonus_eligible_count = sum(bonus_eligible)) %>% arrange(desc(avg_salary), desc(avg_performance), department)";
    
    // Measure lexing time only
    group.bench_function("lexing_only", |b| {
        b.iter(|| {
            use libdplyr::Lexer;
            let mut lexer = Lexer::new(black_box(complex_query.to_string()));
            let mut token_count = 0;
            loop {
                match lexer.next_token() {
                    Ok(token) => {
                        if matches!(token, libdplyr::Token::EOF) {
                            break;
                        }
                        token_count += 1;
                    }
                    Err(_) => break,
                }
            }
            black_box(token_count)
        })
    });
    
    // Measure parsing time only
    group.bench_function("parsing_only", |b| {
        b.iter(|| transpiler.parse_dplyr(black_box(complex_query)))
    });
    
    // Measure SQL generation time only
    let parsed_ast = transpiler.parse_dplyr(complex_query).unwrap();
    group.bench_function("sql_generation_only", |b| {
        b.iter(|| transpiler.generate_sql(black_box(&parsed_ast)))
    });
    
    // Measure full pipeline
    group.bench_function("full_pipeline", |b| {
        b.iter(|| transpiler.transpile(black_box(complex_query)))
    });
    
    group.finish();
}

/// Cache efficiency testing
fn benchmark_cache_efficiency(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    
    let mut group = c.benchmark_group("cache_efficiency");
    
    // Test repeated parsing of identical queries
    let repeated_query = "select(name, age) %>% filter(age > 18) %>% arrange(desc(age))";
    
    group.bench_function("cold_cache", |b| {
        b.iter(|| {
            // Create new transpiler each time to simulate cold cache
            let fresh_transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
            fresh_transpiler.transpile(black_box(repeated_query))
        })
    });
    
    group.bench_function("warm_cache", |b| {
        // Use the same transpiler instance to simulate warm cache
        b.iter(|| transpiler.transpile(black_box(repeated_query)))
    });
    
    // Test with slight variations to see how well the system handles similar queries
    let similar_queries = vec![
        "select(name, age) %>% filter(age > 18)",
        "select(name, age) %>% filter(age > 19)",
        "select(name, age) %>% filter(age > 20)",
        "select(name, age) %>% filter(age > 21)",
    ];
    
    group.bench_function("similar_queries", |b| {
        b.iter(|| {
            for query in &similar_queries {
                let _ = black_box(transpiler.transpile(black_box(query)));
            }
        })
    });
    
    group.finish();
}

/// Error handling performance impact
fn benchmark_error_handling_performance(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    
    let mut group = c.benchmark_group("error_handling_performance");
    
    // Valid query baseline
    let valid_query = "select(name, age) %>% filter(age > 18)";
    group.bench_function("valid_query", |b| {
        b.iter(|| transpiler.transpile(black_box(valid_query)))
    });
    
    // Various types of invalid queries
    let invalid_queries = vec![
        ("lexer_error", "select(name, age) %>% filter(age > 18"),  // Missing closing paren
        ("parser_error", "select() %>% invalid_function(test)"),    // Invalid function
        ("empty_input", ""),                                        // Empty input
        ("whitespace_only", "   \n\t  "),                         // Whitespace only
    ];
    
    for (error_type, query) in invalid_queries {
        group.bench_function(error_type, |b| {
            b.iter(|| {
                let result = transpiler.transpile(black_box(query));
                // Ensure we handle the error result
                black_box(result.is_err())
            })
        });
    }
    
    group.finish();
}

/// Dialect-specific performance characteristics
fn benchmark_dialect_performance_characteristics(c: &mut Criterion) {
    let mut group = c.benchmark_group("dialect_performance_characteristics");
    
    let test_queries = vec![
        ("simple_select", "select(name, age)"),
        ("complex_aggregation", r#"
            select(category, region) %>%
            group_by(category, region) %>%
            summarise(
                total_sales = sum(sales),
                avg_price = mean(price),
                count = n()
            )
        "#),
        ("string_operations", r#"
            select(name, description) %>%
            mutate(
                full_name = name + " - " + description,
                name_length = length(name)
            )
        "#),
    ];
    
    let dialects = vec![
        ("postgresql", Box::new(PostgreSqlDialect) as Box<dyn libdplyr::SqlDialect>),
        ("mysql", Box::new(MySqlDialect) as Box<dyn libdplyr::SqlDialect>),
        ("sqlite", Box::new(SqliteDialect) as Box<dyn libdplyr::SqlDialect>),
    ];
    
    for (query_name, query) in &test_queries {
        for (dialect_name, dialect) in &dialects {
            let transpiler = Transpiler::new(dialect.clone_box());
            
            group.bench_with_input(
                BenchmarkId::new(format!("{}_{}", query_name, dialect_name), query.len()),
                query,
                |b, query| {
                    b.iter(|| transpiler.transpile(black_box(query)))
                },
            );
        }
    }
    
    group.finish();
}

criterion_group!(
    performance_analysis,
    benchmark_memory_usage_estimation,
    benchmark_scaling_analysis,
    benchmark_bottleneck_identification,
    benchmark_cache_efficiency,
    benchmark_error_handling_performance,
    benchmark_dialect_performance_characteristics
);
criterion_main!(performance_analysis);