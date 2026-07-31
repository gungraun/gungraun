#![allow(clippy::unit_arg)]

use std::hint::black_box;

use benchmark_tests::print_env;
use gungraun::prelude::*;

pub const TEST_VAR_1: &str = "__GUNGRAUN_TEST_VAR_1";
pub const TEST_VAR_2: &str = "__GUNGRAUN_TEST_VAR_2";

#[library_benchmark]
#[bench::single(&[TEST_VAR_1])]
fn bench_print_env_single(args: &[&str]) {
    black_box(print_env(black_box(args)))
}

#[library_benchmark]
#[bench::multiple(&[TEST_VAR_1, TEST_VAR_2])]
fn bench_print_env_multiple(args: &[&str]) {
    black_box(print_env(black_box(args)))
}

#[library_benchmark]
#[bench::single(&["FOO=BAR"])]
fn bench_print_env_custom_single(args: &[&str]) {
    black_box(print_env(black_box(args)))
}

#[library_benchmark]
#[bench::multiple(&["FOO=BAR", "BAR=BAZ"])]
fn bench_print_env_custom_multiple(args: &[&str]) {
    black_box(print_env(black_box(args)))
}

#[library_benchmark(config = LibraryBenchmarkConfig::default().env("FOO", "BAR"))]
#[bench::multiple(
    args = (&["FOO=BAR", "BAR=BAZ"]),
    config = LibraryBenchmarkConfig::default().env("BAR", "BAZ")
)]
fn bench_print_env_when_config(args: &[&str]) {
    black_box(print_env(black_box(args)))
}

library_benchmark_group!(
    name = pass_through_single,
    config = LibraryBenchmarkConfig::default()
        .env_clear(true)
        .pass_through_env(TEST_VAR_1),
    benchmarks = bench_print_env_single
);

library_benchmark_group!(
    name = pass_through_multiple,
    config = LibraryBenchmarkConfig::default()
        .env_clear(true)
        .pass_through_envs([TEST_VAR_1, TEST_VAR_2]),
    benchmarks = bench_print_env_multiple
);

library_benchmark_group!(
    name = custom_single,
    config = LibraryBenchmarkConfig::default()
        .env_clear(true)
        .env("FOO", "BAR"),
    benchmarks = bench_print_env_custom_single
);

library_benchmark_group!(
    name = custom_multiple,
    config = LibraryBenchmarkConfig::default()
        .env_clear(true)
        .envs([("FOO", "BAR"), ("BAR", "BAZ")]),
    benchmarks = bench_print_env_custom_multiple
);

library_benchmark_group!(
    name = configs,
    config = LibraryBenchmarkConfig::default().env_clear(false),
    benchmarks = bench_print_env_when_config
);

main!(
    config = LibraryBenchmarkConfig::default().env_clear(false),
    library_benchmark_groups = [
        pass_through_single,
        pass_through_multiple,
        custom_single,
        custom_multiple,
        configs
    ]
);
