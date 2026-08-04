use std::hint::black_box;
use std::process::Command;

use benchmark_tests::thread_in_thread_with_instrumentation;
use gungraun::prelude::*;
use gungraun::{Bbv, Callgrind, Dhat, Drd, EntryPoint, Massif, Memcheck, OutputFormat};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--instr-atstart=no"])
            .entry_point(EntryPoint::None)
        )
        .tool(Dhat::default()
            .entry_point(EntryPoint::None)
        )
)]
fn bench_thread_in_thread() -> Vec<u64> {
    gungraun::client_requests::callgrind::start_instrumentation();
    let result = black_box(thread_in_thread_with_instrumentation());
    gungraun::client_requests::callgrind::stop_instrumentation();
    result
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["instr-atstart=no"])
            .entry_point(EntryPoint::None)
        )
        .tool(Dhat::default()
            .entry_point(EntryPoint::None)
        )
)]
fn bench_thread_in_thread_in_subprocess() {
    gungraun::client_requests::callgrind::start_instrumentation();
    Command::new(env!("CARGO_BIN_EXE_thread"))
        .arg("--thread-in-thread")
        .status()
        .unwrap();
    gungraun::client_requests::callgrind::stop_instrumentation();
}

library_benchmark_group!(
    name = bench_group,
    compare_by_id = true,
    benchmarks = [bench_thread_in_thread, bench_thread_in_thread_in_subprocess]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .truncate_description(None)
            .show_intermediate(true)
        )
        // Helgrind is excluded since an assertion in helgrind itself fails and causes an error.
        // Looks like a bug in valgrind.
        .tool(Dhat::default())
        .tool(Memcheck::default())
        .tool(Drd::with_args([
                "--suppressions=benches/test_lib_bench/threads/valgrind-suppressions.supp"
        ]))
        .tool(Massif::default())
        .tool(Bbv::default()),
    library_benchmark_groups = bench_group
);
