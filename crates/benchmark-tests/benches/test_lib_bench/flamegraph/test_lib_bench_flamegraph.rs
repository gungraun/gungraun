use std::hint::black_box;

use benchmark_tests::setup_worst_case_array;
use gungraun::prelude::*;
use gungraun::{Callgrind, FlamegraphConfig, FlamegraphKind};

#[library_benchmark]
#[bench::all_kinds(
    args = (setup_worst_case_array(10)),
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .flamegraph(
               FlamegraphConfig::default()
                   .title("Bench-level flamegraph both kinds".to_owned())
                   .kind(FlamegraphKind::All)
            )
        )
)]
#[bench::regular_kind(
    args = (setup_worst_case_array(10)),
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .flamegraph(
               FlamegraphConfig::default()
                   .title("Bench-level flamegraph only regular kind".to_owned())
                   .kind(FlamegraphKind::Regular)
            )
        )
)]
#[bench::differential_kind(
    args = (setup_worst_case_array(1000)),
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .flamegraph(
               FlamegraphConfig::default()
                   .title("Bench-level flamegraph only differential kind".to_owned())
                   .kind(FlamegraphKind::Differential)
            )
        )
)]
#[bench::none_kind(
    args = (setup_worst_case_array(10)),
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .flamegraph(
               FlamegraphConfig::default()
                   .title("No bench-level flamegraph".to_owned())
                   .kind(FlamegraphKind::None)
            )
        )
)]
fn bench_level_flamegraphs(array: Vec<i32>) -> Vec<i32> {
    black_box(benchmark_tests::bubble_sort(black_box(array)))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .flamegraph(
               FlamegraphConfig::default().title("Library benchmark flamegraph".to_owned())
            )
        )
)]
fn without_bench_attribute() -> Vec<i32> {
    black_box(benchmark_tests::bubble_sort(black_box(vec![])))
}

#[library_benchmark]
#[bench::worst_case(setup_worst_case_array(10))]
fn main_level_flamegraph_config(array: Vec<i32>) -> Vec<i32> {
    black_box(benchmark_tests::bubble_sort(black_box(array)))
}

#[library_benchmark]
fn function_with_many_stacks() {
    println!("Hello World!");
}

library_benchmark_group!(
    name = benches,
    benchmarks = [
        bench_level_flamegraphs,
        without_bench_attribute,
        main_level_flamegraph_config,
        function_with_many_stacks
    ]
);

#[library_benchmark]
#[bench::fibonacci(5)]
fn recursive_function(n: u64) -> u64 {
    black_box(benchmark_tests::fibonacci(black_box(n)))
}

library_benchmark_group!(
    name = recursive,
    config =
        LibraryBenchmarkConfig::default()
            .tool(Callgrind::default().flamegraph(
                FlamegraphConfig::default().title("Group level flamegraph".to_owned())
            )),
    benchmarks = recursive_function
);

main!(
    config = LibraryBenchmarkConfig::default()
        .env("RUST_BACKTRACE", "1")
        .tool(
            Callgrind::default()
                .flamegraph(FlamegraphConfig::default().title("Main level flamegraph".to_owned()))
        ),
    library_benchmark_groups = [benches, recursive]
);
