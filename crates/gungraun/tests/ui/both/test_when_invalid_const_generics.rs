use gungraun::prelude::*;

mod test_consts_when_invalid_type {
    use super::*;

    #[library_benchmark]
    #[bench::id(consts = (123i32))]
    fn when_one_parameter<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = (123i32))]
    fn when_binary_one_parameter<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [(123i32)])]
    fn when_benches_one_parameter<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [123i32])]
    fn when_binary_benches_one_parameter<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(consts = (123usize, 123i32))]
    fn when_two_parameters<const NUM: usize, const OTHER: usize>() -> usize {
        NUM + OTHER
    }

    #[binary_benchmark]
    #[bench::id(consts = (123usize, 123i32))]
    fn when_binary_two_parameters<const NUM: usize, const OTHER: usize>() -> Command {
        Command::new("some")
            .args([NUM.to_string(), OTHER.to_string()])
            .build()
    }

    #[library_benchmark]
    #[benches::id(consts = [(123usize, 123i32)])]
    fn when_benches_two_parameters<const NUM: usize, const OTHER: usize>() -> usize {
        NUM + OTHER
    }

    #[binary_benchmark]
    #[benches::id(consts = [(123usize, 123i32)])]
    fn when_binary_benches_two_parameters<const NUM: usize, const OTHER: usize>() -> Command {
        Command::new("some")
            .args([NUM.to_string(), OTHER.to_string()])
            .build()
    }
}

mod test_consts_when_wrong_count {
    use super::*;

    #[library_benchmark]
    #[bench::id(consts = (123))]
    fn when_one_const_no_generic() -> usize {
        42
    }

    #[binary_benchmark]
    #[bench::id(consts = (123))]
    fn when_binary_one_const_no_generic() -> Command {
        Command::new("some").build()
    }

    #[library_benchmark]
    #[benches::id(consts = [123])]
    fn when_benches_one_const_no_generic() -> usize {
        42
    }

    #[binary_benchmark]
    #[benches::id(consts = [123])]
    fn when_binary_benches_one_const_no_generic() -> Command {
        Command::new("some").build()
    }

    #[library_benchmark]
    #[bench::id(consts = ())]
    fn when_zero_provided_one_expected<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = ())]
    fn when_binary_zero_provided_one_expected<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [])]
    fn when_benches_zero_provided_one_expected<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [])]
    fn when_binary_benches_zero_provided_one_expected<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(consts = (123, 456))]
    fn when_two_provided_one_expected<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = (123, 456))]
    fn when_binary_two_provided_one_expected<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [(123, 456)])]
    fn when_benches_two_provided_one_expected<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [(123, 456)])]
    fn when_binary_benches_two_provided_one_expected<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(consts = (123))]
    fn when_one_provided_two_expected<const NUM: usize, const OTHER: usize>() -> usize {
        NUM + OTHER
    }

    #[binary_benchmark]
    #[bench::id(consts = (123))]
    fn when_binary_one_provided_two_expected<const NUM: usize, const OTHER: usize>() -> Command {
        Command::new("some")
            .args([NUM.to_string(), OTHER.to_string()])
            .build()
    }

    #[library_benchmark]
    #[benches::id(consts = [(123)])]
    fn when_benches_one_provided_two_expected<const NUM: usize, const OTHER: usize>() -> usize {
        NUM + OTHER
    }

    #[binary_benchmark]
    #[benches::id(consts = [(123)])]
    fn when_binary_benches_one_provided_two_expected<const NUM: usize, const OTHER: usize>(
    ) -> Command {
        Command::new("some")
            .args([NUM.to_string(), OTHER.to_string()])
            .build()
    }
}

mod test_consts_when_duplicate {
    use super::*;

    #[library_benchmark]
    #[bench::id(consts = (123), consts = (456))]
    fn when_bench_duplicate<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = (123), consts = (456))]
    fn when_binary_bench_duplicate<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [123], consts = [456])]
    fn when_benches_duplicate<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [123], consts = [456])]
    fn when_binary_benches_duplicate<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }
}

mod test_consts_when_iter_multiple_elements {
    use super::*;

    #[library_benchmark]
    #[benches::id(iter = 1..10, consts = [(123), (456)])]
    fn when_iter_with_two_consts<const NUM: usize>(arg: usize) -> usize {
        arg + NUM
    }

    #[binary_benchmark]
    #[benches::id(iter = 1..10, consts = [(123), (456)])]
    fn when_binary_iter_with_two_consts<const NUM: usize>(arg: usize) -> Command {
        Command::new("some")
            .args([arg.to_string(), NUM.to_string()])
            .build()
    }

    #[library_benchmark]
    #[benches::id(iter = 1..10, consts = [(123, 456), (789, 012)])]
    fn when_iter_with_two_consts_two_params<const NUM: usize, const OTHER: usize>(
        arg: usize,
    ) -> usize {
        arg + NUM + OTHER
    }

