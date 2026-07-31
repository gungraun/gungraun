use gungraun::prelude::*;

mod test_when_range {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1..=2)]
    fn bench_10(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_vector {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = vec![1, 2])]
    fn bench_20(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_already_iter {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = vec![1, 2].into_iter().map(|a| a + 10))]
    fn bench_30(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_option {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = Some(10))]
    fn bench_31(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_wildcard {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = Some(10))]
    fn bench_32(_: u64) -> String {
        "Some string".to_owned()
    }
}

mod test_when_reference {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = vec![&1, &2])]
    fn bench_100(&a: &u64) -> String {
        a.to_string()
    }

    #[library_benchmark]
    #[benches::nested(iter = vec![&[1, 2], &[2, 3]])]
    fn bench_110(&[a, b]: &[u64; 2]) -> String {
        format!("{a} + {b}")
    }
}

mod test_when_bench_and_iter {
    use super::*;

    #[library_benchmark]
    #[bench::some(1)]
    #[benches::other(iter = 1..=2)]
    fn bench_33(a: u64) -> String {
        a.to_string()
    }

    #[library_benchmark]
    #[benches::other(iter = 1..=2)]
    #[bench::some(1)]
    fn bench_34(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_multiple_benches_with_iter {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = vec![1, 2])]
    #[benches::other(iter = 1..=2)]
    fn bench_35(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_setup {
    use super::*;

    fn setup(a: u64) -> String {
        a.to_string()
    }

    #[library_benchmark]
    #[benches::some(iter = 1..=2, setup = setup)]
    fn bench_40(a: String) -> u64 {
        a.parse().unwrap()
    }
}

mod test_when_teardown {
    use super::*;

    fn teardown(a: String) {
        println!("{a}");
    }

    #[library_benchmark]
    #[benches::some(iter = 1..=2, teardown = teardown)]
    fn bench_50(a: u64) -> String {
        a.to_string()
    }
}

mod test_when_setup_and_teardown {
    use super::*;

    pub fn setup(a: u64) -> String {
        a.to_string()
    }

    pub fn teardown(a: u8) {
        println!("{a}");
    }

    #[library_benchmark]
    #[benches::some(iter = 1..=2, setup = setup, teardown = teardown)]
    fn setup_first_then_teardown(a: String) -> u8 {
        a.parse().unwrap()
    }

    #[library_benchmark]
    #[benches::some(iter = 1..=2, teardown = teardown, setup = setup)]
    fn teardown_first_then_setup(a: String) -> u8 {
        a.parse().unwrap()
    }
}

mod test_when_teardown_and_setup_are_paths {
    use super::*;

    #[library_benchmark]
    #[benches::some(
        iter = 1..=2,
        setup = super::test_when_setup_and_teardown::setup,
        teardown = super::test_when_setup_and_teardown::teardown
    )]
    fn bench_70(a: String) -> u8 {
        a.parse().unwrap()
    }
}

mod test_when_config {
    use super::*;

    #[library_benchmark]
    #[benches::some(iter = 1..=2, config = LibraryBenchmarkConfig::default())]
    fn bench_80(a: u64) -> String {
        a.to_string()
    }

    #[library_benchmark]
    #[benches::mixed_order(
        iter = 1..=2,
        teardown = super::test_when_setup_and_teardown::teardown,
        config = LibraryBenchmarkConfig::default(),
        setup = super::test_when_setup_and_teardown::setup
    )]
    fn bench_81(a: String) -> u8 {
        a.parse().unwrap()
    }
}

fn main() {}
