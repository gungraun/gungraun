use gungraun::prelude::*;

mod test_consts_when_empty {
    use super::*;

    #[library_benchmark]
    #[bench::id(consts = ())]
    fn when_bench() -> usize {
        42
    }

    #[binary_benchmark]
    #[bench::id(consts = ())]
    fn when_binary_bench() -> Command {
        Command::new("some").build()
    }

    #[library_benchmark]
    #[benches::id(consts = [])]
    fn when_benches() -> usize {
        42
    }

    #[binary_benchmark]
    #[benches::id(consts = [])]
    fn when_binary_benches() -> Command {
        Command::new("some").build()
    }
}

mod test_consts_when_expressions {
    use super::*;

    #[library_benchmark]
    #[bench::id(consts = ({ 1 + 2 }))]
    fn when_bench_one_const<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = ({ 1 + 2 }))]
    fn when_binary_bench_one_const<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [{ 1 + 2 }])]
    fn when_benches_one_const<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [{ 1 + 2 }])]
    fn when_binary_benches_one_const<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [{ 1 + 2 }, { 3 + 4 }])]
    fn when_benches_two_const<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [{ 1 + 2 }, { 3 + 4 }])]
    fn when_binary_benches_two_const<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }
}

mod test_consts_without_args {
    use super::*;

    #[library_benchmark]
    #[bench::id(consts = (123))]
    fn when_bench<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(consts = (123))]
    fn when_binary_bench<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [123])]
    fn when_benches_one_consts<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [123])]
    fn when_binary_benches_one_consts<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [(123)])]
    fn when_benches_one_consts_in_parentheses<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [(123)])]
    fn when_binary_benches_one_consts_in_parentheses<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [123, 456])]
    fn when_benches_multiple_consts<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [123, 456])]
    fn when_binary_benches_multiple_consts<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [(123), (456)])]
    fn when_benches_multiple_consts_in_parentheses<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(consts = [(123), (456)])]
    fn when_binary_benches_multiple_consts_in_parentheses<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(consts = [(123, 456), (456, 789)])]
    fn when_benches_multiple_consts_parameter<const NUM: usize, const OTHER: usize>() -> usize {
        NUM + OTHER
    }

    #[binary_benchmark]
    #[benches::id(consts = [(123, 456), (456, 789)])]
    fn when_binary_benches_multiple_consts_parameter<const NUM: usize, const OTHER: usize>(
    ) -> Command {
        Command::new("some")
            .args([NUM.to_string(), OTHER.to_string()])
            .build()
    }
}

mod test_consts_with_args {
    use super::*;

    #[library_benchmark]
    #[bench::id(args = (), consts = ())]
    fn when_args_and_consts_empty() -> usize {
        42
    }

    #[binary_benchmark]
    #[bench::id(args = (), consts = ())]
    fn when_binary_args_and_consts_empty() -> Command {
        Command::new("some").build()
    }

    #[library_benchmark]
    #[benches::id(args = [], consts = [])]
    fn when_benches_args_and_consts_empty() -> usize {
        42
    }

    #[binary_benchmark]
    #[benches::id(args = [], consts = [])]
    fn when_binary_benches_args_and_consts_empty() -> Command {
        Command::new("some").build()
    }

