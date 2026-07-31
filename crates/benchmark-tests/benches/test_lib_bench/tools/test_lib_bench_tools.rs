use std::cell::RefCell;
use std::ffi::OsString;
use std::io;
use std::process::Output;
use std::rc::Rc;

struct Left(Option<Rc<Right>>);
#[expect(dead_code)]
struct Right(Option<Rc<RefCell<Left>>>);

use std::hint::black_box;

use benchmark_tests::{bubble_sort, bubble_sort_allocate, setup_worst_case_array, subprocess};
use gungraun::prelude::*;
use gungraun::{Bbv, Callgrind, Dhat, Drd, EventKind, Helgrind, Massif, Memcheck, OutputFormat};

#[library_benchmark]
#[bench::empty(
    args = (vec![]),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default().enable(false))
)]
#[bench::worst_case_4000(
    args = (4000),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .frames(["*::setup_worst_case_array"])
        ),
    setup = setup_worst_case_array
)]
fn bench_bubble_sort(array: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(array)))
}

#[library_benchmark]
fn bench_bubble_sort_allocate() -> i32 {
    black_box(bubble_sort_allocate(black_box(4000), black_box(2000)))
}

#[library_benchmark]
#[bench::trace_children(
    args = (),
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--toggle-collect=sort::main"]))
        .tool(Dhat::default().frames(["sort::main"]))
        .output_format(OutputFormat::default()
            .show_intermediate(true)
        )
)]
#[bench::no_trace_children(
    args = (),
    config = LibraryBenchmarkConfig::default()
        .valgrind_args(["trace-children=no"])
        .output_format(OutputFormat::default()
            .show_intermediate(true)
        )
)]
fn bench_subprocess() -> io::Result<Output> {
    println!("Do something before calling subprocess");
    black_box(subprocess(
        black_box(env!("CARGO_BIN_EXE_sort")),
        black_box(Vec::<OsString>::new()),
    ))
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool_override(Dhat::default())
.tool_override(
            Memcheck::with_args([
                "--leak-check=full",
                "--errors-for-leak-kinds=all",
                "--error-exitcode=0",
                "--time-stamp=yes"
            ])
        )
        .tool_override(Massif::default())
)]
fn bad_memory() {
    for _ in 0..100_000 {
        let left = Rc::new(RefCell::new(Left(None)));
        let right = Rc::new(Right(Some(Rc::clone(&left))));
        left.borrow_mut().0 = Some(Rc::clone(&right));
    }
}
library_benchmark_group!(
    name = bench_group,
    benchmarks = [
        bench_bubble_sort_allocate,
        bench_subprocess,
        bench_bubble_sort,
        bad_memory
    ]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .tool(
            Callgrind::default()
                .soft_limits([(EventKind::Ir, 5.0), (EventKind::EstimatedCycles, 10.0)])
        )
        .tool(Dhat::with_args(["--time-stamp=yes"]))
        .tool(Massif::default())
        .tool(Bbv::default())
        .tool(Memcheck::with_args(["--time-stamp=yes"]))
        .tool(Drd::default())
        .tool(Helgrind::default()),
    library_benchmark_groups = bench_group
);
