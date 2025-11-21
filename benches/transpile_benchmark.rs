//! Performance benchmarks
//!
//! Measures the conversion performance of libdplyr including:
//! - Basic transpilation performance
//! - Memory usage patterns
//! - Scaling with input size
//! - Dialect-specific performance
//! - Stage-by-stage performance analysis

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use libdplyr::{MySqlDialect, PostgreSqlDialect, SqliteDialect, Transpiler};
use std::hint::black_box as std_black_box;

/// Simple conversion benchmark
fn benchmark_simple_transpile(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "select(name, age) %>% filter(age > 18)";

    c.bench_function("simple transpile", |b| {
        b.iter(|| transpiler.transpile(black_box(dplyr_code)))
    });
}

/// Complex conversion benchmark
fn benchmark_complex_transpile(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = r#"
        select(name, age, category, salary) %>%
        filter(age > 18 & salary > 50000) %>%
        mutate(age_group = age / 10) %>%
        group_by(category, age_group) %>%
        summarise(
            avg_salary = mean(salary),
            count = n(),
            max_age = max(age)
        ) %>%
        arrange(desc(avg_salary))
    "#;

    c.bench_function("complex transpile", |b| {
        b.iter(|| transpiler.transpile(black_box(dplyr_code)))
    });
}

/// Performance comparison by dialect
fn benchmark_dialects(c: &mut Criterion) {
    let dplyr_code = "select(name, age) %>% filter(age > 18) %>% arrange(desc(age))";

    let mut group = c.benchmark_group("dialect_comparison");

    // PostgreSQL
    let pg_transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    group.bench_with_input(
        BenchmarkId::new("postgresql", "standard"),
        &dplyr_code,
        |b, code| b.iter(|| pg_transpiler.transpile(black_box(code))),
    );

    // MySQL
    let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect));
    group.bench_with_input(
        BenchmarkId::new("mysql", "standard"),
        &dplyr_code,
        |b, code| b.iter(|| mysql_transpiler.transpile(black_box(code))),
    );

    // SQLite
    let sqlite_transpiler = Transpiler::new(Box::new(SqliteDialect));
    group.bench_with_input(
        BenchmarkId::new("sqlite", "standard"),
        &dplyr_code,
        |b, code| b.iter(|| sqlite_transpiler.transpile(black_box(code))),
    );

    group.finish();
}

/// Performance measurement by parsing stage
fn benchmark_parsing_stages(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));
    let dplyr_code = "select(name, age) %>% filter(age > 18) %>% arrange(desc(age))";

    let mut group = c.benchmark_group("parsing_stages");

    // Parse only
    group.bench_function("parse_only", |b| {
        b.iter(|| transpiler.parse_dplyr(black_box(dplyr_code)))
    });

    // Full transpilation
    group.bench_function("full_transpile", |b| {
        b.iter(|| transpiler.transpile(black_box(dplyr_code)))
    });

    group.finish();
}

/// Performance measurement by input size
fn benchmark_input_sizes(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));

    let mut group = c.benchmark_group("input_sizes");

    // Small input
    let small_input = "select(name)";
    group.bench_with_input(
        BenchmarkId::new("small", small_input.len()),
        &small_input,
        |b, code| b.iter(|| transpiler.transpile(black_box(code))),
    );

    // Medium input
    let medium_input = "select(name, age, category) %>% filter(age > 18 & category == \"A\") %>% arrange(desc(age))";
    group.bench_with_input(
        BenchmarkId::new("medium", medium_input.len()),
        &medium_input,
        |b, code| b.iter(|| transpiler.transpile(black_box(code))),
    );

    // Large input
    let large_input = r#"
        select(name, age, category, salary, department, hire_date, performance_score) %>%
        filter(
            age > 18 & 
            salary > 50000 & 
            department == "Engineering" & 
            performance_score > 3.5
        ) %>%
        mutate(
            age_group = age / 10,
            salary_tier = salary / 10000,
            years_employed = 2024 - hire_date
        ) %>%
        group_by(department, age_group, salary_tier) %>%
        summarise(
            avg_salary = mean(salary),
            avg_performance = mean(performance_score),
            count = n(),
            max_age = max(age),
            min_age = min(age)
        ) %>%
        arrange(desc(avg_salary), desc(avg_performance))
    "#;
    group.bench_with_input(
        BenchmarkId::new("large", large_input.len()),
        &large_input,
        |b, code| b.iter(|| transpiler.transpile(black_box(code))),
    );

    group.finish();
}

