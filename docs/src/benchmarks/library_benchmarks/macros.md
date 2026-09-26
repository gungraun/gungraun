# The Macros in More Detail

This section is a brief reference to all the macros available in library
benchmarks. Feel free to come back here from other sections if you need a
reference. For the complete documentation of each macro see the [API
documentation][api-docs].

For the following examples it is assumed that there is a file `lib.rs` in a
crate named `my_lib` with the following content:

```rust
pub fn bubble_sort(mut array: Vec<i32>) -> Vec<i32> {
    for i in 0..array.len() {
        for j in 0..array.len() - i - 1 {
            if array[j + 1] < array[j] {
                array.swap(j, j + 1);
            }
        }
    }
    array
}
```

## The `#[library_benchmark]` Attribute

This attribute needs to be present on all benchmark functions specified in the
[`library_benchmark_group`](#the-library_benchmark_group-macro). The benchmark
function can then be further annotated with the inner
[`#[bench]`](#the-bench-attribute) or [`#[benches]`](#the-benches-attribute)
attributes.

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(value: Vec<i32>) -> Vec<i32> { value } }
use gungraun::prelude::*;
use std::hint::black_box;

#[library_benchmark]
#[bench::one(vec![1])]
#[benches::multiple(vec![1, 2], vec![1, 2, 3], vec![1, 2, 3, 4])]
fn bench_bubble_sort(values: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(values)))
}

library_benchmark_group!(name = bubble_sort_group, benchmarks = bench_bubble_sort);
# fn main() {
main!(library_benchmark_groups = bubble_sort_group);
# }
```

The following parameters are accepted:

- `config`: Takes a [`LibraryBenchmarkConfig`]
- `setup`: A global setup function which is applied to all following
  [`#[bench]`](#the-bench-attribute) and [`#[benches]`](#the-benches-attribute)
  attributes if not overwritten by a `setup` parameter of these attributes.
- `teardown`: Similar to `setup` but takes a global `teardown` function.

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(value: Vec<i32>) -> Vec<i32> { value } }
use gungraun::prelude::*;
use gungraun::OutputFormat;
use std::hint::black_box;

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .truncate_description(None)
        )
)]
#[bench::one(vec![1])]
fn bench_bubble_sort(values: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(values)))
}

library_benchmark_group!(name = bubble_sort_group, benchmarks = bench_bubble_sort);
# fn main() {
main!(library_benchmark_groups = bubble_sort_group);
# }
```

### The `#[bench]` Attribute

The basic structure is `#[bench::some_id(/* parameters */)]`. The part after the
`::` must be an id unique within the same `#[library_benchmark]`. This attribute
accepts the following parameters:

- `args`: A tuple with a list of arguments which are passed to the benchmark
  function. The parentheses also need to be present if there is only a single
  argument (`#[bench::my_id(args = (10))]`).
- `consts`: A tuple with const expressions for const generic parameters of the
  benchmark function. The number and order must match the const generics in the
  function signature. Otherwise, the function signature may contain other
  parameters in any order like lifetimes and type parameters. Parentheses are
  required: `consts = (321)` for a single const generic, `consts = (321, 654)`
  for multiple. Const expressions like `consts = ({ 1 + 20 })` are also
  supported.
- `config`: Accepts a [`LibraryBenchmarkConfig`]
- `setup`: A function which takes the arguments specified in the `args`
  parameter and passes its return value to the benchmark function.
- `teardown`: A function which takes the return value of the benchmark function.

If no other parameters besides `args` are present you can simply pass the
arguments as a list of values. So, instead of
`#[bench::my_id(args = (10, 20))]`, you could also use the shorter
`#[bench::my_id(10, 20)]`.

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(value: Vec<i32>) -> Vec<i32> { value } }
use gungraun::prelude::*;
use std::hint::black_box;

// This function is used to create a worst case array we want to sort with our implementation of
// bubble sort
pub fn worst_case(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).rev().collect()
    } else {
        (0..start).rev().collect()
    }
}

#[library_benchmark]
#[bench::one(vec![1])]
#[bench::worst_two(args = (vec![2, 1]))]
#[bench::worst_four(args = (4), setup = worst_case)]
fn bench_bubble_sort(value: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(value)))
}

library_benchmark_group!(name = bubble_sort_group, benchmarks = bench_bubble_sort);
# fn main() {
main!(library_benchmark_groups = bubble_sort_group);
# }
```

### The `#[benches]` Attribute

