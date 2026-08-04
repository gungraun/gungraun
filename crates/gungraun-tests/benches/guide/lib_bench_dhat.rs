mod my_lib {
    pub use gungraun_tests::bubble_sort;
}
use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{Dhat, EntryPoint};
use gungraun_tests::setup_worst_case_array;

#[library_benchmark]
#[bench::worst_case_3(setup_worst_case_array(3))]
fn bench_library(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(array)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);

main!(
    config = LibraryBenchmarkConfig::default().tool(
        Dhat::default().entry_point(EntryPoint::Custom("*::setup_worst_case_array".to_owned()))
    ),
    library_benchmark_groups = my_group
);
