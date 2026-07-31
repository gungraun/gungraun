use gungraun::prelude::*;

#[binary_benchmark]
fn echo_bench() -> gungraun::Command {
    gungraun::Command::new(env!("CARGO_BIN_EXE_echo"))
}

#[binary_benchmark]
fn read_file_bench() -> gungraun::Command {
    gungraun::Command::new(env!("CARGO_BIN_EXE_read-file"))
}

binary_benchmark_group!(name = group_1, benchmarks = echo_bench);
binary_benchmark_group!(name = group_2, benchmarks = [read_file_bench, echo_bench]);

main!(binary_benchmark_groups = [group_1, group_2]);
