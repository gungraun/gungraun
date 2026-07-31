use gungraun::prelude::*;

#[binary_benchmark]
#[bench::one("1")]
#[bench::two("2")]
fn threads(num_threads: &str) -> Command {
    Command::new(env!("CARGO_BIN_EXE_thread"))
        .arg(num_threads)
        .build()
}

binary_benchmark_group!(name = my_group, benchmarks = threads);
main!(binary_benchmark_groups = my_group);
