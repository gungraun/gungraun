mod test_deprecated_plain_functions {
    use gungraun::prelude::*;
    fn some_func() {}
    main!(some_func);
}

mod test_deprecated_callgrind_args_and_functions {
    use gungraun::prelude::*;
    fn some_func() {}
    main!(
        callgrind_args = "some";
        functions = some_func
    );
}

fn main() {}
