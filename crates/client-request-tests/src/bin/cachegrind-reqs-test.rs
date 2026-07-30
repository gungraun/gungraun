#![allow(unused_imports)]

use client_request_tests::MARKER;
use valgrind_requests::{
    self, cachegrind, cstring, valgrind, valgrind_printf, valgrind_println,
    valgrind_println_unchecked,
};

fn do_work(start: i32) -> i32 {
    let mut sum = start;

    for i in 1..10 {
        sum += i;
    }
    sum
}

fn main() {
    unsafe {
        valgrind_println_unchecked!("{MARKER}");
    }

    cachegrind::start_instrumentation();

    let i = do_work(0);

    cachegrind::stop_instrumentation();

    let result = do_work(i);
    valgrind_println!("result: {result}").unwrap();

    std::process::exit(valgrind::running_on_valgrind() as i32);
}
