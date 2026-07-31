mod test_when_library_benchmark_as_group {
    use gungraun::prelude::*;
    #[library_benchmark]
    fn some_func() {}

    main!(library_benchmark_groups = some_func);
}

mod test_when_invalid_config {
    use gungraun::prelude::*;
    #[library_benchmark]
    fn some_func() {}

    library_benchmark_group!(name = my_group, benchmarks = some_func);
    main!(config = "some", library_benchmark_groups = my_group);
}

fn main() {}
