# Cachegrind: A High-Precision Tracing Profiler

## Prerequisites

In order to use `Cachegrind` instead of `Callgrind` you need Valgrind version
`3.22` or above installed (which you can look up with `valgrind --version`). In
this version Valgrind introduced the two [Client requests](./client_requests.md)
`start_instrumentation()` and `stop_instrumentation()`. In order to use client
requests you need to turn them on in the `Cargo.toml` with the `act` feature

```toml
[dev-dependencies]
<<<<<<< HEAD
gungraun = { version = "0.19.4", features = ["client_requests"] }
=======
gungraun = { version = "0.19.3", features = ["act"] }
>>>>>>> f1bf2db7 (feat(perf): Add perf tool support and improvements)
```

## The Cachegrind Feature

There are two ways to use Cachegrind instead of Callgrind. The first and easy
way is to use the `cachegrind` feature, so your `gungraun` spec should finally
look like this:

```toml
[dev-dependencies]
gungraun = { version = "0.19.4", features = ["cachegrind"] }
```

The `cachegrind` feature automatically activates the `act` feature, and there's
no need to specify it again. Now, without having to do anything else, all
benchmarks run with Cachegrind instead of Callgrind. However, this change has
implications which are better understood by showing the second way.

## The Second Way

There are multiple ways to run Cachegrind as the default tool (see also
command-line arguments), but they follow the same principle. For example in the
benchmark file run a specific benchmark function with Cachegrind:

```rust
# extern crate gungraun;
# pub mod my_lib { pub fn bubble_sort(input: Vec<i32>) -> Vec<i32> { input } }

use gungraun::prelude::*;
use gungraun::Tool;
use std::hint::black_box;

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
)]
#[bench::small(vec![3, 2, 1])]
#[bench::bigger(vec![5, 4, 3, 2, 1])]
fn bench_function(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(array)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_function);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

However, this alone is not enough for correct measurements. Choosing
`Cachegrind` as the default tool measures everything including setup and
teardown, ... For this reason we need client requests to tell `Cachegrind` when
to start and stop the instrumentation:

```rust
# extern crate gungraun;
# pub mod my_lib { pub fn bubble_sort(input: Vec<i32>) -> Vec<i32> { input } }

use gungraun::prelude::*;
use gungraun::{client_requests, Tool, Cachegrind};
use std::hint::black_box;

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
        .tool(Cachegrind::with_args(["--instr-at-start=no"]))
)]
#[bench::small(vec![3, 2, 1])]
#[bench::bigger(vec![5, 4, 3, 2, 1])]
fn bench_function(array: Vec<i32>) -> Vec<i32> {
    client_requests::cachegrind::start_instrumentation();
    let r = black_box(my_lib::bubble_sort(black_box(array)));
    client_requests::cachegrind::stop_instrumentation();
    r
}

library_benchmark_group!(name = my_group, benchmarks = bench_function);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Not only the body of the benchmark function changed but also the command-line
argument `--instr-at-start=no` had to be specified to start the instrumentation
with the client request and not (what is the default) when starting the
benchmark executable.

All of the above is exactly what the `cachegrind` feature does. It adds the
client requests to the function body, returns the result from the function and
starts Cachegrind with `--instr-at-start=no`. The consequence and a disadvantage
of Cachegrind is that the function body has to be altered a little bit. It's not
much but running other tools like `Callgrind` on the same benchmark function
like `Cachegrind` would show small differences because the client requests add
`10` - `20` instructions to the function body.

## When to Use Cachegrind

As shown above, running `Cachegrind` can have disadvantages but there are
circumstances under which it is better to use `Cachegrind`. Here's a comparison
of both tools:

| Cachegrind                                                                       | Callgrind                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Less sensitive to architecture-specific call/return heuristics                   | Callgrind's ability to detect function calls and returns depends on the instruction set of the platform it is run on. It works best on x86 and amd64, and unfortunately currently does not work so well on PowerPC, ARM, Thumb or MIPS code. This is because there are no explicit call or return instructions in these instruction sets, so Callgrind has to rely on heuristics to detect calls and returns |
| Bigger tool set: `cg_diff`, `cg_merge` and `cg_annotate`                         | Just `callgrind_annotate`                                                                                                                                                                                                                                                                                                                                                                                    |
| Smaller functionality which shows in a far less amount of command-line arguments | Greater functionality (`--toggle-collect`, ...)                                                                                                                                                                                                                                                                                                                                                              |
| Smaller amount of profile data and metrics                                       | More metrics (`--collect-bus`, ...)                                                                                                                                                                                                                                                                                                                                                                          |
| Client requests add a small amount of build time and have more prerequisites     | No need for client requests and no alteration of the benchmark function body is required which makes it more intuitive to use                                                                                                                                                                                                                                                                                |
