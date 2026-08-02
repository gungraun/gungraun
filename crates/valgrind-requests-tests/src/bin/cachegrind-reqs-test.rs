#![allow(unused_imports)]

use valgrind_requests::{
    self, cachegrind, cstring, valgrind, valgrind_printf, valgrind_println,
    valgrind_println_unchecked,
};
use valgrind_requests_tests::MARKER;

fn do_work(start: i32) -> i32 {
    let mut sum = start;

    for i in 1..10 {
        sum += i;
    }
    sum
}

fn main() {
    let _ = MARKER;

    // SAFETY: This standalone test intentionally exercises the unchecked macro with a static format
    // string and no format arguments.
    #[cfg_attr(not(feature = "_act"), expect(unused_unsafe))]
    unsafe {
        valgrind_println_unchecked!("{MARKER}");
    }

    cachegrind::start_instrumentation();

    let i = do_work(0);

    cachegrind::stop_instrumentation();

    let result = do_work(i);
    valgrind_println!("result: {result}").unwrap();
    let _ = result;

    std::process::exit(i32::from(valgrind::running_on_valgrind() != 0));
}
