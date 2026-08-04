macro_rules! targeted_enum {
    (
        $name:ident {
            $(
                $(#[$variant_attribute:meta])*
                $variant:ident($binding:ident: $value:ty) => $resolved:expr
            ),+ $(,)?
        }
        resolve($target_triple:ident) -> $resolved_type:ty
    ) => {
        #[derive(Debug, Serialize, Deserialize, Clone)]
        #[serde(untagged)]
        pub enum $name {
            $(
                $(#[$variant_attribute])*
                $variant($value),
            )+
        }

        impl $name {
            pub fn resolve(&self, $target_triple: &str) -> $resolved_type {
                match self {
                    $(Self::$variant($binding) => $resolved,)+
                }
            }
        }
    };
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Output;

use anyhow::Result;
use gungraun_tests::serde::runs_on::RunsOn;
use rustc_version::{Channel, VersionMeta};
use serde::{Deserialize, Serialize};
use version_compare::Cmp;

use crate::assert::{Assert, AssertContext};

pub const PACKAGE: &str = "gungraun-tests";

targeted_enum! {
    TargetedI32 {
        /// Backward-compatible scalar path
        Scalar(scalar: i32) => Some(*scalar),
        /// Per-target mapping; `default` is the fallback key
        Targets(map: HashMap<String, Option<i32>>) => map
            .get(target_triple)
            .or_else(|| map.get("default"))
            .and_then(|p| *p),
    }
    resolve(target_triple) -> Option<i32>
}

targeted_enum! {
    TargetedPath {
        /// Backward-compatible scalar path
        Scalar(path: PathBuf) => Some(path.as_path()),
        /// Per-target mapping; `default` is the fallback key
        Targets(map: HashMap<String, PathBuf>) => map
            .get(target_triple)
            .or_else(|| map.get("default"))
            .map(PathBuf::as_path),
    }
    resolve(target_triple) -> Option<&Path>
}

targeted_enum! {
    TargetedRunExpectations {
        /// Per-target mapping; `default` is the fallback key
        Targets(map: HashMap<String, RunExpectations>) => {
            map.get(target_triple).or_else(|| map.get("default"))
        },
        /// Backward-compatible scalar path
        Scalar(config: Box<RunExpectations>) => Some(config),
    }
    resolve(target_triple) -> Option<&RunExpectations>
}

targeted_enum! {
    TargetedString {
        /// Backward-compatible scalar path
        Scalar(string: String) => Some(string.as_str()),
        /// Per-target mapping; `default` is the fallback key
        Targets(map: HashMap<String, Option<String>>) => map
            .get(target_triple)
            .or_else(|| map.get("default"))
            .and_then(|p| p.as_deref()),
    }
    resolve(target_triple) -> Option<&str>
}

targeted_enum! {
    TargetedStrings {
        /// Backward-compatible scalar path
        Scalar(strings: Vec<String>) => strings.as_slice(),
        /// Per-target mapping; `default` is the fallback key
        Targets(map: HashMap<String, Vec<String>>) => map
            .get(target_triple)
            .or_else(|| map.get("default"))
            .map_or_else(|| &[], Vec::as_slice),
    }
    resolve(target_triple) -> &[String]
}

/// Captured result of one cargo bench invocation.
///
/// Example:
/// * stdout/stderr from `cargo bench --package gungraun-tests --bench test_lib_bench_tools`.
#[derive(Debug)]
pub struct CapturedOutput {
    /// Whether the run used an explicit tolerance argument.
    ///
    /// Example: `true` when `--tolerance=0.01` was forwarded to the benchmark.
    pub has_tolerance: bool,
    /// Process output returned by `std::process::Command::output`.
    ///
    /// Example: includes the benchmark process exit status and captured stderr.
    pub output: Output,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// YAML group containing multiple benchmark runs under shared conditions.
///
/// A group can gate all runs to Linux only.
pub struct Group {
    /// Assertions shared by every run in this group.
    ///
    /// A run-level assertion script takes precedence over the group-level script.
    pub expected: Option<GroupExpectations>,
    /// Runs executed for this group after group-level filters match.
    ///
    /// Example: two runs comparing default output and `--show-grid=true` output.
    pub runs: Vec<Run>,
    /// Optional target triple include or exclude condition for the whole group.
    ///
    /// Example: `x86_64-unknown-linux-gnu`.
    #[serde(default, with = "gungraun_tests::serde::runs_on")]
    pub runs_on: Option<RunsOn>,
    /// Optional Rust compiler version or channel condition for the whole group.
    ///
    /// Example: `>=1.86.0` or `=nightly`.
    #[serde(default, with = "gungraun_tests::serde::rust_version")]
    pub rust_version: Option<gungraun_tests::serde::rust_version::VersionComparator>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Expected output checks shared by all runs ([`Run`]) in a [`Group`].
pub struct GroupExpectations {
    /// Shell script run as a fallback when a run has no assertion script of its own.
    ///
    /// The script is executed with `bash -ex` in the benchmark output base directory.
    pub script: Option<TargetedString>,
}

#[derive(Debug, Clone, Copy)]
/// Selected partition of the benchmark list.
///
/// Example: `part = 2`, `total = 4` runs the second quarter of benchmarks.
pub struct Partition {
    /// One-based partition number to run.
    ///
    /// Example: `2` in `--partition=2/4`.
    pub part: usize,
    /// Total number of partitions.
    ///
    /// Example: `4` in `--partition=2/4`.
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// YAML configuration for one benchmark invocation.
///
/// Example: one run can pass extra cargo args, benchmark args, envs, and expectations.
pub struct Run {
    /// Extra cargo arguments passed before `--`.
    ///
    /// Example: `["--features", "client-requests"]`.
    #[serde(default)]
    pub cargo_args: Vec<String>,
    /// Environment variables set for the cargo bench command.
    ///
    /// Example: `{ "RUSTFLAGS": "-C target-feature=-avx2" }`.
    #[serde(default)]
    pub envs: HashMap<String, String>,
    /// Expected process output, exit status, and generated files.
    ///
    /// Example: compare stdout with `expected.stdout` and validate `summary.json`.
    #[serde(default)]
    pub expected: Option<TargetedRunExpectations>,
    /// Number of retries allowed for flaky assertion failures.
    ///
    /// Example: `2` allows up to two retries after the first failed attempt.
    #[serde(default)]
    pub flaky: Option<usize>,
    /// Benchmark binary arguments passed after `--`.
    ///
    /// Example: `["--show-grid=true"]`.
    #[serde(default, rename = "args")]
    pub gungraun_args: Vec<String>,
    /// Directories removed before this run starts.
    ///
    /// Example: `target/gungraun/gungraun-tests/test_lib_bench_tools`.
    #[serde(default)]
    pub rmdirs: Vec<PathBuf>,
    /// Optional target triple include or exclude condition for this run.
    ///
    /// Example: skip a run on `aarch64-apple-darwin`.
    #[serde(default, with = "gungraun_tests::serde::runs_on")]
    pub runs_on: Option<RunsOn>,
    /// Optional Rust compiler version or channel condition for this run.
    ///
    /// Example: `>=1.86.0` or `!=nightly`.
    #[serde(default, with = "gungraun_tests::serde::rust_version")]
    pub rust_version: Option<gungraun_tests::serde::rust_version::VersionComparator>,
    /// Shell snippet executed before the benchmark command.
    ///
    /// Example: `mkdir -p /tmp/gungraun-fixture`.
    #[serde(default)]
    pub setup: Option<String>,
    /// Shell snippet executed after the benchmark command.
    ///
    /// Example: `rm -rf /tmp/gungraun-fixture`.
    #[serde(default)]
    pub teardown: Option<String>,
    /// Data passed to the benchmark source template renderer.
    ///
    /// Example: `{ "tool": "callgrind" }`.
    #[serde(default)]
    pub template_data: HashMap<String, minijinja::Value>,
    /// Optional benchmark tolerance forwarded as `--tolerance=<value>`.
    ///
    /// Example: `0.01`.
    #[serde(default)]
    pub tolerance: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Expected side effects and process result for a run.
///
/// Example: compare stdout against `expected.stdout` and require exit code `0`.
#[expect(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
pub struct RunExpectations {
    /// Expected process exit code.
    ///
    /// Example: `101` for a benchmark expected to panic.
    #[serde(default)]
    pub exit_code: Option<TargetedI32>,
    /// Path to an expected-files manifest relative to the benchmark config directory.
    ///
    /// Example: `expected/files.yml`.
    #[serde(default)]
    pub files: Option<TargetedPath>,
    /// Whether no benchmark output directory is expected.
    ///
    /// Example: `true` for an early argument validation failure.
    #[serde(default)]
    pub no_files: bool,
    /// Whether filtered stderr must be empty.
    ///
    /// Example: `true` when cargo should not emit benchmark diagnostics.
    #[serde(default)]
    pub no_stderr: bool,
    /// Whether filtered stdout must be empty.
    ///
    /// Example: `true` for a quiet successful run.
    #[serde(default)]
    pub no_stdout: bool,
    /// Run a bash script in the `HOME/PACKAGE_DIR/BENCH_NAME` directory
    ///
    /// For example this is the directory of the `test_something` benchmark in which the script is
    /// executed: `project_root/target/gungraun-tests/test_something`
    #[serde(default)]
    pub script: Option<TargetedString>,
    /// Path to expected stderr relative to the benchmark config directory.
    ///
    /// Example: `stderr: expected.stderr`.
    #[serde(default)]
    pub stderr: Option<TargetedPath>,
    /// A string which should be contained in the stderr output
    ///
    /// Example: `stderr: expected.stderr`.
    #[serde(default)]
    pub stderr_contains: TargetedStrings,
    /// Path to expected stdout relative to the benchmark config directory.
    ///
    /// Example: `expected.stdout`.
    #[serde(default)]
    pub stdout: Option<TargetedPath>,
    /// A string which should be contained in the stdout output
    ///
    /// Example: `stdout: expected.stdout`.
    #[serde(default)]
    pub stdout_contains: TargetedStrings,
    /// Whether all-zero metrics are allowed in generated summaries.
    ///
    /// Example: `true` for a run that intentionally does not collect costs.
    #[serde(default)]
    pub zero_metrics: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// YAML configuration loaded from one benchmark `.conf.yml` file.
///
/// Example: `test_lib_bench_tools.conf.yml` with an optional template and groups.
pub struct SystemTestConfig {
    /// Grouped runs defined by this benchmark configuration.
    ///
    /// Example: groups for default and filtered benchmark invocations.
    pub groups: Vec<Group>,
    /// Optional Rust source template rendered before a run.
    ///
    /// Example: `templates/tool_bench.rs.j2`.
    pub template: Option<PathBuf>,
}

impl Group {
    pub fn is_enabled(&self, target_triple: &str, rust_version: &VersionMeta) -> bool {
        is_enabled(
            self.runs_on.as_ref(),
            self.rust_version.as_ref(),
            target_triple,
            rust_version,
        )
    }
}

impl Run {
    pub fn assert(&self, ctx: &AssertContext) -> Result<()> {
        let target_triple = env!("GR_BUILD_TRIPLE");
        if let Some(expected) = self
            .expected
            .as_ref()
            .and_then(|e| e.resolve(target_triple))
        {
            return Assert(expected).assert(ctx, target_triple);
        }

        Ok(())
    }

    pub fn is_enabled(&self, target_triple: &str, rust_version: &VersionMeta) -> bool {
        is_enabled(
            self.runs_on.as_ref(),
            self.rust_version.as_ref(),
            target_triple,
            rust_version,
        )
    }
}

impl RunExpectations {
    pub fn expects_output_capture(&self, target_triple: &str) -> bool {
        self.stdout.is_some()
            || self.no_stdout
            || !self.stdout_contains.resolve(target_triple).is_empty()
            || self.stderr.is_some()
            || self.no_stderr
            || !self.stderr_contains.resolve(target_triple).is_empty()
    }

    pub fn resolve_script<'a>(
        &'a self,
        group_expectations: Option<&'a GroupExpectations>,
        target_triple: &str,
    ) -> Option<&'a str> {
        self.script.as_ref().map_or_else(
            || {
                group_expectations
                    .and_then(|g| g.script.as_ref().and_then(|s| s.resolve(target_triple)))
            },
            |s| s.resolve(target_triple),
        )
    }

    pub fn resolve_exit_code(&self, target_triple: &str) -> Option<i32> {
        self.exit_code
            .as_ref()
            .and_then(|e| e.resolve(target_triple))
    }

    pub fn resolve_files(&self, target_triple: &str) -> Option<&Path> {
        self.files.as_ref().and_then(|f| f.resolve(target_triple))
    }

    pub fn resolve_stderr(&self, target_triple: &str) -> Option<&Path> {
        self.stderr.as_ref().and_then(|p| p.resolve(target_triple))
    }

    pub fn resolve_stderr_contains(&self, target_triple: &str) -> Option<&[String]> {
        let resolved = self.stderr_contains.resolve(target_triple);
        (!resolved.is_empty()).then_some(resolved)
    }

    pub fn resolve_stdout(&self, target_triple: &str) -> Option<&Path> {
        self.stdout.as_ref().and_then(|p| p.resolve(target_triple))
    }

    pub fn resolve_stdout_contains(&self, target_triple: &str) -> Option<&[String]> {
        let resolved = self.stdout_contains.resolve(target_triple);
        (!resolved.is_empty()).then_some(resolved)
    }
}

impl Default for TargetedStrings {
    fn default() -> Self {
        Self::Scalar(vec![])
    }
}

fn is_enabled(
    runs_on: Option<&(bool, String)>,
    rust_version_cmp: Option<&(Cmp, String)>,
    target_triple: &str,
    rust_version: &VersionMeta,
) -> bool {
    let compare_rust_version = |(cmp, version): &(Cmp, String)| {
        if version.starts_with(|p: char| p.is_ascii_digit()) {
            version_compare::compare_to(rust_version.semver.to_string(), version, *cmp).unwrap()
        } else {
            let channel = match version.as_str() {
                "nightly" => Channel::Nightly,
                "stable" => Channel::Stable,
                "dev" => Channel::Dev,
                "beta" => Channel::Beta,
                _ => panic!("Invalid version string: {version}"),
            };
            match cmp {
                version_compare::Cmp::Eq => rust_version.channel == channel,
                version_compare::Cmp::Ne => rust_version.channel != channel,
                _ => panic!(
                    "Invalid comparator for channel: {version}. Only '=' and '!=' are allowed."
                ),
            }
        }
    };

    runs_on.as_ref().is_none_or(|(is_target, target)| {
        if *is_target {
            target == target_triple
        } else {
            target != target_triple
        }
    }) && rust_version_cmp.is_none_or(compare_rust_version)
}
