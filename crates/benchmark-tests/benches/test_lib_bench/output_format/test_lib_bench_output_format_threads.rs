use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{Callgrind, Dhat, EntryPoint, OutputFormat};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .truncate_description(None)
            .show_intermediate(true)
            .show_grid(true)
        )
)]
#[bench::for_comparison(
    "Another very long string to see if the truncation is disabled with the formatting option"
)]
fn bench_with_format(a: &str) -> Vec<u64> {
    println!("{a}");
    black_box(benchmark_tests::find_primes_multi_thread(2))
}

#[library_benchmark]
#[bench::for_comparison(
    "A very long string to see if the truncation of the description is really working"
)]
fn bench_without_format(a: &str) -> Vec<u64> {
    println!("{a}");
    black_box(benchmark_tests::find_primes_multi_thread(1))
}

library_benchmark_group!(
    name = my_group,
    config = LibraryBenchmarkConfig::default()
        .tool(
            Callgrind::with_args([
                "--toggle-collect=benchmark_tests::find_primes_multi_thread",
                "--toggle-collect=benchmark_tests::find_primes"
            ])
            .entry_point(EntryPoint::None)
        )
        .tool(Dhat::default()),
    compare_by_id = true,
    benchmarks = [bench_without_format, bench_with_format]
);

main!(library_benchmark_groups = my_group);
