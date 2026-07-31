use gungraun::prelude::*;

mod test_when_reference {
    use super::*;

    #[library_benchmark]
    #[bench::some(&32)]
    fn bench_1(&num: &usize) -> String {
        num.to_string()
    }
}

mod test_when_reference_nested {
    use super::*;

    #[library_benchmark]
    #[bench::some(&[32, 42])]
    fn bench_10(&[a, b]: &[u64; 2]) -> String {
        (a + b).to_string()
    }
}

mod test_when_ident {
    use super::*;

    #[library_benchmark]
    #[bench::some(32)]
    fn bench_20(num: usize) -> String {
        num.to_string()
    }
}

mod test_when_slice {
    use super::*;

    #[library_benchmark]
    #[bench::some([1, 2])]
    fn bench_3([a, b]: [usize; 2]) -> String {
        (a + b).to_string()
    }
}

mod test_when_struct {
    use super::*;

    struct Point {
        x: u64,
        y: u64,
    }

    #[library_benchmark]
    #[bench::some(Point { x:1, y:2 })]
    fn bench_4(Point { x, y }: Point) -> String {
        (x + y).to_string()
    }
}

mod test_when_tuple_struct {
    use super::*;

    struct Point(u64, u64);

    #[library_benchmark]
    #[bench::some(Point(1, 2))]
    fn bench_5(Point(x, y): Point) -> String {
        (x + y).to_string()
    }
}

mod test_when_tuple {
    use super::*;

    #[library_benchmark]
    #[bench::some((1, 2))]
    fn bench_6((x, y): (u64, u64)) -> String {
        (x + y).to_string()
    }
}

mod test_when_wild_card {
    use super::*;

    #[library_benchmark]
    #[bench::some(1)]
    fn bench_7(_: u64) -> String {
        "some".to_owned()
    }
}

mod test_when_path {
    use super::*;

    #[library_benchmark]
    #[bench::some(|| 1)]
    fn bench_8(func: fn() -> u64) -> String {
        func().to_string()
    }
}

fn main() {}
