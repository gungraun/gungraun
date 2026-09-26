# Configuration

Library benchmarks can be configured with the [`LibraryBenchmarkConfig`] and
with
[Command-line arguments and Environment variables](../../cli_and_env/basics.md).

The `LibraryBenchmarkConfig` can be specified at different levels and sets the
configuration values for the same and lower levels. The values of the
`LibraryBenchmarkConfig` at higher levels can be overridden at a lower level.
Note that some values are additive rather than substitutive. Please see the docs
of the respective functions in [`LibraryBenchmarkConfig`] for more details.

The different levels where a `LibraryBenchmarkConfig` can be specified.

- At top-level with the `main!` macro

```rust
# extern crate gungraun;
use gungraun::prelude::*;

# #[library_benchmark] fn bench() {}
# library_benchmark_group!(name = my_group, benchmarks = bench);
# fn main() {
main!(
    config = LibraryBenchmarkConfig::default(),
    library_benchmark_groups = my_group
);
# }
```

- At group-level in the `library_benchmark_group!` macro

```rust
# extern crate gungraun;
use gungraun::prelude::*;

# #[library_benchmark] fn bench() {}
library_benchmark_group!(
    name = my_group,
    config = LibraryBenchmarkConfig::default(),
    benchmarks = bench
);

# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

- At `#[library_benchmark]` level

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }
use gungraun::prelude::*;

#[library_benchmark(config = LibraryBenchmarkConfig::default())]
fn bench() {
    /* ... */
}

library_benchmark_group!(
    name = my_group,
    config = LibraryBenchmarkConfig::default(),
    benchmarks = bench
);

# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

- and at `#[bench]`, `#[benches]` level

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }
use gungraun::prelude::*;

#[library_benchmark]
#[bench::some_id(args = (1, 2), config = LibraryBenchmarkConfig::default())]
#[benches::multiple(
    args = [(3, 4), (5, 6)],
    config = LibraryBenchmarkConfig::default()
)]
fn bench(a: u8, b: u8) {
    /* ... */
    # _ = (a, b);
}

library_benchmark_group!(
    name = my_group,
    config = LibraryBenchmarkConfig::default(),
    benchmarks = bench
);

# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

[`LibraryBenchmarkConfig`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.LibraryBenchmarkConfig.html
