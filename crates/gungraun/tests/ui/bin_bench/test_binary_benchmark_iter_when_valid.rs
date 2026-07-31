use gungraun::prelude::*;

mod test_when_range {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_vector {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = vec![1, 2])]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_already_iter {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = vec![1, 2].into_iter().map(|a| a + 10))]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_wildcard {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = vec![1, 2])]
    fn bench_10(_: u64) -> gungraun::Command {
        gungraun::Command::new("Some".to_owned())
    }
}

mod test_when_reference {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = vec![&1, &2])]
    fn bench_10(&a: &u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }

    #[binary_benchmark]
    #[benches::nested(iter = vec![&[1, 2], &[2, 3]])]
    fn bench_20(&[a, b]: &[u64; 2]) -> gungraun::Command {
        gungraun::Command::new(format!("{a} + {b}"))
    }
}

mod test_when_bench_and_iter {
    use super::*;

    #[binary_benchmark]
    #[bench::just_bench(1)]
    #[benches::some(iter = vec![1, 2])]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }

    #[binary_benchmark]
    #[benches::some(iter = vec![1, 2])]
    #[bench::just_bench(1)]
    fn bench_20(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_multiple_benches_with_iter {
    use super::*;

    #[binary_benchmark]
    #[benches::some(iter = 1..=2)]
    #[benches::other(iter = vec![1, 2])]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_setup_call {
    use super::*;

    fn setup(a: u64) -> String {
        a.to_string()
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, setup = setup(20))]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_setup_not_returning_type_of_benchmark_function {
    use super::*;

    fn setup(a: u64) -> String {
        a.to_string()
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, setup = setup)]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_teardown_call {
    use super::*;

    fn teardown(a: u64) -> String {
        a.to_string()
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, teardown = teardown(10))]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_teardown_path {
    use super::*;

    fn teardown(a: u64) -> String {
        a.to_string()
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, teardown = teardown)]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_setup_and_teardown {
    use super::*;

    pub fn teardown(a: u64) {
        a.to_string();
    }

    pub fn setup(a: u64) -> String {
        a.to_string()
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, setup = setup, teardown = teardown)]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }

    #[binary_benchmark]
    #[benches::some(iter = 1..=2, teardown = teardown, setup = setup)]
    fn bench_20(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

mod test_when_config {
    use super::*;

    #[binary_benchmark]
    #[benches::some(
        iter = 1..=2,
        config = BinaryBenchmarkConfig::default(),
        setup = super::test_when_setup_and_teardown::setup,
        teardown = super::test_when_setup_and_teardown::teardown
    )]
    fn bench_10(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }

    #[binary_benchmark]
    #[benches::mixed_order(
        iter = 1..=2,
        teardown = super::test_when_setup_and_teardown::teardown,
        config = BinaryBenchmarkConfig::default(),
        setup = super::test_when_setup_and_teardown::setup,
    )]
    fn bench_20(a: u64) -> gungraun::Command {
        gungraun::Command::new(a.to_string())
    }
}

fn main() {}
