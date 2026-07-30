use gungraun::prelude::*;

#[library_benchmark]
fn bench() -> Vec<u64> {
    vec![1]
}

// Since this benchmark function is equal to the `bench` function above, the compiler will optimize
// this one away (it has the longer name). This means for us, we can't match for
// `bench_with_longer_name` in the callgrind output files and need a little bit of trickery.
#[library_benchmark]
fn bench_with_longer_name() -> Vec<u64> {
    vec![1]
}

// The same here but now we annotate the function with a `#[bench]` to take another path in the
// source code of `gungraun-macros`
#[library_benchmark]
#[bench::first(1)]
fn bench_with_bench(value: u64) -> Vec<u64> {
    vec![value]
}

#[library_benchmark]
#[bench::first(1)]
fn bench_with_bench_longer_name(value: u64) -> Vec<u64> {
    vec![value]
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [
        bench,
        bench_with_longer_name,
        bench_with_bench,
        bench_with_bench_longer_name
    ]
);

main!(library_benchmark_groups = my_group);
