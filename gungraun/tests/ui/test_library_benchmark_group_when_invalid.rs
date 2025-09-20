mod test_library_benchmark_group_when_no_name {
    use gungraun::{library_benchmark, library_benchmark_group};
    #[library_benchmark]
    fn some_func() {}

    library_benchmark_group!(benchmarks = some_func);
}

mod test_library_benchmark_group_when_no_benchmarks {
    use gungraun::library_benchmark_group;

    library_benchmark_group!(
        name = some_name;
        benchmarks =
    );
}

mod test_library_benchmark_group_when_unknown_token {
    use gungraun::library_benchmark_group;

    library_benchmark_group!(something);
}

fn main() {}
