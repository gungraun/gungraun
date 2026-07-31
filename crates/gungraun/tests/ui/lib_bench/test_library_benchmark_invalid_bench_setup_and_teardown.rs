use gungraun::prelude::*;

fn setup_no_args() -> u64 {
    42
}

fn setup_u64_to_string(value: u64) -> String {
    format!("{value}")
}

fn setup_str_to_string(value: &str) -> String {
    value.to_owned()
}

fn teardown_unit(_: ()) {}

fn teardown_u64(_: u64) {}

// setup with wrong input type
#[library_benchmark]
#[bench::id(args = ("wrong type"), setup = setup_u64_to_string)]
fn bench10(a: String) {
    _ = a;
}

// setup with wrong output type
#[library_benchmark]
#[bench::id(args = (10), setup = setup_u64_to_string)]
fn bench11(a: u64) {
    _ = a;
}

// global correct setup is overwritten with wrong setup
#[library_benchmark(setup = setup_str_to_string)]
#[bench::id(args = ("wrong type"), setup = setup_u64_to_string)]
fn bench12(a: String) {
    _ = a;
}

// teardown has wrong input type
#[library_benchmark]
#[bench::id(teardown = teardown_unit)]
fn bench20() -> u64 {
    42
}

// global correct teardown is overwritten with wrong teardown
#[library_benchmark(teardown = teardown_u64)]
#[bench::id(teardown = teardown_unit)]
fn bench21() -> u64 {
    42
}

// setup with wrong input type
#[library_benchmark]
#[benches::id(args = ["wrong type"], setup = setup_u64_to_string)]
fn bench30(a: String) {
    _ = a;
}

// setup with wrong output type
#[library_benchmark]
#[benches::id(args = [10], setup = setup_u64_to_string)]
fn bench31(a: u64) {
    _ = a;
}

// global correct setup is overwritten with wrong setup
#[library_benchmark(setup = setup_str_to_string)]
#[benches::id(args = ["wrong type"], setup = setup_u64_to_string)]
fn bench32(a: String) {
    _ = a;
}

// teardown has wrong input type
#[library_benchmark]
#[benches::id(teardown = teardown_unit)]
fn bench40() -> u64 {
    42
}

// global correct teardown is overwritten with wrong teardown
#[library_benchmark(teardown = teardown_u64)]
#[benches::id(teardown = teardown_unit)]
fn bench41() -> u64 {
    42
}

fn main() {}
