mod test_setup_as_path_too_few_arguments {
    use gungraun::prelude::*;

    fn setup(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(setup = setup)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_setup_as_path_too_many_arguments {
    use gungraun::prelude::*;

    fn setup(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(args = ("1", "2"), setup = setup)]
    fn bench_binary(a: &str, b: &str) -> gungraun::Command {
        gungraun::Command::new("some")
            .args([a.to_string(), b.to_string()])
            .build()
    }
}

mod test_teardown_as_path_too_few_arguments {
    use gungraun::prelude::*;

    fn teardown(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(teardown = teardown)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_teardown_as_path_too_many_arguments {
    use gungraun::prelude::*;

    fn teardown(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(args = ("1", "2"), teardown = teardown)]
    fn bench_binary(a: &str, b: &str) -> gungraun::Command {
        gungraun::Command::new("some")
            .args([a.to_string(), b.to_string()])
            .build()
    }
}

mod test_setup_too_few_arguments {
    use gungraun::prelude::*;

    fn setup(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(setup = setup())]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_setup_too_many_arguments {
    use gungraun::prelude::*;

    fn setup(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(setup = setup("1", "2"))]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_teardown_too_few_arguments {
    use gungraun::prelude::*;

    fn teardown(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(teardown = teardown())]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_teardown_too_many_arguments {
    use gungraun::prelude::*;

    fn teardown(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(teardown = teardown("1", "2"))]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_setup_too_many_teardown_too_many_arguments {
    use gungraun::prelude::*;

    fn teardown(_arg: &str) {}
    fn setup(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(setup = setup("1", "2"), teardown = teardown("1", "2"))]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_setup_as_path_too_few_teardown_too_few_arguments {
    use gungraun::prelude::*;

    fn teardown(_arg: &str) {}
    fn setup(_arg: &str) {}

    #[binary_benchmark]
    #[bench::some(setup = setup, teardown = teardown)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

fn main() {}
