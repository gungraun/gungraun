use std::hint::black_box;
use std::process::ExitStatus;

use gungraun::prelude::*;
use gungraun::{
    Bbv, Cachegrind, Callgrind, Dhat, Drd, Helgrind, Massif, Memcheck, OutputFormat, Tool,
};

#[library_benchmark]
#[bench::callgrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Callgrind)
)]
#[bench::cachegrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
)]
#[bench::dhat(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
)]
#[bench::memcheck(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
)]
#[bench::helgrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Helgrind)
)]
#[bench::drd(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DRD)
)]
#[bench::massif(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Massif)
)]
#[bench::bbv(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::BBV)
)]
fn default_tool() -> u64 {
    println!("Create some metrics");
    black_box(24)
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .show_intermediate(true))
)]
#[bench::callgrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Callgrind)
        .tool(Callgrind::with_args(["cache-sim=no", "toggle-collect=*echo::main"]))
)]
#[bench::cachegrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
        .tool(Cachegrind::with_args(["cache-sim=no"]))
)]
#[bench::dhat(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .tool(Dhat::with_args(["trace-children=yes"])
            .frames(["*echo::main"])
        )
)]
#[bench::memcheck(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args(["trace-children=yes"]))
)]
#[bench::helgrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Helgrind)
        .tool(Helgrind::with_args(["trace-children=yes"]))
)]
#[bench::drd(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DRD)
        .tool(Drd::with_args(["trace-children=yes"]))
)]
#[bench::massif(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Massif)
        .tool(Massif::with_args(["trace-children=yes"]))
)]
#[bench::bbv(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::BBV)
        .tool(Bbv::with_args(["trace-children=yes"]))
)]
fn default_tool_with_config() -> std::io::Result<ExitStatus> {
    std::process::Command::new(env!("CARGO_BIN_EXE_echo"))
        .arg("Print something with 'echo'")
        .status()
}

// Ensure using different kinds (out-file and log-file based tools) as default and additional tools
#[library_benchmark]
#[bench::callgrind_and_dhat(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Callgrind)
        .tool(Dhat::default())
)]
#[bench::cachegrind_and_memcheck(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
        .tool(Memcheck::default())
)]
#[bench::dhat_and_callgrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .tool(Callgrind::default())
)]
#[bench::memcheck_and_cachegrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Cachegrind::default())
)]
#[bench::helgrind_and_drd(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Helgrind)
        .tool(Drd::default())
)]
#[bench::drd_and_helgrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DRD)
        .tool(Helgrind::default())
)]
#[bench::massif_and_bbv(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Massif)
        .tool(Bbv::default())
)]
#[bench::bbv_and_massif(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::BBV)
        .tool(Massif::default())
)]
fn default_tool_with_another_tool() -> u64 {
    println!("Create some metrics");
    black_box(24)
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [
        default_tool,
        default_tool_with_config,
        default_tool_with_another_tool
    ]
);

main!(library_benchmark_groups = my_group);
