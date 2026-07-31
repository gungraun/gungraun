use gungraun::prelude::*;

#[library_benchmark]
#[bench::id(config = "some")]
pub fn bench00() {}

#[library_benchmark]
#[bench::id(wrong = LibraryBenchmarkConfig::default())]
pub fn bench10() {}

#[library_benchmark]
#[bench::id(config = )]
pub fn bench20() {}

#[library_benchmark]
#[benches::wrong(wrong = LibraryBenchmarkConfig::default())]
pub fn bench30() {}

#[library_benchmark]
#[benches::missing_expression(args = [], config = )]
pub fn bench50() {}

fn main() {}
