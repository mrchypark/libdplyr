//! Performance benchmarks
//!
//! Measures the conversion performance of libdplyr.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use libdplyr::{Transpiler, PostgreSqlDialect, MySqlDialect, SqliteDialect};

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
        |b, code| {
            b.iter(|| pg_transpiler.transpile(black_box(code)))
        },
    );
    
    // MySQL
    let mysql_transpiler = Transpiler::new(Box::new(MySqlDialect));
    group.bench_with_input(
        BenchmarkId::new("mysql", "standard"),
        &dplyr_code,
        |b, code| {
            b.iter(|| mysql_transpiler.transpile(black_box(code)))
        },
    );
    
    // SQLite
    let sqlite_transpiler = Transpiler::new(Box::new(SqliteDialect));
    group.bench_with_input(
        BenchmarkId::new("sqlite", "standard"),
        &dplyr_code,
        |b, code| {
            b.iter(|| sqlite_transpiler.transpile(black_box(code)))
        },
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
        |b, code| {
            b.iter(|| transpiler.transpile(black_box(code)))
        },
    );
    
    // Medium input
    let medium_input = "select(name, age, category) %>% filter(age > 18 & category == \"A\") %>% arrange(desc(age))";
    group.bench_with_input(
        BenchmarkId::new("medium", medium_input.len()),
        &medium_input,
        |b, code| {
            b.iter(|| transpiler.transpile(black_box(code)))
        },
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
        |b, code| {
            b.iter(|| transpiler.transpile(black_box(code)))
        },
    );
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_simple_transpile,
    benchmark_complex_transpile,
    benchmark_dialects,
    benchmark_parsing_stages,
    benchmark_input_sizes
);
criterion_main!(benches);