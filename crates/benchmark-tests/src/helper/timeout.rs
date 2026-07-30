//! Sleeps for a specified duration then exits normally.
//!
//! Used for testing timeout handling and process termination in benchmarks printing a message after
//! the timeout duration.
//!
//! # Arguments
//!
//! * `[timeout_ms]` - Number of milliseconds to sleep (default: 20000).

use std::io::Error;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Error> {
    println!("Started the timeout program");

    let timeout = std::env::args()
        .nth(1)
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(20000);

    sleep(Duration::from_millis(timeout));

    println!("I terminated normally after a timeout of {timeout} ms");
    Ok(())
}
