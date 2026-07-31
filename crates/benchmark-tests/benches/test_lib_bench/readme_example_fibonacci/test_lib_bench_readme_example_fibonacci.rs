use std::hint::black_box;

use gungraun::Callgrind;
use gungraun::prelude::*;

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

#[library_benchmark]
#[bench::short(10)]
#[bench::long(30)]
fn bench_fibonacci(value: u64) -> u64 {
    black_box(fibonacci(black_box(value)))
}

library_benchmark_group!(name = bench_fibonacci_group, benchmarks = bench_fibonacci);

main!(
    config = LibraryBenchmarkConfig::default().tool(Callgrind::with_args(["collect-jumps=yes"])),
    library_benchmark_groups = bench_fibonacci_group
);
