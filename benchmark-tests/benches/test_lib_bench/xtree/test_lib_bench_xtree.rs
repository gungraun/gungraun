use benchmark_tests::{bubble_sort, leak_memory, setup_worst_case_array, subprocess};
use gungraun::prelude::*;
use gungraun::{Helgrind, Massif, Memcheck, Tool};

#[library_benchmark(setup = setup_worst_case_array)]
#[bench::memcheck_xtree(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args(["xtree-memory=full"]))
)]
#[bench::memcheck_xleak(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args(["xtree-leak=yes"]))
)]
#[bench::memcheck_xtree_and_xleak(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args(["xtree-memory=full", "xtree-leak=yes"]))
)]
#[bench::helgrind(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Helgrind)
        .tool(Helgrind::with_args(["xtree-memory=full"]))
)]
#[bench::massif(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Massif)
        .tool(Massif::with_args(["xtree-memory=full"]))
)]
fn bench_with_xtree_no_leak(array: Vec<i32>) -> Vec<i32> {
    bubble_sort(array)
}

#[library_benchmark]
#[bench::xtree(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args([
            "xtree-memory=full", "error-exitcode=0"
        ]))
)]
#[bench::xleak(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args([
            "xtree-leak=yes", "error-exitcode=0"
        ]))
)]
#[bench::xtree_and_xleak(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args([
            "xtree-memory=full", "xtree-leak=yes", "error-exitcode=0"
        ]))
)]
fn bench_with_memcheck_when_leak() {
    leak_memory(100);
}

#[library_benchmark]
#[bench::memcheck_multi_process(
    args = (10),
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::with_args(["xtree-memory=full", "xtree-leak=yes", "error-exitcode=0"]))
)]
fn bench_with_xtree_in_subprocess(end: usize) -> std::io::Result<std::process::Output> {
    leak_memory(end);
    subprocess(env!("CARGO_BIN_EXE_leak-memory"), [end.to_string()])
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [
        bench_with_xtree_no_leak,
        bench_with_xtree_in_subprocess,
        bench_with_memcheck_when_leak
    ]
);

main!(library_benchmark_groups = my_group);
