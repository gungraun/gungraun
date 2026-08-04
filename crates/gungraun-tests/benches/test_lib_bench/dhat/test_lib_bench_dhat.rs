use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{Dhat, DhatMetric, EntryPoint, SanitizeOutput, Tool};
use gungraun_tests::{bubble_sort, setup_best_case_array, setup_worst_case_array};

#[inline(never)]
fn custom_setup(start: i32) -> Vec<i32> {
    setup_best_case_array(start);
    setup_worst_case_array(start)
}

#[inline(never)]
fn teardown(mut data: Vec<i32>) {
    let other = std::mem::take(&mut data);
    drop(data);
    drop(other);
}

fn is_coverage_run() -> bool {
    std::env::var("CARGO_LLVM_COV").is_ok_and(|e| e == "1")
}

fn hard_limits(tb: u64, tbk: u64, rb: u64, wb: u64) -> Vec<(DhatMetric, u64)> {
    if is_coverage_run() {
        vec![
            (DhatMetric::TotalBytes, tb),
            (DhatMetric::TotalBlocks, tbk),
            (DhatMetric::ReadsBytes, rb * 2),
            (DhatMetric::WritesBytes, wb * 2),
        ]
    } else {
        vec![
            (DhatMetric::TotalBytes, tb),
            (DhatMetric::TotalBlocks, tbk),
            (DhatMetric::ReadsBytes, rb),
            (DhatMetric::WritesBytes, wb),
        ]
    }
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .frames(["*::custom_setup"])
            .hard_limits(hard_limits(40, 2, 80, 120))
        )
)]
#[bench::with_entry_point(args = (5), setup = custom_setup, teardown = teardown)]
#[bench::without_entry_point(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .entry_point(EntryPoint::None)
        ),
    setup = custom_setup,
    teardown = teardown
)]
fn heap(data: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(data)))
}

#[library_benchmark]
#[bench::with_entry_point(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::with_args(["--mode=copy"])
            .hard_limits([
                (DhatMetric::TotalBytes, 20),
                (DhatMetric::TotalBlocks, 1)
            ])
        ),
    setup = custom_setup,
)]
#[bench::without_entry_point(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::with_args(["--mode=copy"])
            .entry_point(EntryPoint::None)
        ),
    setup = custom_setup,
)]
fn copy(mut src: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
    let mut dst: Vec<i32> = Vec::with_capacity(src.len());
    let src_len = src.len();

    // SAFETY: `src` contains `src_len` initialized `i32`s and `dst` has capacity for `src_len`
    // elements, so the non-overlapping source and destination ranges are valid.
    unsafe {
        std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), src_len);
    }
    // SAFETY: Shortening a vector to zero is within its capacity and prevents dropping the copied
    // elements twice.
    unsafe {
        src.set_len(0);
    }
    // SAFETY: The preceding copy initialized exactly `src_len` elements in `dst`.
    unsafe {
        dst.set_len(src_len);
    }

    (src, dst)
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::with_args(["--mode=ad-hoc"])
            .hard_limits([
                (DhatMetric::TotalUnits, 15),
                (DhatMetric::TotalEvents, 1)
            ])
        ),
)]
#[bench::with_entry_point(
    args = (5),
    setup = setup_worst_case_array
)]
#[bench::without_entry_point(
    args = (5),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .entry_point(EntryPoint::None)
        ),
    setup = setup_worst_case_array
)]
fn ad_hoc(data: Vec<i32>) -> Vec<i32> {
    gungraun::client_requests::dhat::ad_hoc_event(15);
    black_box(bubble_sort(black_box(data)))
}

// This test also shows that dhat compiles differently and the default toggle matching
// `__gungraun_wrapper_mod` is not present in the dhat output.
#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .hard_limits(hard_limits(20, 1, 0, 20))
        )
)]
#[bench::five(5)]
fn alloc_in_func(start: i32) -> Vec<i32> {
    setup_worst_case_array(start)
}

#[library_benchmark]
#[bench::default()]
#[bench::yes(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .sanitize_output(SanitizeOutput::Yes)
        ),
)]
#[bench::no(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .sanitize_output(SanitizeOutput::No)
        ),
)]
#[bench::keep_orig(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .sanitize_output(SanitizeOutput::KeepOrig)
        ),
)]
fn sanitize() -> Vec<i32> {
    black_box(bubble_sort(black_box(setup_worst_case_array(5))))
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [heap, copy, ad_hoc, alloc_in_func, sanitize]
);
main!(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .pass_through_env("CARGO_LLVM_COV"),
    library_benchmark_groups = my_group
);
