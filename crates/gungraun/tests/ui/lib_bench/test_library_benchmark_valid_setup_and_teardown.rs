use gungraun::prelude::*;

fn setup_no_args() -> u64 {
    42
}

fn teardown_unit(_: ()) {}

// no parameters
#[library_benchmark]
fn bench10() {}

// no parameters but useless parentheses
#[library_benchmark()]
fn bench11() {}

// just teardown
#[library_benchmark(teardown = teardown_unit)]
fn bench12() {}

// just setup
#[library_benchmark(setup = setup_no_args)]
fn bench13(value: u64) {
    _ = value;
}

// just config
#[library_benchmark(config = LibraryBenchmarkConfig::default())]
fn bench14() {}

// with all valid parameters
#[library_benchmark(
    config = LibraryBenchmarkConfig::default(),
    setup = setup_no_args,
    teardown = teardown_unit
)]
fn bench20(value: u64) {
    _ = value;
}

// Mix the order of the parameters
#[library_benchmark(
    teardown = teardown_unit,
    setup = setup_no_args,
    config = LibraryBenchmarkConfig::default()
)]
fn bench21(value: u64) {
    _ = value;
}

fn main() {}
