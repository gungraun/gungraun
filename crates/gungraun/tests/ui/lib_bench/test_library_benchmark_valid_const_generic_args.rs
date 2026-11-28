use gungraun::prelude::*;

struct Stringlet<const SIZE: usize>([u64; SIZE]);

impl<const SIZE: usize> Stringlet<SIZE> {
    fn new(value: u64) -> Self {
        Self([value; SIZE])
    }

    fn sum(&self) -> u64 {
        self.0.iter().sum()
    }
}

// Const generic parameter used in an argument type. Values and arguments are supplied per bench
// case via `consts` and `args`.
#[library_benchmark]
#[bench::foo_zero(consts = (0), args = (Stringlet::<0>::new(0)))]
#[benches::foo_one(consts = [1], args = [Stringlet::<1>::new(1), Stringlet::<1>::new(2)])]
fn foo<const SIZE: usize>(s: Stringlet<SIZE>) -> u64 {
    s.sum()
}

// Const generic parameter combined with a type parameter and an argument type depending on the
// const parameter
#[library_benchmark]
#[benches::mixed(
    consts = [2, 4],
    args = [(Stringlet::<2>::new(1), 1u8), (Stringlet::<4>::new(1), 2u8)]
)]
fn mixed<const SIZE: usize, T>(s: Stringlet<SIZE>, t: T) -> u64
where
    T: Default + Copy + Into<u64>,
{
    let _ = t;
    s.sum() + T::default().into()
}

// A `type` alias with the same name as the const parameter checks that the forwarded const argument
// is disambiguated from a type in the generic argument list. Also covers lifetime and type
// parameters before the const parameter.
type N = u8;

#[library_benchmark]
#[bench::sized_ref(consts = (3), args = (&[1u8, 2, 3]))]
fn sized_ref<'a, T, const N: usize>(x: &'a [T; N]) -> usize
where
    T: Copy,
{
    x.len()
}

// A `setup` function providing the argument whose type depends on the const generic parameter
fn stringlet_setup() -> Stringlet<4> {
    Stringlet::<4>::new(7)
}

#[library_benchmark]
#[bench::with_setup(setup = stringlet_setup, consts = (4))]
fn with_setup<const SIZE: usize>(s: Stringlet<SIZE>) -> u64 {
    s.sum()
}

// An `iter` bench with elements whose type depends on the const generic parameter
#[library_benchmark]
#[benches::iter_consts(consts = [2], iter = [Stringlet::<2>::new(1), Stringlet::<2>::new(2)])]
fn iter_consts<const SIZE: usize>(s: Stringlet<SIZE>) -> u64 {
    s.sum()
}

// A local macro emitting both the `consts` and the `args` lists from a single list of sizes
// avoids duplicating the const value at the invocation site.
macro_rules! stringlet_benches {
    ($id:ident; ($($size:literal),+); $item:item) => {
        #[library_benchmark]
        #[benches::$id(consts = [$($size),+], args = [$(Stringlet::<$size>::new(1)),+])]
        $item
    };
}

stringlet_benches! {
    many;
    (1, 2, 4);
    fn many_sizes<const SIZE: usize>(s: Stringlet<SIZE>) -> u64 {
        s.sum()
    }
}

fn main() {}
