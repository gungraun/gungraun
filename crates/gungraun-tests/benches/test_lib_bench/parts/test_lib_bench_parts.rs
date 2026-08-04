use std::hint::black_box;
use std::process::{Command, ExitStatus};

use benchmark_tests::{find_primes, find_primes_multi_thread_with_instrumentation};
use gungraun::prelude::*;
use gungraun::{Callgrind, EntryPoint, OutputFormat};

#[library_benchmark]
#[bench::dump_every_bb(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-every-bb=90000"]))
)]
#[bench::dump_before(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-before=*::find_primes"]))
)]
#[bench::dump_after(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-after=*::find_primes"]))
)]
fn bench_no_thread() -> Vec<u64> {
    black_box(find_primes(0, 20000))
}

// There is no test for --dump-every-bb since this test in combination with multiple threads and
// the instrumentation usage as it is done in this benchmark produces too different results on
// different systems. We already test --dump-every-bb in the other benchmark functions and the main
// goal is to test the handling and sanitization of the output files of the gungraun runner in the
// presence of multiple threads and parts.
#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--instr-atstart=no"])
            .entry_point(EntryPoint::None)
        )
)]
#[bench::dump_before(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-before=*::find_primes"]))
)]
#[bench::dump_after(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-after=*::find_primes"]))
)]
fn bench_multiple_threads() -> Vec<u64> {
    gungraun::client_requests::callgrind::start_instrumentation();
    let result = black_box(find_primes_multi_thread_with_instrumentation(black_box(2)));
    gungraun::client_requests::callgrind::stop_instrumentation();
    result
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--instr-atstart=no"])
            .entry_point(EntryPoint::None)
        )
)]
#[bench::dump_every_bb(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-every-bb=90000"]))
)]
#[bench::dump_before(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-before=*::find_primes"]))
)]
#[bench::dump_after(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--dump-after=*::find_primes"]))
)]
fn bench_multiple_threads_in_subprocess() -> ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_thread"))
        .arg("--thread-in-thread")
        .status()
        .unwrap()
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [
        bench_no_thread,
        bench_multiple_threads,
        bench_multiple_threads_in_subprocess
    ]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default().show_intermediate(true)),
    library_benchmark_groups = my_group
);
