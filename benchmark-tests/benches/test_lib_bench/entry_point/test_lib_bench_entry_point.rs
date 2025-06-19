use std::hint::black_box;

use benchmark_tests::assert::Assert;
use iai_callgrind::{
    library_benchmark, library_benchmark_group, main, Callgrind, EntryPoint, EventKind,
    FlamegraphConfig, LibraryBenchmarkConfig, ValgrindTool,
};
use iai_callgrind_runner::runner::callgrind::hashmap_parser::SourcePath;
use iai_callgrind_runner::runner::summary::{BenchmarkSummary, ToolMetricSummary};

#[inline(never)]
fn nested() -> u64 {
    benchmark_tests::fibonacci(10)
}

#[inline(never)]
fn some_func() -> u64 {
    nested()
}

#[library_benchmark]
#[bench::none(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .entry_point(EntryPoint::None)
        )
)]
#[bench::default(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .entry_point(EntryPoint::Default)
        )
)]
#[bench::nested(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .entry_point(EntryPoint::from("test_lib_bench_entry_point::nested"))
        )
)]
fn bench_lib() -> u64 {
    black_box(some_func())
}

// TODO: DOUBLE CHECK DUE TO CHANGE to use ToolSummary, ToolMetricSummary::CallgrindSummary, ...
// We need to check some gory details in the group teardown to see if the entry point is being
// applied correctly.
fn assert_none() {
    let assert = Assert::new(module_path!(), "my_group", "bench_lib", "none").unwrap();
    assert
        .summary(|b| {
            let callgrind_summary = b
                .profiles
                .iter()
                .find(|p| p.tool == ValgrindTool::Callgrind)
                .unwrap();
            let ToolMetricSummary::Callgrind(metrics_summary) =
                &callgrind_summary.summaries.parts[0].metrics_summary
            else {
                panic!();
            };
            let new_ir = metrics_summary
                .diff_by_kind(&EventKind::Ir)
                .unwrap()
                .metrics
                .left()
                .unwrap();
            *new_ir > 400000
        })
        .unwrap();
}

// TODO: DOUBLE CHECK DUE TO same change in assert_none
fn assert_default() {
    let check_summary = |b: BenchmarkSummary| {
        let callgrind_summary = b
            .profiles
            .iter()
            .find(|p| p.tool == ValgrindTool::Callgrind)
            .unwrap();
        let ToolMetricSummary::Callgrind(metrics_summary) =
            &callgrind_summary.summaries.parts[0].metrics_summary
        else {
            panic!();
        };
        let new_ir = metrics_summary
            .diff_by_kind(&EventKind::Ir)
            .unwrap()
            .metrics
            .left()
            .unwrap();
        *new_ir < 3000
    };

    let assert = Assert::new(module_path!(), "my_group", "bench_lib", "default").unwrap();
    assert.summary(check_summary).unwrap();
    assert
        .callgrind_map(|m| {
            let main_costs = m.map
                .iter()
                .find_map(|(k, v)| (k.func == "main").then(|| v.metrics.clone()))
                .unwrap();

            let benchmark_function_costs = m.map
                    .iter()
                    .find_map(|(k, v)| {
                        (
                            k.func == "test_lib_bench_entry_point::bench_lib::__iai_callgrind_wrapper_mod::bench_lib" &&
                            k.file == Some(SourcePath::Relative(file!().into()))
                        )
                        .then(|| v.metrics.clone())
                    })
                    .unwrap();
            main_costs == benchmark_function_costs
        })
        .unwrap();
}

// TODO: DOUBLE CHECK DUE TO same change in assert_none
fn assert_nested() {
    let check_summary = |b: BenchmarkSummary| {
        let callgrind_summary = b
            .profiles
            .iter()
            .find(|p| p.tool == ValgrindTool::Callgrind)
            .unwrap();
        let ToolMetricSummary::Callgrind(metrics_summary) =
            &callgrind_summary.summaries.parts[0].metrics_summary
        else {
            panic!();
        };
        let new_ir = metrics_summary
            .diff_by_kind(&EventKind::Ir)
            .unwrap()
            .metrics
            .left()
            .unwrap();
        *new_ir < 3000
    };

    let assert = Assert::new(module_path!(), "my_group", "bench_lib", "nested").unwrap();
    assert.summary(check_summary).unwrap();
    assert
        .callgrind_map(|m| {
            let main_costs = m
                .map
                .iter()
                .find_map(|(k, v)| (k.func == "main").then(|| v.metrics.clone()))
                .unwrap();
            let nested_function_costs = m
                .map
                .iter()
                .find_map(|(k, v)| {
                    (k.func == "test_lib_bench_entry_point::nested"
                        && k.file == Some(SourcePath::Relative(file!().into())))
                    .then(|| v.metrics.clone())
                })
                .unwrap();
            main_costs == nested_function_costs
        })
        .unwrap();
}

fn assert_benchmarks() {
    assert_none();
    assert_default();
    assert_nested();
}

library_benchmark_group!(
    name = my_group;
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .flamegraph(FlamegraphConfig::default())
        );
    teardown = assert_benchmarks();
    benchmarks = bench_lib
);
main!(library_benchmark_groups = my_group);
