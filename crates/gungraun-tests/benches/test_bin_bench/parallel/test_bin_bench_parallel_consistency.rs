use benchmark_tests::fibonacci;
use gungraun::prelude::*;

fn setup_no_output() {
    fibonacci(5);
}

fn teardown_no_output() {
    fibonacci(5);
}

fn setup_with_output(tag: &str) {
    println!("SETUP in {tag}");
}

fn teardown_with_output(tag: &str) {
    println!("TEARDOWN in {tag}");
}

#[binary_benchmark]
#[bench::setup_and_teardown_without_output(
    setup = setup_no_output(),
    teardown = teardown_no_output()
)]
#[bench::setup_and_teardown_with_output(
    setup = setup_with_output("bench"),
    teardown = teardown_with_output("bench")
)]
fn with_output_in_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_echo")).arg("FOO").build()
}

#[binary_benchmark]
fn subprocess() -> Command {
    Command::new(env!("CARGO_BIN_EXE_subprocess"))
        .args([env!("CARGO_BIN_EXE_echo"), "BAR"])
        .build()
}

binary_benchmark_group!(
    name = my_group,
    benchmarks = [with_output_in_command, subprocess]
);

binary_benchmark_group!(
    name = group_assistants,
    setup = setup_with_output("group"),
    teardown = teardown_with_output("group"),
    benchmarks = with_output_in_command
);

main!(binary_benchmark_groups = [my_group, group_assistants]);
