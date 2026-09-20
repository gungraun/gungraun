use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{Perf, Tool};

fn setup_to_stderr(value: u64) -> u64 {
    eprintln!("setup: stderr: {value}");
    value + 20
}

fn setup_to_stdout(value: u64) -> u64 {
    println!("setup: stdout: {value}");
    value + 10
}

fn teardown_to_stderr(value: u64) {
    eprintln!("teardown: stderr: {value}");
}

fn teardown_to_stdout(value: u64) {
    println!("teardown: stdout: {value}");
}

#[library_benchmark]
#[bench::setup_stdout_teardown_stderr(
    args = (1),
    setup = setup_to_stdout,
    teardown = teardown_to_stderr
)]
#[bench::setup_stderr_teardown_stdout(
    args = (1),
    setup = setup_to_stderr,
    teardown = teardown_to_stdout
)]
fn bench(value: u64) -> u64 {
    println!("bench: stdout: {value}");
    eprintln!("bench: stderr: {value}");
    value + black_box(100)
}

#[library_benchmark]
#[bench::enabled_stdout("Events enabled", Box::new(std::io::stdout()))]
#[bench::enabled_stderr("Events enabled", Box::new(std::io::stderr()))]
#[bench::disabled_stdout("Events disabled", Box::new(std::io::stdout()))]
#[bench::disabled_stderr("Events disabled", Box::new(std::io::stderr()))]
fn print_events_line(line: &str, mut writer: Box<dyn std::io::Write>) -> std::io::Result<()> {
    writeln!(writer, "{line}")
}

library_benchmark_group!(
    name = bench_fibonacci_group,
    benchmarks = [bench, print_events_line]
);

library_benchmark_group!(
    name = perf,
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().event_set("task-clock,cpu-clock,context-switches")),
    benchmarks = [bench, print_events_line]
);

main!(library_benchmark_groups = [bench_fibonacci_group, perf]);