    #[library_benchmark]
    #[bench::id(args = (), consts = (123))]
    fn when_args_empty_and_consts_one<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[bench::id(args = (), consts = (123))]
    fn when_binary_args_empty_and_consts_one<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [], consts = [123])]
    fn when_benches_args_empty_and_consts_one<const NUM: usize>() -> usize {
        NUM
    }

    #[binary_benchmark]
    #[benches::id(args = [], consts = [123])]
    fn when_binary_benches_args_empty_and_consts_one<const NUM: usize>() -> Command {
        Command::new("some").arg(NUM.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (123), consts = ())]
    fn when_args_one_and_consts_empty(num: usize) -> usize {
        num
    }

    #[binary_benchmark]
    #[bench::id(args = ("123"), consts = ())]
    fn when_binary_args_one_and_consts_empty(num: &str) -> Command {
        Command::new("some").arg(num).build()
    }

    #[library_benchmark]
    #[benches::id(args = [123], consts = [])]
    fn when_benches_args_one_and_consts_empty(num: usize) -> usize {
        num
    }

    #[binary_benchmark]
    #[benches::id(args = ["123"], consts = [])]
    fn when_binary_benches_args_one_and_consts_empty(num: &str) -> Command {
        Command::new("some").arg(num).build()
    }

    #[library_benchmark]
    #[bench::id(args = (123i32), consts = (456))]
    fn when_args_one_and_consts_one_different_type<const NUM: usize>(num: i32) -> usize {
        NUM + (num as usize)
    }

    #[binary_benchmark]
    #[bench::id(args = ("123"), consts = (456))]
    fn when_binary_args_one_and_consts_one_different_type<const NUM: usize>(num: &str) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[benches::id(args = [123], consts = [456])]
    fn when_benches_args_one_and_consts_one_different_type<const NUM: usize>(num: i32) -> usize {
        NUM + (num as usize)
    }

    #[binary_benchmark]
    #[benches::id(args = ["123"], consts = [456])]
    fn when_binary_benches_args_one_and_consts_one_different_type<const NUM: usize>(
        num: &str,
    ) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[bench::id(args = (123), consts = (456))]
    fn when_args_one_and_consts_one_same_type<const NUM: usize>(num: usize) -> usize {
        NUM + num
    }

    #[binary_benchmark]
    #[bench::id(args = ("123"), consts = (456))]
    fn when_binary_args_one_and_consts_one_same_type<const NUM: usize>(num: &str) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[benches::id(args = [123], consts = [456])]
    fn when_benches_args_one_and_consts_one_same_type<const NUM: usize>(num: usize) -> usize {
        NUM + num
    }

    #[binary_benchmark]
    #[benches::id(args = ["123"], consts = [456])]
    fn when_binary_benches_args_one_and_consts_one_same_type<const NUM: usize>(
        num: &str,
    ) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }
}

mod test_benches_consts_with_args {
    use super::*;

    #[library_benchmark]
    #[benches::id(args = [123], consts = [321, 654])]
    fn when_args_one_and_consts_one_more_than_args<const NUM: usize>(num: usize) -> usize {
        NUM + num
    }

    #[binary_benchmark]
    #[benches::id(args = ["123"], consts = [321, 654])]
    fn when_binary_args_one_and_consts_one_more_than_args<const NUM: usize>(num: &str) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[benches::id(args = [123], consts = [321, 654, 987])]
    fn when_args_one_and_consts_two_more_than_args<const NUM: usize>(num: usize) -> usize {
        NUM + num
    }

    #[binary_benchmark]
    #[benches::id(args = ["123"], consts = [321, 654, 987])]
    fn when_binary_args_one_and_consts_two_more_than_args<const NUM: usize>(num: &str) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[benches::id(args = [123, 345], consts = [321])]
    fn when_args_one_more_than_consts<const NUM: usize>(num: usize) -> usize {
        NUM + num
    }

    #[binary_benchmark]
    #[benches::id(args = ["123", "345"], consts = [321])]
    fn when_binary_args_one_more_than_consts<const NUM: usize>(num: &str) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[benches::id(args = [123, 345, 678], consts = [321])]
    fn when_args_two_more_than_consts<const NUM: usize>(num: usize) -> usize {
        NUM + num
    }

    #[binary_benchmark]
    #[benches::id(args = ["123", "345", "678"], consts = [321])]
    fn when_binary_args_two_more_than_consts<const NUM: usize>(num: &str) -> Command {
        Command::new("some").args([num, &NUM.to_string()]).build()
    }
}

mod test_consts_with_file {
    use super::*;

    #[library_benchmark]
    #[benches::id(file = "gungraun/tests/fixtures/numbers.fix", consts = [])]
    fn when_file_is_valid_and_consts_empty(line: String) -> String {
        line
    }

    #[binary_benchmark]
    #[benches::id(file = "gungraun/tests/fixtures/numbers.fix", consts = [])]
    fn when_binary_file_is_valid_and_consts_empty(line: String) -> Command {
        Command::new("some").arg(line).build()
    }

    #[library_benchmark]
    #[benches::id(file = "gungraun/tests/fixtures/numbers.fix", consts = [123])]
    fn when_file_is_valid_and_consts_is_one<const NUM: usize>(line: String) -> String {
        format!("{line}: {NUM}")
    }

