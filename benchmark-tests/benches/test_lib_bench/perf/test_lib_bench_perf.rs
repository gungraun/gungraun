use std::hint::black_box;
use std::time::Duration;

use benchmark_tests::{bubble_sort, setup_worst_case_array};
use gungraun::prelude::*;
use gungraun::{Callgrind, Perf, Tool};

#[library_benchmark]
#[bench::default()]
#[bench::one(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["instructions"])
            // to provoke a warning which tells us that the args are parsed but only if record is
            // enabled
            .record_args(["-D", "1000"])
            .alpha(0.001)
        )
)]
#[bench::same_twice(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["instructions,instructions"])
        )
)]
#[bench::two_sets_same_event(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["instructions", "instructions"])
        )
)]
#[bench::two_sets_different_events(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets(["instructions", "task-clock"])
            // Testing:
            // 1. that just adding `record_args` without enabling record doesn't do anything.
            // 2. sadly, these arguments don't have any side-effects we can verify but at least we
            //    can test that we don't provoke an error or panic when record is `true` or `false`
            .record_args(["--call-graph=dwarf", "-F", "99"])
        )
)]
#[bench::ten_sets(
    args = [],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .event_sets([
                "instructions:u","cycles:u","ref-cycles","task-clock","cpu-clock","faults",
                "context-switches","branches","branch-misses","cache-misses"
            ])
        )
)]
fn event_sets() -> Vec<i32> {
    black_box(bubble_sort(setup_worst_case_array(black_box(1000))))
}

// Far too many slots to succeed without multiplexing on a regular cpu when using sampling. The low
// `min_pcnt_running` value ensures we still count all events which have a valid counter-value.
#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .min_pcnt_running(0.0)
            .event_sets([
                "instructions,instructions,instructions,instructions,instructions,\
                instructions,instructions,instructions,instructions,instructions,\
                instructions,instructions,instructions,instructions,instructions,\
                instructions,instructions,instructions,instructions,instructions,\
                instructions,instructions,instructions,instructions,instructions,\
                instructions,instructions,instructions,instructions,instructions"
            ])
        )
)]
fn thirty_events_sampled_then_multiplexing() -> Vec<i32> {
    black_box(bubble_sort(setup_worst_case_array(black_box(1000))))
}

#[library_benchmark]
#[bench::default_perf_with_callgrind(
    args = [],
    config = LibraryBenchmarkConfig::default().tool(Callgrind::default())
)]
#[bench::default_callgrind_with_perf(
    args = [],
    config = LibraryBenchmarkConfig::default().default_tool(Tool::Callgrind).tool(Perf::default())
)]
fn with_other_tool() -> Vec<i32> {
    black_box(bubble_sort(setup_worst_case_array(black_box(1000))))
}

library_benchmark_group!(
    name = without_sampling,
    benchmarks = [event_sets, with_other_tool]
);

library_benchmark_group!(
    name = with_sampling,
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default().sample_duration(Duration::from_secs(1))),
    benchmarks = [
        event_sets,
        with_other_tool,
        thirty_events_sampled_then_multiplexing
    ]
);

library_benchmark_group!(
    name = record,
    config = LibraryBenchmarkConfig::default().tool(Perf::default().record(true)),
    benchmarks = event_sets
);

main!(
    config = LibraryBenchmarkConfig::default().default_tool(Tool::Perf),
    library_benchmark_groups = [without_sampling, with_sampling, record]
);
