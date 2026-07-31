use gungraun::prelude::*;

#[library_benchmark]
fn bench() -> u64 {
    0
}

#[library_benchmark]
fn additional() -> u64 {
    0
}

fn some_setup() {
    println!("Setup");
}

fn some_teardown() {
    println!("Teardown");
}

mod test_group_minimal {
    use super::*;

    library_benchmark_group!(name = single_without_brackets, benchmarks = bench);
    library_benchmark_group!(name = single_with_brackets, benchmarks = [bench]);
    library_benchmark_group!(name = multiple, benchmarks = [bench, additional]);
}

mod test_group_with_config {
    use super::*;

    library_benchmark_group!(
        name = just_a_name,
        config = LibraryBenchmarkConfig::default(),
        benchmarks = [bench]
    );
}

mod test_group_with_all {
    use super::*;

    library_benchmark_group!(
        name = just_a_name,
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        setup = some_setup(),
        teardown = some_teardown(),
        benchmarks = [bench]
    );
}

mod test_main_single_without_brackets {
    use super::*;

    library_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(library_benchmark_groups = just_a_name);
}

mod test_main_single_with_brackets {
    use super::*;

    library_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(library_benchmark_groups = [just_a_name]);
}

mod test_main_multiple {
    use super::*;

    library_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    library_benchmark_group!(name = another_one, benchmarks = [bench]);
    main!(library_benchmark_groups = [just_a_name, another_one]);
}

mod test_main_with_config {
    use super::*;

    library_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(
        config = LibraryBenchmarkConfig::default(),
        library_benchmark_groups = [just_a_name]
    );
}

mod test_main_with_all {
    use super::*;

    library_benchmark_group!(name = just_a_name, benchmarks = [bench]);
    main!(
        config = LibraryBenchmarkConfig::default(),
        setup = some_setup(),
        teardown = some_teardown(),
        library_benchmark_groups = [just_a_name]
    );
}

fn main() {}