    #[binary_benchmark]
    #[benches::id(file = "gungraun/tests/fixtures/numbers.fix", consts = [123])]
    fn when_binary_file_is_valid_and_consts_is_one<const NUM: usize>(line: String) -> Command {
        Command::new("some").args([line, NUM.to_string()]).build()
    }

    #[library_benchmark]
    #[benches::id(file = "gungraun/tests/fixtures/numbers.fix", consts = [(123, 345)])]
    fn when_file_is_valid_and_consts_is_two<const NUM: usize, const OTHER: usize>(
        line: String,
    ) -> String {
        format!("{line}: {NUM}")
    }

    #[binary_benchmark]
    #[benches::id(file = "gungraun/tests/fixtures/numbers.fix", consts = [(123, 345)])]
    fn when_binary_file_is_valid_and_consts_is_two<const NUM: usize, const OTHER: usize>(
        line: String,
    ) -> Command {
        Command::new("some")
            .args([line, NUM.to_string(), OTHER.to_string()])
            .build()
    }
}

mod test_consts_with_iter {
    use super::*;

    #[library_benchmark]
    #[benches::id(iter = 1..10, consts = [])]
    fn when_iter_and_consts_empty(arg: usize) -> usize {
        arg
    }

    #[binary_benchmark]
    #[benches::id(iter = 1..10, consts = [])]
    fn when_binary_iter_and_consts_empty(arg: usize) -> Command {
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(iter = 1..10, consts = [123])]
    fn when_iter_and_consts_is_one<const NUM: usize>(arg: usize) -> usize {
        arg + NUM
    }

    #[binary_benchmark]
    #[benches::id(iter = 1..10, consts = [123])]
    fn when_binary_iter_and_consts_is_one<const NUM: usize>(arg: usize) -> Command {
        Command::new("some")
            .args([arg.to_string(), NUM.to_string()])
            .build()
    }

    #[library_benchmark]
    #[benches::id(iter = 1..10, consts = [(123, 456)])]
    fn when_iter_and_consts_is_two<const NUM: usize, const OTHER: usize>(arg: usize) -> usize {
        arg + NUM + OTHER
    }

    #[binary_benchmark]
    #[benches::id(iter = 1..10, consts = [(123, 456)])]
    fn when_binary_iter_and_consts_is_two<const NUM: usize, const OTHER: usize>(
        arg: usize,
    ) -> Command {
        Command::new("some")
            .args([arg.to_string(), NUM.to_string(), OTHER.to_string()])
            .build()
    }
}

mod test_consts_with_other_generics {
    use super::*;

