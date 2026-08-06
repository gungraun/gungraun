//! Runs a subprocess with the given arguments.
//!
//! Executes the specified binary and waits for it to complete.
//!
//! # Arguments
//!
//! * `<executable>` - Path to the executable to run.
//! * `[args...]` - Arguments to pass to the executable.

use std::process::Command;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let exe = args.next().expect("A subprocess path should be present");

    Command::new(exe)
        .args(args)
        .status()
        .expect("Running the subprocess should succeed");
}
