use std::path::PathBuf;

use gungraun::prelude::*;
use gungraun::{Bench, BinaryBenchmark, OutputFormat, Sandbox, Stdio, binary_benchmark_attribute};

const ECHO: &str = env!("CARGO_BIN_EXE_echo");
const ENV: &str = env!("CARGO_BIN_EXE_env");
const FILE_EXISTS: &str = env!("CARGO_BIN_EXE_file-exists");

binary_benchmark_group!(
    name = short_group_syntax,
    benchmarks = |group| {
        group.binary_benchmark(
            BinaryBenchmark::new("some_id")
                .bench(Bench::new("bench_id").command(Command::new(ECHO))),
        );
    }
);

binary_benchmark_group!(
    name = long_group_syntax,
    benchmarks = |group: &mut BinaryBenchmarkGroup| {
        group
            // Using the same ids in a different group should be fine
            .binary_benchmark(BinaryBenchmark::new("some_id")
                .bench(Bench::new("bench_id")
                    .command(Command::new(ECHO))));
    }
);

// To be able to test the binary_benchmark_attribute! macro
#[binary_benchmark(
    config = BinaryBenchmarkConfig::default().env("BINARY_BENCHMARK_ATTRIBUTE_ENV", "3")
)]
#[bench::case_1(
    args = ("10"),
    config = BinaryBenchmarkConfig::default().env("BENCH_IN_ATTRIBUTE_ENV", "10")
)]
#[bench::case_2(
    args = ("20"),
    config = BinaryBenchmarkConfig::default().env("BENCH_IN_ATTRIBUTE_ENV", "20")
)]
fn bench_attribute(id: &str) -> gungraun::Command {
    gungraun::Command::new(ENV)
        .args([
            "--check",
            "MAIN_ENV=1",
            "GROUP_ENV=2",
            "BINARY_BENCHMARK_ATTRIBUTE_ENV=3",
            "COMMAND_IN_ATTRIBUTE_ENV=5",
        ])
        .arg(format!("BENCH_IN_ATTRIBUTE_ENV={id}"))
        .env("COMMAND_IN_ATTRIBUTE_ENV", "5")
        .build()
}

binary_benchmark_group!(
    name = check_config,
    config = BinaryBenchmarkConfig::default()
        .env("GROUP_ENV", "2")
        .output_format(OutputFormat::default().truncate_description(None)),
    benchmarks = |group| {
        group
            .binary_benchmark(
                BinaryBenchmark::new("bench_env")
                    .config(BinaryBenchmarkConfig::default().env("BINARY_BENCHMARK_ENV", "3"))
                    .bench(
                        Bench::new("with_config")
                            .config(BinaryBenchmarkConfig::default().env("BENCH_ENV", "4"))
                            .command(
                                Command::new(ENV)
                                    .arg("--is-cleared=true")
                                    .env("COMMAND_ENV", "5")
                                    .build(),
                            )
                            .command(
                                Command::new(ENV)
                                    .args([
                                        "--check",
                                        "MAIN_ENV=1",
                                        "GROUP_ENV=2",
                                        "BINARY_BENCHMARK_ENV=3",
                                        "BENCH_ENV=4",
                                        "COMMAND_ENV=5",
                                    ])
                                    .env("COMMAND_ENV", "5")
                                    .build(),
                            )
                            .command(
                                Command::new(ENV)
                                    .args([
                                        "--check",
                                        "MAIN_ENV=1",
                                        "GROUP_ENV=2",
                                        "BINARY_BENCHMARK_ENV=3",
                                        "BENCH_ENV=4",
                                        "COMMAND_ENV=6",
                                    ])
                                    .env("COMMAND_ENV", "6")
                                    .build(),
                            ),
                    ),
            )
            .binary_benchmark(binary_benchmark_attribute!(bench_attribute));
    }
);

binary_benchmark_group!(
    name = override_sandbox,
    benchmarks = |group| {
        group.binary_benchmark(
            BinaryBenchmark::new("override_sandbox")
                .config(BinaryBenchmarkConfig::default().sandbox(Sandbox::new(false)))
                .bench(
                    Bench::new("some_id")
                        .command(Command::new(FILE_EXISTS).arg("Cargo.toml").arg("true")),
                ),
        );
    }
);

