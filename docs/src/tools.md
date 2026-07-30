# Additional Tools

In addition to or instead of the default tool `Callgrind`, you can use Gungraun
to run Linux [`perf`](./perf.md) or other Valgrind profiling tools like
[`DHAT`](./dhat.md), `Massif` or [`Cachegrind`](./cachegrind.md) and the
experimental `BBV` but also error checking tools like `Memcheck`, `Helgrind` and
`DRD`.

Note that support for `Massif` or `BBV` is currently only basic and doesn't show
useful stats and metrics in the terminal output of Gungraun. But, the output
files are generated as usual and are ready to be examined with tools like
`ms_print`.

See also the [Valgrind User Manual][valgrind-manual] for all the details about
each tool and their command line arguments.

## Selecting Tools

It's possible to change the default tool `Callgrind` to any supported tool with
the [command-line argument](./cli_and_env/basics.md) `--default-tool=<tool>` or
environment variable `GUNGRAUN_DEFAULT_TOOL=<tool>`. `<tool>` may be one of
`callgrind`, `cachegrind`, `dhat`, `massif`, `memcheck`, `helgrind`, `drd`,
`exp-bbv`, `perf`.

Running tools in addition to the default tool can be achieved with
`--tools=<tools>` or `GUNGRAUN_TOOLS=<tools>` where `<tools>` is a `,`-separated
list of one or more of the `<tool>` values above.

Gungraun resolves tool selection separately for each benchmark. A configured
tool is retained only if it is supported on the benchmark's compilation target
and has not been disabled with `enable(false)`. If the configured default is not
retained, the first remaining tool becomes the effective default. When the
target supports at least one tool family but none of a benchmark's configured
tools are retained, that benchmark is skipped.

Target support does not guarantee runtime availability. A retained tool must
still be installed and executable, and `perf` may additionally require suitable
system permissions. Runtime failures remain errors. Use a
[custom tool runner](./cli_and_env/tool_runner.md) when the tool must run in a
container or another execution environment.

The tool configurations can be changed in the benchmark file by specifying the
structs `Callgrind`, `Cachegrind`, ..., `Bbv` in `LibraryBenchmarkConfig::tool`
or `BinaryBenchmarkConfig::tool`.

Note that it is fully sufficient to specify a configuration to actually run the
tool. For example to run [`DHAT`](./dhat.md) with its default configuration for
all library benchmarks in the same file in addition to `Callgrind` without the
need for `--tools=dhat`:

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }
use gungraun::prelude::*;
use gungraun::Dhat;
use std::hint::black_box;

#[library_benchmark]
fn bench_library() -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(vec![3, 2, 1])))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);

# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()),
    library_benchmark_groups = my_group
);
# }
```

All tools which produce an `ERROR SUMMARY` `(Memcheck, DRD, Helgrind)` have
[`--error-exitcode=201`][valgrind-exit-codes] set, so if there are any errors,
the benchmark run fails with `201`. You can overwrite this default for example
for `Memcheck`

```rust
# extern crate gungraun;
use gungraun::Memcheck;

Memcheck::with_args(["--error-exitcode=0"]);
```

which would restore the default of `0` from Valgrind.

[valgrind-exit-codes]:
    https://valgrind.org/docs/manual/manual-core.html#manual-core.erropts
[valgrind-manual]: https://valgrind.org/docs/manual/manual.html
