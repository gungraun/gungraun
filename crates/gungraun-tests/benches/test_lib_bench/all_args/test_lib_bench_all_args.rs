use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{Callgrind, EntryPoint, OutputFormat};

#[cfg(any(target_vendor = "apple", target_os = "freebsd"))]
pub fn get_data_collections_options_yes() -> LibraryBenchmarkConfig {
    LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "instr-atstart=yes",
            "collect-atstart=yes",
            "toggle-collect=benchmark_tests::fibonacci",
            "collect-jumps=yes",
            "collect-systime=yes",
            "collect-bus=yes",
        ]))
        .clone()
}

#[cfg(not(any(target_vendor = "apple", target_os = "freebsd")))]
pub fn get_data_collections_options_yes() -> LibraryBenchmarkConfig {
    LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "instr-atstart=yes",
            "collect-atstart=yes",
            "toggle-collect=benchmark_tests::fibonacci",
            "collect-jumps=yes",
            "collect-systime=nsec",
            "collect-bus=yes",
        ]))
        .clone()
}

#[library_benchmark]
#[bench::dump_creation_options_yes(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "dump-line=yes",
            "dump-instr=yes",
            "compress-strings=yes",
            "compress-pos=yes",
            "combine-dumps=yes"
        ]))
)]
#[bench::dump_creation_options_no(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "dump-line=no",
            "dump-instr=no",
            "compress-strings=no",
            "compress-pos=no",
            "combine-dumps=no"
        ]))
)]
#[bench::activity_options(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "dump-every-bb=10000",
            "dump-before=benchmark_tests::fibonacci",
            "zero-before=benchmark_tests::fibonacci",
            "dump-after=benchmark_tests::fibonacci"
        ]))
)]
#[bench::data_collection_options_yes(
    config = get_data_collections_options_yes()
)]
#[bench::separate_callers_1(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "separate-callers=1",
        ]))
)]
#[bench::separate_callers_func_1(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "separate-callers1=benchmark_tests::fibonacci",
        ]))
)]
#[bench::separate_recs_3(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "separate-recs=3",
        ]))
)]
#[bench::separate_recs_func_3(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "separate-recs3=benchmark_tests::fibonacci",
        ]))
)]
#[bench::skip_plt_yes(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "skip-plt=yes",
        ]))
)]
#[bench::skip_plt_no(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "skip-plt=no",
        ]))
)]
#[bench::fn_skip(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "fn-skip=benchmark_tests::fibonacci",
        ]))
)]
#[bench::simulation_options_yes(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "cache-sim=yes",
            "branch-sim=yes",
        ]))
)]
#[bench::simulation_options_no(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "cache-sim=no",
            "branch-sim=no",
        ]))
)]
#[bench::cache_simulation_options_yes(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "cache-sim=yes",
            "simulate-wb=yes",
            "simulate-hwpref=yes",
            "cacheuse=yes",
        ]))
)]
#[bench::cache_simulation_options_no(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "cache-sim=yes",
            "simulate-wb=no",
            "simulate-hwpref=no",
            "cacheuse=no",
        ]))
)]
#[bench::cache_simulation_options_sizes(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "cache-sim=yes",
            "I1=65536,16,128",
            "D1=65536,16,128",
            "LL=67108864,32,128",
        ]))
)]
fn bench_library() -> u64 {
    black_box(benchmark_tests::fibonacci(black_box(10)))
}

#[library_benchmark]
#[bench::data_collection_options_no(
    config = LibraryBenchmarkConfig::default()
        .tool(
            Callgrind::with_args([
                "instr-atstart=no",
                "collect-atstart=no",
                "collect-jumps=no",
                "collect-systime=no",
                "collect-bus=no",
            ])
            .entry_point(EntryPoint::None)
        )
)]
fn bench_with_client_request() -> u64 {
    gungraun::client_requests::callgrind::start_instrumentation();
    gungraun::client_requests::callgrind::toggle_collect();
    black_box(benchmark_tests::fibonacci(black_box(10)))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .show_intermediate(true)
        )
)]
#[bench::separate_threads_yes(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "separate-threads=yes",
        ]))
)]
#[bench::separate_threads_no(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "separate-threads=no",
        ]))
)]
fn bench_multi_threads() -> Vec<u64> {
    black_box(benchmark_tests::find_primes_multi_thread_with_instrumentation(black_box(2)))
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [
        bench_library,
        bench_with_client_request,
        bench_multi_threads
    ]
);

main!(library_benchmark_groups = my_group);
