mod test_when_empty {
    use gungraun::prelude::*;

    library_benchmark_group!();
}

mod test_when_no_name {
    use gungraun::prelude::*;

    #[library_benchmark]
    fn some_func() {}

    library_benchmark_group!(benchmarks = some_func);

    // comma syntax
    library_benchmark_group!(
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        max_parallel = 0,
        setup = setup(),
        teardown = teardown(),
        benchmarks = some_func
    );
    // semicolon syntax
    library_benchmark_group!(
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        max_parallel = 0;
        setup = setup();
        teardown = teardown();
        benchmarks = some_func
    );

    library_benchmark_group!(benchmarks = [some_func]);
    library_benchmark_group!(benchmarks = some_func, some_func);
    library_benchmark_group!(benchmarks = [some_func, some_func]);
}

mod test_when_same_name {
    use gungraun::prelude::*;

    #[library_benchmark]
    fn some_func() {}

    library_benchmark_group!(name = some, benchmarks = some_func);
    library_benchmark_group!(name = some, benchmarks = some_func);
}

mod test_when_no_benchmark_argument {
    use gungraun::prelude::*;

    library_benchmark_group!(name = some_name);
}

mod test_when_no_benchmarks {
    use gungraun::prelude::*;

    // comma syntax
    library_benchmark_group!(
        name = some_name,
        benchmarks =
    );
    library_benchmark_group!(
        name = some_name,
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        max_parallel = 0,
        setup = setup(),
        teardown = teardown(),
        benchmarks =
    );

    // semicolon syntax
    library_benchmark_group!(
        name = some_name;
        benchmarks =
    );
    library_benchmark_group!(
        name = some_name;
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        max_parallel = 0;
        setup = setup();
        teardown = teardown();
        benchmarks =
    );
}

mod test_when_unknown_token {
    use gungraun::prelude::*;

    library_benchmark_group!(something);
}

mod test_when_max_parallel {
    use gungraun::prelude::*;

    #[library_benchmark]
    fn some_func() {}

    // wrong type
    library_benchmark_group!(name = some_1, max_parallel = None, benchmarks = some_func);
    library_benchmark_group!(name = some_2, max_parallel = 0i32, benchmarks = some_func);
    // wrong type, multiple benches
    library_benchmark_group!(
        name = some_3,
        max_parallel = None,
        benchmarks = [some_func, some_func]
    );

    // semicolon syntax
    library_benchmark_group!(name = some_4; max_parallel = None; benchmarks = some_func);
    library_benchmark_group!(name = some_5; max_parallel = 0i32; benchmarks = some_func);
    library_benchmark_group!(
        name = some_6;
        max_parallel = None;
        benchmarks = some_func, some_func
    );
}

fn main() {}
