use std::thread::sleep;
use std::time::Duration;

use gungraun::{binary_benchmark, binary_benchmark_group, main, Pipe, Stdin};

const PIPE: &str = env!("CARGO_BIN_EXE_pipe");

fn setup_child() {
    sleep(Duration::from_millis(500));
    print!("SETUP CHILD PROCESS");
}

#[binary_benchmark(setup = setup_child())]
fn benchmark() -> gungraun::Command {
    gungraun::Command::new(PIPE)
        .stdin(Stdin::Setup(Pipe::Stdout))
        .build()
}

binary_benchmark_group!(
    name = group;
    benchmarks = benchmark
);

main!(binary_benchmark_groups = group);
