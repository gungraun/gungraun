use std::hint::black_box;

use benchmark_tests::{bubble_sort, setup_worst_case_array};
use gungraun::Dhat;
use gungraun::prelude::*;

#[inline(never)]
fn setup_with_output(input: i32) -> Vec<i32> {
    let result = setup_worst_case_array(input);
    println!(
        "last number of worst case array: {}",
        result.last().unwrap()
    );
    result
}

fn teardown_with_output(inputs: Vec<i32>) -> Vec<i32> {
    println!(
        "first number of inputs in teardown: {}",
        inputs.first().unwrap(),
    );
    inputs
}

#[library_benchmark]
#[bench::without_assists(vec![5, 4, 3, 2, 1])]
#[bench::with_setup(
    args = [10],
    setup = setup_worst_case_array
)]
#[bench::with_output_in_assists(
    args = [8],
    setup = setup_with_output,
    teardown = teardown_with_output
)]
fn with_output_in_bench(array: Vec<i32>) -> Vec<i32> {
    let result = black_box(bubble_sort(black_box(array)));
    println!("last number of sorted array: {}", result.last().unwrap());
    result
}

#[library_benchmark(config = LibraryBenchmarkConfig::default()
    .tool(Dhat::default()
        .frames(["*::setup_worst_case_array", "*::setup_with_output"])
    )
)]
#[bench::without_assists(
    args = (vec![5, 4, 3, 2, 1]),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .frames(["*::*without_assists"])
        )
)]
#[bench::with_setup(
    args = (10),
    setup = setup_worst_case_array
)]
#[bench::with_output_in_assists(
    args = (8),
    setup = setup_with_output,
    teardown = teardown_with_output
)]
fn multiple_tools(array: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(array)))
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [with_output_in_bench, multiple_tools]
);
main!(library_benchmark_groups = my_group);
