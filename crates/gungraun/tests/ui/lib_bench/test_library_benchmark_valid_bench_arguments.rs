use gungraun::prelude::*;

#[library_benchmark]
#[bench::id1(42)]
#[bench::id2(0)]
#[bench::id3(255)]
fn bench1(my: u8) -> u8 {
    my
}

#[library_benchmark]
fn bench2() {}

#[library_benchmark]
#[bench::id1()]
#[bench::id2()]
fn bench3() {}

#[library_benchmark]
#[bench::id1(55, 200)]
pub fn bench4(my: u8, other: u8) -> u8 {
    my + other
}

fn some_setup(my: u8) -> u64 {
    my as u64 + 1
}

// check that some_setup is accessible
#[library_benchmark]
#[bench::id1(some_setup(8))]
pub fn bench5(my: u64) -> u64 {
    if my != 9 {
        panic!("Should pass");
    } else {
        my
    }
}

#[library_benchmark]
#[bench::id1(args = (some_setup(8)))]
pub fn bench8(my: u64) -> u64 {
    if my != 9 {
        panic!("Should pass");
    } else {
        my
    }
}

#[library_benchmark(config = LibraryBenchmarkConfig::default())]
fn bench6() {}

#[library_benchmark]
#[bench::id1(config = LibraryBenchmarkConfig::default())]
#[bench::id2(args = (), config = LibraryBenchmarkConfig::default())]
fn bench7() {}

#[library_benchmark]
#[bench::id1(args = (8), config = LibraryBenchmarkConfig::default())]
fn bench9(arg: u8) -> u8 {
    arg
}

#[library_benchmark]
#[bench::id1(args = (some_setup(8)), config = LibraryBenchmarkConfig::default())]
pub fn bench10(my: u64) -> u64 {
    if my != 9 {
        panic!("Should pass");
    } else {
        my
    }
}

fn main() {}