binary_benchmark_group!(
    name = setup_override,
    benchmarks = |group| {
        group
            .binary_benchmark(
                BinaryBenchmark::new("first_override")
                    .setup(|| std::fs::write("foo.txt", "foo").unwrap())
                    .bench(
                        Bench::new("first_bench")
                            .setup(|| std::fs::write("aaa.txt", "aaa").unwrap())
                            .command(Command::new(FILE_EXISTS).args(["aaa.txt", "true"]))
                            .command(Command::new(FILE_EXISTS).args(["foo.txt", "false"])),
                    )
                    .bench(
                        Bench::new("second_bench")
                            .setup(|| std::fs::write("bbb.txt", "bbb").unwrap())
                            .command(Command::new(FILE_EXISTS).args(["bbb.txt", "true"]))
                            .command(Command::new(FILE_EXISTS).args(["foo.txt", "false"])),
                    ),
            )
            .binary_benchmark(
                BinaryBenchmark::new("second_override")
                    .setup(|| std::fs::write("bar.txt", "bar").unwrap())
                    .bench(
                        Bench::new("third_bench")
                            .setup(|| std::fs::write("ccc.txt", "ccc").unwrap())
                            .command(Command::new(FILE_EXISTS).args(["ccc.txt", "true"]))
                            .command(Command::new(FILE_EXISTS).args(["bar.txt", "false"])),
                    ),
            )
    }
);

binary_benchmark_group!(
    name = teardown_override,
    benchmarks = |group| {
        group
            .binary_benchmark(
                BinaryBenchmark::new("first_override")
                    .teardown(|| {
                        std::process::Command::new(FILE_EXISTS)
                            .arg("foo.txt")
                            .arg("true")
                            .status()
                            .unwrap();
                    })
                    .bench(
                        Bench::new("with_teardown_override_two_commands")
                            .teardown(|| {
                                std::process::Command::new(FILE_EXISTS)
                                    .arg("aaa.txt")
                                    .arg("true")
                                    .status()
                                    .unwrap();
                                std::process::Command::new(FILE_EXISTS)
                                    .arg("foo.txt")
                                    .arg("false")
                                    .status()
                                    .unwrap();
                            })
                            .command(
                                Command::new(ECHO)
                                    .stdout(Stdio::File(PathBuf::from("aaa.txt")))
                                    .arg("aaa")
                                    .build(),
                            )
                            .command(
                                Command::new(ECHO)
                                    .stdout(Stdio::File(PathBuf::from("aaa.txt")))
                                    .arg("aaa")
                                    .build(),
                            ),
                    )
                    .bench(
                        Bench::new("without_teardown_override").command(
                            Command::new(ECHO)
                                .stdout(Stdio::File(PathBuf::from("foo.txt")))
                                .arg("foo")
                                .build(),
                        ),
                    )
                    .bench(
                        Bench::new("with_teardown_override_single_command")
                            .teardown(|| {
                                std::process::Command::new(FILE_EXISTS)
                                    .arg("bbb.txt")
                                    .arg("true")
                                    .status()
                                    .unwrap();
                                std::process::Command::new(FILE_EXISTS)
                                    .arg("foo.txt")
                                    .arg("false")
                                    .status()
                                    .unwrap();
                            })
                            .command(
                                Command::new(ECHO)
                                    .stdout(Stdio::File(PathBuf::from("bbb.txt")))
                                    .arg("bbb")
                                    .build(),
                            ),
                    ),
            )
            .binary_benchmark(
                BinaryBenchmark::new("second_override")
                    .teardown(|| {
                        std::process::Command::new(FILE_EXISTS)
                            .arg("bar.txt")
                            .arg("true")
                            .status()
                            .unwrap();
                    })
                    .bench(
                        Bench::new("with_teardown_override")
                            .teardown(|| {
                                std::process::Command::new(FILE_EXISTS)
                                    .arg("bbb.txt")
                                    .arg("true")
                                    .status()
                                    .unwrap();
                                std::process::Command::new(FILE_EXISTS)
                                    .arg("bar.txt")
                                    .arg("false")
                                    .status()
                                    .unwrap();
                            })
                            .command(
                                Command::new(ECHO)
                                    .stdout(Stdio::File(PathBuf::from("bbb.txt")))
                                    .arg("bbb")
                                    .build(),
                            ),
                    )
                    .bench(
                        Bench::new("without_teardown_override").command(
                            Command::new(ECHO)
                                .stdout(Stdio::File(PathBuf::from("bar.txt")))
                                .arg("bar")
                                .build(),
                        ),
                    ),
            )
    }
);

main!(
    config = BinaryBenchmarkConfig::default()
        .env("MAIN_ENV", "1")
        .sandbox(Sandbox::new(true)),
    binary_benchmark_groups = [
        short_group_syntax,
        long_group_syntax,
        check_config,
        override_sandbox,
        setup_override,
        teardown_override
    ]
);
