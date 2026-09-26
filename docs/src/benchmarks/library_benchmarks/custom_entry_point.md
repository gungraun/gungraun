# Custom Entry Points

The [`EntryPoint`] can be set to `EntryPoint::None` which disables the entry
point, `EntryPoint::Default` which uses the benchmark function as entry point or
`EntryPoint::Custom` which will be discussed in more detail in this chapter.
This section is dedicated to the entry point of `Callgrind`.
[Dhat](../../dhat.md) uses an entry point, too and although both are interpreted
very similar there are differences which are fully described in the
[Dhat](../../dhat.md) chapter.

To understand custom entry points, take a small detour into how [Callgrind] and
Gungraun work under the hood.

## Gungraun Under the Hood

`Callgrind` collects metrics and associates them with a function. This happens
based on the compiled code not the source code, so it is possible to hook into
any function not only public functions. `Callgrind` can be configured to switch
instrumentation on and off based on a function name with
[`--toggle-collect`][callgrind-arguments]. By default, Gungraun sets this toggle
(which we call [`EntryPoint`]) to the benchmarking function. Setting the toggle
implies `--collect-atstart=no`. So, all events before (in the `setup`) and after
the benchmark function (in the `teardown`) are not collected. Somewhat
simplified, but conveying the basic idea, here is a commented example:

```rust
// <-- collect-at-start=no

# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }
use gungraun::prelude::*;
use std::hint::black_box;

#[library_benchmark]
fn bench() -> Vec<i32> { // <-- DEFAULT ENTRY POINT starts collecting events
    black_box(my_lib::bubble_sort(black_box(vec![3, 2, 1])))
} // <-- stop collecting events

library_benchmark_group!( name = my_group, benchmarks = bench);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

### Pitfall: Inlined Functions

The fact that `Callgrind` acts on the compiled code harbors a pitfall. The
compiler with compile-time optimizations switched on (which is usually the case
when compiling benchmarks) inlines functions if it sees an advantage in doing
so. Gungraun takes care, that this doesn't happen with the benchmark function,
so `Callgrind` can find and hook into the benchmark function. But, in your
production code you actually don't want to stop the compiler from doing its job
just to be able to benchmark that function. So, be cautious with benchmarking
private functions and only choose functions of which it is known that they are
not being inlined.

## Hook into Private Functions

The basic idea is to choose a public function in your library acting as access
point to the actual function you want to benchmark. As outlined before, this
works only reliably for functions which are not inlined by the compiler.

```rust
# extern crate gungraun;
use gungraun::prelude::*;
use gungraun::{EntryPoint, Callgrind};
use std::hint::black_box;

mod my_lib {
    #[inline(never)]
    fn bubble_sort(input: Vec<i32>) -> Vec<i32> {
        // The algorithm
#        input
    }

    pub fn access_point(input: Vec<i32>) -> Vec<i32> {
        println!("Doing something before the function call");
        bubble_sort(input)
    }
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .entry_point(EntryPoint::Custom("*::my_lib::bubble_sort".to_owned()))
        )
)]
#[bench::small(vec![3, 2, 1])]
#[bench::bigger(vec![5, 4, 3, 2, 1])]
fn bench_private(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::access_point(black_box(array)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_private);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Note the `#[inline(never)]` we use in this example to make sure the
`bubble_sort` function is not getting inlined.

We use a [wildcard][callgrind-arguments] `*::my_lib::bubble_sort` for
`EntryPoint::Custom` for demonstration purposes. You might want to tighten this
pattern. If you don't know how the pattern looks like, use `EntryPoint::None`
first then run the benchmark. Now, investigate the
[Callgrind output file](../../cli_and_env/output/out_directory.md). This output
file is pretty low-level but all you need to do is search for the entries which
start with `fn=...`. In the example above this entry might look like
`fn=algorithms::my_lib::bubble_sort` if `my_lib` would be part of the top-level
`algorithms` module. Or, using grep:

```shell
grep '^fn=.*::bubble_sort$' target/gungraun/the_package/benchmark_file_name/my_group/bench_private.bigger/callgrind.bench_private.bigger.out
```

Having found the pattern, you can eventually use `EntryPoint::Custom`.

[Callgrind]: https://valgrind.org/docs/manual/cl-manual.html
[callgrind-arguments]:
    https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options
[`EntryPoint`]: https://docs.rs/gungraun/0.20.0/gungraun/enum.EntryPoint.html
