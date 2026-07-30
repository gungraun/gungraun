use std::hint::black_box;

use benchmark_tests::{bubble_sort, setup_best_case_array, setup_worst_case_array};
use gungraun::Dhat;
use gungraun::prelude::*;

#[library_benchmark]
#[bench::case_3(vec![1, 2, 3])]
#[benches::multiple(args = [vec![1, 2], vec![1, 2, 3, 4]])]
fn bench_bubble_sort_best_case(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

#[library_benchmark]
#[bench::case_3(vec![3, 2, 1])]
#[benches::multiple(args = [vec![2, 1], vec![4, 3, 2, 1]])]
fn bench_bubble_sort_worst_case(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

#[library_benchmark]
#[bench::case_3(vec![2, 3, 1])]
#[benches::no_compare_multiple(args = [vec![2, 1], vec![2, 4, 3, 1]])]
fn bench_bubble_sort_mixed_case(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default().frames(["*::setup_worst_case_array"]))
)]
#[bench::case_3(setup_worst_case_array(3))]
fn bench_bubble_sort_dhat(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default().frames(["*::setup_best_case_array"]))
)]
#[bench::case_3(setup_best_case_array(3))]
fn bench_bubble_sort_dhat_other(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

library_benchmark_group!(
    name = bubble_sort_compare_one,
    compare_by_id = true,
    benchmarks = bench_bubble_sort_best_case
);

library_benchmark_group!(
    name = bubble_sort_compare_two,
    compare_by_id = true,
    benchmarks = [bench_bubble_sort_best_case, bench_bubble_sort_worst_case]
);

library_benchmark_group!(
    name = bubble_sort_compare_three,
    compare_by_id = true,
    benchmarks = [
        bench_bubble_sort_best_case,
        bench_bubble_sort_worst_case,
        bench_bubble_sort_mixed_case
    ]
);

library_benchmark_group!(
    name = bubble_sort_compare_with_dhat,
    compare_by_id = true,
    benchmarks = [
        bench_bubble_sort_dhat,
        bench_bubble_sort_dhat_other,
        bench_bubble_sort_mixed_case
    ]
);

#[library_benchmark]
fn bench_bubble_sort_no_id_1() -> Vec<i32> {
    black_box(bubble_sort(vec![]))
}

#[library_benchmark]
fn bench_bubble_sort_no_id_2() -> Vec<i32> {
    black_box(bubble_sort(black_box(setup_worst_case_array(6))))
}

library_benchmark_group!(
    name = bubble_sort_compare_no_id,
    compare_by_id = true,
    benchmarks = [bench_bubble_sort_no_id_1, bench_bubble_sort_no_id_2]
);

main!(
    library_benchmark_groups = [
        bubble_sort_compare_one,
        bubble_sort_compare_two,
        bubble_sort_compare_three,
        bubble_sort_compare_no_id,
        bubble_sort_compare_with_dhat
    ]
);
