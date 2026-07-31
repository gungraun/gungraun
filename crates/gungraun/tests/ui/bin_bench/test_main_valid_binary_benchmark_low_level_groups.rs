mod test_main_when_default_config {
    use gungraun::prelude::*;

    binary_benchmark_group!(
        name = some,
        benchmarks = |_group: &mut BinaryBenchmarkGroup| {
            // do nothing
        }
    );

    main!(
        config = BinaryBenchmarkConfig::default(),
        binary_benchmark_groups = some
    );
}

mod test_main_when_config_is_ref {
    use gungraun::prelude::*;

    binary_benchmark_group!(
        name = some,
        benchmarks = |_group: &mut BinaryBenchmarkGroup| {
            // do nothing
        }
    );

    main!(
        config = &BinaryBenchmarkConfig::default(),
        binary_benchmark_groups = some
    );
}

mod test_main_when_config_is_mut_ref {
    use gungraun::prelude::*;
    use gungraun::Cachegrind;

    binary_benchmark_group!(
        name = some,
        benchmarks = |_group: &mut BinaryBenchmarkGroup| {
            // do nothing
        }
    );

    main!(
        config = BinaryBenchmarkConfig::default().tool(Cachegrind::with_args(["--just=testing"])),
        binary_benchmark_groups = some
    );
}

mod test_main_when_no_config {
    use gungraun::prelude::*;

    binary_benchmark_group!(
        name = some,
        benchmarks = |_group: &mut BinaryBenchmarkGroup| {
            // do nothing
        }
    );

    main!(binary_benchmark_groups = some);
}

mod test_main_when_multiple_groups {
    use gungraun::prelude::*;

    binary_benchmark_group!(
        name = some,
        benchmarks = |_group: &mut BinaryBenchmarkGroup| {
            // do nothing
        }
    );

    binary_benchmark_group!(
        name = some_other,
        benchmarks = |_group: &mut BinaryBenchmarkGroup| {
            // do nothing
        }
    );

    main!(binary_benchmark_groups = [some, some_other]);
}

fn main() {}
