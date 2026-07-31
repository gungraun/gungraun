//! A fake benchmark binary that echoes all command-line arguments.
//!
//! Used for testing argument passing in binary benchmarks. Each argument is printed on its own
//! line, including the binary name as the first argument.
//!
//! # Arguments
//!
//! Accepts any arguments, all of which are printed to stdout.

fn main() {
    for arg in std::env::args() {
        println!("{arg}");
    }
}
