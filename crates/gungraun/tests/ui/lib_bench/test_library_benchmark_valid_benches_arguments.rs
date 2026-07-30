use gungraun::prelude::*;

fn some_setup(my: u8) -> u64 {
    my as u64 + 1
}

fn some_two_setup(a: i32, b: i32) -> (i32, i32) {
    (a + b, a - b)
}

fn some_two_to_one_setup(a: i32, b: i32) -> i32 {
    a + b
}

#[library_benchmark]
#[benches::id10(8)]
#[benches::id15(8 + 9)]
#[benches::id20((8))]
#[benches::id30(8, 9)]
#[benches::id35(8, 9 + 10)]
#[benches::id40((8), (9))]
#[benches::id50(args = [8])]
#[benches::id60(args = [8, 9])]
fn bench110(arg: u8) -> u8 {
    arg
}

#[library_benchmark]
#[benches::id10(((8, 9)))]
#[benches::id15(((8, 9)), ((10, 20)))]
#[benches::id20(args = [((8, 9))])]
#[benches::id30(args = [((8, 9)), ((10, 20))])]
fn bench115((arg1, arg2): (u8, u8)) -> u8 {
    arg1 + arg2
}

#[library_benchmark]
#[benches::id10((1, 2))]
#[benches::id20((1, 2), (3, 4))]
#[benches::id30(args = [(1, 2)])]
#[benches::id40(args = [(1, 2), (3, 4)])]
fn bench120(first: u8, second: u8) -> u8 {
    first + second
}

#[library_benchmark]
#[benches::id10(args = [1, 2], setup = some_setup)]
#[benches::id20(args = [1 + 2, 2], setup = some_setup)]
#[benches::id30(args = [(1 + 2), 2], setup = some_setup)]
fn bench125(arg1: u64) -> u64 {
    arg1
}

#[library_benchmark]
#[benches::id10(args = [(1, 2)], setup = some_two_setup)]
#[benches::id20(args = [(1, 2), (3, 4)], setup = some_two_setup)]
fn bench130((a, b): (i32, i32)) -> i32 {
    a + b
}

#[library_benchmark]
#[benches::id10(args = [(1, 2)], setup = some_two_to_one_setup)]
#[benches::id20(args = [(1, 2), (3, 4)], setup = some_two_to_one_setup)]
fn bench140(a: i32) -> i32 {
    a
}

fn main() {}
