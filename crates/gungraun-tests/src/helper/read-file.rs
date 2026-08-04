//! Reads a file and verifies its content matches expected value.
//!
//! Asserts that the file's content exactly equals the provided expected string.
//!
//! # Arguments
//!
//! * `<file>` - Path to the file to read.
//! * `<expected>` - Expected content that the file should contain.

fn main() {
    let mut args = std::env::args_os().skip(1);
    let file = args.next().unwrap();
    let expected = args.next().unwrap();

    let actual = std::fs::read_to_string(file).unwrap();

    assert_eq!(actual, expected.to_string_lossy());
}
