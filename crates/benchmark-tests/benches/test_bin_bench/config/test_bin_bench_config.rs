use std::process::Command;

use gungraun::Dhat;
use gungraun::prelude::*;

fn check_args() -> Vec<String> {
    let mut default_args = vec![
        "--check".to_owned(),
        "MAIN_ENV=1".to_owned(),
        "GROUP_ENV=2".to_owned(),
        "BINARY_BENCHMARK_ENV=3".to_owned(),
        "BENCH_ENV=4".to_owned(),
        "COMMAND_ENV=5".to_owned(),
    ];
    if let Ok(value) = std::env::var("CLI_ENV_TEST") {
        if value == "true" {
            default_args.push("CLI_ENV=0".to_string());
        }
    }

    default_args
}

fn is_cleared_args(is_cleared: bool) -> Vec<String> {
    if let Ok(value) = std::env::var("CLI_ENV_CLEAR_TEST_VALUE") {
        vec![format!("--is-cleared={value}")]
    } else if is_cleared {
        vec![format!("--is-cleared=true")]
    } else {
        vec![format!("--is-cleared=false")]
    }
}

#[binary_benchmark(config = BinaryBenchmarkConfig::default().env("BINARY_BENCHMARK_ENV", "3"))]
#[bench::with_env(config = BinaryBenchmarkConfig::default().env("BENCH_ENV", "4"))]
fn bench_binary() -> gungraun::Command {
    gungraun::Command::new(env!("CARGO_BIN_EXE_env"))
        .args(check_args())
        .env("COMMAND_ENV", "5")
        .build()
}

fn check_setup_is_not_cleared() {
    println!("SETUP:");
    Command::new(env!("CARGO_BIN_EXE_env"))
        .args(["--is-cleared=false"])
        .status()
        .unwrap();
    Command::new(env!("CARGO_BIN_EXE_env"))
        .args(check_args())
        .status()
        .unwrap();
}

fn check_teardown_is_not_cleared() {
    println!("TEARDOWN:");
    Command::new(env!("CARGO_BIN_EXE_env"))
        .args(["--is-cleared=false"])
        .status()
        .unwrap();
    Command::new(env!("CARGO_BIN_EXE_env"))
        .args(check_args())
        .status()
        .unwrap();
}

#[binary_benchmark(config = BinaryBenchmarkConfig::default().env("BINARY_BENCHMARK_ENV", "3"))]
#[bench::with_env(
    setup = check_setup_is_not_cleared,
    teardown = check_teardown_is_not_cleared,
    config = BinaryBenchmarkConfig::default()
        .env("BENCH_ENV", "4")
        .tool(Dhat::default())
)]
fn check_env_is_cleared() -> gungraun::Command {
    gungraun::Command::new(env!("CARGO_BIN_EXE_env"))
        .args(is_cleared_args(true))
        .env("COMMAND_ENV", "5")
        .build()
}

binary_benchmark_group!(
    name = my_group,
    config = BinaryBenchmarkConfig::default().env("GROUP_ENV", "2"),
    benchmarks = [bench_binary, check_env_is_cleared]
);

main!(
    config = BinaryBenchmarkConfig::default().env("MAIN_ENV", "1"),
    binary_benchmark_groups = my_group
);
