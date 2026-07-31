use gungraun::prelude::*;

#[library_benchmark(wrong = LibraryBenchmarkConfig::default())]
fn bench1() {}

#[library_benchmark(config = "wrong")]
fn bench2() {}

#[library_benchmark(config = )]
fn bench3() {}

#[library_benchmark(setup = )]
fn bench4() {}

#[library_benchmark(teardown = )]
fn bench4() {}

fn main() {}
