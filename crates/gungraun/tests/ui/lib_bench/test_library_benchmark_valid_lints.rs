// Generated `black_box` calls must not trigger unused qualifications (issue #784).
#![forbid(unused_qualifications)]

use gungraun::prelude::*;
use std::hint::black_box;

fn fibonacci(value: u64) -> u64 {
    if value < 2 {
        value
    } else {
        fibonacci(value - 1) + fibonacci(value - 2)
    }
}

fn double(value: u64) -> u64 {
    value * 2
}

fn consume(value: u64) {
    _ = value;
}

fn make_bytes() -> Vec<u8> {
    b"some bytes".to_vec()
}

// The reported quickstart: an imported `black_box` alongside named benchmarks.
#[library_benchmark]
#[bench::short(10)]
#[bench::long(30)]
fn bench_fibonacci(value: u64) -> u64 {
    black_box(fibonacci(black_box(value)))
}

// Explicit arguments with a typed setup.
#[library_benchmark]
#[bench::explicit(args = (2), setup = double, teardown = consume)]
fn bench_args(value: u64) -> u64 {
    black_box(value)
}

// Iterator mode with setup and teardown.
#[library_benchmark]
#[benches::mapped(iter = 0..5, setup = double, teardown = consume)]
fn bench_iter(value: u64) -> u64 {
    black_box(value)
}

// Standalone mode with a typed setup.
#[library_benchmark(setup = make_bytes)]
fn bench_standalone(bytes: Vec<u8>) -> usize {
    black_box(bytes.len())
}

fn main() {}
