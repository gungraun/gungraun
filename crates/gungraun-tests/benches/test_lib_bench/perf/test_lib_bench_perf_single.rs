use std::hint::black_box;

use gungraun::prelude::*;
use gungraun_tests::{bubble_sort, setup_worst_case_array};

#[library_benchmark]
#[bench::thousand(args = [1000], setup = setup_worst_case_array)]
fn bench_bubble_sort(arg: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(arg)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_bubble_sort);
main!(library_benchmark_groups = my_group);
