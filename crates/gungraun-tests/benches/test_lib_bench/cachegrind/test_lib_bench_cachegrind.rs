use std::hint::black_box;

use benchmark_tests::{bubble_sort, setup_best_case_array};
use gungraun::prelude::*;
use gungraun::{Cachegrind, Callgrind};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default())
        .tool(Cachegrind::with_args(["cache-sim=yes"]))
)]
#[bench::default_config(args = (10), setup = setup_best_case_array)]
#[bench::overwrite_default_cachegrind_config(
    args = (10),
    setup = setup_best_case_array,
    config = LibraryBenchmarkConfig::default()
        .tool(Cachegrind::with_args(["cache-sim=no"]))
)]
#[bench::overwrite_default_callgrind_config(
    args = (10),
    setup = setup_best_case_array,
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["cache-sim=no"]))
)]
#[bench::tool_override(
    args = (10),
    setup = setup_best_case_array,
    config = LibraryBenchmarkConfig::default()
        .tool_override(Cachegrind::with_args(["cache-sim=no"]))
)]
fn test_config_overwrite(array: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(array)))
}

#[library_benchmark]
#[bench::no_config(vec![3, 2, 1])]
#[bench::cachegrind_config(
    args = (vec![3, 2, 1]),
    config = LibraryBenchmarkConfig::default()
        .tool(Cachegrind::with_args(["cache-sim=no"]))
)]
#[bench::callgrind_config(
    args = (vec![3, 2, 1]),
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["cache-sim=no"]))
)]
fn bench_default_tool(array: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(array)))
}

#[library_benchmark]
#[bench::set_instr_at_start(
    args = (vec![3, 2, 1]),
    config = LibraryBenchmarkConfig::default()
        .tool(Cachegrind::with_args(["instr-at-start=no"]))
)]
fn manual_cachegrind_setup(array: Vec<i32>) -> Vec<i32> {
    gungraun::client_requests::cachegrind::start_instrumentation();
    let r = black_box(bubble_sort(black_box(array)));
    gungraun::client_requests::cachegrind::stop_instrumentation();
    r
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [
        bench_default_tool,
        test_config_overwrite,
        manual_cachegrind_setup
    ]
);

main!(library_benchmark_groups = my_group);
