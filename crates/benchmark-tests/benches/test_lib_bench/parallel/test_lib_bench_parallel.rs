#![allow(unreachable_code)]

use std::env;
use std::thread::sleep;
use std::time::Duration;

use gungraun::prelude::*;
use tempfile::Builder;

const SUFFIX: &str = "test-file";
const PREFIX: &str = module_path!();

#[library_benchmark]
fn print_parallel() {
    // Wait for the timeout function to create the test file
    sleep(Duration::from_millis(500));

    // panics if the file does not exist since timeout hasn't run yet
    remove_test_files(true);

    // Now, we're ready to print the intended message
    println!("I am running in parallel");
}

#[library_benchmark]
fn timeout() {
    // Create the test file which is expected to exist in the `print_parallel` benchmark although we
    // started this benchmark after `print_parallel`
    let (mut file, path) = Builder::new()
        .prefix(PREFIX)
        .suffix(SUFFIX)
        .tempfile_in(std::env::temp_dir())
        .unwrap()
        .keep()
        .unwrap();

    std::fs::write(&path, "Created in timeout function").unwrap();
    sleep(Duration::from_secs(1));

    // This code is not supposed to run if `print_parallel` panicked
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
    name = no_max_parallel,
    setup = remove_test_files(false),
    teardown = remove_test_files(false),
    benchmarks = [print_parallel, timeout]
);

fn max_parallel() -> usize {
    if let Ok(var) = std::env::var("__MAX_PARALLEL") {
        var.parse::<usize>()
            .expect("__MAX_PARALLEL should be a valid number")
    } else {
        panic!("__MAX_PARALLEL needs to be set with a valid value");
    }
}

library_benchmark_group!(
    name = max_parallel_group,
    max_parallel = max_parallel(),
    setup = remove_test_files(false),
    teardown = remove_test_files(false),
    benchmarks = [print_parallel, timeout]
);

main!(library_benchmark_groups = [no_max_parallel, max_parallel_group]);
