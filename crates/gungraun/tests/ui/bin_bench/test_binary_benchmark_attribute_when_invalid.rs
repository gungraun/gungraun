mod test_when_config_has_wrong_type {
    use gungraun::prelude::*;

    #[binary_benchmark(config = "string")]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_command_is_mut_ref {
    use gungraun::prelude::*;

    #[binary_benchmark]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some").arg("some_arg")
    }
}

mod test_when_wrong_return_type_with_equal_name {
    use gungraun::prelude::*;

    #[binary_benchmark]
    fn bench_binary() -> gungraun::Command {
        std::process::Command::new("some")
    }
}

mod test_when_wrong_return_type_in_signature {
    use gungraun::prelude::*;

    #[binary_benchmark]
    fn bench_binary() -> String {
        String::new()
    }
}

mod test_when_wrong_return_type_in_signature_with_equal_name {
    use gungraun::prelude::*;

    #[binary_benchmark]
    fn bench_binary() -> std::process::Command {
        std::process::Command::new("some")
    }
}

mod test_when_setup_does_not_exist {
    use gungraun::prelude::*;

    #[binary_benchmark(setup = does_not_exist())]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_setup_as_path_too_few_arguments {
    use gungraun::prelude::*;

    fn setup(_arg: usize) {}

    #[binary_benchmark(setup = setup)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_teardown_does_not_exist {
    use gungraun::prelude::*;

    #[binary_benchmark(teardown = does_not_exist())]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

mod test_when_teardown_as_path_too_few_arguments {
    use gungraun::prelude::*;

    fn teardown(_arg: usize) {}

    #[binary_benchmark(teardown = teardown)]
    fn bench_binary() -> gungraun::Command {
        gungraun::Command::new("some")
    }
}

fn main() {}