    #[library_benchmark]
    #[bench::id(args = (0), consts = (123))]
    fn when_bench_first_const_then_other<const NUM: usize, T>(arg: T) -> T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (0), consts = (123))]
    fn when_binary_bench_first_const_then_other<const NUM: usize, T: std::fmt::Display>(
        arg: T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (0), consts = (123))]
    fn when_bench_first_other_then_const<T, const NUM: usize>(arg: T) -> T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (0), consts = (123))]
    fn when_binary_bench_first_other_then_const<T: std::fmt::Display, const NUM: usize>(
        arg: T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (0), consts = (123, 456))]
    fn when_bench_two_const_then_other<const NUM: usize, const OTHER: usize, T>(arg: T) -> T {
        println!("{NUM} {OTHER}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (0), consts = (123, 456))]
    fn when_binary_bench_two_const_then_other<
        const NUM: usize,
        const OTHER: usize,
        T: std::fmt::Display,
    >(
        arg: T,
    ) -> Command {
        println!("{NUM} {OTHER}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (0), consts = (123, 456))]
    fn when_bench_two_const_and_other_mixed<const NUM: usize, T, const OTHER: usize>(arg: T) -> T {
        println!("{NUM} {OTHER}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (0), consts = (123, 456))]
    fn when_binary_bench_two_const_and_other_mixed<
        const NUM: usize,
        T: std::fmt::Display,
        const OTHER: usize,
    >(
        arg: T,
    ) -> Command {
        println!("{NUM} {OTHER}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (0), consts = (123, 456))]
    fn when_bench_other_then_two_const<T, const NUM: usize, const OTHER: usize>(arg: T) -> T {
        println!("{NUM} {OTHER}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (0), consts = (123, 456))]
    fn when_binary_bench_other_then_two_const<
        T: std::fmt::Display,
        const NUM: usize,
        const OTHER: usize,
    >(
        arg: T,
    ) -> Command {
        println!("{NUM} {OTHER}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [0], consts = [123])]
    fn when_benches_first_const_then_other<const NUM: usize, T>(arg: T) -> T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [0], consts = [123])]
    fn when_binary_benches_first_const_then_other<const NUM: usize, T: std::fmt::Display>(
        arg: T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [0], consts = [123])]
    fn when_benches_first_other_then_const<T, const NUM: usize>(arg: T) -> T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [0], consts = [123])]
    fn when_binary_benches_first_other_then_const<T: std::fmt::Display, const NUM: usize>(
        arg: T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [0], consts = [(123, 456)])]
    fn when_benches_two_const_then_other<const NUM: usize, const OTHER: usize, T>(arg: T) -> T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [0], consts = [(123, 456)])]
    fn when_binary_benches_two_const_then_other<
        const NUM: usize,
        const OTHER: usize,
        T: std::fmt::Display,
    >(
        arg: T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [0], consts = [(123, 345)])]
    fn when_benches_other_then_two_const<T, const NUM: usize, const OTHER: usize>(arg: T) -> T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [0], consts = [(123, 345)])]
    fn when_binary_benches_other_then_two_const<
        T: std::fmt::Display,
        const NUM: usize,
        const OTHER: usize,
    >(
        arg: T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [0], consts = [(123, 456)])]
    fn when_benches_other_and_const_mixed<const NUM: usize, T, const OTHER: usize>(arg: T) -> T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [0], consts = [(123, 456)])]
    fn when_binary_benches_other_and_const_mixed<
        const NUM: usize,
        T: std::fmt::Display,
        const OTHER: usize,
    >(
        arg: T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }
}

mod test_consts_with_lifetimes_and_other_generics {
    use super::*;

    #[library_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_bench_first_const_lifetime<const NUM: usize, 'a, T>(arg: &'a T) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_binary_bench_first_const_lifetime<const NUM: usize, 'a, T: std::fmt::Display>(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_bench_first_lifetime_then_const<'a, T, const NUM: usize>(arg: &'a T) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_binary_bench_first_lifetime_then_const<'a, T: std::fmt::Display, const NUM: usize>(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_bench_const_and_lifetimes_mixed_a_then_t<'a, const NUM: usize, T>(arg: &'a T) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_binary_bench_const_and_lifetimes_mixed_a_then_t<
        'a,
        const NUM: usize,
        T: std::fmt::Display,
    >(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_bench_const_and_lifetimes_mixed_t_then_a<T, const NUM: usize, 'a>(arg: &'a T) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[bench::id(args = (&0), consts = (123))]
    fn when_binary_bench_const_and_lifetimes_mixed_t_then_a<
        T: std::fmt::Display,
        const NUM: usize,
        'a,
    >(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_benches_first_const_then_lifetime<const NUM: usize, 'a, T>(arg: &'a T) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_binary_benches_first_const_then_lifetime<const NUM: usize, 'a, T: std::fmt::Display>(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_benches_first_lifetime_then_const<'a, T, const NUM: usize>(arg: &'a T) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_binary_benches_first_lifetime_then_const<'a, T: std::fmt::Display, const NUM: usize>(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_benches_const_and_lifetimes_mixed_a_then_t<'a, const NUM: usize, T>(
        arg: &'a T,
    ) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_binary_benches_const_and_lifetimes_mixed_a_then_t<
        'a,
        const NUM: usize,
        T: std::fmt::Display,
    >(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }

    #[library_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_benches_const_and_lifetimes_mixed_t_then_a<T, const NUM: usize, 'a>(
        arg: &'a T,
    ) -> &'a T {
        println!("{NUM}");
        arg
    }

    #[binary_benchmark]
    #[benches::id(args = [&0], consts = [123])]
    fn when_binary_benches_const_and_lifetimes_mixed_t_then_a<
        T: std::fmt::Display,
        const NUM: usize,
        'a,
    >(
        arg: &'a T,
    ) -> Command {
        println!("{NUM}");
        Command::new("some").arg(arg.to_string()).build()
    }
}

fn main() {}
