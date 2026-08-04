use std::hint::black_box;

use gungraun::prelude::*;

#[library_benchmark]
#[bench::forty_two(42)]
#[benches::down(42, 31)]
fn benches_with_id(input: u64) -> u64 {
    black_box(input)
}

#[library_benchmark]
fn minimal_bench() -> u64 {
    black_box(42)
}

library_benchmark_group!(name = group_1, benchmarks = minimal_bench);
library_benchmark_group!(
    name = group_2,
    benchmarks = [minimal_bench, benches_with_id]
);

main!(library_benchmark_groups = [group_1, group_2]);
