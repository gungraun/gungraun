#![allow(unused_imports)]

use client_request_tests::MARKER;
use valgrind_requests::{
    self, callgrind, cstring, valgrind, valgrind_printf, valgrind_println,
    valgrind_println_unchecked,
};

fn do_work(start: i32) -> i32 {
    let mut sum = start;

    for i in 1..10 {
        sum += i;
    }
    sum
}

fn client_requests_1() -> i32 {
    let mut sum = do_work(0);

    callgrind::zero_stats();

    sum += do_work(sum);
    callgrind::dump_stats();

    sum += do_work(sum);
    callgrind::dump_stats_at(unsafe { cstring!("Please dump here") });

    do_work(sum)
}

fn client_requests_2() -> i32 {
    let mut sum = client_requests_1();

    callgrind::toggle_collect();

    sum += client_requests_1();
    callgrind::toggle_collect();

    sum
}

fn main() {
    unsafe {
        valgrind_println_unchecked!("{MARKER}");
    }

    client_requests_2();

    callgrind::stop_instrumentation();

    client_requests_2();

    callgrind::start_instrumentation();

    client_requests_2();

    callgrind::stop_instrumentation();

    client_requests_2();

    callgrind::start_instrumentation();

    std::process::exit(valgrind::running_on_valgrind() as i32);
}
