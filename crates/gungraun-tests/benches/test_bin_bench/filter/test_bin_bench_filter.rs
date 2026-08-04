use gungraun::prelude::*;

const ECHO: &str = env!("CARGO_BIN_EXE_echo");

#[binary_benchmark]
#[bench::forty_two(42)]
#[benches::down(42, 31)]
fn benches_with_id(input: u64) -> Command {
    Command::new(ECHO).arg(input.to_string()).build()
}

#[binary_benchmark]
fn minimal_bench() -> Command {
    Command::new(ECHO).arg("minimal").build()
}

binary_benchmark_group!(name = group_1, benchmarks = minimal_bench);

binary_benchmark_group!(
    name = group_2,
    benchmarks = [minimal_bench, benches_with_id]
);

main!(binary_benchmark_groups = [group_1, group_2]);
