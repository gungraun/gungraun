//! This is an example for setting up library benchmarks. It's best to read all the comments from
//! top to bottom to get a better understanding of the api.

use std::fs::File;
use std::hint::black_box;
use std::io::{BufRead, BufReader};

// These two functions from the benchmark-tests library serve as functions we want to benchmark
use benchmark_tests::{bubble_sort, fibonacci};
use gungraun::prelude::*;
use gungraun::{Callgrind, Dhat, EntryPoint, EventKind, Massif};

// This function is used to create the worst case array we want to sort with our implementation of
// bubble sort
fn setup_worst_case_array(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).rev().collect()
    } else {
        (0..start).rev().collect()
    }
}

// This function is used to create the best case array we want to sort with our implementation of
// bubble sort
fn setup_best_case_array(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).collect()
    } else {
        (0..start).collect()
    }
}

// The #[library_benchmark] attribute lets you define a benchmark function which you can later use
// in the `library_benchmark_groups!` macro. Just using the #[library_benchmark] attribute as a
// standalone is fine for simple function calls without parameters. However, we actually want to
// benchmark cases which would need to set up a vector with more elements, but everything we set up
// within the benchmark function itself is attributed to the event counts. See the next benchmark
// `bench_bubble_sort` function for a better example which uses the `bench` attribute to set up the
// benchmark with different vectors.
#[library_benchmark]
// If possible, it's best to return something from a benchmark function
fn bench_bubble_sort_empty() -> Vec<i32> {
    // The `black_box` is needed to tell the compiler to not optimize what's inside the black_box or
    // else the benchmarks might return inaccurate results.
    black_box(bubble_sort(black_box(vec![])))
}

// This benchmark uses the `bench` attribute to set up benchmarks with different setups. The big
// advantage is that the setup costs and event counts aren't attributed to the benchmark (and
// opposed to the old API we don't have to deal with Callgrind arguments, toggles, ...)
//
// The `bench` attribute consist of the attribute name itself, a unique id after `::` and
// optionally arguments with expressions which are passed to the benchmark function as parameter.
// Here we pass a single argument with `Vec<i32>` type to the benchmark. Wrap both input and
// output values in `black_box` to prevent the compiler from optimizing away computations.
#[library_benchmark]
// This bench is setting up the same benchmark case as above in the `bench_bubble_sort_empty` with
// the advantage that the setup costs for creating a vector (even if it is empty) aren't attributed
// to the benchmark.
#[bench::empty(vec![])]
// Some other use cases to play around with
#[bench::worst_case_6(vec![6, 5, 4, 3, 2, 1])]
#[bench::best_case_6(vec![1, 2, 3, 4, 5, 6, 7])]
#[bench::best_case_20(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20])]
// Function calls are fine too
#[bench::worst_case_4000(setup_worst_case_array(4000))]
#[bench::best_case_4000(setup_best_case_array(4000))]
// The argument of the benchmark function defines the type of the argument from the `bench` cases.
fn bench_bubble_sort(array: Vec<i32>) -> Vec<i32> {
    // Wrap both input and output in `black_box`.
    black_box(bubble_sort(black_box(array)))
}

// This benchmark serves as an example for a benchmark function having more than one argument
// (Actually, to benchmark the fibonacci function, a single argument would have been sufficient)
#[library_benchmark]
// Any expression is allowed as argument
#[bench::fib_5_plus_fib_10(255 - 250, 10)]
#[bench::fib_30_plus_fib_20(30, 20)]
fn bench_fibonacci_sum(first: u64, second: u64) -> u64 {
    black_box(fibonacci(black_box(first)) + fibonacci(black_box(second)))
}

// You can use the `benches` attribute to specify multiple benchmark runs in one go. You can specify
// multiple `benches` attributes or mix the `benches` attribute with `bench` attributes.
#[library_benchmark]
// This is the simple form. Each `,`-separated element is another benchmark run and is passed to the
// benchmarking function as parameter. So, this is the same as specifying two `#[bench]` attributes
// #[bench::multiple_0(vec![1])] and #[bench::multiple_1(vec![5])].
#[benches::multiple(vec![1], vec![5])]
// You can also use the `args` argument to achieve the same. Using `args` is necessary if you also
// want to specify a `config` or `setup` function.
#[benches::with_args(args = [vec![1], vec![5]], config = LibraryBenchmarkConfig::default())]
// Usually, each element in `args` is passed directly to the benchmarking function. You can instead
// reroute them to a `setup` function. In that case the return value of the setup function is passed
// as parameter to the benchmarking function.
#[benches::with_setup(args = [1, 5], setup = setup_worst_case_array)]
fn bench_bubble_sort_with_benches_attribute(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

/// Read the content of a file and produce an iterable vector with valid `u64` numbers as elements
fn read_file(path: &str) -> Vec<u64> {
    BufReader::new(File::open(path).unwrap())
        .lines()
        .filter_map(|line| line.ok().and_then(|line| line.parse::<u64>().ok()))
        .collect()
}

#[library_benchmark]
// A simple example with the iter parameter which uses ranges. `iter` accepts every argument that
// implements `IntoIterator` and there are no other limitations.
#[benches::ranges(iter = 1..3)]
// A more complex example to read the content of a file and produce a single benchmark cases per
// iterator element.
#[benches::from_file(iter = read_file("benches/fixtures/with_empty_line.fix"))]
fn bench_fibonacci_with_iter(a: u64) -> u64 {
    black_box(fibonacci(black_box(a)))
}

// A benchmarking function with multiple parameters requires the elements to be specified as tuples.
#[library_benchmark]
#[benches::multiple((1, 2), (3, 4))]
#[benches::with_args(args = [(1, 2), (3, 4)])]
fn bench_bubble_sort_with_multiple_parameters(a: i32, b: i32) -> Vec<i32> {
    black_box(bubble_sort(black_box(vec![a, b])))
}

// It's possible to specify a `LibraryBenchmarkConfig` valid for all benches of this
// `library_benchmark`. Since we only use the default here for demonstration purposes actually
// nothing changes. The default configuration is always applied.
#[library_benchmark(config = LibraryBenchmarkConfig::default())]
fn bench_fibonacci_with_config() -> u64 {
    black_box(fibonacci(black_box(8)))
}

// A `config` per `bench` or `benches` attribute is also possible using the alternative `bench`
// or `benches` attribute with key = value pairs
//
// Note that `LibraryBenchmarkConfig` is additive for tools and environment variables and appends
// them to the variables of `configs` of higher levels (like #[library_benchmark(config = ...)]).
// Only the last definition of a such configuration values is taken into account. Other
// non-collection like configuration values (like `RegressionConfig`) are overridden.
//
// Completely overriding previous definitions of valgrind tools instead of appending them with
// `LibraryBenchmarkConfig::tool` or can be achieved with `LibraryBenchmarkConfig::tool_override`.
#[library_benchmark]
#[bench::fib_with_config(
    args = (3, 4),
    config = LibraryBenchmarkConfig::default()
        .tool_override(Massif::default())
)]
fn bench_fibonacci_with_config_at_bench_level(first: u64, second: u64) -> u64 {
    black_box(fibonacci(black_box(first) + black_box(second)))
}

