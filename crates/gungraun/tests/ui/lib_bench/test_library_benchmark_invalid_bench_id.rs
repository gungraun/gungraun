use gungraun::prelude::*;

// missing id
#[library_benchmark]
#[bench]
fn bench1() {}

#[library_benchmark]
#[bench::missing_parentheses]
fn bench2() {}

#[library_benchmark]
#[bench::same()]
#[bench::same()]
fn bench3() {}

#[library_benchmark]
#[benches::no_args()]
fn bench4() {}

#[library_benchmark]
#[benches::same(1)]
#[benches::same(1)]
fn bench5(_arg: i32) {}

fn main() {}
