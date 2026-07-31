use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use gungraun::prelude::*;
use gungraun::{Callgrind, OutputFormat, Sandbox};

fn create_script(path: &str) {
    let script = r#"#!/usr/bin/env sh
        /bin/cat "$1"
        "#;

    std::fs::write(path, script).unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o777);
    std::fs::set_permissions(path, permissions).unwrap();

    create_file();
}

fn create_file() {
    std::fs::write("some.txt", b"content of file\n").unwrap()
}

#[binary_benchmark(
    config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)), setup = create_file()
)]
#[bench::relative(
    args = ("./cat"),
    setup = create_script,
    config = BinaryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["trace-children=yes"]))
)]
#[bench::absolute(
    args = ("/bin/cat"),
    setup = { PathBuf::from("/bin/cat").exists().then_some(0).unwrap(); create_file(); }
)]
#[bench::crate_binary(env!("CARGO_BIN_EXE_cat"))]
#[bench::use_path("cat")]
fn bench_paths(path: &str) -> Command {
    Command::new(path).arg("some.txt").build()
}

binary_benchmark_group!(name = my_group, benchmarks = bench_paths);
main!(
    config = BinaryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["trace-children=no"]))
        .output_format(OutputFormat::default().truncate_description(None)),
    binary_benchmark_groups = my_group
);
