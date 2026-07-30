# Valgrind Client Requests

<!-- TODO: Update docs to new act, stubs usage, add perf -->

Gungraun ships with its own package [valgrind-requests] for [Valgrind's Client
Request Mechanism][valgrind-client-req]. Gungraun's client requests have zero
overhead (relative to the "C" implementation of Valgrind) on many targets which
are also natively supported by Valgrind. In short, Gungraun provides a complete
and performant implementation of Valgrind Client Requests.

## Installation

[valgrind-requests] is a standalone package but is integrated and re-exported in
Gungraun under the `client_requests` module. Client requests are deactivated by
default but can be activated with the `act` feature.

```toml
[dev-dependencies]
gungraun = { version = "0.19.4", features = ["client_requests"] }
```

If you need the client requests in your production code, you don't want them to
do anything when not running under Valgrind with Gungraun benchmarks. You can
achieve this by adding Gungraun with the `stubs` feature to your runtime
dependencies and with the `act` feature to your `dev-dependencies`:

```toml
[dependencies]
gungraun = { version = "0.19.4", default-features = false, features = [
    "stubs"
] }

[dev-dependencies]
gungraun = { version = "0.19.4", features = ["act"] }
```

With just the `stubs` feature activated, the client requests compile down to
nothing and don't add any overhead to your production code. It simply provides
the "stubs", method signatures and macros without body. Only with the activated
`act` feature they will be actually executed.

When using Gungraun with client requests, the Valgrind header files must exist
in your standard include path (most of the time `/usr/include`). This is usually
the case if you've installed Valgrind with your distribution's package manager.
If not, you can point the `VALGRIND_REQUESTS_VALGRIND_INCLUDE` or
`VALGRIND_REQUESTS_<triple>_VALGRIND_INCLUDE` environment variables to the
include path. So, if the headers can be found in
`/home/foo/repo/valgrind/{valgrind.h, callgrind.h, ...}`, the correct include
path would be `VALGRIND_REQUESTS_VALGRIND_INCLUDE=/home/foo/repo` (not
`/home/foo/repo/valgrind`)

## Usage

Use them in your code for example like so:

```rust
# extern crate gungraun;
use gungraun::client_requests::callgrind;

# fn main() {
fn main() {
    // Start callgrind event counting if not already started earlier
    callgrind::start_instrumentation();

    // do something important

    // Switch event counting off
    callgrind::stop_instrumentation();
}
# }
```

## Library Benchmarks

In [library benchmarks](./benchmarks/library_benchmarks.md) you might need to
use [`EntryPoint::None`][EntryPoint] to make the client requests work as
expected:

```rust
# extern crate gungraun;
use gungraun::prelude::*;

use std::hint::black_box;

pub mod my_lib {
    use gungraun::client_requests::callgrind;

    #[inline(never)]
    fn bubble_sort(input: Vec<i32>) -> Vec<i32> {
        // The algorithm
#       input
    }

    pub fn pre_bubble_sort(input: Vec<i32>) -> Vec<i32> {
        println!("Doing something before the function call");
        callgrind::start_instrumentation();

        let result = bubble_sort(input);

        callgrind::stop_instrumentation();
        result
    }
}

#[library_benchmark]
#[bench::small(vec![3, 2, 1])]
#[bench::bigger(vec![5, 4, 3, 2, 1])]
fn bench_function(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::pre_bubble_sort(black_box(array)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_function);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

The default [`EntryPoint`][EntryPoint] sets the
[`--toggle-collect`][callgrind-arguments] to the benchmark function (here
`bench_function`) and `--collect-atstart=no`. So, `Callgrind` starts collecting
the events when entering the benchmark function, not the moment
`start_instrumentation` is called. This behaviour can be remedied with
`EntryPoint::None`:

```rust
# extern crate gungraun;
use gungraun::prelude::*;
use gungraun::{Callgrind, EntryPoint};
use std::hint::black_box;

pub mod my_lib {
    use gungraun::client_requests::callgrind;

    #[inline(never)]
    fn bubble_sort(input: Vec<i32>) -> Vec<i32> {
        // The algorithm
#       input
    }

    pub fn pre_bubble_sort(input: Vec<i32>) -> Vec<i32> {
        println!("Doing something before the function call");
        callgrind::start_instrumentation();

        let result = bubble_sort(input);

        callgrind::stop_instrumentation();
        result
    }
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--collect-atstart=no"])
            .entry_point(EntryPoint::None)
        )
)]
#[bench::small(vec![3, 2, 1])]
#[bench::bigger(vec![5, 4, 3, 2, 1])]
fn bench_function(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::pre_bubble_sort(black_box(array)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_function);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

When the standard toggle is switched off with `EntryPoint::None`, make sure
`--collect-atstart=no` is set in the Callgrind arguments, for example via
`Callgrind::with_args(["--collect-atstart=no"])` as shown above.

Please see the [`docs`][api-docs] for more details!

[api-docs]: https://docs.rs/valgrind-requests/latest/valgrind_requests/
[callgrind-arguments]:
    https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options
[EntryPoint]: https://docs.rs/gungraun/0.19.4/gungraun/enum.EntryPoint.html
[valgrind-client-req]:
    https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq
[valgrind-requests]: https://docs.rs/valgrind-requests/latest/valgrind_requests/
