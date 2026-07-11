use std::hint::black_box;
use std::time::Duration;

use benchmark_tests::{bubble_sort, setup_worst_case_array};
use gungraun::prelude::*;
use gungraun::{Callgrind, Perf, PerfRunMode, Tool};

#[library_benchmark]
#[bench::multiple_event_sets(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["instructions", "instructions"])
        )
)]
#[bench::dynamic_batch(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(PerfRunMode::DynamicBatch)
        )
)]
#[bench::fixed_batch(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(PerfRunMode::FixedBatch(50))
        )
)]
#[bench::default_calibrate(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(PerfRunMode::DefaultCalibrate)
        )
)]
#[bench::calibrate_with_count(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(PerfRunMode::Calibrate(Duration::from_millis(100)))
        )
)]
#[bench::raw(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
        .run_mode(PerfRunMode::Direct)
        )
)]
fn bench_perf() -> Vec<i32> {
    black_box(bubble_sort(setup_worst_case_array(black_box(1))))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default().tool(Callgrind::default().args(["--branch-sim=yes"]))
)]
#[bench::some(args = [100], setup = setup_worst_case_array)]
fn bench_perf_more(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

#[library_benchmark]
#[bench::raw(
    args = [10000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(PerfRunMode::DynamicBatch)
        ),
    setup = setup_worst_case_array
)]
fn bench_perf_record(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

#[library_benchmark]
#[bench::ten_thousand(
    args = [10],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            // .alpha(0.001)
            .run_mode(PerfRunMode::Calibrate(Duration::from_secs(1)))
            // .event_set("faults,instructions:u,cycles:u,task-clock,cpu-clock,context-switches,branch-misses,cache-misses")
            // .event_set("instructions,ocr.demand_data_rd.l3_hit.snoop_hit_with_fwd")
            .record(true)
            .record_args(["--verbose"])
            .sample_duration(Duration::from_secs(2))
            .soft_limits([("*instructions*", 1.0), ("task-clock*", 10.0)])
            .hard_limits([("*instructions*", None, 500)])
        )
        .tool(Callgrind::default().args(["branch-sim=yes"])),
    setup = setup_worst_case_array
)]
fn bench_perf_samples(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

// fn setup() -> String {
//     format!("h")
// }
//
// #[library_benchmark(setup = setup)]
// fn bench_perf_standalone<T>(num: T) -> T
// where
//     T: std::fmt::Display,
// {
//     black_box(num)
// }

library_benchmark_group!(
    name = my_group,
    // benchmarks = [bench_perf, bench_perf_more, bench_perf_record]
    benchmarks = [bench_perf_samples]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default()),
    library_benchmark_groups = my_group
);

// use gungraun::Callgrind;
// main!(
//     config = LibraryBenchmarkConfig::default()
//         .default_tool(Tool::Perf)
//         .tool(Perf::default())
//         .tool(Callgrind::default()),
//     library_benchmark_groups = my_group
// );
