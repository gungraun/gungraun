use gungraun::prelude::*;
use gungraun::{self, Dhat, Pipe, Sandbox, Stdin, Stdio};

const ECHO: &str = env!("CARGO_BIN_EXE_echo");
const PIPE: &str = env!("CARGO_BIN_EXE_pipe");

fn setup() {
    print!("Something");
}

fn create_files(stdout: &str, stderr: &str) {
    println!("create file for stdout: {stdout}");
    std::fs::File::create(stdout).unwrap();
    println!("create file for stderr: {stderr}");
    std::fs::File::create(stderr).unwrap();
}

fn check_files(stdout: &str, stderr: &str) {
    assert_eq!(std::fs::read_to_string(stdout).unwrap(), "1 2\n");
    assert!(std::fs::read_to_string(stderr).unwrap().is_empty());
}

#[binary_benchmark]
#[bench::both_inherit(Stdio::Inherit, Stdio::Inherit)]
#[bench::both_null(Stdio::Null, Stdio::Null)]
#[bench::both_piped(Stdio::Pipe, Stdio::Pipe)]
#[bench::both_file_when_exists(
    args = [Stdio::File("file.stdout".into()), Stdio::File("file.stderr".into())],
    setup = create_files("file.stdout", "file.stderr"),
    teardown = check_files("file.stdout", "file.stderr"),
    config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true))
)]
#[bench::both_file_when_not_exists(
    args = [Stdio::File("file.stdout".into()), Stdio::File("file.stderr".into())],
    teardown = check_files("file.stdout", "file.stderr"),
    config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true))
)]
fn bench_echo(stdout: Stdio, stderr: Stdio) -> gungraun::Command {
    gungraun::Command::new(ECHO)
        .args(["1", "2"])
        .stdout(stdout)
        .stderr(stderr)
        .build()
}

#[binary_benchmark(
    setup = setup(),
    config = BinaryBenchmarkConfig::default()
        .tool(Dhat::default())
)]
fn bench_pipe() -> gungraun::Command {
    gungraun::Command::new(PIPE)
        .stdin(Stdin::Setup(Pipe::Stdout))
        .build()
}

binary_benchmark_group!(name = simple, benchmarks = [bench_echo, bench_pipe]);
main!(binary_benchmark_groups = simple);
