use gungraun::prelude::*;
use gungraun::{Bench, BinaryBenchmark, binary_benchmark_attribute};

const ECHO: &str = env!("CARGO_BIN_EXE_echo");

#[binary_benchmark]
fn benchmark_without_id() -> gungraun::Command {
    gungraun::Command::new(ECHO)
}

#[binary_benchmark]
fn benchmark_without_id_other() -> gungraun::Command {
    gungraun::Command::new(ECHO)
}

// The comparison should not happen since there is no id
binary_benchmark_group!(
    name = compare_without_id,
    compare_by_id = true,
    benchmarks = [benchmark_without_id, benchmark_without_id_other]
);

#[binary_benchmark]
#[bench::foo()]
fn benchmark_with_id() -> gungraun::Command {
    gungraun::Command::new(ECHO)
}

#[binary_benchmark]
#[bench::foo()]
fn benchmark_with_id_other() -> gungraun::Command {
    gungraun::Command::new(ECHO)
}

binary_benchmark_group!(
    name = different_benchmarks,
    compare_by_id = true,
    benchmarks = [benchmark_with_id, benchmark_with_id_other]
);

#[binary_benchmark]
#[bench::foo("foo")]
#[bench::bar("bar")]
fn benchmark_two_benches(arg: &str) -> gungraun::Command {
    gungraun::Command::new(ECHO).arg(arg).build()
}

binary_benchmark_group!(
    name = not_all_ids_present_in_first,
    compare_by_id = true,
    benchmarks = [benchmark_two_benches, benchmark_with_id]
);

binary_benchmark_group!(
    name = not_all_ids_present_in_second,
    compare_by_id = true,
    benchmarks = [benchmark_with_id, benchmark_two_benches]
);

binary_benchmark_group!(
    name = compare_no_id_with_id,
    compare_by_id = true,
    benchmarks = [benchmark_without_id, benchmark_with_id]
);

binary_benchmark_group!(
    name = compare_low_level,
    compare_by_id = true,
    benchmarks = |group: &mut BinaryBenchmarkGroup| {
        group
            .binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(Bench::new("foo").command(gungraun::Command::new(ECHO))),
            )
            .binary_benchmark(
                BinaryBenchmark::new("low_level_other")
                    .bench(Bench::new("foo").command(gungraun::Command::new(ECHO).arg("foo"))),
            )
    }
);

binary_benchmark_group!(
    name = compare_low_level_multiple,
    compare_by_id = true,
    benchmarks = |group: &mut BinaryBenchmarkGroup| {
        group
            .binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(Bench::new("foo").command(gungraun::Command::new(ECHO).arg("foo")))
                    .bench(Bench::new("bar").command(gungraun::Command::new(ECHO).arg("bar"))),
            )
            .binary_benchmark(
                BinaryBenchmark::new("low_level_other")
                    .bench(Bench::new("foo").command(gungraun::Command::new(ECHO)))
                    .bench(Bench::new("bar").command(gungraun::Command::new(ECHO))),
            )
    }
);

binary_benchmark_group!(
    name = compare_low_level_with_attribute,
    compare_by_id = true,
    benchmarks = |group: &mut BinaryBenchmarkGroup| {
        group
            .binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(Bench::new("foo").command(gungraun::Command::new(ECHO))),
            )
            .binary_benchmark(binary_benchmark_attribute!(benchmark_with_id))
    }
);

main!(
    binary_benchmark_groups = [
        compare_without_id,
        different_benchmarks,
        not_all_ids_present_in_first,
        not_all_ids_present_in_second,
        compare_no_id_with_id,
        compare_low_level,
        compare_low_level_with_attribute,
        compare_low_level_multiple
    ]
);
