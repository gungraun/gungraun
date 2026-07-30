use std::hint::black_box;

use benchmark_tests::{bubble_sort, fibonacci, setup_worst_case_array};
use gungraun::prelude::*;
use gungraun::{Dhat, DhatMetric, OutputFormat};

#[inline(never)]
fn setup_with_alloc(num: i32) -> Vec<i32> {
    setup_worst_case_array(num)
}

#[inline(never)]
fn setup_with_print_and_alloc(num: i32) -> Vec<i32> {
    println!("{num}");
    setup_worst_case_array(num)
}

#[inline(never)]
fn allocate_in_setup(inputs: fn() -> Vec<i32>) -> Vec<i32> {
    black_box(inputs())
}

fn teardown(num: u64) -> Result<u64, String> {
    if num < 2 {
        Ok(num)
    } else {
        Err("Something".to_owned())
    }
}

#[library_benchmark]
#[benches::empty(iter = vec![])]
fn bench_empty(num: u64) -> u64 {
    black_box(fibonacci(black_box(num)))
}

#[library_benchmark]
#[benches::one(iter = vec![(1, 2)])]
#[benches::two(iter = vec![(1, 2), (2, 3)])]
fn bench_when_tuple((a, b): (u64, u64)) -> u64 {
    black_box(fibonacci(black_box(a) + black_box(b)))
}

#[library_benchmark]
#[benches::vector(iter = vec![1, 2])]
#[benches::range(iter = 1..=2)]
#[benches::with_teardown(iter = vec![1, 2], teardown = teardown)]
fn bench_single(num: u64) -> u64 {
    black_box(fibonacci(black_box(num)))
}

// Bubble sort doesn't allocate heap memory by itself but makes reads and writes. The reads and
// writes are only reported by dhat if the allocation was recorded, too.
#[library_benchmark]
#[benches::with_setup(
    iter = vec![1, 2],
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .frames(["*::setup_worst_case_array"])
        ),
    setup = setup_worst_case_array
)]
#[benches::with_nested_setup(
    iter = vec![1, 2],
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .frames(["*::setup_with_alloc", "*::setup_worst_case_array"])
        ),
    setup = setup_with_alloc
)]
#[benches::with_alloc_delayed_in_setup(
    iter = vec![|| vec![2, 1], || vec![1]],
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .frames(["*::allocate_in_setup"])
        ),
    setup = allocate_in_setup
)]
#[benches::measure_just_nested_setup(
    iter = vec![2, -2],
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .frames(["*::setup_worst_case_array"])
            .hard_limits([(DhatMetric::TotalBytes, 8)])
        ),
    setup = setup_with_print_and_alloc
)]
fn bench_allocation(inputs: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(inputs)))
}

#[library_benchmark]
#[benches::single(iter = vec![1u32, 2u32])]
fn bench_generic<T>(num: T) -> u64
where
    T: Into<u64>,
{
    black_box(fibonacci(black_box(num).into()))
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [
        bench_empty,
        bench_when_tuple,
        bench_single,
        bench_allocation,
        bench_generic
    ]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default().truncate_description(None)),
    library_benchmark_groups = my_group
);
