use std::hint::black_box;

use benchmark_tests::{bubble_sort, setup_worst_case_array};
use gungraun::prelude::*;
use gungraun::{Callgrind, EventKind, FlamegraphConfig};

#[library_benchmark(config = LibraryBenchmarkConfig::default()
    .tool(Callgrind::with_args(["--cache-sim=yes"]))
)]
#[bench::with_10(setup_worst_case_array(10))]
fn bench_with_cache_sim(value: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(value)))
}

#[library_benchmark(config = LibraryBenchmarkConfig::default()
    .tool(Callgrind::with_args(["--cache-sim=no"]))
)]
#[bench::with_10(setup_worst_case_array(10))]
fn bench_without_cache_sim(value: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(value)))
}

library_benchmark_group!(
    name = bench_cache_sim,
    benchmarks = [bench_with_cache_sim, bench_without_cache_sim]
);

main!(
    config = LibraryBenchmarkConfig::default().tool(
        Callgrind::default()
            .soft_limits([(EventKind::Ir, 10.0)])
            .flamegraph(FlamegraphConfig::default())
    ),
    library_benchmark_groups = bench_cache_sim
);
