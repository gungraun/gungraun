#![cfg_attr(not(feature = "_act"), expect(unused_unsafe))]

use valgrind_requests::{self, cstring, memcheck, valgrind, valgrind_println_unchecked};
use valgrind_requests_tests::MARKER;

fn leak_memory() {
    for _ in 0..1 {
        let leaked_box = Box::leak(Box::new(vec![1]));
        // SAFETY: The vector is initialized with one element, so index zero is in bounds.
        let first = unsafe { leaked_box.get_unchecked(0) };
        // SAFETY: This standalone test intentionally exercises the unchecked macro with a static
        // format string and the valid reference above.
        unsafe {
            valgrind_println_unchecked!("First value of leaked memory: {first}");
        }
        let _ = first;
        let _ = leaked_box;
    }
}

fn main() {
    let _ = MARKER;
    // SAFETY: This standalone test intentionally exercises the unchecked macro with a static format
    // string and no format arguments.
    unsafe {
        valgrind_println_unchecked!("{MARKER}");
    }

    // SAFETY: The literal is NUL-terminated, has no interior NUL bytes, and static storage
    // duration.
    let leak_check = unsafe { cstring!("--leak-check=summary\0") };
    valgrind::clo_change(leak_check);

    memcheck::do_leak_check();
    let _ = memcheck::count_leaks();

    leak_memory();

    memcheck::do_leak_check();
    let _ = memcheck::count_leaks();

    leak_memory();

    memcheck::do_new_leak_check();
    let _ = memcheck::count_leaks();

    std::process::exit(i32::from(valgrind::running_on_valgrind() != 0));
}
