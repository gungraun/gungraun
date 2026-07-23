use gungraun::prelude::*;
use gungraun::{Bbv, Dhat, Drd, Helgrind, Massif, Memcheck, OutputFormat, Perf};

#[binary_benchmark]
#[bench::trace_children()]
#[bench::no_trace_children(
    config = BinaryBenchmarkConfig::default()
        .valgrind_args(["trace-children=no"])
)]
fn bench_subprocess() -> Command {
    Command::new(env!("CARGO_BIN_EXE_subprocess"))
        .arg(env!("CARGO_BIN_EXE_sort"))
        .build()
}

binary_benchmark_group!(name = bench_group, benchmarks = bench_subprocess);
main!(
    config = BinaryBenchmarkConfig::default()
        .output_format(
            OutputFormat::default()
                .show_intermediate(true)
                .truncate_description(None)
        )
        .tool(Dhat::with_args(["--time-stamp=yes"]))
        .tool(Massif::default())
        .tool(Bbv::default())
        .tool(Memcheck::with_args(["--time-stamp=yes"]))
        .tool(Drd::default())
        .tool(Helgrind::default())
        .tool(Perf::default()),
    binary_benchmark_groups = bench_group
);
