# Gungraun Project Knowledge Base

Load `.opencode/AGENTS.md` too when present; it contains user-local authoring
and search rules.

## Overview

Gungraun is a Rust 2024 workspace for deterministic Valgrind-based library and
binary benchmarking. Public macros and runtime metadata feed a separate runner,
which executes tools, interprets metrics, and emits summaries.

## Structure

```text
gungraun/
|- gungraun/                 # Public benchmark API and runtime transport
|- gungraun-macros/          # Attribute and main proc-macro expansion
|- gungraun-runner/          # Executable orchestration and metric processing
|- gungraun-common/          # Small shared protocol primitives
|- gungraun-summary/         # Versioned, feature-gated summary schema
|- valgrind-requests/        # no_std Valgrind client-request API
|- benchmark-tests/          # End-to-end benchmark harness and fixtures
|- client-request-tests/     # Native/cross-architecture request tests
|- docs/                     # mdBook source and generated schema references
|- scripts/                  # Release and repository maintenance helpers
|- Justfile                  # Canonical developer and CI commands
`- Cargo.toml                # Workspace members, versions, and dependencies
```

## Where To Look

| Task                  | Location                                                       | Notes                                              |
| --------------------- | -------------------------------------------------------------- | -------------------------------------------------- |
| Public benchmark API  | `gungraun/src/`                                                | Prelude, benchmark groups, config, runtime handoff |
| Attribute expansion   | `gungraun-macros/src/`                                         | Parsing, validation, generated runner glue         |
| CLI startup           | `gungraun-runner/src/main.rs`                                  | Warning setup and top-level execution              |
| Runner orchestration  | `gungraun-runner/src/runner/`                                  | Bench selection, execution, sandboxing             |
| Tool commands         | `gungraun-runner/src/runner/tool/`                             | Valgrind command/config/path/run lifecycle         |
| Metrics and summaries | `gungraun-runner/src/metrics/`, `gungraun-runner/src/summary/` | Keep model and processing roles distinct           |
| Shared protocol       | `gungraun-common/src/`                                         | Exit and command-line transport types              |
| Summary API/schema    | `gungraun-summary/src/`, `gungraun-summary/schemas/`           | Versioned public format                            |
| Client requests       | `valgrind-requests/src/`                                       | Core API, tool modules, arch assembly              |
| System-test harness   | `benchmark-tests/src/bench.rs`                                 | Runs fixtures and compares structured output       |
| Benchmark cases       | `benchmark-tests/benches/`, `benchmark-tests/tests/`           | Inputs plus `.conf.yml` expectations               |
| Cross-target tests    | `client-request-tests/`                                        | QEMU/native request execution                      |
| Build recipes         | `Justfile`                                                     | Prefer recipes over direct tool invocations        |
| CI matrix             | `.github/workflows/`                                           | MSRV, platforms, formatting, tests, release        |

## Code Map

| Symbol               | Type          | Location                           | Role                                             |
| -------------------- | ------------- | ---------------------------------- | ------------------------------------------------ |
| `Runner`             | runtime API   | `gungraun/src/__internal/mod.rs`   | Transfers macro-generated benchmark metadata     |
| `library_benchmark`  | proc macro    | `gungraun-macros/src/lib.rs`       | Expands library benchmark declarations           |
| `binary_benchmark`   | proc macro    | `gungraun-macros/src/lib.rs`       | Expands binary benchmark declarations            |
| `main`               | entry point   | `gungraun-runner/src/main.rs`      | Starts runner and prints deferred warnings       |
| `Tool`               | runner model  | `gungraun-runner/src/runner/tool/` | Configures and invokes Valgrind tools            |
| `BenchmarkRunner`    | test harness  | `benchmark-tests/src/bench.rs`     | Executes benchmark fixtures and validates output |
| `do_client_request!` | request macro | `valgrind-requests/src/lib.rs`     | Encodes architecture-specific Valgrind requests  |
| `v6`                 | schema module | `gungraun-summary/src/lib.rs`      | Current public summary representation            |

## Conventions

- Rust edition 2024; workspace MSRV is 1.85.1.
- Follow `rustfmt.toml`: Unix newlines, 100-character comments,
  module-granularity imports, and `StdExternalCrate` grouping.
- Import order is standard library, external crates, then workspace modules;
  sort imports and module declarations alphabetically.
- Co-locate unit tests in `mod tests`; use crate `tests/` for integration tests.
- Runner integration behavior is primarily exercised through `benchmark-tests`.
- Public items exposed by feature-gated `api`, `summary`, or `schema` modules
  are semver-sensitive. Most other `gungraun-runner` visibility is
  workspace-internal.
- Use typed library errors. Runner user-facing errors flow through
  `gungraun-runner/src/error.rs`; reserve `JobError(anyhow::Error)` for internal
  jobs.

## Anti-Patterns

- Do not invoke direct `cargo` commands when an equivalent `just` recipe exists.
- Do not edit `Cargo.lock` manually or add dependencies without approval.
- Do not update expected benchmark output before checking that the behavior
  change is intentional.
- Do not collapse runner metric data models and processing logic into one layer.
- Do not make `valgrind-requests` feature `act` imply `alloc`; active
  no-allocation builds are supported.
- Do not require allocation for core `CStr`-based client requests.
- Do not remove `TODO`, `FIXME`, `WARNING`, or `HACK` markers without resolving
  and testing the underlying issue.

## Commands

```bash
just fmt                          # Rust formatting; nightly toolchain
just fmt-prettier                 # JSON, YAML, and Markdown formatting
just check-fmt-all                # All formatting checks
just lint                         # Stable Clippy
just test <package_name>          # One workspace package
just test-all                     # Main workspace suite
just test-ui                      # Compile-fail tests at MSRV
just test-doc                     # Documentation tests
just full-bench-test <bench_name> # One benchmark system test
just full-bench-test-all          # All benchmark system tests
just build-hack-valgrind-requests # Feature-power-set request builds
```

## Notes

- `just test-all` excludes client-request tests and benchmark system tests; run
  their dedicated recipes when changing those domains.
- `stubs` is the minimum `valgrind-requests` API feature. `act` implies `stubs`;
  `alloc` only enables allocation-backed conveniences.
- Run `just fmt-prettier` after editing any `AGENTS.md` file.
- Schema generation, benchmark expectation overwrite, cross-target request
  tests, mdBook, and release recipes are documented in `Justfile`.
