#![allow(unused_imports)]

use valgrind_requests::{
    self, valgrind, valgrind_printf, valgrind_printf_backtrace,
    valgrind_printf_backtrace_unchecked, valgrind_printf_unchecked, valgrind_println,
    valgrind_println_backtrace, valgrind_println_backtrace_unchecked, valgrind_println_unchecked,
};
use valgrind_requests_tests::MARKER;

#[cfg_attr(not(feature = "_act"), expect(unused_unsafe))]
fn main() {
    let invalid_cstring = "INV\0LID";
    let valid_cstring = "foo";
    let _ = (invalid_cstring, valid_cstring, MARKER);

    // SAFETY: This test intentionally exercises the unchecked macro with a static format string.
    unsafe {
        valgrind_println_unchecked!("{MARKER}");
    }

    valgrind_printf!("printf: {valid_cstring}\n").unwrap();
    valgrind_printf!("printf (invalid): {invalid_cstring}\n").unwrap_err();
    valgrind_println!().unwrap();
    // SAFETY: This test intentionally exercises the unchecked macro with a static format string.
    unsafe {
        valgrind_printf_unchecked!("printf unchecked: {valid_cstring}\n");
    }
    // SAFETY: This test intentionally exercises the unchecked macro with a static format string.
    unsafe {
        valgrind_printf_unchecked!("printf unchecked (invalid): {invalid_cstring}\n");
    }
    // SAFETY: This test intentionally exercises the unchecked macro with no format arguments.
    unsafe {
        valgrind_println_unchecked!();
    }

    valgrind_println!("println: {valid_cstring}").unwrap();
    valgrind_println!("println (invalid): {invalid_cstring}").unwrap_err();
    // SAFETY: This test intentionally exercises the unchecked macro with a static format string.
    unsafe {
        valgrind_println_unchecked!("println unchecked: {valid_cstring}");
    }
    // SAFETY: This test intentionally exercises the unchecked macro with a static format string.
    unsafe {
        valgrind_println_unchecked!("println unchecked (invalid): {invalid_cstring}");
    }
    // SAFETY: This test intentionally exercises the unchecked macro with no format arguments.
    unsafe {
        valgrind_println_unchecked!();
    }

    valgrind_printf_backtrace!("printf backtrace: {valid_cstring}\n").unwrap();
    valgrind_printf_backtrace!("printf backtrace (invalid): {invalid_cstring}\n").unwrap_err();
    valgrind_println_backtrace!().unwrap();
    // SAFETY: This test intentionally exercises the unchecked macro with a static format string.
    unsafe {
        valgrind_printf_backtrace_unchecked!("printf backtrace unchecked: {valid_cstring}\n");
    }
    // SAFETY: This test intentionally exercises the unchecked macro with a static format string.
    unsafe {
        valgrind_printf_backtrace_unchecked!(
            "printf backtrace unchecked (invalid): {invalid_cstring}\n"
        );
    }
    // SAFETY: This test intentionally exercises the unchecked macro with no format arguments.
    unsafe {
        valgrind_println_backtrace_unchecked!();
    }

    valgrind_println_backtrace!("println backtrace: {valid_cstring}").unwrap();
    valgrind_println_backtrace!("println backtrace (invalid): {invalid_cstring}").unwrap_err();
    valgrind_println_backtrace_unchecked!("println backtrace unchecked: {valid_cstring}");
    valgrind_println_backtrace_unchecked!(
        "println backtrace unchecked (invalid): {invalid_cstring}"
    );
    // SAFETY: This test intentionally exercises the unchecked macro with no format arguments.
    unsafe {
        valgrind_println_backtrace_unchecked!();
    }

    std::process::exit(i32::from(valgrind::running_on_valgrind() != 0));
}
