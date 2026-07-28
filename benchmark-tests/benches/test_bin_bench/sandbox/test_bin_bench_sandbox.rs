use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{OutputFormat, Perf, PerfRunMode, Sandbox, Stdio, Tool};

const FILE_EXISTS: &str = env!("CARGO_BIN_EXE_file-exists");

fn check_file_exists(path: &str, should_exist: bool) {
    if should_exist {
        assert!(PathBuf::from(path).is_file());
        println!("File exists: '{path}'")
    } else {
        assert!(!PathBuf::from(path).exists());
        println!("File does not exist: '{path}'")
    }
}

#[binary_benchmark]
#[bench::sandbox_with_fixture(
    args = ("one_line.fix", true),
    setup = check_file_exists,
    teardown = check_file_exists,
    config = BinaryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true)
            .fixtures(["benchmark-tests/benches/fixtures/one_line.fix"])
        )
)]
#[bench::sandbox_without_fixture(
    args = ("one_line.fix", false),
    setup = check_file_exists,
    teardown = check_file_exists,
    config = BinaryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true))
)]
fn with_sandbox(path: &str, exists: bool) -> gungraun::Command {
    gungraun::Command::new(FILE_EXISTS)
        .arg(path)
        .arg(exists.to_string())
        .build()
}

#[binary_benchmark()]
#[bench::check_file(
    args = ("benches/fixtures/one_line.fix", true),
    config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(false)),
    setup = check_file_exists,
    teardown = check_file_exists
)]
fn without_sandbox(path: &str, should_exist: bool) -> gungraun::Command {
    gungraun::Command::new(FILE_EXISTS)
        .arg(path)
        .arg(should_exist.to_string())
        .build()
}

fn setup_directory_and_file() {
    std::fs::create_dir("foo").unwrap();
    std::fs::write("foo/bar.txt", "bar").unwrap();
    println!("Created directory 'foo' with file 'bar.txt'");
}

fn teardown_directory_and_file() {
    std::fs::remove_file("foo/bar.txt").unwrap();
    // profraw files can appear during benchmark coverage runs
    for entry in std::fs::read_dir("foo").unwrap().filter(|e| {
        e.as_ref().map_or(true, |e| {
            let path = e.path();
            path.is_file() && path.extension().is_none_or(|p| p == OsStr::new("profraw"))
        })
    }) {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir("foo").unwrap();
    println!("Deleted directory 'foo' with file 'bar.txt'");
}

#[binary_benchmark(setup = setup_directory_and_file(), teardown = teardown_directory_and_file)]
fn with_current_dir() -> gungraun::Command {
    gungraun::Command::new(FILE_EXISTS)
        .current_dir("foo")
        .arg("bar.txt")
        .arg("true")
        .build()
}

// This also verifies perf runs setup only once in sampling or calibration mode, the setup creates a
// directory and if it would be executed repeatedly the directory creation would fail with an error.
#[binary_benchmark(setup = setup_directory_and_file())]
fn perf() -> gungraun::Command {
    gungraun::Command::new(FILE_EXISTS)
        .current_dir("foo")
        .arg("bar.txt")
        .arg("true")
        // The binary prints 1 line per sample and clutters the output. We only need it to exit
        // successfully
        .stdout(Stdio::Null)
        .build()
}

binary_benchmark_group!(
    name = perf_direct,
    config = BinaryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().event_set("task-clock,cpu-clock,faults,context-switches")),
    benchmarks = [with_sandbox, without_sandbox, with_current_dir]
);

binary_benchmark_group!(
    name = perf_sampling_and_calibration,
    config = BinaryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(
            Perf::default()
                .sample_duration(Duration::from_secs(2))
                .run_mode(PerfRunMode::Calibrate(Duration::from_secs(2)))
                // This is the usual event set without faults because it produces flaky terminal
                // output in the second comparison run
                .event_set(
                    "task-clock,cpu-clock,context-switches"
                )
        ),
    benchmarks = perf
);

binary_benchmark_group!(
    name = valgrind_tool,
    benchmarks = [with_sandbox, without_sandbox, with_current_dir]
);

main!(
    config = BinaryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true))
        .output_format(OutputFormat::default().truncate_description(None)),
    binary_benchmark_groups = [valgrind_tool, perf_direct, perf_sampling_and_calibration]
);
