use gungraun::Tool;
use gungraun::prelude::*;

#[binary_benchmark]
#[bench::crate_binary(env!("CARGO_BIN_EXE_cat"))]
#[bench::use_path("cat")]
fn bench_paths(path: &str) -> Command {
    Command::new(path).arg("some.txt").build()
}

binary_benchmark_group!(name = my_group, benchmarks = bench_paths);
main!(
    config = BinaryBenchmarkConfig::default().default_tool(Tool::Perf),
    binary_benchmark_groups = my_group
);
