mod test_when_binary_benchmark_group_is_not_a_group {
    use gungraun::prelude::*;

    fn some_func() {}

    main!(binary_benchmark_groups = some_func);
}

mod test_when_config_is_not_a_binary_benchmark_config {
    use gungraun::prelude::*;

    binary_benchmark_group!(
        name = some,
        benchmarks = |group: &mut BinaryBenchmarkGroup| {
            // do nothing
        }
    );

    main!(config = "some string", binary_benchmark_groups = some);
}

mod test_when_no_group {
    use gungraun::prelude::*;
    main!(binary_benchmark_groups = );
}

mod test_when_invalid_syntax {
    use gungraun::prelude::*;
    main!(something);
}

fn main() {}
