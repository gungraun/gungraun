use gungraun::prelude::*;

mod test_empty_file {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/empty.fix")]
    fn bench_library(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/empty.fix")]
    fn bench_binary(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }
}

mod test_file_does_not_exist {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/does_not_exist")]
    fn bench_library(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/does_not_exist")]
    fn bench_binary(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }
}

mod test_invalid_utf8 {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/invalid-utf8.fix")]
    fn bench_library(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/invalid-utf8.fix")]
    fn bench_binary(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }
}

mod test_args_and_file_parameter {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(
    file = "gungraun/tests/fixtures/numbers.fix",
    args = [("valid_arg".to_owned()), "another".to_owned()]
)]
    fn bench_library(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(
    file = "gungraun/tests/fixtures/numbers.fix",
    args = [("valid_arg".to_owned()), "another".to_owned()]
)]
    fn bench_binary(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }
}

mod test_wrong_benchmark_argument_type {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/numbers.fix")]
    fn bench_library(value: u64) -> String {
        value.to_string()
    }

    #[binary_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/numbers.fix")]
    fn bench_binary(value: u64) -> String {
        value.to_string()
    }
}

mod test_wrong_parameter_type_1 {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(file = ("gungraun/tests/fixtures/numbers.fix", String))]
    fn bench_library(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(file = ("gungraun/tests/fixtures/numbers.fix", String))]
    fn bench_binary(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }
}

mod test_wrong_parameter_type_2 {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(file = String::from("gungraun/tests/fixtures/numbers.fix"))]
    fn bench_library(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(file = String::from("gungraun/tests/fixtures/numbers.fix"))]
    fn bench_binary(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }
}

mod test_wrong_amount_of_benchmark_function_parameters {
    use super::*;

    #[library_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/numbers.fix")]
    fn bench_library(value: String, _other: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/numbers.fix")]
    fn bench_binary(value: String, _other: String) -> u64 {
        value.parse::<u64>().unwrap()
    }
}

mod test_wrong_benchmark_function_parameters_when_setup {
    use super::*;

    fn my_setup(line: String) -> u64 {
        line.parse().unwrap()
    }

    #[library_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/numbers.fix", setup = my_setup)]
    fn bench_library(value: String) -> u64 {
        value.parse::<u64>().unwrap()
    }

    #[binary_benchmark]
    #[benches::my_id(file = "gungraun/tests/fixtures/numbers.fix", setup = { my_setup("some string".to_owned()); })]
    fn bench_binary(value: u64) -> gungraun::Command {
        gungraun::Command::new("nope")
    }
}

fn main() {}
