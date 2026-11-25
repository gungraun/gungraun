<!-- spell-checker: ignore DCMAKE -->

# Standalone Binary Benchmarks

Use a compiled executable and a TOML configuration to run Callgrind benchmarks
without a consumer Rust harness or Cargo project:

```shell
gungraun-runner run --config perf-guard.toml
gungraun-runner check --config perf-guard.toml
```

These commands require a runner build containing this feature. The published
`0.20.0` runner does not contain them. The existing Rust binary and library
benchmark APIs remain available with their existing behavior.

## Build, capture, and enforce

The runner executes an already-built program. Keep CMake, Make, Cargo, or your
other build tool responsible for compilation. The repository's
`examples/standalone` directory provides a complete C/CMake example:

```shell
cd examples/standalone
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
gungraun-runner run --config perf-guard.toml
gungraun-runner check --config perf-guard.toml
```

`run` is capture mode. Missing caps are allowed and the uncapped case IDs are
printed. Any supplied caps are still checked, but a successful capture is
explicitly identified as a capture, not an enforced guard. Use `check` for CI:
it requires all declared cases to have positive instruction caps before any case
executes. Neither command modifies the configuration or approves current
measurements as new limits.

## Version 1 configuration

```toml
schema_version = 1
output_dir = "target/gungraun"

[defaults]
program = "build/workload"
cwd = "."
expected_exit = 0

[defaults.env]
LC_ALL = "C"

[defaults.callgrind]
entry_point = "gungraun_example_region"

[[cases]]
id = "Sum.32"
args = ["32"]
[cases.callgrind.hard_limits]
Ir = 100_000

[[cases]]
id = "Sum.1000"
args = ["1000"]
[cases.callgrind.hard_limits]
Ir = 100_000
```

These are demonstration caps for the example workload. Review measurements from
your own binary, compiler settings, and toolchain before choosing its limits.

| Field                      | Meaning and default                                                                                                                                                                        |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `schema_version`           | Required integer; version `1` is supported.                                                                                                                                                |
| `output_dir`               | Optional output root; defaults to `target/gungraun`.                                                                                                                                       |
| `cases`                    | Required nonempty array of cases.                                                                                                                                                          |
| `cases.id`                 | Required unique ID: 1–100 ASCII letters, digits, `.`, `_`, or `-`, starting with a letter or digit. IDs must also differ when ignoring ASCII case, to prevent output-directory collisions. |
| `program`                  | Required after applying defaults. A path to the executable, or a bare name searched in PATH.                                                                                               |
| `args`                     | Literal string array; defaults to `[]`. A case array replaces the default array, including an explicit empty array.                                                                        |
| `cwd`                      | Working directory; defaults to the configuration directory.                                                                                                                                |
| `expected_exit`            | Integer exit code from 0 to 255; defaults to `0`. Signals are always execution failures in version 1.                                                                                      |
| `env`                      | String-valued environment table. Per-case keys override defaults.                                                                                                                          |
| `callgrind.entry_point`    | Optional nonempty function name/pattern mapped to the existing custom entry point. Without it, normal whole-program collection applies.                                                    |
| `callgrind.hard_limits.Ir` | Optional positive integer in capture mode; required in check mode. Absolute instruction count for the complete measured region.                                                            |

Except for `id`, the per-case settings in this table can also appear under
`defaults`. Overrides merge individual fields: adding a case's hard limit does
not discard the default entry point. Setting an entry point does not discard an
inherited cap. Environment tables merge by key; argument arrays replace.

Unknown fields at every table level, invalid schema versions, duplicate IDs,
empty configurations, zero/negative/non-integer caps, and invalid environment
entries are rejected. `Ir` is the only configurable hard-limit event in version
1; other event names, including otherwise valid Callgrind metrics, are rejected.

### Paths and literal values

The configuration path is resolved, including symlinks, before its parent
directory becomes the path root. An executable containing a path separator,
`cwd`, and `output_dir` are relative to that root. Absolute paths stay absolute.
The executable path is independent of `cwd`. Invocation from another directory
therefore works without editing the manifest:

```shell
gungraun-runner check --config /project/perf/perf-guard.toml
```

A bare program name such as `workload` uses the effective manifest `PATH` if one
is declared, otherwise the invocation's runtime `PATH`. Relative PATH entries
are anchored to the configuration directory. Use `./workload` for an executable
beside the configuration. A missing bare name fails configuration loading; a
missing explicit path fails through the ordinary execution flow.

Arguments and environment values stay literal. `${NAME}`, `$HOME`, `*`, and
shell syntax are not expanded. No shell, template engine, or generated script is
involved. NUL bytes are rejected; environment keys must also be nonempty and
must not contain `=`.

The normal runner environment-clearing behavior is enabled. Inherited
`LD_PRELOAD` and `LD_LIBRARY_PATH` are preserved by that existing behavior;
manifest environment entries are then applied. Other variables needed by the
workload must be declared explicitly. Standalone execution does not use a
temporary fixture sandbox: `cwd` is the actual working directory.

### Measurement boundary and total

The named entry point uses the existing Callgrind toggle mechanism, including
its supported `*` and `?` patterns. Keep the target symbol present and prevent
the boundary function from being inlined. A C++ `extern "C"` function provides a
straightforward symbol name. Put fixture preparation before the function and
correctness assertions after it. Existing Callgrind client requests remain part
of the executable; this format adds no instrumentation protocol.