This attribute is used to specify multiple benchmarks. It accepts the same
parameters as the [`#[bench]`](#the-bench-attribute) attribute: `args`,
`config`, `setup`, `teardown`, `consts` and additionally the `iter` and `file`
parameters which are explained in full detail in
[Specifying Multiple Benchmarks at Once](./multiple_benches.md). In contrast to
the `args` parameter in [`#[bench]`](#the-bench-attribute), `args` takes an
array of arguments.

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(value: Vec<i32>) -> Vec<i32> { value } }
use gungraun::prelude::*;
use std::hint::black_box;

pub fn worst_case(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).rev().collect()
    } else {
        (0..start).rev().collect()
    }
}

#[library_benchmark]
#[benches::worst_two_and_three(args = [vec![2, 1], vec![3, 2, 1]])]
#[benches::worst_four_to_nine(args = [4, 5, 6, 7, 8, 9], setup = worst_case)]
fn bench_bubble_sort(value: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(value)))
}

library_benchmark_group!(name = bubble_sort_group, benchmarks = bench_bubble_sort);
# fn main() {
main!(library_benchmark_groups = bubble_sort_group);
# }
```

Similarly, `consts` takes an array of const expressions. If the amount of `args`
and `consts` elements don't match then the last element of the parameter with
the lower amount of elements is repeated:

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
// This creates three benchmarks with args "foo" repeated and the three consts
#[benches::small(args = ["foo"], consts = [10, 50, 100])]
// This creates two benchmarks with args "foo" and "baz" and the consts `10000`
// repeated
#[benches::big(args = ["bar", "baz"], consts = [10000])]
fn bench_buffer<const SIZE: usize, T>(arg_t: T) -> Vec<Vec<u8>>
where T: std::fmt::Display {
    black_box(my_lib::create_buffer::<SIZE, T>(black_box(arg_t)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_buffer);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

## The library_benchmark_group! Macro

The `library_benchmark_group` macro accepts the following parameters (in this
order and separated by a comma):

- **`name`** (mandatory): A unique name used to identify the group for the
  `main!` macro
- **`config`** (optional): A [`LibraryBenchmarkConfig`] which is applied to all
  benchmarks within the same group.
- **`compare_by_id`** (optional): The default is false. If true, all benches in
  the benchmark functions specified in the `benchmarks` parameter are compared
  with each other as long as the ids (the part after the `::` in
  `#[bench::id(...)]`) match. See also
  [Comparing benchmark functions](./compare_by_id.md)
- **`max_parallel`** (optional): The default is no limit. If set, `0` means no
  limit (same as not specifying), `1` disables parallel execution for this
  group, and values `>= 2` limit the maximum number of parallel benchmarks. This
  option only has an effect when parallel execution is enabled via `--parallel`
  or `GUNGRAUN_PARALLEL`. See
  [Running Benchmarks in Parallel](../../cli_and_env/parallel.md#limiting-parallelism-per-group)
  for more details.
- **`setup`** (optional): A setup function or any valid expression which is run
  before all benchmarks of this group
- **`teardown`** (optional): A teardown function or any valid expression which
  is run after all benchmarks of this group
- **`benchmarks`** (mandatory): An array of names of benchmark functions
  annotated with `#[library_benchmark]`. Multiple names use the array-syntax
  (`benchmarks = [bench_1, bench_2]`). For convenience a single function name
  can be specified without brackets (`benchmarks = bench_1`).

Note the `setup` and `teardown` parameters are different to the ones of
`#[library_benchmark]`, `#[bench]` and `#[benches]`. They accept an expression
or function call as in `setup = group_setup_function()`. Also, these `setup` and
`teardown` functions are not overridden by the ones from any of the before
mentioned attributes.

## The main! Macro

This macro is the entry point for Gungraun and creates the benchmark harness. It
accepts the following top-level arguments in this order (separated by a comma):

- **`config`** (optional): Optionally specify a [`LibraryBenchmarkConfig`]
- **`setup`** (optional): A setup function or any valid expression which is run
  before all benchmarks
- **`teardown`** (optional): A setup function or any valid expression which is
  run after all benchmarks
- **`library_benchmark_groups`** (mandatory): The names of one or more library
  benchmark groups. Multiple names use the array-syntax
  (`library_benchmark_groups = [group_1, group_2]`). A single group can be
  specified without brackets (`library_benchmark_groups = group_1`).

Like the `setup` and `teardown` of the
[`library_benchmark_group`](#the-library_benchmark_group-macro), these
parameters accept an expression and are not overridden by the `setup` and
`teardown` of the `library_benchmark_group`, `#[library_benchmark]`, `#[bench]`
or `#[benches]` attribute.

[api-docs]: https://docs.rs/gungraun/0.20.0/gungraun/
[`LibraryBenchmarkConfig`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.LibraryBenchmarkConfig.html
