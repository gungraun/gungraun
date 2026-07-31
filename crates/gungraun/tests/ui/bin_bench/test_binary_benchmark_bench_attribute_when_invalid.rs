mod test_when_config_has_wrong_type {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(config = "string")]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_setup_not_exists {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(setup = setup())]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_setup_as_path_not_exists {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(setup = setup)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_teardown_as_path_not_exists {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(teardown = teardown)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_args_has_no_value {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(args = )]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

// TODO: THE SPAN SHOULD POINT TO #[bench] instead of #[binary_benchmark]
mod test_when_args_not_present_but_expected {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some()]
    fn bench_binary(arg: &str) -> gungraun::Command {
        gungraun::Command::new("some").arg(arg).build()
    }
}

mod test_when_args_has_too_few_arguments {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(args = ())]
    fn bench_binary(arg: &str) -> gungraun::Command {
        gungraun::Command::new("some").arg(arg).build()
    }
}

mod test_when_args_has_too_many_arguments {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(args = (1))]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_args_type_is_wrong {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(args = (1))]
    fn bench_binary(arg: &str) -> gungraun::Command {
        gungraun::Command::new("some").arg(arg).build()
    }
}

mod test_when_multiple_args_types_are_wrong {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(args = (1, 2))]
    fn bench_binary(arg: &str) -> gungraun::Command {
        gungraun::Command::new("some").arg(arg).build()
    }
}

mod test_when_list_has_too_many_arguments {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(1)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_list_has_too_few_arguments {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(1)]
    fn bench_binary(first: usize, second: usize) -> gungraun::Command {
        gungraun::Command::new("some")
            .args([first.to_string(), second.to_string()])
            .build()
    }
}

mod test_when_list_type_is_wrong {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(1)]
    fn bench_binary(arg: &str) -> gungraun::Command {
        gungraun::Command::new("some").arg(arg).build()
    }
}

mod test_when_multiple_list_types_are_wrong {
    use gungraun::prelude::*;

    #[binary_benchmark]
    #[bench::some(1, 2)]
    fn bench_binary(arg: &str) -> gungraun::Command {
        gungraun::Command::new("some").arg(arg).build()
    }
}

fn main() {}
