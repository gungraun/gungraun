use gungraun::prelude::*;
use gungraun::{Bench, BinaryBenchmark};

#[binary_benchmark]
fn bench() -> Command {
    Command::new("bench")
}

#[binary_benchmark]
fn additional() -> Command {
    Command::new("additional")
}

fn some_setup() {
    println!("Setup")
}

fn some_teardown() {
    println!("Teardown")
}

mod test_group_minimal {
    use super::*;

    binary_benchmark_group!(name = single_without_brackets, benchmarks = bench);
    binary_benchmark_group!(name = single_with_brackets, benchmarks = [bench]);
    binary_benchmark_group!(name = multiple, benchmarks = [bench, additional]);
}

mod test_group_with_config {
    use super::*;

    binary_benchmark_group!(
        name = just_a_name,
        config = BinaryBenchmarkConfig::default(),
        benchmarks = [bench]
    );
}

mod test_group_with_all {
    use super::*;

    binary_benchmark_group!(
        name = just_a_name,
        config = BinaryBenchmarkConfig::default(),
        compare_by_id = true,
        setup = some_setup(),
        teardown = some_teardown(),
        benchmarks = [bench]
    );
}

mod test_group_with_body {
    use super::*;

    binary_benchmark_group!(
        name = just_a_name,
        benchmarks = |group| {
            group.binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(Bench::new("foo").command(Command::new("echo"))),
            )
        }
    );
}

mod test_group_with_config_and_body {
    use super::*;

    binary_benchmark_group!(
        name = just_a_name,
        config = BinaryBenchmarkConfig::default(),
        benchmarks = |group| {
            group.binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(Bench::new("foo").command(Command::new("echo"))),
            )
        }
    );
}

mod test_group_with_all_and_body {
    use super::*;

    binary_benchmark_group!(
        name = just_a_name,
        config = BinaryBenchmarkConfig::default(),
        compare_by_id = true,
        setup = some_setup(),
        teardown = some_teardown(),
        benchmarks = |group| {
            group.binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(Bench::new("foo").command(Command::new("echo"))),
            )
        }
    );
}

mod test_main_single_without_brackets {
    use super::*;

    binary_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(binary_benchmark_groups = just_a_name);
}

mod test_main_single_with_brackets {
    use super::*;

    binary_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(binary_benchmark_groups = [just_a_name]);
}

mod test_main_multiple {
    use super::*;

    binary_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    binary_benchmark_group!(name = another_one, benchmarks = [bench]);
    main!(binary_benchmark_groups = [just_a_name, another_one]);
}

mod test_main_with_config {
    use super::*;

    binary_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(
        config = BinaryBenchmarkConfig::default(),
        binary_benchmark_groups = [just_a_name]
    );
}

mod test_main_with_all {
    use super::*;

    binary_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(
        config = BinaryBenchmarkConfig::default(),
        setup = some_setup(),
        teardown = some_teardown(),
        binary_benchmark_groups = [just_a_name]
    );
}

fn main() {}
