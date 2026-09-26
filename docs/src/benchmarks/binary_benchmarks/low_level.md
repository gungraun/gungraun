# Low-Level API

This section does not go into full detail of the low-level API since it is fully
documented in the [API documentation][api-docs].

## The Basic Structure

The entry point of the low-level API is the `binary_benchmark_group`

```rust
# extern crate gungraun;
# macro_rules! env { ($m:tt) => {{ "/some/path" }} }
use gungraun::prelude::*;
use gungraun::{binary_benchmark_attribute, BinaryBenchmark, Bench};

binary_benchmark_group!(
    name = my_group,
    benchmarks = |group: &mut BinaryBenchmarkGroup| {
        group.binary_benchmark(BinaryBenchmark::new("bench_binary")
            .bench(Bench::new("some_id")
                .command(gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
                    .arg("foo.txt")
                    .build()
                )
            )
        )
    }
);

# fn main() {
main!(binary_benchmark_groups = my_group);
# }
```

The low-level API mirrors the high-level API, "structifying" the macros.

The `binary_benchmark_group!` is also a struct now, the `BinaryBenchmarkGroup`.
It cannot be instantiated. Instead, it is passed as argument to the expression
of the `benchmarks` parameter in a `binary_benchmark_group`. You can choose any
name instead of `group`; `group` is used throughout the examples.

There's the shorter `benchmarks = |group| /* ... */` instead of
`benchmarks = |group: &mut BinaryBenchmarkGroup| /* ... */`. We use the more
verbose variant in the examples because it is more informative for benchmarking
starters.

Furthermore, the `#[library_benchmark]` macro correlates with
`gungraun::LibraryBenchmark` and `#[bench]` with `gungraun::Bench`. The
parameters of the macros are now functions in the respective structs. The return
value of the benchmark function, the `gungraun::Command`, is now also a function
`gungraun::Bench::command`.

Note there is no `gungraun::Benches` struct since specifying multiple commands
with `gungraun::Bench::command` behaves exactly the same way as the `#[benches]`
attribute. So, the `file` parameter of `#[benches]` is a part of
`gungraun::Bench` and can be used with the `gungraun::Bench::file` function.

## Intermixing High-Level and Low-Level API

It is recommended to start with the high-level API using the
`#[binary_benchmark]` attribute, since you can fall back to the low-level API in
a few steps with the `binary_benchmark_attribute!` macro as shown below. The
other way around is much more involved.

```rust
# extern crate gungraun;
# macro_rules! env { ($m:tt) => {{ "/some/path" }} }
use gungraun::prelude::*;
use gungraun::{binary_benchmark_attribute, BinaryBenchmark, Bench};

#[binary_benchmark]
#[bench::some_id("foo")]
fn attribute_benchmark(arg: &str) -> gungraun::Command {
    gungraun::Command::new(env!("CARGO_BIN_EXE_my-binary"))
        .arg(arg)
        .build()
}

binary_benchmark_group!(
    name = low_level,
    benchmarks = |group: &mut BinaryBenchmarkGroup| {
        group
            .binary_benchmark(binary_benchmark_attribute!(attribute_benchmark))
            .binary_benchmark(
                BinaryBenchmark::new("low_level_benchmark")
                    .bench(
                        Bench::new("some_id").command(
                            gungraun::Command::new(env!("CARGO_BIN_EXE_my-binary"))
                                .arg("bar")
                                .build()
                        )
                    )
            )
    }
);

# fn main() {
main!(binary_benchmark_groups = low_level);
# }
```

As shown above, there's no need to transcribe the function `attribute_benchmark`
with the `#[binary_benchmark]` attribute into the low-level API structures. Just
keep it as it is and add it to a the `group` with
`group.binary_benchmark(binary_benchmark_attribute!(attribute_benchmark))`.
That's it! You can continue hacking on your benchmarks in the low-level API.

[api-docs]: https://docs.rs/gungraun/0.20.0/gungraun/index.html