/// Throughput-based benchmarks measuring operations per second
fn benchmark_throughput(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));

    let mut group = c.benchmark_group("throughput");

    // Simple queries throughput
    let simple_queries = vec![
        "select(name)",
        "select(age)",
        "select(category)",
        "filter(age > 18)",
        "filter(name == \"John\")",
        "arrange(age)",
        "arrange(desc(name))",
    ];

    group.throughput(Throughput::Elements(simple_queries.len() as u64));
    group.bench_function("simple_queries_batch", |b| {
        b.iter(|| {
            for query in &simple_queries {
                let _ = std_black_box(transpiler.transpile(black_box(query)));
            }
        })
    });

    // Complex queries throughput
    let complex_queries = vec![
        "select(name, age) %>% filter(age > 18) %>% arrange(desc(age))",
        "select(category, salary) %>% filter(salary > 50000) %>% group_by(category) %>% summarise(avg_sal = mean(salary))",
        "select(name, age, dept) %>% mutate(age_group = age / 10) %>% filter(age_group > 2)",
    ];

    group.throughput(Throughput::Elements(complex_queries.len() as u64));
    group.bench_function("complex_queries_batch", |b| {
        b.iter(|| {
            for query in &complex_queries {
                let _ = std_black_box(transpiler.transpile(black_box(query)));
            }
        })
    });

    group.finish();
}

/// Memory allocation patterns and efficiency
fn benchmark_memory_patterns(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));

    let mut group = c.benchmark_group("memory_patterns");

    // Test repeated parsing of the same query (should be efficient)
    let repeated_query = "select(name, age) %>% filter(age > 18)";
    group.bench_function("repeated_same_query", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let _ = std_black_box(transpiler.transpile(black_box(repeated_query)));
            }
        })
    });

    // Test parsing many different small queries
    let small_queries: Vec<String> = (0..50)
        .map(|i| format!("select(col_{}) %>% filter(col_{} > {})", i, i, i * 10))
        .collect();

    group.bench_function("many_small_queries", |b| {
        b.iter(|| {
            for query in &small_queries {
                let _ = std_black_box(transpiler.transpile(black_box(query.as_str())));
            }
        })
    });

    // Test very large single query
    let large_columns: Vec<String> = (0..100).map(|i| format!("col_{i}")).collect();
    let large_query = format!("select({})", large_columns.join(", "));

    group.bench_function("single_large_query", |b| {
        b.iter(|| transpiler.transpile(black_box(large_query.as_str())))
    });

    group.finish();
}

/// Stress testing with edge cases and extreme inputs
fn benchmark_stress_tests(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));

    let mut group = c.benchmark_group("stress_tests");

    // Deep nesting test
    let deep_nested = "select(a) %>% filter(a > 1) %>% filter(a < 100) %>% filter(a != 50) %>% filter(a % 2 == 0) %>% arrange(a)";
    group.bench_function("deep_nesting", |b| {
        b.iter(|| transpiler.transpile(black_box(deep_nested)))
    });

    // Wide operations test (many columns)
    let wide_columns: Vec<String> = (0..50).map(|i| format!("column_{i}")).collect();
    let wide_select = format!("select({})", wide_columns.join(", "));
    group.bench_function("wide_operations", |b| {
        b.iter(|| transpiler.transpile(black_box(wide_select.as_str())))
    });

    // Complex expressions test
    let complex_expr = r#"
        select(name, age, salary) %>%
        mutate(
            complex_calc = (age * 2.5 + salary / 1000) * 0.8,
            category = age > 30 & salary > 50000,
            score = (age + salary / 1000) / 2
        ) %>%
        filter(complex_calc > 100 & category == TRUE) %>%
        arrange(desc(score))
    "#;
    group.bench_function("complex_expressions", |b| {
        b.iter(|| transpiler.transpile(black_box(complex_expr)))
    });

    group.finish();
}

