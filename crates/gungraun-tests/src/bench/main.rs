//! End-to-end system-test harness for the gungraun benchmarking pipeline.
//!
//! This binary crate is the entry point invoked by `just system-test <bench>`. It does not call
//! into the gungraun libraries directly; instead it wraps a `cargo bench --package gungraun-tests
//! --bench <name>` invocation and treats the real benchmark pipeline - the `gungraun` public API,
//! the `gungraun-macros` attribute expansion (`library_benchmark` / `binary_benchmark`), the
//! `gungraun-runner` executor, and Valgrind/Perf - as the system under test, comparing its captured
//! output against checked-in expectations.
//!
//! # Why this exists
//!
//! The three production crates must be exercised together against real Valgrind/Perf runs, because
//! unit tests cannot spawn the runner, observe summary files, or compare captured stdout/stderr. A
//! change in any one crate is caught here against the documented usage surface.
//!
//! # Why it is shaped like `cargo bench`
//!
//! The harness reuses `cargo bench` as its execution substrate rather than inventing a completely
//! new runner. Each system-test case is a real `bench` target under `benches/`, paired with a
//! `.conf.yml` that declares its groups, runs, args, envs, and expectations. The cases are
//! therefore identical to what real users write, so the test surface IS the documented usage
//! surface.
//!
//! Several mechanisms adapt a bare `cargo bench` call into a deterministic, schema-validated system
//! test. This list gives an introduction and overview of these mechanisms but does not claim to
//! be complete:
//!
//! - **Runner indirection**: `GUNGRAUN_RUNNER` points each bench binary at the freshly built
//!   `gungraun-runner`, so the pipeline under test is always the one in this checkout, not a stale
//!   install; see [`runner`].
//! - **Declarative cases**: a `.conf.yml` per case drives discovery, grouping, and expectations;
//!   see [`config`]. This file is the main requirement for a system test to be recognized and run
//!   by this wrapper.
//! - **Partitioning and resume**: `--partition=x/y` shards the suite and `--continue` resumes from
//!   the `gungraun-tests.continue` marker so a failed shard re-runs without repeating green cases;
//!   see [`runner`].
//! - **Output normalization**: PIDs, absolute paths, per-build command hashes, percentages, metric
//!   values, timings, ... are scrubbed before comparison so captures stay stable across hosts; see
//!   [`filter`].
//! - **Coverage awareness**: under `CARGO_LLVM_COV=1` the instrumentation changes the machine code
//!   and therefore DHAT's metrics, so coverage runs take a separate comparison path; see [`filter`]
//!   and [`runner`].
//! - **Fixture regeneration**: `BENCH_OVERWRITE=yes` flips the comparison from asserting to
//!   rewriting the expected fixtures, so regenerating output after an intentional change uses the
//!   same code path; see [`mod@assert`]. Manual editing of fixtures is seldom necessary but all
//!   fixture changes need to be reviewed as if they were written manually.
//! - **Templating**: a case may declare a Jinja `template` rendered into a throwaway
//!   `test_bench_template` target so one source spawns many parameterized runs; see [`runner`].
//! - **Flaky tolerance**: a run may set `flaky: N` to retry on assertion failure without masking a
//!   genuinely broken case; see [`config`].
//! - **Schema validation**: every generated `summary.json` is validated against the versioned
//!   summary schema, and file manifests against their YAML; see [`expected_files`].
//! - **Assertion engine**: exit codes, stdout/stderr, file manifests, and non-zero metric checks
//!   are orchestrated per run; see [`mod@assert`].
//! - **Shared IO**: YAML/JSON deserialization and informational logging are centralized; see
//!   [`io`].
//!
//! `main` itself is only the CLI front end: it parses positional bench names plus `--filter`,
//! `--partition`, and `--continue`, then hands control to [`runner::SystemTestRunner`].

mod assert;
mod config;
mod expected_files;
mod filter;
mod io;
mod runner;

use anyhow::{Context, Result, anyhow, bail};
use config::Partition;
use runner::SystemTestRunner;

fn main() -> Result<()> {
    // The cli args:
    // positional arguments
    let mut benches = Vec::default();
    // --filter=some_wildcard_filter_*
    let mut filter = Option::default();
    // --partition=x/y
    let mut partition = Option::default();
    // --continue
    let mut resume = false;

    for arg in std::env::args().skip(1) {
        match arg.split_once('=') {
            Some(("--filter", value)) => filter = Some(value.to_owned()),
            Some(("--partition", value)) => {
                let (part_str, total_str) = value
                    .split_once('/')
                    .ok_or_else(|| anyhow!("Invalid partition: {value}"))?;
                let part = part_str
                    .parse::<usize>()
                    .with_context(|| format!("Invalid partition part: {part_str}"))?;
                let total = total_str
                    .parse::<usize>()
                    .with_context(|| format!("Invalid partition total: {total_str}"))?;

                if total == 0 {
                    bail!("The total of a partition should be greater than zero");
                }
                if part == 0 || part > total {
                    bail!("The part of a partition should be within bounds: 0 < x <= total");
                }

                partition = Some(Partition { part, total });
            }
            Some(_) => bail!("Invalid argument: {arg}"),
            None if arg == "--continue" => resume = true,
            None => benches.push(arg),
        }
    }

    SystemTestRunner::new(&benches, filter.as_deref(), partition, resume)?.run()
}
