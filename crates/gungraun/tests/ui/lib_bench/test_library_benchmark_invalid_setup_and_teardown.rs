use gungraun::prelude::*;

fn setup_no_args() -> u64 {
    42
}

fn setup_string(value: u64) -> String {
    format!("{value}")
}

fn teardown_unit(_: ()) {}

#[library_benchmark(setup = does_not_exist)]
fn bench10(value: u64) {
    _ = value;
}

#[library_benchmark(teardown = does_not_exist)]
fn bench11() {}

// setup has missing argument
#[library_benchmark(setup = setup_no_args)]
fn bench20(value: u64) {
    _ = value;
}

// setup has wrong output type
#[library_benchmark(setup = setup_string)]
fn bench21(value: u64) {
    _ = value;
}

// teardown has wrong argument type
#[library_benchmark(teardown = teardown_unit)]
fn bench40() -> u64 {
    42
}

fn main() {}
