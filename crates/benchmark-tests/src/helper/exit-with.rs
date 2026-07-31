//! Exits with a specified exit code or panics.
//!
//! Used for testing exit code handling and error scenarios in benchmarks.
//!
//! # Arguments
//!
//! * `<code>` - Either an integer exit code, or the string `panic` to trigger a panic.

fn main() {
    let arg = std::env::args()
        .nth(1)
        .expect("At least one argument with the exit code or `panic` should be present");

    if arg == "panic" {
        panic!("Exited with panic as requested");
    } else if let Ok(code) = arg.parse::<i32>() {
        std::process::exit(code);
    } else {
        panic!("Illegal argument: {arg}");
    }
}
