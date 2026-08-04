# Benchmark Tests Domain Knowledge

## Overview

End-to-end system tests for Gungraun. Executes real benchmark binaries under
Valgrind, captures output, and compares against checked-in expectations. Runs
outside `just test-all`; prefer `just system-test <bench>` for targeted
verification.

## Structure

```text
crates/benchmark-tests/
|- src/bench.rs          # Harness binary: discovers, runs, asserts
|- src/lib.rs            # Shared helpers: bubble_sort, fibonacci, primes, env
|- src/helper/           # Fake binaries: echo, cat, sort, exit-with, leak-memory, ...
|- benches/              # Benchmark cases grouped by domain
|  |- test_lib_bench/*/  # Library benchmark cases
|  |- test_bin_bench/*/  # Binary benchmark cases
|  |- guide/             # Documentation examples also serving as tests
|- tests/                # Unit tests for parsers and internal components
|- fixtures/             # Static files for file-parameter benchmarks
```

## Where To Look

| Task                    | Location                                            | Notes                                                                                 |
| ----------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Harness orchestration   | `src/bench.rs`                                      | `SystemTestRunner` loads `.conf.yml`, runs `cargo bench`, filters and compares output |
| Benchmark case source   | `benches/test_*/<name>/<name>.rs`                   | Library or binary benchmark under test                                                |
| Case configuration      | `benches/test_*/<name>/<name>.conf.yml`             | Groups, runs, args, envs, expectations                                                |
| Expected stdout/stderr  | `benches/test_*/<name>/expected_stdout*`            | Filtered before comparison (numbers normalized)                                       |
| Expected file manifests | `benches/test_*/<name>/expected_files*.yml`         | Per-group/function/id file lists                                                      |
| Helper binaries         | `src/helper/*.rs`                                   | Small tools consumed by binary benchmarks                                             |
| Parser tests            | `tests/test_callgrind/`, `test_dhat/`, `test_tool/` | Unit tests for runner output parsers                                                  |
| Just recipes            | `../../Justfile`                                    | Targeted: `system-test`, `system-test-overwrite`; quick: `bench`                      |

## Conventions

- **Harness/case relationship**: `src/bench.rs` discovers all
  `benches/**/*.conf.yml`, builds `gungraun-runner`, then runs each configured
  group. A case is a `.conf.yml` plus its `.rs` source and expected fixtures.
- **System-test commands**: Use `just system-test <bench>` to verify one
  benchmark against expectations. Use `just bench <bench>` only for a quick run
  without expectation verification. `just system-test-all` verifies the full
  suite and takes approximately 20-30 minutes.
- **`.conf.yml` header**: System test configuration files must include a test
  case description comment block at the top with the following fields:

| Field             | Required | Purpose                                                                                      |
| ----------------- | -------- | -------------------------------------------------------------------------------------------- |
| Test Case         | Yes      | Unique identifier for the test usually the file name of the test without the `.rs` suffix    |
| Description       | Yes      | Brief explanation of what is being tested                                                    |
| Test Steps        | Yes      | Numbered sequence of actions performed during the test                                       |
| Test Inputs       | Yes      | Specific inputs, configurations, or scenarios being tested                                   |
| Expected Outcomes | Yes      | Clear, measurable expectations for correct execution                                         |
| Preconditions     | No       | Conditions that must be met before test execution                                            |
| Postconditions    | No       | Expected state after test execution (only include if it adds value beyond Expected Outcomes) |
| Test Environment  | No       | Specific environment requirements (e.g., tool versions)                                      |

- **Groups and runs**: A configuration has `groups`, each with `runs`. Runs
  specify `args`, `cargo_args`, `envs`, `setup`/`teardown`, and `expected`.
  Groups can be gated by `runs_on` target triple or `rust_version`.
- **Output comparison**: The harness filters stdout/stderr to normalize PIDs,
  absolute paths, commands with random hashes, percentages, and timing details.
  Coverage runs get additional normalization.
- **File assertions**: `expected.files` points to a YAML manifest listing
  `group`, `function`, `id`, and required files per benchmark output directory.
  `summary.json` is validated against the JSON schema.
- **Expected-output update**: Prefer `just system-test-overwrite <bench>` or
  `BENCH_OVERWRITE=yes just system-test <bench>` when only one benchmark is
  affected. Use `just system-test-all-overwrite` only when all fixtures need
  regeneration. Validate the behavior change before committing updates.
- **Flaky runs**: A run can set `flaky: N` to allow N retries on assertion
  failure.
- **Templated benchmarks**: When `template` is set in the config, the harness
  renders a Jinja template into `test_bench_template.rs` before running.

## Anti-Patterns

- Do not update expected output before confirming the behavior change is
  intentional.
- Do not add a `Notes` section to `.conf.yml` headers; use only the fields
  listed above.
- Always ask and wait for explicit user approval before running
  `just system-test-all` or `just system-test-all-overwrite`.
- Do not run benchmark system tests via `cargo test`; they require the `bench`
  binary harness.
