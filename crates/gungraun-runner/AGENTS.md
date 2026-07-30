# Gungraun Runner Internals

**Scope:** `gungraun-runner` crate — orchestration, tool execution, and summary
emission.

## Overview

The runner is spawned as a subprocess by the `gungraun` harness. It receives
benchmark metadata over stdin, configures Valgrind tools (or Linux perf),
executes benchmarks, parses tool output, and emits terminal summaries and JSON
summary files.

## Structure

```text
crates/gungraun-runner/src/
|- main.rs              # Entry point: logging, color setup, deferred warnings
|- runner/
|  |- run.rs            # Top-level orchestration: CLI parse, version check, dispatch
|  |- lib_bench.rs      # Library benchmark execution loop
|  |- bin_bench.rs      # Binary benchmark execution loop
|  |- tasks.rs          # Thread-pool job scheduling and process lifecycle
|  |- tool/             # Tool config, command building, path management, parsing
|  |- callgrind/        # Callgrind-specific parser, model, regression
|  |- cachegrind/       # Cachegrind-specific parser, model, regression
|  |- dhat/             # DHAT-specific parser, model, regression
|  |- perf/             # Perf-specific parser, model, regression
|  |- common.rs         # Shared types: Config, ModulePath, Sandbox
|  |- args.rs           # CLI argument parsing
|  |- format.rs         # Terminal output formatting
|  |- meta.rs           # Benchmark metadata handling
|- metrics/             # Metric values and comparison logic
|- summary/             # Summary data model and processing
|- error.rs             # User-facing error enum
|- api.rs               # Serializable data contract shared with the harness
```

## Where To Look

| Task                   | Location                                     | Notes                                                                                              |
| ---------------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Runner startup         | `main.rs`, `runner/run.rs`                   | `main` sets up logging; `run::run` parses CLI, checks versions, receives config, dispatches        |
| Benchmark execution    | `runner/lib_bench.rs`, `runner/bin_bench.rs` | Kind-specific run loops; both delegate to `tasks.rs` for job scheduling                            |
| Tool command lifecycle | `runner/tool/run.rs`                         | Builds `std::process::Command`, spawns, captures output, validates exit status                     |
| Tool configuration     | `runner/tool/config.rs`                      | `ToolConfig` maps user config to concrete tool arguments                                           |
| Output path management | `runner/tool/path.rs`                        | `ToolOutputPath` handles baseline naming and directory layout                                      |
| Metric model           | `metrics/model.rs`                           | `Metric`, `MetricsSummary`, `AnnotatedMetric` — pure data                                          |
| Metric processing      | `metrics/logic.rs`                           | Diff calculation, aggregation, regression threshold checks                                         |
| Summary model          | `summary/model.rs`                           | `BenchmarkSummary`, `ToolMetricSummary` — serializable schema                                      |
| Summary processing     | `summary/logic.rs`                           | Building summaries from parsed tool output                                                         |
| Error handling         | `error.rs`                                   | `Error` enum for user-facing messages; `JobError(anyhow::Error)` for internal thread-pool failures |
| Integration tests      | `../benchmark-tests/tests/`                  | Runner behavior tested via fixture benchmarks and `.conf.yml` expectations                         |

## Conventions

- `runner` is the default feature and enables the full execution runtime. Most
  items it exposes are workspace-internal, not end-user API.
- `api` and `api.rs` define the serializable, version-coupled harness↔runner
  contract. Enums re-exported by `gungraun` are user-facing: give every enum and
  variant comprehensive rustdoc. Keep runner-only transport types internal.
- `summary` exposes API, metric-model, and summary-model types without runner
  orchestration. for example `gungraun-summary` consumes this feature. `schema`
  extends `summary` with schema generation support.
- Keep both metric and summary data models separate from their runner-only
  `logic.rs` modules. Only the models are available through `summary`/`schema`.
- User-facing errors live in `error.rs`; internal job failures are wrapped in
  `Error::JobError(anyhow::Error)`.
- Tool-specific parsers and regression configs live in their own
  `runner/<tool>/` submodules.
- `runner/tool/run.rs` is the single place that spawns external processes.

## Anti-Patterns

- Do not collapse metric model and processing into one file.
- Do not spawn external processes outside `runner/tool/run.rs`.
- Do not treat all `pub` items as public API; most are workspace-internal.
