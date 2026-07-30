use std::hint::black_box;
use std::path::Path;
use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{Perf, Sandbox, Tool};

fn check_file_exists(path: &str, should_exist: bool) {
    if should_exist {
        assert!(Path::new(path).is_file());
    } else {
        assert!(!Path::new(path).exists());
    }
}

fn create_dir(path: &str) -> &str {
    std::fs::create_dir(path).unwrap();
    path
}

fn remove_dir(dir: String) {
    std::fs::remove_dir(&dir).unwrap();
}

#[library_benchmark]
#[bench::when_true_with_fixture(
    args = ("one_line.fix", true),
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true)
            .fixtures(["crates/benchmark-tests/benches/fixtures/one_line.fix"])
        )
)]
#[bench::when_true_without_fixture(
    args = ("one_line.fix", false),
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true))
)]
#[bench::when_false_with_fixture(
    args = ("one_line.fix", false),
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(false)
            // Specifying fixtures should do nothing
            .fixtures(["crates/benchmark-tests/benches/fixtures/one_line.fix"])
        )
)]
#[bench::when_false_without_fixture(
    args = ("benches/fixtures/one_line.fix", true),
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(false))
)]
fn sandbox(path: &str, should_exist: bool) {
    check_file_exists(black_box(path), black_box(should_exist));
}

#[library_benchmark]
#[bench::with_sandbox(
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true)
            .fixtures(["crates/benchmark-tests/benches/fixtures/foo"])
        )
        .current_dir("foo")
)]
#[bench::without_sandbox(
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(false))
        .current_dir("benches/fixtures/foo")
)]
fn current_dir() {
    check_file_exists(black_box("bar.txt"), black_box(true));
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true)
            .fixtures(["crates/benchmark-tests/benches/fixtures/foo"])
        )
        .current_dir("foo"),
    setup = create_dir,
    teardown = remove_dir
)]
#[bench::sampling(
    args = ["sampling_dir"],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .sample_duration(Duration::from_secs(2))
        )
)]
#[bench::calibration(
    args = ["calibration_dir"],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .run_mode(gungraun::PerfRunMode::DefaultCalibrate)
        )
)]
#[bench::sampling_and_calibration(
    args = ["sampling_and_calibration_dir"],
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default()
            .sample_duration(Duration::from_secs(2))
            .run_mode(gungraun::PerfRunMode::DefaultCalibrate)
        )
)]
fn bench_perf(control_dir: &str) -> String {
    // Repeat to increase the benchmark metrics enough for the calibration run without sampling to
    // have a verifiable terminal output.
    for _ in 0..100 {
        check_file_exists(black_box("bar.txt"), black_box(true));
    }
    control_dir.to_owned()
}

library_benchmark_group!(name = valgrind, benchmarks = [sandbox, current_dir]);
library_benchmark_group!(
    name = perf,
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().event_set("task-clock,cpu-clock,context-switches")),
    benchmarks = [sandbox, current_dir, bench_perf]
);

main!(
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    library_benchmark_groups = [valgrind, perf]
);
