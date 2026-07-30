# Gungraun Macros Knowledge Base

## Overview

`gungraun-macros` is the proc-macro crate behind the public
`#[library_benchmark]`, `#[binary_benchmark]`, and `#[derive(IntoInner)]`
attributes. It parses attribute parameters, validates them against the annotated
function signature, and generates benchmark harness code: shim modules, run-mode
dispatchers, and `__BENCHES` constant arrays that the `gungraun` runtime
consumes via `library_benchmark_group!` / `binary_benchmark_group!`.

## Where To Look

| Task                           | Location                    | Notes                                                                                             |
| ------------------------------ | --------------------------- | ------------------------------------------------------------------------------------------------- |
| Public proc macro entry points | `src/lib.rs`                | `library_benchmark`, `binary_benchmark`, `into_inner`                                             |
| Library benchmark expansion    | `src/lib_bench.rs`          | `LibraryBenchmark`, `Bench`, `PerfRenderer` for shim/run-mode code generation                     |
| Binary benchmark expansion     | `src/bin_bench.rs`          | `BinaryBenchmark`, `Bench`, `AssistantRenderer` for setup/teardown glue                           |
| Shared parsing and validation  | `src/common.rs`             | `Args`, `BenchesArgs`, `Consts`, `BenchConfig`, `Setup`, `Teardown`, `File`, `Iter`               |
| `IntoInner` derive             | `src/derive_macros.rs`      | Builder tuple-struct `From` impls                                                                 |
| Constants                      | `src/defaults.rs`           | `MAX_BYTES_ARGS` for display string truncation                                                    |
| Macro diagnostic UI tests      | `../gungraun/tests/ui/`     | Compile-fail (`test_*_invalid*.rs`) and pass (`test_*_valid*.rs`) cases run via `trybuild`        |
| Runtime macro consumers        | `../gungraun/src/macros.rs` | `library_benchmark_attribute!`, `binary_benchmark_attribute!` read `__BENCHES` and `__get_config` |

## Conventions

- Uses `syn` for AST parsing, `quote` for token generation, and
  `proc-macro-error3` for span-aware diagnostics.
- Attribute parameters are parsed as `MetaNameValue` pairs where possible, with
  fallback to positional args for `bench`/`benches`.
- Library benchmark expansion places the original function in the constant-named
  `__gungraun_wrapper_mod`. This gives Callgrind's `DEFAULT_TOGGLE` a stable
  path segment even when user benchmarks are nested in modules, or the compiler
  inlines or merges identical functions despite `#[inline(never)]`.
- Per-benchmark `__gungraun_wrapper_id_mod*` shims are separate execution and
  DHAT fallback frames. Their bodies contain only benchmark-call plumbing; keep
  setup, teardown, and other framework allocation outside them so DHAT does not
  attribute orchestration heap usage to the benchmark. Binary benchmark
  expansion does not generate `__gungraun_wrapper_mod`.
- Benchmark metadata is exposed through a `pub const __BENCHES` slice of
  internal structs (`InternalMacroLibBench`, `InternalMacroBinBench`).
- `CargoMetadata` is fetched at macro-expansion time to resolve relative `file`
  paths against the workspace root.
- Library benchmarks use function-pointer `setup`/`teardown`; binary benchmarks
  use expressions (with a special case when the expression is a function path,
  routing `args` into it).

## Anti-Patterns

- Do not duplicate parameter validation between `lib_bench.rs` and
  `bin_bench.rs`. Shared logic belongs in `common.rs`.
- Do not change the `__BENCHES` struct layout or `__get_config` signature
  without updating the consumers in `crates/gungraun/src/macros.rs`.
- Do not move setup, teardown, or framework allocation into
  `__gungraun_wrapper_id_mod*` shims; doing so contaminates DHAT measurements.
- Do not add new attribute parameters without corresponding UI tests in
  `../gungraun/tests/ui/`.
