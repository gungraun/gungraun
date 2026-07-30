use gungraun::prelude::*;

mod test_when_together_with_other_main_parameter {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, args = [0, 1])]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new("echo")
    }
}

mod test_when_iter_twice {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, iter = vec![0, 1])]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new("echo")
    }
}

mod test_when_no_bench_arg {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10() -> gungraun::Command {
        gungraun::Command::new("echo")
    }
}

// TODO: The error message is not great. Same in library benchmarks
#[rustfmt::skip]
// The error message is not great:
// error[E0689]: can't call method `into_iter` on ambiguous numeric type `{integer}`
//   --> tests/ui/test_binary_benchmark_iter_when_invalid.rs:36:5
//    |
// 36 |     #[binary_benchmark]
//    |     ^^^^^^^^^^^^^^^^^^^
//    |
//    = note: this error originates in the attribute macro `binary_benchmark` (in Nightly
//      builds, run with -Z macro-backtrace for more info)
// help: you must specify a type for this binding, like `i32`
//    |
// 36 |     #[binary_benchmark]: i32
//    |                        +++++
//
mod test_when_not_an_iterator {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1)]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new("echo")
    }
}

mod test_when_too_many_bench_args {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10(a: u64, b: u64) -> gungraun::Command {
        gungraun::Command::new("echo")
    }
}

mod test_when_wrong_argument_type {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10(a: String) -> gungraun::Command {
        gungraun::Command::new("echo")
    }
}

mod test_when_setup_has_wrong_argument_type {
    use super::*;

    fn setup(a: String) -> u64 {
        a.parse().unwrap()
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, setup = setup)]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_teardown_has_wrong_argument_type {
    use super::*;

    fn teardown(a: String) -> u64 {
        a.parse().unwrap()
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, teardown = teardown)]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

fn main() {}
