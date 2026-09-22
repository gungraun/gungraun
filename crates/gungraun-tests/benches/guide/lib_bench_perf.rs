mod my_lib {
    pub use gungraun_tests::fibonacci;
}

use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{Perf, Tool};

#[library_benchmark]
#[bench::fibonacci_30(30)]
fn bench_library(n: u64) -> u64 {
    my_lib::fibonacci(n)
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);

main!(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().sample_duration(Duration::from_secs(2))),
    library_benchmark_groups = my_group
);
