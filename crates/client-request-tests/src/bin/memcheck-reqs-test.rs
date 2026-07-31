use client_request_tests::MARKER;
use valgrind_requests::{self, cstring, memcheck, valgrind, valgrind_println_unchecked};

fn leak_memory() {
    for _ in 0..1 {
        let leaked_box = Box::leak(Box::new(vec![1]));
        unsafe {
            valgrind_println_unchecked!(
                "First value of leaked memory: {}",
                leaked_box.get_unchecked(0)
            )
        };
        let _ = leaked_box;
    }
}

fn main() {
    unsafe { valgrind_println_unchecked!("{MARKER}") };

    unsafe { valgrind::clo_change(cstring!("--leak-check=summary\0")) };

    memcheck::do_leak_check();
    let _ = memcheck::count_leaks();

    leak_memory();

    memcheck::do_leak_check();
    let _ = memcheck::count_leaks();

    leak_memory();

    memcheck::do_new_leak_check();
    let _ = memcheck::count_leaks();

    std::process::exit(valgrind::running_on_valgrind() as i32);
}
