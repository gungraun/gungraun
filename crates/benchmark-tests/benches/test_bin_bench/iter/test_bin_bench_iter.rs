use std::ffi::OsStr;

use gungraun::prelude::*;
use gungraun::{Bench, BinaryBenchmark, OutputFormat, Sandbox, binary_benchmark_attribute};

const ECHO: &str = env!("CARGO_BIN_EXE_echo");
const FILE_EXISTS: &str = env!("CARGO_BIN_EXE_file-exists");

fn create_file(path: &str) {
    println!("Creating file '{path}'");
    std::fs::write(path, []).unwrap();
}

fn create_files(paths: &[&str]) {
    println!("Creating files '{}'", paths.join("', '"));
    for path in paths {
        create_file(path);
    }
}

fn remove_file(path: &str) {
    println!("Removing '{path}'");
    std::fs::remove_file(path).unwrap();
}

fn print_files() {
    for entry in std::fs::read_dir(".")
        .unwrap()
        // Filter profraw files which might be created during benchmark coverage runs
        .filter(|p| {
            p.as_ref().map_or(true, |e| {
                e.path()
                    .extension()
                    .is_none_or(|e| e != OsStr::new("profraw"))
            })
        })
        .collect::<Result<Vec<_>, std::io::Error>>()
        .unwrap()
    {
        println!("{}", entry.path().display())
    }
}

#[binary_benchmark]
#[benches::one(iter = vec![(1, 2)])]
#[benches::two(iter = vec![(1, 2), (3, 4)])]
fn bench_tuple((first, second): (u64, u64)) -> gungraun::Command {
    gungraun::Command::new(ECHO)
        .arg(first.to_string())
        .arg(second.to_string())
        .build()
}

#[binary_benchmark(
    config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true))
)]
#[benches::with_setup_path(iter = ["one.txt", "two.txt"], setup = create_file)]
#[benches::with_setup(
    iter = ["one.txt", "two.txt"],
    setup = create_files(&["one.txt", "two.txt"])
)]
#[benches::with_setup_path_and_teardown(
    iter = ["one.txt", "two.txt"],
    setup = create_file,
    teardown = print_files()
)]
#[benches::with_setup_and_teardown_both_path(
    iter = ["one.txt", "two.txt"],
    setup = create_file,
    teardown = remove_file
)]
#[benches::with_setup_and_teardown_path(
    iter = ["one.txt", "two.txt"],
    setup = create_files(&["one.txt", "two.txt"]),
    teardown = remove_file
)]
fn bench_assists(path: &str) -> gungraun::Command {
    gungraun::Command::new(FILE_EXISTS)
        .arg(path)
        .arg("true")
        .build()
}

binary_benchmark_group!(name = high_level, benchmarks = [bench_tuple, bench_assists]);

binary_benchmark_group!(
    name = low_level,
    benchmarks = |group: &mut BinaryBenchmarkGroup| {
        group
            .binary_benchmark(binary_benchmark_attribute!(bench_assists))
            .binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(Bench::new("foo").command(gungraun::Command::new(ECHO).arg("foo"))),
            )
            .binary_benchmark(
                BinaryBenchmark::new("low_level_other")
                    .bench(Bench::new("bar").command(gungraun::Command::new(ECHO).arg("bar"))),
            )
    }
);

main!(
    config = BinaryBenchmarkConfig::default()
        .output_format(OutputFormat::default().truncate_description(None)),
    binary_benchmark_groups = [high_level, low_level]
);
