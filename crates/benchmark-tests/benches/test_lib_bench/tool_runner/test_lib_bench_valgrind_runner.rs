use std::hint::black_box;

use gungraun::prelude::*;

#[library_benchmark]
fn simple() -> u64 {
    black_box(42)
}

library_benchmark_group!(name = my_group, benchmarks = simple);
main!(library_benchmark_groups = my_group);
