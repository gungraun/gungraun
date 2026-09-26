# Sandbox

The [`Sandbox`] is a temporary directory used for a single benchmark run. It is
created before the benchmark-specific `setup` and deleted after the
benchmark-specific `teardown`. If `setup` or `teardown` are not present, the
benchmark function still runs inside the sandbox.

The same `Sandbox` type is used by library and binary benchmarks. Binary
benchmarks use `BinaryBenchmarkConfig::sandbox` instead of
`LibraryBenchmarkConfig::sandbox`, but the isolation and fixture behavior is the
same.

## Why Using a Sandbox?

Although this might be surprising at first, the main goal of the sandbox is to
create a stable path and more stable benchmark metrics. As long as `$TMPDIR` is
unset or consistently set to `/tmp`, the temporary directory has a constant
length on unix machines (except android which uses `/data/local/tmp`). The
directory itself is created with a constant length but random name like
`/tmp/.a23sr8fk`.

The reason for this is that the layout of the benchmark binary can change with
the Linux environment. For example the environment variables and in turn the
current directory are copied to the stack before the program starts which
changes the layout and therefore has an effect on the measurements [^1][^2].

For example, if a member of your project has set up the project in
`/home/bob/workspace/our-project` running the benchmarks in this directory, and
the ci runs the benchmarks in `/runner/our-project`, the event counts might
differ. If possible, the benchmarks should be run in a constant environment.
[Clearing the environment variables](../important.md) is also such a measure.

Other good reasons for using a `Sandbox` are convenience, e.g. if you create
files during `setup`, the benchmark function, or `teardown` and do not want to
delete all files manually. Or, maybe more importantly, if benchmarked code is
destructive and deletes files, it is usually safer to run such code in a
temporary directory where it cannot cause damage to your or other file systems.

The `Sandbox` is deleted after the benchmark, regardless of whether the
benchmark run was successful or not. The latter is not guaranteed if you only
rely on `teardown`, as `teardown` is only executed if the benchmark returns
without error.

```rust
# extern crate gungraun;
# mod my_lib { pub fn count_bytes(path: &str) -> u64 { path.len() as u64 } }
use gungraun::prelude::*;
use gungraun::Sandbox;

use std::hint::black_box;

fn create_file(path: &str) -> String {
    std::fs::write(path, "some content").unwrap();
    path.to_owned()
}

#[library_benchmark]
#[bench::foo(
    args = ("foo.txt"),
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    setup = create_file
)]
fn bench_library(path: String) -> u64 {
    black_box(my_lib::count_bytes(black_box(&path)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

In this example, as part of the `setup`, the `create_file` function with the
argument `foo.txt` is executed in the `Sandbox` before the benchmark function is
executed. The benchmark function is executed in the same `Sandbox` and therefore
the file `foo.txt` with the content `some content` exists thanks to the `setup`.
After the execution of the benchmark, the `Sandbox` is completely removed,
deleting all files created during `setup`, the benchmark function, and
`teardown` if it had been present in this example.

## Fixtures

Since `setup` is run in the sandbox, you can't copy fixtures from your project's
workspace into the sandbox that easily anymore. The `Sandbox` can be configured
to copy `fixtures` into the temporary directory with `Sandbox::fixtures`:

```rust
# extern crate gungraun;
# mod my_lib { pub fn count_bytes(path: &str) -> u64 { path.len() as u64 } }
use gungraun::prelude::*;
use gungraun::Sandbox;

use std::hint::black_box;

#[library_benchmark]
#[bench::foo(
    args = ("foo.txt"),
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true)
            .fixtures(["benches/foo.txt"])),
)]
fn bench_library(path: &str) -> u64 {
    black_box(my_lib::count_bytes(black_box(path)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

The above will copy the fixture file `foo.txt` in the `benches` directory into
the sandbox root as `foo.txt`. Relative paths in `Sandbox::fixtures` are
interpreted relative to the workspace root. In a multi-crate workspace this is
the directory with the top-level `Cargo.toml` file. Paths in `Sandbox::fixtures`
are not limited to files, they can be directories, too.

If you have more complex demands, you can access the workspace root via the
environment variable `_WORKSPACE_ROOT` in `setup` and `teardown`. Suppose, there
is a fixture located in `/home/the_project/foo_crate/benches/fixtures/foo.txt`
with `the_project` being the workspace root and `foo_crate` a workspace member.
If the benchmark is expected to create a file `bar.json`, which needs further
inspection after the benchmarks have run, you can copy it into a temporary
directory `tmp` (which may or may not exist) in `foo_crate`:

```rust
# extern crate gungraun;
# mod my_lib {
#     pub fn create_output(path: &str) {
#         std::fs::write(path, "{}").unwrap();
#     }
# }
use gungraun::prelude::*;
use gungraun::Sandbox;

use std::path::PathBuf;

fn copy_fixture(path: &str) -> String {
    let workspace_root = PathBuf::from(std::env::var_os("_WORKSPACE_ROOT").unwrap());
    std::fs::copy(
        workspace_root
            .join("foo_crate")
            .join("benches")
            .join("fixtures")
            .join(path),
        path,
    )
    .unwrap();
    path.to_owned()
}

// This function will fail if `bar.json` does not exist, which is fine as this
// file is expected to be created by the benchmarked code. So, if this file does
// not exist, an error will occur and the benchmark will fail. Although
// benchmarks are not expected to test the correctness of the application, the
// `teardown` can be used to check postconditions for a successful run.
fn copy_back(path: &str) {
    let workspace_root = PathBuf::from(std::env::var_os("_WORKSPACE_ROOT").unwrap());
    let dest_dir = workspace_root.join("foo_crate").join("tmp");
    if !dest_dir.exists() {
        std::fs::create_dir(&dest_dir).unwrap();
    }
    std::fs::copy(path, dest_dir.join(path)).unwrap();
}

#[library_benchmark]
#[bench::foo(
    args = ("foo.txt"),
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    setup = copy_fixture,
    teardown = copy_back
)]
fn bench_library(_path: String) -> &'static str {
    my_lib::create_output("bar.json");
    "bar.json"
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

## Current Directory

By default, a benchmark with sandboxing enabled runs in the sandbox root.
Without sandboxing, the benchmark uses the directory set by `cargo bench`.
`LibraryBenchmarkConfig::current_dir` changes this working directory. If you use
a relative `current_dir` with sandboxing enabled, it must point inside the
sandbox, which is often useful together with copied fixture directories:

```rust
# extern crate gungraun;
# mod my_lib { pub fn count_bytes(path: &str) -> u64 { path.len() as u64 } }
use gungraun::prelude::*;
use gungraun::Sandbox;

use std::hint::black_box;

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true).fixtures(["benches/fixtures"]))
        .current_dir("fixtures")
)]
#[bench::foo("foo.txt")]
fn bench_library(path: &str) -> u64 {
    black_box(my_lib::count_bytes(black_box(path)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

[^1]: https://www.youtube.com/watch?v=r-TLSBdHe1A

[^2]:
    https://users.cs.northwestern.edu/~robby/courses/322-2013-spring/mytkowicz-wrong-data.pdf

[`Sandbox`]: https://docs.rs/gungraun/0.20.0/gungraun/struct.Sandbox.html
