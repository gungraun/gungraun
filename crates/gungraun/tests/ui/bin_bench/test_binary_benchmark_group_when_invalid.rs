mod test_when_empty {
    use gungraun::prelude::*;

    binary_benchmark_group!();
}

mod test_when_no_name {
    use gungraun::prelude::*;

    #[binary_benchmark]
    fn bench() -> Command {
        Command::new("foo")
    }

    binary_benchmark_group!(
        benchmarks =
    );

    binary_benchmark_group!(benchmarks = bench);

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        max_parallel = 0,
        setup = setup(),
        teardown = teardown(),
        benchmarks =
    );

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        max_parallel = 0,
        setup = setup(),
        teardown = teardown(),
        benchmarks = bench
    );

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        max_parallel = 0,
        setup = setup();
        teardown = teardown();
        benchmarks =
    );

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        max_parallel = 0,
        setup = setup();
        teardown = teardown();
        benchmarks = bench
    );

    binary_benchmark_group!(benchmarks = [bench]);
    binary_benchmark_group!(benchmarks = bench, bench);
    binary_benchmark_group!(benchmarks = [bench, bench]);
}

mod test_low_level_when_no_benchmark {
    use gungraun::prelude::*;

    // comma syntax
    binary_benchmark_group!(
        name = some,
        benchmarks =
    );

    binary_benchmark_group!(
        name = some,
        benchmarks = |group|
    );

    binary_benchmark_group!(
        name = some,
        benchmarks = |group: &mut BinaryBenchmarkGroup|
    );

    // semicolon syntax
    binary_benchmark_group!(
        name = some;
        benchmarks =
    );

    binary_benchmark_group!(
        name = some;
        benchmarks = |group|
    );

    binary_benchmark_group!(
        name = some;
        benchmarks = |group: &mut BinaryBenchmarkGroup|
    );
}

mod test_when_no_benchmark_argument {
    use gungraun::prelude::*;

    binary_benchmark_group!(name = some);
    binary_benchmark_group!(name = some,);
    binary_benchmark_group!(
        name = some;
    );
}

mod test_when_max_parallel {
    use gungraun::prelude::*;

    #[binary_benchmark]
    fn some_func() -> Command {
        Command::new("foo")
    }

    // wrong type
    binary_benchmark_group!(name = some_1, max_parallel = None, benchmarks = some_func);
    binary_benchmark_group!(name = some_2, max_parallel = 0i32, benchmarks = some_func);
    // wrong type, multiple benches
    binary_benchmark_group!(
        name = some_3,
        max_parallel = None,
        benchmarks = [some_func, some_func]
    );

    // semicolon syntax
    binary_benchmark_group!(name = some_4; max_parallel = None; benchmarks = some_func);
    binary_benchmark_group!(name = some_5; max_parallel = 0i32; benchmarks = some_func);
    binary_benchmark_group!(
        name = some_6;
        max_parallel = None;
        benchmarks = some_func, some_func
    );
}

fn main() {}
