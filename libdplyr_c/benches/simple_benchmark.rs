//! Simple performance benchmark for libdplyr_c
//!
//! This is a minimal benchmark to test the basic functionality

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::ffi::CString;
use std::ptr;

// Import the functions we need to test
extern "C" {
    fn dplyr_compile(
        code: *const i8,
        options: *const u8, // Simplified for now
        out_sql: *mut *mut i8,
        out_error: *mut *mut i8,
    ) -> i32;
    
    fn dplyr_free_string(ptr: *mut i8);
}

// Simple benchmark function
fn bench_simple_query(c: &mut Criterion) {
    c.bench_function("simple_select", |b| {
        b.iter(|| {
            let query = CString::new("select(mpg)").unwrap();
            let mut out_sql: *mut i8 = ptr::null_mut();
            let mut out_error: *mut i8 = ptr::null_mut();
            
            let result = unsafe {
                dplyr_compile(
                    query.as_ptr(),
                    ptr::null(),
                    &mut out_sql,
                    &mut out_error,
                )
            };
            
            if result == 0 && !out_sql.is_null() {
                unsafe {
                    dplyr_free_string(out_sql);
                }
            }
            if !out_error.is_null() {
                unsafe {
                    dplyr_free_string(out_error);
                }
            }
            
            black_box(result);
        });
    });
}

criterion_group!(benches, bench_simple_query);
criterion_main!(benches);