# Gungraun Public API Crate

**Scope:** User-facing benchmark facade, macro-generated harness transport, and
feature-gated re-exports. The root `AGENTS.md` covers workspace-wide build,
test, and style rules.

## Where To Look

| Task               | Location                                                       | Notes                                                                            |
| ------------------ | -------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Public facade      | `src/lib.rs`                                                   | Prelude, feature-gated modules, macro and enum re-exports                        |
| Library config     | `src/lib_bench.rs`                                             | `LibraryBenchmarkConfig` builder wrapping `__internal` types                     |
| Binary config      | `src/bin_bench.rs`                                             | `BinaryBenchmarkConfig`, `Command`, `Bench`, `BinaryBenchmark`, `Delay` builders |
| Shared tools       | `src/common.rs`                                                | `Callgrind`, `Dhat`, `Sandbox`, `OutputFormat`, `PerfRunMode`, etc.              |
| Runner transport   | `src/__internal/mod.rs`                                        | `Runner` spawns `gungraun-runner`; macro metadata structs; internal re-exports   |
| Macro glue         | `src/macros.rs`                                                | `main!`, `library_benchmark_group!`, `binary_benchmark_group!` re-exports        |
| Compile-fail tests | `tests/ui_tests.rs`                                            | `trybuild`-based UI tests; requires `__ui_tests` feature                         |
| UI fixtures        | `tests/ui/lib_bench/`, `tests/ui/bin_bench/`, `tests/ui/both/` | Macro validation cases                                                           |
| Macro tests        | `tests/macros/`                                                | Expansion and integration tests                                                  |

## Conventions

- `default` feature enables the full benchmark API. `perf` or `perf_stubs` is
  mandatory. `stubs` exposes `client_requests` (Valgrind client requests). `act`
  enables active Valgrind instrumentation.
- Public structs wrap `gungraun`'s internal runner types; enums from
  `gungraun_runner::api` are re-exported directly at the `gungraun` crate root.
- `__internal::Runner` encodes benchmark metadata via `bincode`, then spawns
  `gungraun-runner` over stdin. This is the sole bridge between macro-generated
  harnesses and the runner.
- The runner re-executes the benchmark harness to run library benchmarks and
  binary benchmark setup/teardown. Harness startup must support both metadata
  collection and runner-directed execution.
- `prelude` contains the most common macros and config structs. Use
  `#[doc(no_inline)]` on re-exports so docs show the original source location.
- Feature-gate every public item. `__internal` is `#[doc(hidden)]` and must
  never appear in user-facing docs.

## Anti-Patterns

- Do not re-export `gungraun-runner` structs directly in `lib.rs`. Always wrap
  them with a builder in `lib_bench.rs` or `bin_bench.rs`.
- Do not use `__internal` types in public function signatures or documentation
  examples.
- Do not bypass the builder pattern and construct internal runner types
  manually. The internal API is not stable.
- Do not add public items without a corresponding feature gate. The crate is
  heavily feature-sensitive because downstream users may only enable `stubs` or
  `perf`.
