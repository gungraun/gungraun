//! Verifies whether a file exists or not.
//!
//! Asserts that a file's existence matches the expected state.
//!
//! # Arguments
//!
//! * `<path>` - Path to the file to check.
//! * `<true|false>` - Expected existence: `true` if file should exist, `false` if it should not.

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(args.next().unwrap());
    let true_or_false = args.next().unwrap();

    let expected_exists = match true_or_false.to_string_lossy().to_string().as_str() {
        "true" => true,
        "false" => false,
        value => panic!("Unexpected value: '{value}'"),
    };

    match (path.exists(), expected_exists) {
        (true, true) => {
            println!("Verifying that file exists succeeded");
        }
        (true, false) => {
            panic!("Expected file to not exist but it exists");
        }
        (false, true) => {
            panic!("Expected file to exist but it did not exist");
        }
        (false, false) => {
            println!("Verifying that file doesn't exist succeeded");
        }
    }
}