    #[binary_benchmark]
    #[benches::id(iter = 1..10, consts = [(123, 456), (789, 012)])]
    fn when_binary_iter_with_two_consts_two_params<const NUM: usize, const OTHER: usize>(
        arg: usize,
    ) -> Command {
        Command::new("some")
            .args([arg.to_string(), NUM.to_string(), OTHER.to_string()])
            .build()
    }
}

mod test_consts_when_invalid_syntax {
    use super::*;

    #[library_benchmark]
    #[bench::id(consts = "invalid")]
    fn when_bench_string_not_tuple<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = "invalid")]
    fn when_binary_bench_string_not_tuple<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = ["invalid"])]
    fn when_benches_string_not_tuple<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = ["invalid"])]
    fn when_binary_benches_string_not_tuple<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(consts = some_ident)]
    fn when_bench_ident_not_expression<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = some_ident)]
    fn when_binary_bench_ident_not_expression<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = some_ident)]
    fn when_bench_ident_not_expression<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = some_ident)]
    fn when_binary_bench_ident_not_expression<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }
}

mod test_consts_when_mixed_with_type_generics {
    use super::*;

    #[library_benchmark]
    #[bench::id(args = (1), consts = (123))]
    fn when_const_count_wrong_with_type_param<T>(arg: T) -> T {
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (1), consts = (123))]
    fn when_binary_const_count_wrong_with_type_param<T>(arg: T) -> Command
    where
        T: std::fmt::Display,
    {
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [1], consts = [123])]
    fn when_benches_const_count_wrong_with_type_param<T>(arg: T) -> T {
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [1], consts = [123])]
    fn when_binary_benches_const_count_wrong_with_type_param<T>(arg: T) -> Command
    where
        T: std::fmt::Display,
    {
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (1), consts = (123, 456))]
    fn when_two_consts_one_expected_with_type_param<const NUM: usize, T>(arg: T) -> T {
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (1), consts = (123, 456))]
    fn when_binary_two_consts_one_expected_with_type_param<const NUM: usize, T>(arg: T) -> Command
    where
        T: std::fmt::Display,
    {
        Command::new("some")
            .args([NUM.to_string(), arg.to_string()])
            .build()
    }

    #[library_benchmark]
    #[benches::id(args = [1], consts = [123, 456])]
    fn when_benches_two_consts_one_expected_with_type_param<const NUM: usize, T>(arg: T) -> T {
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [1], consts = [123, 456])]
    fn when_binary_benches_two_consts_one_expected_with_type_param<const NUM: usize, T>(
        arg: T,
    ) -> Command
    where
        T: std::fmt::Display,
    {
        Command::new("some")
            .args([NUM.to_string(), arg.to_string()])
            .build()
    }

    #[library_benchmark]
    #[bench::id(args = (1), consts = (123))]
    fn when_missing_generic_type_parameter<const NUM: usize>(arg: T) -> T {
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (1), consts = (123))]
    fn when_binary_missing_generic_type_parameter<const NUM: usize>(arg: T) -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [1], consts = [123])]
    fn when_benches_missing_generic_type_parameter<const NUM: usize>(arg: T) -> T {
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [1], consts = [123])]
    fn when_binary_benches_missing_generic_type_parameter<const NUM: usize>(arg: T) -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }
}

mod test_consts_with_args_wrong_count {
    use super::*;

    #[library_benchmark]
    #[bench::id(args = (1), consts = ())]
    fn when_args_one_consts_zero<const NUM: usize>(val: usize) -> usize {
        val + NUM
    }

    #[binary_benchmark]
    #[bench::id(args = ("1"), consts = ())]
    fn when_binary_args_one_consts_zero<const NUM: usize>(val: &str) -> Command {
        Command::new("some").args([val, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[benches::id(args = [(1)], consts = [])]
    fn when_benches_args_one_consts_zero<const NUM: usize>(val: usize) -> usize {
        val + NUM
    }

    #[binary_benchmark]
    #[benches::id(args = ["1"], consts = [])]
    fn when_binary_benches_args_one_consts_zero<const NUM: usize>(val: &str) -> Command {
        Command::new("some").args([val, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[bench::id(args = (), consts = (123))]
    fn when_args_zero_consts_one(val: usize) -> usize {
        val
    }

    #[binary_benchmark]
    #[bench::id(args = (), consts = (123))]
    fn when_binary_args_zero_consts_one() -> Command {
        Command::new("some").build()
    }

    #[library_benchmark]
    #[benches::id(args = [], consts = [123])]
    fn when_benches_args_zero_consts_one(val: usize) -> usize {
        val
    }

    #[binary_benchmark]
    #[benches::id(args = [], consts = [123])]
    fn when_binary_benches_args_zero_consts_one() -> Command {
        Command::new("some").build()
    }
}

fn main() {}
