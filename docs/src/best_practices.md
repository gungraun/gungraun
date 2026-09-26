# Benchmarking Tips and Best Practices

This page describes best practices for writing benchmarks with Gungraun that
produce accurate, meaningful, and reproducible results.

## Use a Separate Workspace Member for Benchmarks

For library benchmarks, place your benchmarks in a separate workspace member
directory (for example, `benchmarks/`) rather than in the same crate as the code
being benchmarked.

Rust inlines functions freely within a crate but does not inline across crate
boundaries without explicit `#[inline]` attributes or link-time optimization
(LTO). This fundamental difference means in-crate benchmarks can present
significantly better performance than what users of your library will actually
experience.

This problem is not specific to Gungraun and it is generally a good idea to use
a separate workspace member for benchmarks no matter the benchmarking framework.

### Setting Up a Separate Benchmarks Crate

Create a workspace member for your benchmarks:

```toml
# `Cargo.toml` (workspace root of your crate `my_lib`)
[workspace]
members = ["benchmarks"]

# ...
```

```toml
# `benchmarks/Cargo.toml`
[package]
name = "benchmarks"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
my_lib = { path = ".." }

[dev-dependencies]
gungraun = "0.20.0"

# Assuming there is a gungraun benchmark in `benchmarks/benches/gungraun.rs`
[[bench]]
harness = false
name = "gungraun"
path = "benches/gungraun.rs"
```

You can now run the benchmarks with `cargo bench -p benchmarks` or
`cargo bench --workspace`.

This approach avoids the inlining problem entirely and gives you measurements
that reflect what users will observe. It also lets you use a different Rust
version or profile settings for benchmarking than your main project requires.

## Keep Benchmark Functions Clean

The body of your benchmark function should contain only the code you want to
measure. Setup and teardown logic should be handled elsewhere to avoid
attributing their costs to the function under test.

Gungraun provides built-in mechanisms for this:

```rust
# extern crate gungraun;
# fn process_data(data: Vec<u64>) -> u64 { data.len() as u64 }
use gungraun::prelude::*;
use std::hint::black_box;

fn expensive_setup(n: u64) -> Vec<u64> {
    (0..n).collect()
}

#[library_benchmark]
#[bench::with_setup(args = [100], setup = expensive_setup)]
fn bench_processing(data: Vec<u64>) -> u64 {
    black_box(process_data(black_box(data)))
}

# library_benchmark_group!(name = my_group, benchmarks = bench_processing);
# fn main() { main!(library_benchmark_groups = my_group); }
```

See [Setup and Teardown](./benchmarks/library_benchmarks/setup_and_teardown.md)
for the full range of options including teardown functions.

## Use `black_box` Appropriately

Wrap values in [`std::hint::black_box`][rust-black-box] to prevent the compiler
from optimizing away computations. As a general rule of thumb, you wrap all
input and output values:

```rust
# extern crate gungraun;
# fn expensive_computation(n: u64) -> u64 { n }
use gungraun::prelude::*;
use std::hint::black_box;

#[library_benchmark]
#[bench::low(5)]
fn bench_example(n: u64) -> u64 {
    // Ensure `n` and `result` are used and not optimized away
    black_box(expensive_computation(black_box(n)))
}

# library_benchmark_group!(name = example, benchmarks = bench_example);
# fn main() { main!(library_benchmark_groups = example); }
```

## Use a Sandbox When Feasible

A sandbox gives each benchmark run a temporary working directory with a more
stable path, removes files created during the run, and helps protect your
workspace from benchmarked code that creates, overwrites, or deletes files.

Although this might be surprising at first, the main goal of the sandbox is to
create a stable path in `/tmp` and a subdirectory with a random name but
constant length. See
[`Why using a Sandbox`](./benchmarks/library_benchmarks/configuration/sandbox.md#why-using-a-sandbox)
for the reasons.

## Design for CI

Gungraun excels in CI environments because it produces consistent measurements
even in noisy virtualized systems. To get the most from CI benchmarking:

- Use [baselines](./cli_and_env/baselines.md) to detect regressions across
  branches and runs
- Avoid benchmarks that depend on external state (network, filesystem timing,
  random seeds without fixed seeds)

See [Regressions](./regressions.md) for configuring regression detection.

## Understanding Your Metrics

Gungraun measures instruction counts, cache behavior, and estimated cycles.
These correlate with but do not equal wall-clock time:

- Instruction counts are precise and portable across systems
- Cache simulation approximates real cache behavior
- Estimated cycles provide a rough wall-clock approximation

For user-perceived latency validation, combine Gungraun with wall-clock
benchmarks like Criterion.rs. Use Gungraun for detecting regressions and
microoptimizations; use wall-clock benchmarks for validating end-to-end
performance claims.

## Further Links

- An excellent source of performance and benchmarking tips can be found in the
  [Rust Performance Book](https://nnethercote.github.io/perf-book/introduction.html)
  by [N.Nethercote](https://nnethercote.github.io/), the creator of Valgrind.
- A collection of very good tips for reproducible benchmarking in Linux
  environments used on
  [Julia](https://github.com/JuliaCI/BenchmarkTools.jl/blob/main/docs/src/linuxtips.md)
  benchmark runners. These tips are predominantly useful for [Perf](./perf.md)
  and wall-clock benchmarks which rely on real hardware.
- If you are setting up a dedicated benchmark runner to avoid virtualized
  hardware and get the most out of [Perf](./perf.md) benchmarks, have a look at
  the basic setup of
  [Julia benchmark runners](https://github.com/JuliaCI/julia-perf/blob/master/docs/perf-runner.md)

## Where to Go Next

- [Library Benchmark Quickstart](./benchmarks/library_benchmarks/quickstart.md)
  to start writing benchmarks
- [Binary Benchmarks](./benchmarks/binary_benchmarks.md) for benchmarking
  executables
- [Regressions](./regressions.md) for regression checks for example in the CI

[rust-black-box]: https://doc.rust-lang.org/std/hint/fn.black_box.html
