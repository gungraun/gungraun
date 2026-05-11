# valgrind-requests

Idiomatic Rust bindings for Valgrind client requests, with zero-indirection
execution and zero-cost fallback.

## About

`valgrind-requests` is a standalone crate providing idiomatic Rust bindings for
[Valgrind's Client Request Mechanism][client-requests]. It is a retake and
rewrite from scratch of the abandoned [`valgrind_request`][valgrind_request].
This implementation is considered complete and covers all Valgrind tools with
client requests (Memcheck, Callgrind, Cachegrind, Helgrind, DRD, DHAT) on all
platforms which Valgrind supports.

This crate was formerly hardcoded into [Gungraun]. [Gungraun] still uses
`valgrind-requests` but this crate was extracted in the hope to be useful
outside of the [Gungraun] crate.

`valgrind-requests` uses inline assembly for zero-indirection execution on
supported platforms - avoiding the function call overhead of a C FFI layer. The
`stubs` feature compiles all client requests to no-ops with zero runtime cost,
making it safe to ship in production code without conditional compilation.

## Features

- **`act`** _(default)_: Enables actual execution of client requests when
  running under Valgrind. Implies `stubs`.
- **`stubs`**: Enables the client request definitions and build-time code
  generation, but all client requests compile to no-ops.

## Installation

```toml
[dependencies]
valgrind-requests = "1.0"
```

or `cargo add valgrind-requests`.

To use the zero-cost fallback for example if you want to use the client requests
for tests or benchmarks and need to make annotations in production code:

```toml
[dependencies]
valgrind-requests = { version = "1.0", default-features = false, features = ["stubs"] }

[dev-dependencies]
valgrind-requests = { version = "1.0" }
```

The stubs compile down to nothing and your production code is as performant as
without any annotations.

The client requests require the Valgrind header files. These are typically
installed by your distribution's package manager alongside Valgrind and
discovered automatically.

If the headers cannot be found, set the `VALGRIND_REQUESTS_VALGRIND_INCLUDE`
environment variable to the include path containing the `valgrind/` directory.
For cross-compilation, you can use the target-specific variant:

```shell
VALGRIND_REQUESTS_X86_64_UNKNOWN_LINUX_GNU_VALGRIND_INCLUDE=/path/to/include cargo build
```

## Quick start

```rust
use valgrind_requests::{valgrind, valgrind_println};

fn main() {
    if valgrind::running_on_valgrind() != 0 {
        // This'll print to the log file/logging output of Valgrind
        valgrind_println!("running under valgrind!").unwrap();
    } else {
        println!("not running under valgrind");
    }
}
```

```rust
use valgrind_requests::memcheck::{self, LeakCounts};

#[test]
fn test_memcheck() {
    // Some ffi code which could be responsible for a memory leak

    let LeakCounts {
        dubious,
        leaked,
        reachable,
        suppressed: _,
    } = memcheck::count_leaks();

    assert!(dubious == 0 && leaked == 0 && reachable == 0);
}
```

## Gungraun example

Measure only a specific code region with Callgrind

```rust
// my_lib.rs

use valgrind_requests::callgrind;

fn private_func() -> u64 {
    // heavy work

    return 42
}

pub fn some_func() -> u64 {
    // do something which isn't very interesting to benchmark

    // then only measure this code region
    callgrind::start_instrumentation();
    let r = private_func();
    println!("{r}");
    callgrind::stop_instrumentation();

    return r;
}
```

For example, in a [Gungraun] benchmark this could be used

```rust
use gungraun::prelude::*;
use gungraun::{Callgrind, EntryPoint};

use std::hint::black_box;

use my_lib::some_func;

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        // disable the instrumentation at benchmark start (We start manually)
        .tool(Callgrind::with_args(["instr-atstart=no"])
            // disable the default entry point which is the benchmark function
            .entry_point(EntryPoint::None)
        )
)]
fn bench_my_lib() -> u64 {
    black_box(some_func())
}

library_benchmark_group!(name = my_group, benchmarks = bench_my_lib);
main!(library_benchmark_groups = my_group);
```

## Modules

The client requests are organized into modules representing the source header
file:

- [`valgrind`] - core client requests from `valgrind.h`
- [`memcheck`] - `memcheck.h` - ([Memcheck: a memory error
  detector][memcheck-docs])
- [`callgrind`] - `callgrind.h` - [Callgrind: a call-graph generating cache and
  branch prediction profiler][callgrind-docs]
- [`cachegrind`] - `cachegrind.h` - ([Cachegrind: a high-precision tracing
  profiler][cachegrind-docs])
- [`helgrind`] - `helgrind.h`- [Helgrind: a thread error
  detector][helgrind-docs]
- [`drd`] - `drd.h` - [DRD: a thread error detector][drd-docs]
- [`dhat`] - `dhat.h` - [DHAT: a dynamic heap analysis tool][dhat-docs]

The [`valgrind_printf!`], [`valgrind_printf_unchecked!`], [`valgrind_println!`],
[`valgrind_println_unchecked!`], [`valgrind_printf_backtrace!`],
[`valgrind_printf_backtrace_unchecked!`], [`valgrind_println_backtrace!`], and
[`valgrind_println_backtrace_unchecked!`] macros live in the crate root.

## Platform support

If possible, client requests execute with zero indirection and the same overhead
as the original Valgrind C macros usable [even in high performance
code][client-requests]. On Valgrind-supported platforms for which
zero-indirection isn't implemented by us, a native C FFI binding is used which
introduces at least an additional frame on the stack and the costs for the
function call. That means all targets covered by Valgrind are also covered by
`valgrind-requests`. Targets not supported by Valgrind produce a compile error.

| Target               | Zero-indirection | Notes                                         |
| -------------------- | ---------------- | --------------------------------------------- |
| `x86_64/linux`       | yes              | except the x32 ABI                            |
| `x86_64/android`     | yes              | except the x32 ABI                            |
| `x86_64/freebsd`     | yes              | -                                             |
| `x86_64/macos`       | yes              | the versions supported by Valgrind            |
| `x86_64/windows+gnu` | yes              | -                                             |
| `x86_64/solaris`     | yes              | -                                             |
| `x86/linux`          | yes              | -                                             |
| `x86/android`        | yes              | -                                             |
| `x86/freebsd`        | yes              | -                                             |
| `x86/macos`          | yes              | the versions supported by Valgrind            |
| `x86/windows+gnu`    | yes              | -                                             |
| `x86/solaris`        | yes              | -                                             |
| `arm/linux`          | yes              | -                                             |
| `arm/android`        | yes              | -                                             |
| `aarch64/linux`      | yes              | -                                             |
| `aarch64/android`    | yes              | -                                             |
| `aarch64/freebsd`    | yes              | -                                             |
| `aarch64/macos`      | yes              | [LouisBrunner/valgrind-macos][valgrind-macos] |
| `riscv64/linux`      | yes              | -                                             |
| `s390x/linux`        | yes              | -                                             |
| `powerpc/linux`      | yes              | rust >= 1.95.0                                |
| `powerpc64/linux`    | yes              | rust >= 1.95.0                                |
| `powerpc64le/linux`  | yes              | rust >= 1.95.0                                |
| `mips32/linux`       | no               | no rust inline assembly available             |
| `mips64/linux`       | no               | no rust inline assembly available             |
| `nanomips/linux`     | no               | no zero-indirection planned                   |
| `x86/windows+msvc`   | no               | no zero-indirection planned                   |

To disable the native C FFI binding as fallback you can set the environment
variable `VALGRIND_REQUESTS_STRATEGY=strict` (possible values are: `strict`,
`fallback`)

## License

Licensed under Apache-2.0 or MIT, at your option.

[`callgrind`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/callgrind
[callgrind-docs]:
    https://valgrind.org/docs/manual/cl-manual.html#cl-manual.clientrequests
[`cachegrind`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/cachegrind
[cachegrind-docs]:
    https://valgrind.org/docs/manual/cg-manual.html#cg-manual.clientrequests
[client-requests]:
    https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq
[`dhat`]: https://docs.rs/valgrind-requests/latest/valgrind_requests/dhat
[dhat-docs]: https://valgrind.org/docs/manual/dh-manual.html
[`drd`]: https://docs.rs/valgrind-requests/latest/valgrind_requests/drd
[drd-docs]:
    https://valgrind.org/docs/manual/drd-manual.html#drd-manual.clientreqs
[Gungraun]: https://github.com/gungraun/gungraun
[`helgrind`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/helgrind
[helgrind-docs]:
    https://valgrind.org/docs/manual/hg-manual.html#hg-manual.client-requests
[`memcheck`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/memcheck
[memcheck-docs]:
    https://valgrind.org/docs/manual/mc-manual.html#mc-manual.clientreqs
[`valgrind`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/valgrind
[valgrind-macos]: https://github.com/LouisBrunner/valgrind-macos
[`valgrind_println!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_println.html
[`valgrind_println_backtrace!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_println_backtrace.html
[`valgrind_println_backtrace_unchecked!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_println_backtrace_unchecked.html
[`valgrind_println_unchecked!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_println_unchecked.html
[`valgrind_printf!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_printf.html
[`valgrind_printf_backtrace!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_printf_backtrace.html
[`valgrind_printf_backtrace_unchecked!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_printf_backtrace_unchecked.html
[`valgrind_printf_unchecked!`]:
    https://docs.rs/valgrind-requests/latest/valgrind_requests/macro.valgrind_printf_unchecked.html
[valgrind_request]: https://crates.io/crates/valgrind_request
