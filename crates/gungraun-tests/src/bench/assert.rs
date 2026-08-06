//! Post-run assertion engine.
//!
//! After [`runner`][super::runner] executes a `cargo bench` run, [`Assert`] (a thin wrapper over
//! [`RunExpectations`]) drives the ordered checks as [configured][super::config] for that run.
//!
//! Stream comparison is deliberately dual-mode: under `BENCH_OVERWRITE=yes` it rewrites the
//! expected fixtures instead of comparing, so regenerating output after an intentional behavior
//! change runs the same code path as asserting it. Coverage runs route stdout through extra
//! normalization (see [`filter`][super::filter]) because instrumented binaries emit different
//! noise.

use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Write};
use std::fs::{self, File};
use std::io::{Read, Write as IOWrite, stderr, stdout};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Ok, Result};
use glob::glob;
use gungraun_tests::common::Summary;
use tempfile::tempdir;
use valico::json_schema::schema::ScopedSchema;

use super::config::{GroupExpectations, PACKAGE, RunExpectations};
use super::expected_files::ExpectedFilesManifest;
use super::io::{deserialize_yaml_str, print_info};
use crate::filter::CapturedOutput;

/// Identifies which process stream (`stdout` or `stderr`) is being inspected.
///
/// Carried through assertion helpers so a single code path can format log messages and select the
/// right normalization for either stream.
#[derive(Debug)]
enum StreamKind {
    /// Standard output of the benchmark process.
    Stdout,
    /// Standard error of the benchmark process.
    Stderr,
}

/// Entry point for the post-run assertion engine.
///
/// A thin newtype over a [`RunExpectations`] reference. Calling [`Assert::assert`] drives the
/// ordered checks (output capture, exit code, optional script, expected files, non-zero metrics,
/// ...) configured for that run.
///
/// The comparison path is dual-mode: under `BENCH_OVERWRITE=yes` it rewrites the expected fixtures
/// instead of comparing them, so regenerating output after an intentional behavior change runs the
/// same code path as asserting it (see the [`mod@super`] module docs).
#[derive(Debug)]
pub struct Assert<'a>(
    /// The [`RunExpectations`] for this run.
    pub &'a RunExpectations,
);

/// Read-only inputs needed to evaluate a run's [`RunExpectations`].
///
/// Built by the system-test runner per `cargo bench` invocation and passed by reference to
/// [`Assert::assert`]. All paths are borrows; the context does not own the run artifacts.
#[derive(Debug)]
pub struct AssertContext<'a> {
    /// Name of the system-test benchmark case (the `--bench <name>` argument).
    ///
    /// Used for example to locate the per-benchmark output directory under `home_dir`
    /// (`PACKAGE/bench_name`).
    pub bench_name: &'a str,
    /// Captured stdout/stderr plus the filtering helpers used to normalize them before comparison
    /// (see [`crate::filter::CapturedOutput`]).
    pub captured_output: &'a CapturedOutput,
    /// Directory of the benchmark's `.conf.yml`.
    ///
    /// Used to resolve relative paths in the expectations (expected stdout/stderr fixtures,
    /// [`ExpectedFilesManifest`], ...).
    pub config_dir: &'a Path,
    /// Group-level [`GroupExpectations`], similar to [`RunExpectations`].
    ///
    /// [`RunExpectations`] take precedence.
    pub group_expectations: Option<&'a GroupExpectations>,
    /// Base directory under which benchmark output is written.
    ///
    /// The per-bench output root is `home_dir/PACKAGE/bench_name` (where `PACKAGE` is
    /// `gungraun-tests`).
    pub home_dir: &'a Path,
    /// Whether the run executed under `CARGO_LLVM_COV=1`.
    ///
    /// Coverage instrumentation perturbs DHAT metrics and stdout, so coverage runs take a separate
    /// normalization path in [`Assert::assert_or_overwrite_output_stream`].
    pub is_coverage_run: bool,
    /// Parsed summary JSON schema used to validate each `summary.json` produced under `home_dir`.
    pub schema: &'a ScopedSchema<'a>,
}

