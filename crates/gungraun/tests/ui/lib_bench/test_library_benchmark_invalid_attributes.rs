use gungraun::prelude::*;

#[library_benchmark]
#[b]
fn bench1() {}

#[library_benchmark]
#[inline(never)]
fn bench2() {}

#[inline(never)]
#[library_benchmark]
fn bench3() {}

#[bench::id()]
#[library_benchmark]
fn bench4() {}

fn main() {}