/// Lexer-specific performance tests
fn benchmark_lexer_performance(c: &mut Criterion) {
    use libdplyr::Lexer;

    let mut group = c.benchmark_group("lexer_performance");

    // Simple tokenization
    let simple_input = "select(name, age) %>% filter(age > 18)";
    group.bench_function("simple_tokenization", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(simple_input.to_string()));
            let mut tokens = Vec::new();
            while let Ok(token) = lexer.next_token() {
                if matches!(token, libdplyr::Token::EOF) {
                    break;
                }
                tokens.push(token);
            }
            std_black_box(tokens);
        })
    });

    // Complex tokenization with many operators
    let complex_input = r#"
        select(name, age, salary, department) %>%
        filter(age >= 18 & age <= 65 & salary > 30000 & department != "temp") %>%
        mutate(
            age_group = age / 10,
            salary_bracket = salary / 10000,
            is_senior = age > 40 & salary > 80000
        )
    "#;
    group.bench_function("complex_tokenization", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(complex_input.to_string()));
            let mut tokens = Vec::new();
            while let Ok(token) = lexer.next_token() {
                if matches!(token, libdplyr::Token::EOF) {
                    break;
                }
                tokens.push(token);
            }
            std_black_box(tokens);
        })
    });

    group.finish();
}

/// SQL generation performance tests
fn benchmark_sql_generation(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));

    let mut group = c.benchmark_group("sql_generation");

    // Pre-parse ASTs for generation-only benchmarks
    let simple_ast = transpiler
        .parse_dplyr("select(name, age) %>% filter(age > 18)")
        .unwrap();
    let complex_ast = transpiler
        .parse_dplyr(
            r#"
        select(name, age, category, salary) %>%
        filter(age > 18 & salary > 50000) %>%
        mutate(age_group = age / 10) %>%
        group_by(category, age_group) %>%
        summarise(avg_salary = mean(salary), count = n()) %>%
        arrange(desc(avg_salary))
    "#,
        )
        .unwrap();

    group.bench_function("simple_sql_generation", |b| {
        b.iter(|| transpiler.generate_sql(black_box(&simple_ast)))
    });

    group.bench_function("complex_sql_generation", |b| {
        b.iter(|| transpiler.generate_sql(black_box(&complex_ast)))
    });

    // Test different dialects on the same AST
    let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect));
    let sqlite_transpiler = Transpiler::new(Box::new(SqliteDialect));

    group.bench_function("postgresql_generation", |b| {
        b.iter(|| transpiler.generate_sql(black_box(&complex_ast)))
    });

    group.bench_function("mysql_generation", |b| {
        b.iter(|| mysql_transpiler.generate_sql(black_box(&complex_ast)))
    });

    group.bench_function("sqlite_generation", |b| {
        b.iter(|| sqlite_transpiler.generate_sql(black_box(&complex_ast)))
    });

    group.finish();
}

/// Regression tests to catch performance degradation
fn benchmark_regression_tests(c: &mut Criterion) {
    let transpiler = Transpiler::new(Box::new(PostgreSqlDialect));

    let mut group = c.benchmark_group("regression_tests");

    // Baseline performance targets (these should not regress significantly)
    let baseline_queries = vec![
        ("simple_select", "select(name, age)"),
        ("simple_filter", "filter(age > 18)"),
        ("simple_mutate", "mutate(new_col = age * 2)"),
        ("simple_arrange", "arrange(desc(age))"),
        ("simple_group_by", "group_by(category)"),
        ("simple_summarise", "summarise(avg_age = mean(age))"),
        (
            "basic_pipeline",
            "select(name, age) %>% filter(age > 18) %>% arrange(desc(age))",
        ),
    ];

    for (name, query) in baseline_queries {
        group.bench_function(name, |b| b.iter(|| transpiler.transpile(black_box(query))));
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_simple_transpile,
    benchmark_complex_transpile,
    benchmark_dialects,
    benchmark_parsing_stages,
    benchmark_input_sizes,
    benchmark_throughput,
    benchmark_memory_patterns,
    benchmark_stress_tests,
    benchmark_lexer_performance,
    benchmark_sql_generation,
    benchmark_regression_tests
);
criterion_main!(benches);