The limit applies to the existing runner's **current profile total** across the
process/part/thread records Callgrind produced. It does not select one thread,
compare unrelated cases together, or divide by a workload's operation count. For
example, a fixture containing 32 operations still has one whole-region `Ir`
count. Existing multi-process/thread collection and aggregation semantics apply;
a custom entry point must actually enable collection in each intended execution
context. See [multi-threaded and multi-process measurement] for those boundary
semantics.

The existing absolute-limit rule is `measured Ir > cap`: **equality passes**. A
missing or zero current `Ir` is an error even if it is numerically below the
cap. A misspelled or unexecuted entry point therefore cannot pass the guard.

## CLI and failure contract

Both subcommands accept these options:

| Option                                       | Behavior                                                                                        |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `--config PATH`                              | Required manifest path, relative to the invocation directory.                                   |
| `--output-dir PATH`                          | Overrides the manifest output root, relative to the resolved configuration directory.           |
| `--output-format default\|json\|pretty-json` | Selects the native terminal report format. Native per-case JSON summaries are always requested. |
| `--valgrind-bin PATH`                        | Explicit Valgrind path, relative to the invocation directory.                                   |
| `--allow-aslr`                               | Allows ASLR, useful in containers where disabling it is forbidden.                              |

`run --help` and `check --help` describe this interface. Filters, baseline
loads/saves, test-only execution, arbitrary tool flags, alternate profilers,
parallelism settings, and harness environment overrides are not part of this
interface. Unsupported CLI options fail; inherited `GUNGRAUN_FILTER`,
`GUNGRAUN_TEST`, limit overrides, and similar harness controls cannot weaken the
standalone gate. Logging and color controls remain available.

All cases in the manifest are required. A complete current result is needed for
each ID. Missing cases, duplicate results, invalid profiler output, a missing
binary, an unexpected exit, a signal, and profiler failures produce a nonzero
aggregate status. Cases run serially. Exceeded caps allow subsequent independent
cases to run; execution errors retain the runner's fail-fast behavior. An
expected nonzero `expected_exit` is accepted deliberately, so leave it at `0`
for fixtures whose assertions report failure with a nonzero exit.

| Exit code | Meaning                                                                   |
| --------- | ------------------------------------------------------------------------- |
| `0`       | Completed capture for `run`, or complete enforced guard pass for `check`. |
| `1`       | Configuration, execution, or current-measurement failure.                 |
| `2`       | Invalid command-line arguments.                                           |
| `3`       | One or more supplied absolute limits were exceeded.                       |

Status messages identifying capture or enforcement go to stderr, so JSON
reporting on stdout keeps its native format. Only `check` reports
`Guard passed`.

### Failure artifacts and historical output

The default output tree is `target/gungraun/standalone/cases/CASE_ID`. Native
profiles, tool logs, and `summary.json` files remain available for diagnosis. A
summary is only produced for a measurement the runner could parse; a failed
launch cannot produce a successful measurement summary. Retained partial output
is evidence of a failure, not a completed guard.

Before execution, old summaries for declared cases are removed. Previous native
profiles may remain as historical comparison data, but only typed results
returned by the current execution can satisfy the gate. A previous successful
run therefore cannot turn a failed or absent current measurement into success.
The configuration file and reviewed caps are never rewritten.

Configuration validation happens before that cleanup. If the manifest itself is
invalid, existing files are left as historical evidence; they do not change the
failing command's status.

## Installation and CI

During development, install the runner from a checkout containing this feature.
From the repository root:

```shell
cargo install --path crates/gungraun-runner --locked
```

This tool installation requires Rust and Cargo. Running the installed standalone
runner does not. It does not compile consumer Rust code or invoke Cargo
metadata. Target compilation and runner installation are separate costs; no
cold-CI speedup is implied by eliminating the consumer harness.

Once a release containing this feature exists, use the [runner installation
instructions] to install a prebuilt binary. Pin the version and platform and
verify the archive's checksum. Add the runner to `PATH` and install Valgrind on
a supported platform. Existing `0.20.0` binaries do not provide these commands.

A non-Rust consumer using `setup-gungraun` must use an explicit version instead
of automatic detection from `Cargo.toml`. For development builds, install the
runner separately and set `runner-strategy: none` when using the action only for
Valgrind.

With the pinned runner and Valgrind already installed, a CI job can build the
target, enforce the manifest, and retain evidence even on failure:

```yaml
- name: Build workloads
  run: |
      cmake -S perf -B perf/build -DCMAKE_BUILD_TYPE=Release
      cmake --build perf/build
- name: Enforce reviewed instruction caps
  run: gungraun-runner check --config perf/perf-guard.toml
- name: Preserve profiles and summaries
  if: always()
  uses: actions/upload-artifact@v7
  with:
      name: instruction-guard
      path: perf/target/gungraun
```

## Migrating an external binary harness

Move each executable command and its reviewed absolute instruction cap into a
case, keeping shared paths, environment, and entry points under `defaults`.
Replace the harness invocation with `gungraun-runner check --config PATH`. Rust
registration files, Cargo files, and build steps used exclusively for that
harness can then be removed.

Keep the native workload build, correctness assertions, reference measurements,
and separate throughput/latency measurements. Preserve old Rust-harness profiles
separately: version 1 does not import them. Adding a case extends the required
set; a strict run with a missing cap fails rather than accepting partial
coverage.

[multi-threaded and multi-process measurement]:
    ../library_benchmarks/threads_and_subprocesses.md
[runner installation instructions]: ../../installation/gungraun.md
