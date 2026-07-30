use std::hint::black_box;
use std::process::Command;

use benchmark_tests::find_primes_multi_thread;
use gungraun::prelude::*;
use gungraun::{Bbv, Callgrind, Dhat, Drd, Massif, Memcheck, OutputFormat};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "toggle-collect=*::find_primes"
        ]))
        .tool(Dhat::default().frames(["*::find_primes"]))
)]
#[bench::one(1)]
#[bench::two(2)]
fn bench_find_primes_multi_thread(num_threads: usize) -> Vec<u64> {
    black_box(find_primes_multi_thread(black_box(num_threads)))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "toggle-collect=thread::main",
            "toggle-collect=*::find_primes",
        ]))
        .tool(Dhat::default()
            .frames([
                "thread::main",
                "*::find_primes"
        ]))
)]
#[bench::one(1)]
#[bench::two(2)]
fn bench_thread_in_subprocess(num_threads: usize) {
    Command::new(env!("CARGO_BIN_EXE_thread"))
        .arg(num_threads.to_string())
        .status()
        .unwrap();
}

library_benchmark_group!(
    name = bench_group,
    compare_by_id = true,
    benchmarks = [bench_find_primes_multi_thread, bench_thread_in_subprocess]
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