impl Assert<'_> {
    /// Drive all configured checks for a single benchmark run.
    ///
    /// `target_triple` selects the per-target variants of each expectation (see
    /// [`RunExpectations`]).
    ///
    /// Skips the non-zero-metrics check if `zero_metrics` is set, or when the expected-files path
    /// already returned `true` because an overwrite just regenerated them.
    pub fn assert(&self, ctx: &AssertContext, target_triple: &str) -> Result<()> {
        let expected = self.0;

        if expected.expects_output_capture(target_triple) {
            Self::assert_or_overwrite_output(expected, ctx, target_triple)?;
        }

        Self::assert_exit_code(
            expected.resolve_exit_code(target_triple),
            ctx.captured_output,
        );

        // a run-local script takes precedence over a group script if present
        if let Some(script) = expected.resolve_script(ctx.group_expectations, target_triple) {
            Self::run_assert_script(script, ctx)?;
        }

        if let Some(manifest) = expected.resolve_files(target_triple) {
            // If `true`, overwriting effectively skips the assertion of zero metrics
            if Self::assert_or_overwrite_expected_files(manifest, ctx)? {
                return Ok(());
            }
        } else if expected.no_files {
            Self::assert_no_files(ctx);
        } else {
            // do nothing
        }

        if !expected.zero_metrics {
            Self::assert_not_all_metrics_zero(ctx)?;
        }

        Ok(())
    }

    /// Echo the captured streams to the real stdout/stderr, then enforce the configured stream
    /// expectations.
    ///
    /// For each stream (stderr first, then stdout), exactly one of the following applies:
    ///
    /// 1. `no_stderr`/`no_stdout`: the filtered stream must be empty.
    /// 2. `stderr_contains`/`stdout_contains`: the raw stream must contain each listed substring.
    /// 3. `stderr`/`stdout`: the filtered stream is byte-compared against the expected file (or
    ///    rewrites it under `BENCH_OVERWRITE=yes`).
    /// 4. Otherwise the stream is only echoed, not asserted.
    ///
    /// The captured bytes are written to the real stdout/stderr unfiltered so diagnostic output
    /// (e.g. panic messages) reaches the user verbatim.
    fn assert_or_overwrite_output(
        expected: &RunExpectations,
        ctx: &AssertContext,
        target_triple: &str,
    ) -> Result<()> {
        let output = &ctx.captured_output.output;

        print_info("STDERR:");
        stderr()
            .write_all(&output.stderr)
            .context("Failed to write captured stderr")?;

        print_info("STDOUT:");
        stdout()
            .write_all(&output.stdout)
            .context("Failed to write captured stdout")?;

        if expected.no_stderr {
            Self::assert_output_no_stream(
                &CapturedOutput::filter_stderr(&output.stderr),
                &StreamKind::Stderr,
            );
        } else if let Some(resolved) = expected.resolve_stderr_contains(target_triple) {
            Self::assert_output_stream_contains(
                std::str::from_utf8(&output.stderr)?,
                &StreamKind::Stderr,
                resolved,
            );
        } else if let Some(resolved) = expected.resolve_stderr(target_triple) {
            Self::assert_or_overwrite_output_stream(
                &output.stderr,
                &StreamKind::Stderr,
                ctx.config_dir,
                resolved,
                ctx.is_coverage_run,
                CapturedOutput::filter_stderr,
            )?;
        } else {
            // do nothing
        }

        if expected.no_stdout {
            Self::assert_output_no_stream(
                &ctx.captured_output.filter_stdout(&output.stdout),
                &StreamKind::Stdout,
            );
        } else if let Some(resolved) = expected.resolve_stdout_contains(target_triple) {
            Self::assert_output_stream_contains(
                std::str::from_utf8(&output.stdout)?,
                &StreamKind::Stdout,
                resolved,
            );
        } else if let Some(resolved) = expected.resolve_stdout(target_triple) {
            Self::assert_or_overwrite_output_stream(
                &output.stdout,
                &StreamKind::Stdout,
                ctx.config_dir,
                resolved,
                ctx.is_coverage_run,
                |c| ctx.captured_output.filter_stdout(c),
            )?;
        } else {
            // do nothing
        }

        Ok(())
    }

    /// Compare a single captured `stream` against its expected fixture at `path` in the
    /// `config_dir`, or rewrite the fixture.
    ///
    /// The captured `stream` bytes are passed through `filter` (for example
    /// [`CapturedOutput::filter_stdout`]) before any comparison so that host-specific noise
    /// (PIDs, absolute paths, percentages, metric values, ...) is scrubbed.
    ///
    /// Behavior is selected at compile time by the `BENCH_OVERWRITE` env var:
    ///
    /// - `BENCH_OVERWRITE=yes`: if filtered and expected differ, the filtered bytes overwrite the
    ///   fixture at `config_dir.join(path)`. Returns `Ok(())` either way.
    /// - Otherwise: if `is_coverage_run` is `true`, both sides are passed through
    ///   [`CapturedOutput::normalize_coverage_stdout`] to mask instrumentation-specific noise. The
    ///   filtered stream must then equal the expected fixture byte-for-byte, or this panics with a
    ///   [`pretty_assertions::StrComparison`] diff.
    ///
    /// `stream_kind` is used only for formatting log messages and the coverage-run branch.
    fn assert_or_overwrite_output_stream<F>(
        stream: &[u8],
        stream_kind: &StreamKind,
        config_dir: &Path,
        path: &Path,
        is_coverage_run: bool,
        filter: F,
    ) -> Result<()>
    where
        F: FnOnce(&[u8]) -> String,
    {
        let mut expected_stream: Vec<u8> = Vec::new();
        File::open(config_dir.join(path))
            .with_context(|| format!("File should exist: '{}'", path.display()))?
            .read_to_end(&mut expected_stream)
            .with_context(|| format!("Failed to read '{}'", path.display()))?;

        let mut filtered = filter(stream);
        let mut expected_string: String = String::from_utf8_lossy(&expected_stream).into();

        if option_env!("BENCH_OVERWRITE").map_or(false, |s| s.eq_ignore_ascii_case("yes")) {
            if filtered == expected_string {
                print_info(format!(
                    "Skip overwrite since verifying {stream_kind} '{}' was successful",
                    path.display()
                ));
            } else {
                print!(
                    "{}",
                    pretty_assertions::StrComparison::new(&filtered, &expected_string)
                );

                File::create(config_dir.join(path))
                    .with_context(|| {
                        format!(
                            "Failed to create expected {stream_kind} '{}'",
                            path.display()
                        )
                    })?
                    .write_all(filtered.as_bytes())
                    .with_context(|| {
                        format!("Failed to write {stream_kind} '{}'", path.display())
                    })?;

                print_info(format!(
                    "Overwriting {stream_kind} '{}' successful",
                    path.display()
                ));
            }
        } else {
            if matches!(stream_kind, StreamKind::Stdout) && is_coverage_run {
                filtered = CapturedOutput::normalize_coverage_stdout(&filtered);
                expected_string = CapturedOutput::normalize_coverage_stdout(&expected_string);
            }

            assert!(
                filtered == expected_string,
                "Assertion of {stream_kind} '{}' failed: {}",
                path.display(),
                pretty_assertions::StrComparison::new(&filtered, &expected_string)
            );

            print_info(format!(
                "Verifying {stream_kind} '{}' successful",
                path.display()
            ));
        }

        Ok(())
    }

    /// Verify the raw `stream` contains every substring listed in `contains`.
    ///
    /// Unlike the byte-comparison path, the input here is the unfiltered stream (the substrings are
    /// matched verbatim). Each hit is logged; the first missing substring panics with a message
    /// identifying the stream.
    fn assert_output_stream_contains(stream: &str, stream_kind: &StreamKind, contains: &[String]) {
        for expected in contains {
            if stream.contains(expected) {
                print_info(format!(
                    "Verifying {stream_kind} contains '{expected}' succeeded"
                ));
            } else {
                panic!(
                    "Assertion of {stream_kind} failed: Expected {stream_kind} to contain \
                     '{expected}'"
                );
            }
        }
    }

    /// Verify that a filtered `stream` of this [`StreamKind`] is empty.
    ///
    /// Used for example by the `no_stdout`/`no_stderr` expectations. The caller is responsible for
    /// filtering the stream before calling.
    fn assert_output_no_stream(stream: &str, stream_kind: &StreamKind) {
        if stream.is_empty() {
            print_info(format!(
                "Verifying {stream_kind} successful: Expected no {stream_kind}"
            ));
        } else {
            panic!("Assertion of {stream_kind} failed: Expected no {stream_kind}");
        }
    }

    /// Load the expected-files manifest at `manifest` path and either assert or regenerate it.
    ///
    /// The manifest path is resolved against `ctx.config_dir`. Its optional `home_dir` overrides
    /// `ctx.home_dir` when locating benchmark output; otherwise the per-bench output root is
    /// `ctx.home_dir/PACKAGE/bench_name`.
    ///
    /// Returns `Ok(true)` if the manifest was rewritten under `BENCH_OVERWRITE=yes`.
    fn assert_or_overwrite_expected_files(manifest: &Path, ctx: &AssertContext) -> Result<bool> {
        let manifest_path = ctx.config_dir.join(manifest);
        let manifest_content = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read '{}'", manifest.display()))?;
        let expected_files_manifest: ExpectedFilesManifest =
            deserialize_yaml_str(&manifest_content, &manifest_path)?;

        let output_dir = if let Some(home_dir) = &expected_files_manifest.home_dir {
            home_dir.join(PACKAGE).join(ctx.bench_name)
        } else {
            ctx.home_dir.join(PACKAGE).join(ctx.bench_name)
        };

        if option_env!("BENCH_OVERWRITE").map_or(false, |s| s.eq_ignore_ascii_case("yes")) {
            expected_files_manifest.overwrite(
                &output_dir,
                &manifest_content,
                &manifest.display().to_string(),
                &manifest_path,
            )?;
            return Ok(true);
        }

        Self::assert_expected_files(expected_files_manifest, &output_dir, ctx).map(|()| false)
    }

    /// Assert that every entry in the [`ExpectedFilesManifest`] matches the benchmark output, and
    /// that no extra per-group directory exists.
    ///
    /// For each manifest entry, the entry's own `assert` resolves and validates its output
    /// directory against `schema`. Two accumulators are kept per group: the set of directories that
    /// exist on disk (via `glob`), and the set of directories the manifest visited. If the on-disk
    /// set has any directory the `manifest` did not cover, this panics listing the stragglers - a
    /// regression guard for benchmarks that silently stopped emitting output.
    fn assert_expected_files(
        manifest: ExpectedFilesManifest,
        output_dir: &Path,
        ctx: &AssertContext,
    ) -> Result<()> {
        let mut dirs_by_group = HashMap::new();
        let mut visited_dirs = HashMap::new();

        for manifest_entry in manifest.data {
            dirs_by_group
                .entry(manifest_entry.group.clone())
                .or_insert_with(|| {
                    glob(&format!(
                        "{}/{}/*/",
                        output_dir.display(),
                        manifest_entry.group
                    ))
                    .expect("The glob pattern should be valid")
                    .map(Result::unwrap)
                    .collect::<HashSet<PathBuf>>()
                });

            let expected_dir = manifest_entry.assert(output_dir, ctx.schema)?;
            visited_dirs
                .entry(manifest_entry.group)
                .and_modify(|s: &mut HashSet<PathBuf>| {
                    s.insert(expected_dir.clone());
                })
                .or_insert_with(|| HashSet::from([expected_dir]));
        }

        let not_visited = dirs_by_group
            .into_iter()
            .flat_map(|(key, value)| {
                value
                    .difference(&visited_dirs[&key])
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<PathBuf>>();

        assert!(
            not_visited.is_empty(),
            "Expected no other benchmark in directory '{}' but found: {:#?}",
            output_dir.display(),
            not_visited
        );

        Ok(())
    }

    /// Assert that the benchmark produced no output directory at all.
    ///
    /// Used when `no_files` is set in the config. If the directory exists, this panics listing
    /// every file beneath it (paths relativized against the package directory for readability).
    fn assert_no_files(ctx: &AssertContext) {
        let package_dir = ctx.home_dir.join(PACKAGE);
        let output_dir = package_dir.join(ctx.bench_name);

        if output_dir.exists() {
            let files = glob(&format!("{}/**/*", output_dir.display()))
                .expect("The glob pattern should be valid")
                .map(Result::unwrap)
                .fold(String::new(), |mut acc, p| {
                    let display = p
                        .strip_prefix(&package_dir)
                        .expect("The package directory should be a prefix of the discovered file")
                        .display();
                    let _ = writeln!(acc, "  {display}");
                    acc
                });
            panic!(
                "The benchmark directory '{}' was not expected to exist but found:\n{files}",
                output_dir.display()
            );
        } else {
            print_info(format!(
                "Verifying the benchmark directory '{}' not exists was successful",
                output_dir.display()
            ));
        }
    }

    /// Assert the captured process status in [`CapturedOutput`] matches the configured expectation.
    ///
    /// - `Some(expected)`: the process must have exited with that exact code. If it died by signal
    ///   instead, the signal number is reported.
    /// - `None`: the process must simply have exited successfully (any zero status).
    fn assert_exit_code(exit_code: Option<i32>, captured_output: &CapturedOutput) {
        match exit_code {
            Some(expected) => match captured_output.output.status.code() {
                Some(actual) => {
                    assert_eq!(
                        expected, actual,
                        "Expected benchmark to exit with code '{expected}' but exited with code \
                         '{actual}'"
                    );
                    print_info(format!(
                        "Verifying exit code was successful: Process exited with '{actual}'"
                    ));
                }
                None => panic!(
                    "Expected benchmark to exit with code '{expected}' but exited with signal '{}'",
                    captured_output
                        .output
                        .status
                        .signal()
                        .expect("The exit status should be a signal")
                ),
            },
            None => assert!(
                captured_output.output.status.success(),
                "Expected benchmark to exit with success"
            ),
        }
    }

    /// Assert that every `summary.json` under the per-bench output directory reports at least one
    /// non-zero cost.
    ///
    /// This is a smoke check that the benchmark actually ran the tool and produced data, not an
    /// empty or all-zero summary.
    fn assert_not_all_metrics_zero(ctx: &AssertContext) -> Result<()> {
        let output_dir = ctx.home_dir.join(PACKAGE).join(ctx.bench_name);

        // These checks heavily depends on the creation of the `summary.json` files, but we
        // create them by default.
        for path in glob(&format!("{}/**/summary.json", output_dir.display()))
            .expect("The glob pattern should be valid")
            .map(Result::unwrap)
        {
            let summary = Summary::new(&path)
                .with_context(|| format!("Failed to read summary '{}'", path.display()))?;
            summary.assert_costs_not_all_zero();
            print_info("Verifying costs not all zero successful");
        }

        Ok(())
    }

    /// Run a bash assertion script in the benchmark's output directory.
    ///
    /// The `script` body is written to a temporary file and executed with `bash -ex` using the
    /// per-bench output directory (`home_dir/PACKAGE/bench_name`) as the working directory. The
    /// script must exit zero; otherwise this panics reporting the full `ExitStatus`.
    ///
    /// The temp file lives only for the duration of the call.
    fn run_assert_script(script: &str, ctx: &AssertContext) -> Result<()> {
        let temp_dir =
            tempdir().context("Failed to create a temporary directory for the assertion script")?;

        let output_dir = ctx.home_dir.join(PACKAGE).join(ctx.bench_name);
        let assert_path = temp_dir.path().join("assert");

        std::fs::write(&assert_path, script).with_context(|| {
            format!(
                "Failed to write assertion script '{}'",
                assert_path.display()
            )
        })?;
        print_info("Running assertion script:");
        let status = Command::new("bash")
            .current_dir(output_dir)
            .args(["-ex"])
            .arg(assert_path)
            .status()
            .context("Failed to spawn the assertion script")?;

        assert!(
            status.success(),
            "Running assertion script failed with {status:?}"
        );

        Ok(())
    }
}

impl Display for StreamKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdout => f.write_str("stdout"),
            Self::Stderr => f.write_str("stderr"),
        }
    }
}
