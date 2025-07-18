use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn simple_benchmark(c: &mut Criterion) {
    c.bench_function("simple test", |b| {
        b.iter(|| {
            black_box(1 + 1)
        })
    });
}

criterion_group!(benches, simple_benchmark);
criterion_main!(benches);