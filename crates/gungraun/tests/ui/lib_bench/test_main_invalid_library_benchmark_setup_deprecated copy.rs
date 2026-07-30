mod test_when_deprecated {
    use gungraun::prelude::*;
    fn some_func() {}
    main!(run = cmd = "some", id = "id", args = []);
}

fn main() {}
