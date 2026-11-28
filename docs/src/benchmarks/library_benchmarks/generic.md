# Generic Benchmark Functions

Benchmark functions can be generic. And `setup` and `teardown` functions, too.
There's actually not much more to say about it since generic benchmark (`setup`
and `teardown`) functions behave exactly the same way as you would expect it
from any other generic function apart from `const` parameters which are
explained in [Const Generic Parameters](#const-generic-parameters) below.

However, there is a common pitfall. If you have a function
`count_lines_in_file_fast` which expects as parameter a `PathBuf` and although
it is convenient especially when you have to specify many paths, don't do this:

```rust
# extern crate gungraun;
# mod my_lib { pub fn count_lines_in_file_fast(_path: std::path::PathBuf) -> u64 { 1 } }
use gungraun::prelude::*;

use std::hint::black_box;
use std::path::PathBuf;

#[library_benchmark]
#[bench::first("path/to/file")]
fn generic_bench<T>(path: T) -> u64 where T: Into<PathBuf> {
    black_box(my_lib::count_lines_in_file_fast(black_box(path.into())))
}

library_benchmark_group!(name = my_group, benchmarks = generic_bench);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Since `path.into()` is called in the benchmark function itself, the conversion
from a `&str` to a `PathBuf` is attributed to the benchmark metrics. This is
almost never what you intended. You should instead convert the argument to a
`PathBuf` in a generic `setup` function like that:

```rust
# extern crate gungraun;
# mod my_lib { pub fn count_lines_in_file_fast(_path: std::path::PathBuf) -> u64 { 1 } }
use gungraun::prelude::*;

use std::hint::black_box;
use std::path::PathBuf;

fn convert_to_pathbuf<T>(path: T) -> PathBuf where T: Into<PathBuf> {
    path.into()
}

#[library_benchmark]
#[bench::first(args = ("path/to/file"), setup = convert_to_pathbuf)]
fn not_generic_anymore(path: PathBuf) -> u64 {
    black_box(my_lib::count_lines_in_file_fast(black_box(path)))
}

library_benchmark_group!(name = my_group, benchmarks = not_generic_anymore);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

That way you can still enjoy the convenience to use string literals instead of
`PathBuf` in your `#[bench]` (or `#[benches]`) arguments and have clean
benchmark metrics.

## Const Generic Parameters

For benchmark functions with const generic parameters, use the `consts`
parameter in `#[bench]` and `#[benches]` to specify the const values. The syntax
is the same as for the `args` parameter:

```rust
# extern crate gungraun;
# mod my_lib { pub fn create_buffer(_: usize) -> Vec<u8> { vec![] } }
use gungraun::prelude::*;
use std::hint::black_box;

#[library_benchmark]
#[bench::small(consts = (100))]
#[bench::medium(consts = (1000))]
#[bench::large(consts = (10000))]
// or multiple consts
#[benches::multiple(consts = [200, 2000, 20000])]
fn bench_buffer<const SIZE: usize>() -> Vec<u8> {
    black_box(my_lib::create_buffer(black_box(SIZE)))
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [bench_buffer]
);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

For functions with multiple const generics:

```rust
# extern crate gungraun;
# mod my_lib { pub fn create_matrix(_: usize, _: usize) -> Vec<Vec<u8>> {
#     vec![] }
# }
use gungraun::prelude::*;
use std::hint::black_box;

#[library_benchmark]
#[bench::small_matrix(consts = (10, 10))]
#[bench::wide_matrix(consts = (5, 20))]
fn bench_matrix<const ROWS: usize, const COLS: usize>() -> Vec<Vec<u8>> {
    black_box(my_lib::create_matrix(black_box(ROWS), black_box(COLS)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_matrix);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Const expressions like `consts = ({ 1 + 20 })` are also supported. You can
combine `args` and `consts` to benchmark with both regular arguments and const
generics. See
[Specifying Multiple Benches at Once](./multiple_benches.md#the-consts-parameter)
for more details.

The type of an argument can itself depend on a const generic parameter. In that
case, construct the `args` values for the same const value as specified in
`consts`:

```rust
# extern crate gungraun;
# mod my_lib {
#     pub struct FixedBuffer<const SIZE: usize>([u64; SIZE]);
#     impl<const SIZE: usize> FixedBuffer<SIZE> {
#         pub fn new(value: u64) -> Self {
#             FixedBuffer([value; SIZE])
#         }
#         pub fn sum(&self) -> u64 {
#             self.0.iter().sum()
#         }
#     }
# }
use std::hint::black_box;

use gungraun::prelude::*;
use my_lib::FixedBuffer;

#[library_benchmark]
#[bench::small(consts = (4), args = (FixedBuffer::<4>::new(1)))]
#[benches::multiple(consts = [8, 16], args = [FixedBuffer::<8>::new(1), FixedBuffer::<16>::new(2)])]
fn bench_fixed_buffer<const SIZE: usize>(buffer: FixedBuffer<SIZE>) -> u64 {
    black_box(buffer.sum())
}

library_benchmark_group!(
    name = my_group,
    benchmarks = [bench_fixed_buffer]
);
# fn main() {
#     main!(library_benchmark_groups = my_group);
# }
```

`const` parameters can be combined freely in any order with lifetime and type
parameters in the benchmark function signature:

```rust
# extern crate gungraun;
# mod my_lib {
#   pub fn create_buffer<const SIZE: usize, T>(_: T) -> Vec<Vec<u8>> {
#       Vec::with_capacity(SIZE)
#   }
# }
use gungraun::prelude::*;
use std::hint::black_box;

#[library_benchmark]
#[bench::small(args = ("foo"), consts = (10))]
#[bench::big(args = ("bar"), consts = (10000))]
fn bench_buffer<const SIZE: usize, T>(arg_t: T) -> Vec<Vec<u8>>
where T: std::fmt::Display {
    black_box(my_lib::create_buffer::<SIZE, T>(black_box(arg_t)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_buffer);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```
