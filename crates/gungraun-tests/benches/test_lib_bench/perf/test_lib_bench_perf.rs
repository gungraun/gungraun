use std::hint::black_box;
use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{Callgrind, Perf, PerfRunMode, Tool, perf_disable, perf_enable, perf_log};
use gungraun_tests::{bubble_sort, fibonacci, setup_worst_case_array};

fn print_debug<T>(input: T) -> usize
where
    T: std::fmt::Debug,
{
    let output = format!("BENCH: {input:?}");
    println!("{output}");
    output.len()
}

#[library_benchmark]
#[bench::default(1000)]
#[bench::one(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["task-clock"])
            // to provoke a warning which tells us that the args are parsed but only if record is
            // enabled
            .record_args(["-e", "foo"])
            .alpha(0.001)
        )
)]
#[bench::one_low_n(
    args = [10],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["task-clock"])
        )
)]
// The first one is discarded, so the result should be still only one measurement (without mean, rse
// calculation, ...)
#[bench::same_twice(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["task-clock,task-clock"])
        )
)]
#[bench::two_sets_same_event(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["task-clock", "task-clock"])
        )
)]
#[bench::two_sets_different_events(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["task-clock", "cpu-clock"])
            // Testing:
            // 1. that just adding `record_args` without enabling record doesn't do anything.
            // 2. sadly, these arguments don't have any side-effects we can verify but at least we
            //    can test that we don't provoke an error or panic when record is `true` or `false`
            .record_args(["--call-graph=dwarf", "-F", "99"])
        )
)]
#[bench::ten_sets(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets([
                "task-clock","cpu-clock","faults","context-switches","cpu-clock","task-clock",
                "context-switches","faults","task-clock","cpu-clock"
            ])
        )
)]
// dummy always has zero counter-value
#[bench::dummy(
    args = [1000],
    config = LibraryBenchmarkConfig::default().tool(Perf::default().event_sets(["dummy"]))
)]
// Since dummy is always zero this should result in an empty data set
#[bench::dummy_non_zero(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["dummy"])
            .non_zero_metrics(["dummy*"])
        )
)]
#[bench::default_calibrate(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(PerfRunMode::DefaultCalibrate)
        )
)]
#[bench::default_calibrate_low_n(
    args = [10],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            // Setting the override is necessary to avoid intermittent zero instruction metrics.
            .sample_duration(Duration::from_secs(2))
            .run_mode(PerfRunMode::DefaultCalibrate)
            .event_set("task-clock,cpu-clock,context-switches")
        )
)]
#[bench::calibrate_1_secs(
    args = [1000],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(PerfRunMode::Calibrate(Duration::from_secs(1)))
        )
)]
fn event_sets(n: i32) -> Vec<i32> {
    black_box(bubble_sort(setup_worst_case_array(black_box(n))))
}

#[library_benchmark]
#[bench::default_perf_with_callgrind(
    args = [],
    config = LibraryBenchmarkConfig::default().tool(Callgrind::default())
)]
#[bench::default_perf_with_callgrind_disable_perf(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Callgrind::default())
        .tool(Perf::default().enable(false))
)]
#[bench::default_callgrind_with_perf(
    args = [],
    config = LibraryBenchmarkConfig::default().default_tool(Tool::Callgrind).tool(Perf::default())
)]
#[bench::default_callgrind_disable_perf(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Callgrind)
        .tool(Perf::default().enable(false))
)]
fn with_other_tool() -> Vec<i32> {
    black_box(bubble_sort(setup_worst_case_array(black_box(1000))))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .disable_entry_point(true)
        )
)]
#[bench::with_arg(20)]
fn disabled_entry_point(n: u64) -> u64 {
    perf_log!("Printing to log shouldn't be measured");

    let lock = perf_enable!();
    let x = black_box(fibonacci(black_box(n)));
    perf_disable!(lock);

    x
}

// This is expected to produce an empty data set but no errors/panics
#[library_benchmark(
    config = LibraryBenchmarkConfig::default().tool(Perf::default().disable_entry_point(true))
)]
fn disabled_entry_point_without_measurement() -> u64 {
    black_box(fibonacci(black_box(20)))
}

#[library_benchmark]
#[bench::one_thousand(1000)]
fn generic<T>(arg: T) -> usize
where
    T: std::fmt::Debug,
{
    black_box(print_debug(black_box(arg)))
}

#[library_benchmark]
#[bench::one_thousand(args = [], consts = [1000])]
fn with_consts<const FOO: usize>() -> usize {
    black_box(print_debug(black_box(FOO)))
}

#[library_benchmark]
#[benches::two_to_four(iter = 2..=4)]
fn iter(arg: i32) -> Vec<i32> {
    black_box(bubble_sort(setup_worst_case_array(black_box(arg))))
}

library_benchmark_group!(
    name = without_sampling,
    benchmarks = [
        event_sets,
        with_other_tool,
        disabled_entry_point,
        disabled_entry_point_without_measurement,
        generic,
        with_consts,
        iter
    ]
);

library_benchmark_group!(
    name = with_sampling,
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default().sample_duration(Duration::from_secs(1))),
    benchmarks = [
        event_sets,
        with_other_tool,
        disabled_entry_point,
        disabled_entry_point_without_measurement,
        generic,
        with_consts,
        iter
    ]
);

library_benchmark_group!(
    name = record,
    config = LibraryBenchmarkConfig::default().tool(Perf::default().record(true)),
    benchmarks = [
        event_sets,
        disabled_entry_point,
        disabled_entry_point_without_measurement,
    ]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().event_set("task-clock,cpu-clock,faults,context-switches")),
    library_benchmark_groups = [without_sampling, with_sampling, record]
);
