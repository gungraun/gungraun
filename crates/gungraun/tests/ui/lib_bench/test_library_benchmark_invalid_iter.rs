use gungraun::prelude::*;

mod test_when_together_with_other_main_parameter {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1..=2, args = [0, 1])]
    fn bench_10() -> u64 {
        10
    }
}

mod test_when_iter_twice {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1..=2, iter = vec![0, 1])]
    fn bench_10(a: u64) -> u64 {
        a + 1
    }
}

mod test_when_iter_no_bench_arg {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10() -> u64 {
        1
    }
}

mod test_when_iter_too_many_bench_arg {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10(a: u64, b: u64) -> u64 {
        a + b
    }
}

mod test_when_wrong_argument_type {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10(a: &str) -> String {
        a.to_owned()
    }
}

mod test_when_setup_then_wrong_argument_type {
    use super::*;

    fn setup(a: u64) -> String {
        a.to_string()
    }

    #[library_benchmark]
    #[benches::some(iter = 1..=2, setup = setup)]
    fn bench_10(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_setup_has_wrong_argument_type {
    use super::*;

    fn setup(a: String) -> u64 {
        a.parse().unwrap()
    }

    #[library_benchmark]
    #[benches::some(iter = 1..=2, setup = setup)]
    fn bench_10(a: String) -> String {
        format!("foo: {a}")
    }
}

mod test_when_teardown_has_wrong_argument_type {
    use super::*;

    fn teardown(a: u64) {
        println!("{a}");
    }

    #[library_benchmark]
    #[benches::some(iter = 1..=2, teardown = teardown)]
    fn bench_10(a: u64) -> String {
        format!("foo: {a}")
    }
}

mod test_when_not_an_iterator {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1)]
    fn bench_10(a: u64) -> String {
        format!("foo: {a}")
    }
}

fn main() {}
