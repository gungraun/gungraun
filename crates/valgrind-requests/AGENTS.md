# valgrind-requests

## Overview

`valgrind-requests` is a `no_std` crate providing idiomatic Rust bindings for
Valgrind's client-request mechanism. It uses inline assembly for
zero-indirection execution on supported platforms and compiles to zero-cost
no-ops in stubs-only builds.

## Structure

| Path                                                                   | Role                                                                                                                    |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`                                                           | Feature matrix: `act` (implies `stubs`), `stubs`, `alloc`, `std` (implies `alloc`). Default: `["act", "std"]`.          |
| `build.rs`                                                             | Bindgen wrapper around Valgrind headers; sets `client_requests_support` cfg; compiles C fallback when `act` + `native`. |
| `src/lib.rs`                                                           | `do_client_request!` and `is_def!` macros; feature-gated `valgrind_print*` macros/functions; `no_std` gate.             |
| `src/arch/mod.rs`                                                      | Platform router selecting inline-asm or C FFI fallback based on build-script cfg.                                       |
| `src/arch/{x86_64,x86,arm,aarch64,riscv64,s390x,powerpc,powerpc64}.rs` | Architecture-specific `asm!` implementations of `valgrind_do_client_request_expr`.                                      |
| `src/arch/native.rs`                                                   | C FFI fallback for platforms without Rust inline-asm support.                                                           |
| `src/bindings.rs`                                                      | `include!` of build-generated `OUT_DIR/bindings.rs` (request constants from headers).                                   |
| `src/native_bindings.rs`                                               | C FFI declarations for `valgrind_printf` and `valgrind_printf_backtrace`.                                               |
| `src/valgrind.rs`                                                      | Core requests from `valgrind.h`: `running_on_valgrind`, mempool/stack helpers, `monitor_command`, etc.                  |
| `src/{callgrind,cachegrind,memcheck,helgrind,drd,dhat}.rs`             | Tool-specific request modules mirroring their header files.                                                             |
| `src/error.rs`                                                         | `ClientRequestError` type (requires `alloc`).                                                                           |
| `../client-request-tests/`                                             | Cross-target integration tests and QEMU runners (outside this crate).                                                   |

## Conventions

- Core APIs are `no_std`; `std` implies `alloc`, but `act` does **not** imply
  `alloc`. Active no-allocation builds are fully supported.
- `stubs` is the minimum feature providing the public API surface with all
  requests compiled to no-ops.
- Tool modules strip the `VALGRIND_` or tool-prefix from function names (e.g.,
  `VR_RUNNING_ON_VALGRIND` becomes `valgrind::running_on_valgrind`).
- All client request functions are `#[inline(always)]`.
- Allocation-free printing uses `CStr` borrows (`valgrind_print`,
  `valgrind_print_backtrace`); formatting macros (`valgrind_printf!`,
  `valgrind_println!`) require `alloc`.
- `is_def!` checks a compile-time constant so unavailable requests panic with a
  version-aware message instead of silently failing.

## Anti-Patterns

- Do not require `alloc` for core `CStr`-based requests; the crate is designed
  to work without an allocator.
- Do not assume `std` is available in downstream code; keep new APIs
  `no_std`-compatible unless they explicitly need `alloc`/`std`.
- Do not make `act` imply `alloc`; keep the feature dependency graph exactly as
  `act -> stubs`, `std -> alloc`.
- Do not edit `build.rs` without checking cross-target behavior in
  `client-request-tests`; the build script is the primary source of platform
  support truth.

## Verification

Run `just build-hack-valgrind-requests` to exercise the full feature-power-set
build matrix and catch cfg or dependency errors early.
