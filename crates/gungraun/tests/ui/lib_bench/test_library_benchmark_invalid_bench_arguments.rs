use gungraun::prelude::*;

// missing argument of the benchmark
#[library_benchmark]
#[bench::id(42)]
#[benches::multi(42)]
fn bench10() {}

// missing argument of the bench attribute
#[library_benchmark]
#[bench::id()]
fn bench20(my: i32) {}

#[library_benchmark]
#[benches::multi(args = [])]
fn bench25(my: i32) {}

// too many arguments of the bench attribute
#[library_benchmark]
#[bench::id(42, 8)]
fn bench30(my: i32) {}

#[library_benchmark]
#[benches::multi((42, 8))]
fn bench30(my: i32) {}

// incorrect argument type
#[library_benchmark]
#[bench::id("hello")]
fn bench40(my: u8) {}

#[library_benchmark]
#[benches::multi("hello")]
fn bench45(my: u8) {}

#[library_benchmark]
#[bench::id(42, "hello")]
fn bench47(first: u8, second: u8) {}

// incorrect return type
#[library_benchmark]
#[bench::id(42)]
fn bench5(my: u8) -> String {
    my
}

fn main() {}
