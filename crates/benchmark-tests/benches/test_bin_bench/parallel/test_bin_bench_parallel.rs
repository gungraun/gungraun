use std::path::PathBuf;
use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{Delay, DelayKind, Stdio};

const SUFFIX: &str = "test-file";
const PREFIX: &str = module_path!();

fn test_file() -> PathBuf {
    std::env::temp_dir().join(format!("{PREFIX}.{SUFFIX}"))
}

#[binary_benchmark]
fn file_exists() -> Command {
    Command::new(env!("CARGO_BIN_EXE_file-exists"))
        .delay(Delay::new(DelayKind::DurationElapse(
            Duration::from_millis(500),
        )))
        .arg(test_file())
        .arg("true")
        .build()
}

#[binary_benchmark]
fn create_file() -> Command {
    Command::new(env!("CARGO_BIN_EXE_echo"))
        .arg("FOO")
        .stdout(Stdio::File(test_file()))
        .build()
}

fn remove_test_file(panic_if_not_exists: bool) {
    let test_file = test_file();
    let exists = test_file.exists();

    if exists {
        std::fs::remove_file(test_file).unwrap();
    }

    if panic_if_not_exists && !exists {
        panic!("The test file was expected to be removed but did not exist");
    }
}

fn max_parallel() -> usize {
    if let Ok(var) = std::env::var("__MAX_PARALLEL") {
        var.parse::<usize>()
            .expect("__MAX_PARALLEL should be a valid number")
    } else {
        panic!("__MAX_PARALLEL needs to be set with a valid value");
    }
}

// The point here is to start `file_exists` first and then `create_file`
binary_benchmark_group!(
    name = no_max_parallel,
    setup = remove_test_file(false),
    teardown = remove_test_file(false),
    benchmarks = [file_exists, create_file]
);

binary_benchmark_group!(
    name = max_parallel_group,
    max_parallel = max_parallel(),
    setup = remove_test_file(false),
    teardown = remove_test_file(false),
    benchmarks = [file_exists, create_file]
);

main!(binary_benchmark_groups = [no_max_parallel, max_parallel_group]);
