use std::hint::black_box;

use gungraun::prelude::*;

#[library_benchmark]
fn minimal_bench() -> u64 {
    black_box(42)
}

#[library_benchmark]
fn other_bench() -> u64 {
    black_box(42)
}

library_benchmark_group!(name = group_1, benchmarks = minimal_bench);
library_benchmark_group!(name = group_2, benchmarks = [other_bench, minimal_bench]);

main!(library_benchmark_groups = [group_1, group_2]);
