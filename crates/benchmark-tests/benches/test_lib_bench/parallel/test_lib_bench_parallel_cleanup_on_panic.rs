#![allow(unreachable_code)]

use std::env;
use std::thread::sleep;
use std::time::Duration;

use gungraun::prelude::*;
use tempfile::Builder;

const SUFFIX: &str = "test-file";
const PREFIX: &str = module_path!();

#[library_benchmark]
fn exit_with_panic() {
    // Wait for the timeout function to create the test file
    sleep(Duration::from_millis(500));

    // panics if the file does not exist since timeout hasn't run yet which fails the test due to a
    // different panic message.
    remove_test_files(true);

    // Now, we're ready to panic with the intended message and initiate the cleanup of all running
    // parallel threads/jobs
    panic!("I am panicking as requested");
}

#[library_benchmark]
fn timeout() {
    // Create the test file which is expected to exist in the `exit_with_panic` function
    let (mut file, path) = Builder::new()
        .prefix(PREFIX)
        .suffix(SUFFIX)
        .tempfile_in(std::env::temp_dir())
        .unwrap()
        .keep()
        .unwrap();

    // This is not strictly necessary but ensures we have something to print after the sleep call.
    std::fs::write(&path, "Created in timeout function").unwrap();
    sleep(Duration::from_secs(5));

    // This code is not supposed to run if all threads and jobs were interrupted correctly.
    std::io::copy(&mut file, &mut std::io::stdout()).unwrap();
}

fn remove_test_files(panic_if_not_exists: bool) {
    let dir = std::env::temp_dir();
    let pattern = format!("{}/{PREFIX}*{SUFFIX}", dir.display());

    let mut exists = false;
    for path in glob::glob(&pattern).unwrap().map(Result::unwrap) {
        std::fs::remove_file(path).unwrap();
        exists = true;
    }

    if panic_if_not_exists && !exists {
        panic!("The test file was expected to be removed but did not exist");
    }
}

library_benchmark_group!(
    name = my_group,
    setup = remove_test_files(false),
    teardown = remove_test_files(false),
    benchmarks = [exit_with_panic, timeout]
);
main!(library_benchmark_groups = my_group);
