use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{Dhat, EntryPoint, Tool};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .tool(Dhat::default()
            .entry_point(
                EntryPoint::Custom("benchmark_tests::find_primes".to_owned())
            )
        )
)]
fn bench_library() -> Vec<u64> {
    black_box(benchmark_tests::find_primes_multi_thread(black_box(1)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
main!(library_benchmark_groups = my_group);
