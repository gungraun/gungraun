use gungraun::prelude::*;

#[library_benchmark]
#[bench::id(invalid = "value")]
pub fn bench0() {}

#[library_benchmark]
#[bench::id(args = "value")]
pub fn bench1() {}

fn main() {}