// Use the `benchmarks` argument of the `library_benchmark_group!` macro to collect all benchmarks
// you want and put them into the same group. The `name` is a unique identifier which is used in the
// `main!` macro to collect all `library_benchmark_groups`.
//
// It's also possible to specify a `LibraryBenchmarkConfig` valid for all benchmarks of this
// `library_benchmark_group`. We configure the regression checks to fail the whole benchmark run as
// soon as a performance regression happens. This'll overwrite `limits` and `fail_fast` of the
// configuration of the `main!` macro.
library_benchmark_group!(
    name = bubble_sort,
    config = LibraryBenchmarkConfig::default().tool(
        Callgrind::default()
            .soft_limits([(EventKind::Ir, 10.0)])
            .fail_fast(false)
    ),
    benchmarks = [
        bench_bubble_sort_empty,
        bench_bubble_sort,
        bench_bubble_sort_with_benches_attribute,
        bench_bubble_sort_with_multiple_parameters
    ]
);

// In our example file here, we could have put the fibonacci benchmarks into the same group as the
// bubble sort benchmarks. But, using a separate group allows us to specify different configurations
// for the benchmarks in this group. Since there are no heap allocations in the fibonacci function,
// we decided to switch DHAT off for all benchmarks in this group.
//
// Additionally, having separate groups can help organizing your benchmarks. The different groups
// are shown separately in the output of the callgrind run and the output files of a callgrind run
// are put in separate folders for each group.
library_benchmark_group!(
    name = fibonacci,
    config = LibraryBenchmarkConfig::default().tool(Dhat::default().enable(false)),
    benchmarks = [
        bench_fibonacci_sum,
        bench_fibonacci_with_config,
        bench_fibonacci_with_config_at_bench_level,
        bench_fibonacci_with_iter
    ]
);

// Finally, the mandatory main! macro which collects all `library_benchmark_groups` and optionally
// accepts a `config = ...` argument before the `library_benchmark_groups` argument. The main! macro
// creates a benchmarking harness and runs all the benchmarks defined in the groups and benches.
//
// Per default there are no regression checks. We configure the regression checks using
// `EventKind::Ir` (Total instructions executed) with a limit of `+5%` and
// `EventKind::EstimatedCycles` with a limit of `+10%`. These numbers are deliberate and may vary
// for your benchmarks. This `LibraryBenchmarkConfig` applies to all benchmarks in all groups
// (specified below) if it is not overwritten.
//
// In addition to or instead of running Callgrind it's possible to run other valgrind tools like
// Cachegrind, DHAT, Massif, (the experimental) BBV, Memcheck, Helgrind or DRD. Below we specify to
// run DHAT in addition to callgrind for all benchmarks (if not specified otherwise and/or
// overridden in a lower-level configuration). It's also possible to change the default tool to
// something else than callgrind with `LibraryBenchmarkConfig::default_tool`for example if you're
// just interested in running DHAT. Running cachegrind instead of callgrind is also possible but
// requires additional steps. This is best described in the guide:
// https://gungraun.github.io/gungraun/latest/html/index.html. You can also find a lot of other
// Gungraun feature descriptions there.
//
// The output files of the profiling tools (DHAT, Massif, BBV) can be found next to the output files
// of the callgrind runs in `target/gungraun/...`.
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(
            Callgrind::default()
                .soft_limits([(EventKind::Ir, 5.0), (EventKind::EstimatedCycles, 10.0)])
        )
        .tool(Dhat::default()
                // The default entry point of `Dhat`, similar to `Callgrind`, excludes setup costs.
                // As a result, it does not account for memory reads and writes in the bubble sort
                // function, since the arrays are allocated outside the benchmark function during
                // its setup phase. To address this issue, we utilize a custom entry point.
                // Fortunately, all our bubble sort benchmark functions contain the string
                // `bench_bubble_sort`, making it easy to identify them.
                .entry_point(EntryPoint::Custom("*bench_bubble_sort*".to_owned())
        )),
    library_benchmark_groups = [bubble_sort, fibonacci]
);
