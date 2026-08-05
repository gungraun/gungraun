//! Declarative model of a system-test case and its expected outcomes.
//!
//! This is the serde shape of the `.conf.yml` files that drive the harness. [`SystemTestConfig`] is
//! one file: `groups` of `runs`, where each [`Run`] carries its `cargo_args`, `gungraun_args`,
//! `envs`, `setup`/`teardown`, a `flaky` retry budget, and a [`RunExpectations`] block.
//!
//! The `targeted_enum!` macro at the top of the module generates the `Targeted*` enums
//! ([`TargetedPath`], [`TargetedI32`], ...) that carry per-target-triple overrides, so a single
//! case can express different expected output per platform without forking the config.
//!
//! [`CapturedOutput`] and [`Partition`] are the small CLI/transport types shared with
//! [`runner`][super::runner] and [`assert`][super::assert].
//!
//! Concentrating the whole on-disk format in one module gives the schema exactly one definition to
//! change when a case gains a new knob.
//!
//! The `.conf.yml` schema in a [`SystemTestConfig`] is basically structured as follows:
//!
//! ```yaml
//! template: "some_rust_template.rs.j2" # optional path to a rust template
//! groups:
//!   # `Group`
//!   - expected: # `GroupExpectations`
//!     runs:
//!       # `Run`
//!       - expected: # `RunExpectations`
//!         ...: # other `Run` fields
//!     ... # other `Group` fields
//! ```

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
/// The stdout/stderr from `cargo bench --package gungraun-tests --bench test_lib_bench_tools`
#[derive(Debug)]
pub struct CapturedOutput {
    /// Whether the run used an explicit tolerance argument.
    ///
    /// For example returns `true` when `--tolerance=0.01` was forwarded to the benchmark.
    pub has_tolerance: bool,
    /// Process output returned by [`std::process::Command::output`] of the cargo --bench run
    pub output: Output,
}

/// YAML group containing multiple benchmark runs under shared conditions.
///
/// A group can gate all runs to a target triple or Rust version and share assertions across its
/// runs.
///
/// # Examples
///
/// A group with two runs sharing a group-level expectation:
///
/// ```yaml
/// groups:
///   - expected:
///       script: |
///         echo "group fallback"
///     runs:
///       - args: ["--nocapture"]
///       - args: ["--nocapture", "--tools=perf"]
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Group {
    /// Assertions shared by every run in this group.
    ///
    /// A similar run-level assertion takes precedence over the group-level assertion.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - expected:
    ///       script: |
    ///         echo "shared assertion"
    ///     runs:
    ///       - args: ["--nocapture"]
    /// ```
    pub expected: Option<GroupExpectations>,
    /// Runs executed for this group after group-level filters like `runs_on` match.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - args: ["--nocapture"]
    ///         expected:
    ///           stdout: expected_stdout.1
    ///       - args: ["--nocapture", "--tools=perf"]
    /// ```
    pub runs: Vec<Run>,
    /// Optional target triple include or exclude condition for the whole group.
    ///
    /// Prefix the triple with `!` to exclude it.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs_on: "x86_64-unknown-linux-gnu"
    ///     runs:
    ///       - args: ["--nocapture"]
    /// ```
    #[serde(default, with = "gungraun_tests::serde::runs_on")]
    pub runs_on: Option<RunsOn>,
    /// Optional Rust compiler version or channel condition for the whole group.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - rust_version: ">=1.86.0"
    ///     runs:
    ///       - args: ["--nocapture"]
    /// ```
    #[serde(default, with = "gungraun_tests::serde::rust_version")]
    pub rust_version: Option<gungraun_tests::serde::rust_version::VersionComparator>,
}

/// Assertions shared by all runs ([`Run`]) in a [`Group`].
///
/// # Examples
///
/// A group-level fallback script run when a run has no `script` of its own:
///
/// ```yaml
/// groups:
///   - expected:
///       script: |
///         echo "group fallback"
///     runs:
///       - args: ["--nocapture"]
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GroupExpectations {
    /// Shell script run as a fallback when a run has no assertion script of its own.
    ///
    /// The script is executed with `bash -ex` in the benchmark output base directory.
    ///
    /// # Examples
    ///
    /// A default fallback script shared by all runs in a group:
    ///
    /// ```yaml
    /// groups:
    ///   - expected:
    ///       script: |
    ///         echo "group fallback"
    ///     runs:
    ///       - args: ["--nocapture"]
    /// ```
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

/// YAML configuration for one benchmark invocation.
///
/// A run describes a single `cargo bench` invocation under Valgrind/Perf, including forwarded
/// arguments, environment, setup/teardown, and expectations.
///
/// # Examples
///
/// ```yaml
/// groups:
///   - runs:
///       - args: ["--nocapture"]
///         envs:
///           RUSTFLAGS: "-C target-feature=-avx2"
///         expected:
///           stdout: expected_stdout.1
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Run {
    /// Gungraun arguments passed after `cargo bench ... --`.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - args: ["--show-grid=true"]
    /// ```
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra cargo arguments passed before `--`.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - cargo_args: ["--features", "client-requests"]
    /// ```
    #[serde(default)]
    pub cargo_args: Vec<String>,
    /// Environment variables set for the cargo bench command.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - envs:
    ///           RUSTFLAGS: "-C target-feature=-avx2"
    /// ```
    #[serde(default)]
    pub envs: HashMap<String, String>,
    /// Expected [`RunExpectations`] like process output, exit status, generated files, ...
    ///
    /// # Examples
    ///
    /// Define assertions which are run after the benchmark run
    ///
    /// ```yaml
    /// - groups:
    ///     - runs:
    ///         - expected: # `RunExpectations`
    ///             exit_code: 1
    /// ```
    ///
    /// On different systems (like FreeBSD) assertions and benchmark output can differ. The
    /// `default` assertions are run everywhere if not a more specific target triple is defined:
    ///
    /// ```yaml
    /// - groups:
    ///     - runs:
    ///         - expected:
    ///             default: # `RunExpectations`
    ///               exit_code: 1
    ///             x86_64-unknown-freebsd: # `RunExpectations`
    ///               exit_code: 0
    /// ```
    #[serde(default)]
    pub expected: Option<TargetedRunExpectations>,
    /// Number of retries allowed for flaky assertion failures.
    ///
    /// # Examples
    ///
    /// Allow up to two retries after the first failed attempt:
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - flaky: 2
    /// ```
    #[serde(default)]
    pub flaky: Option<usize>,
    /// Directories removed before this run starts, usually to clean up stale
    /// test data.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - rmdirs:
    ///           - /tmp/gungraun-fixture
    /// ```
    #[serde(default)]
    pub rmdirs: Vec<PathBuf>,
    /// Optional target triple include or exclude condition for this run.
    ///
    /// This system test run is only executed if this condition is `true`. Prefix the triple with
    /// `!` to exclude it.
    ///
    /// # Examples
    ///
    /// Skip a run on FreeBSD:
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - runs_on: "!x86_64-unknown-freebsd"
    ///         args: ["--nocapture"]
    /// ```
    #[serde(default, with = "gungraun_tests::serde::runs_on")]
    pub runs_on: Option<RunsOn>,
    /// Optional Rust compiler version or channel condition for this run.
    ///
    /// This system test run is only executed if this condition is `true`.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - rust_version: ">=1.86.0"
    ///         args: ["--nocapture"]
    /// ```
    #[serde(default, with = "gungraun_tests::serde::rust_version")]
    pub rust_version: Option<gungraun_tests::serde::rust_version::VersionComparator>,
    /// Bash snippet executed before the benchmark command with `bash -ex`
    ///
    /// # Examples
    ///
    /// ```yaml
    /// - runs:
    ///     - setup: |
    ///         mkdir -p /tmp/gungraun-fixture
    /// ```
    #[serde(default)]
    pub setup: Option<String>,
    /// Bash snippet executed after the benchmark command with `bash -ex`
    ///
    /// # Examples
    ///
    /// ```yaml
    /// - runs:
    ///     - teardown: |
    ///         rm -rf /tmp/gungraun-fixture
    /// ```
    #[serde(default)]
    pub teardown: Option<String>,
    /// Data passed to the template renderer. Only useful if a template is defined
    ///
    /// # Examples
    ///
    /// ```yaml
    /// template: "some_template.rs.j2"
    /// groups:
    ///   - runs:
    ///       - template_data:
    ///           some_value: 1.0
    /// ```
    #[serde(default)]
    pub template_data: HashMap<String, serde_json::Value>,
    /// Optional tolerance value forwarded as `--tolerance=<value>`.
    ///
    /// Use this option instead of passing the gungraun `args`: `--tolerance=<value>`
    ///
    /// # Examples
    ///
    /// Don't do this
    ///
    /// ```yaml
    /// - groups:
    ///     - runs:
    ///         - args: ["--tolerance=0.01"]
    /// ```
    ///
    /// Use this option instead:
    ///
    /// ```yaml
    /// - groups:
    ///     - runs:
    ///         - tolerance: 0.01
    /// ```
    #[serde(default)]
    pub tolerance: Option<f64>,
}

/// Expected side effects and process result for a run.
///
/// `RunExpectations` is typically nested under a run's `expected` field, either directly for the
/// default target or keyed by target triple.
///
/// # Examples
///
/// Require a zero exit code on FreeBSD and by default an exit code of `1`:
///
/// ```yaml
/// groups:
///   - runs:
///       - args: ["--nocapture"]
///         expected:
///           default:
///             exit_code: 1
///           x86_64-unknown-freebsd:
///             exit_code: 0
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
#[expect(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
pub struct RunExpectations {
    /// Expected process exit code.
    ///
    /// # Examples
    ///
    /// A benchmark expected to panic:
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           exit_code: 101
    /// ```
    #[serde(default)]
    pub exit_code: Option<TargetedI32>,
    /// Path to an [`ExpectedFilesManifest`] relative to the system test config directory.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           files: expected/files.yml
    /// ```
    ///
    /// [`ExpectedFilesManifest`]: super::expected_files::ExpectedFilesManifest
    #[serde(default)]
    pub files: Option<TargetedPath>,
    /// Whether no benchmark output directory is expected.
    ///
    /// # Examples
    ///
    /// For an early argument validation failure:
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           no_files: true
    /// ```
    #[serde(default)]
    pub no_files: bool,
    /// Whether filtered stderr must be empty.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           no_stderr: true
    /// ```
    #[serde(default)]
    pub no_stderr: bool,
    /// Whether filtered stdout must be empty.
    ///
    /// # Examples
    ///
    /// For a quiet successful run:
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           no_stdout: true
    /// ```
    #[serde(default)]
    pub no_stdout: bool,
    /// Run a bash script in the `HOME/PACKAGE_DIR/BENCH_NAME` directory.
    ///
    /// For example this is the directory of the `test_something` system test in which the script
    /// is executed:
    ///
    /// `project_root/target/gungraun-tests/test_something`.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           script: |
    ///             echo "run-specific assertion"
    /// ```
    #[serde(default)]
    pub script: Option<TargetedString>,
    /// Path to expected stderr relative to the benchmark config directory.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           stderr: expected.stderr
    /// ```
    #[serde(default)]
    pub stderr: Option<TargetedPath>,
    /// A string which should be contained in the stderr output.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           stderr_contains: "panicked"
    /// ```
    #[serde(default)]
    pub stderr_contains: TargetedStrings,
    /// Path to expected stdout relative to the benchmark config directory.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           stdout: expected.stdout
    /// ```
    #[serde(default)]
    pub stdout: Option<TargetedPath>,
    /// A string which should be contained in the stdout output.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           stdout_contains: "result:"
    /// ```
    #[serde(default)]
    pub stdout_contains: TargetedStrings,
    /// Whether all-zero metrics are allowed in generated summaries.
    ///
    /// # Examples
    ///
    /// For a run that intentionally does not collect costs:
    ///
    /// ```yaml
    /// groups:
    ///   - runs:
    ///       - expected:
    ///           zero_metrics: true
    /// ```
    #[serde(default)]
    pub zero_metrics: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// YAML configuration loaded from one benchmark `.conf.yml` file.
///
/// `SystemTestConfig` is the root type parsed from each `*.conf.yml` file under `benches/` and
/// describes the grouped runs and optional template for one benchmark case.
///
/// # Examples
///
/// A minimal configuration with a single group and run:
///
/// ```yaml
/// groups:
///   - runs:
///       - args: ["--nocapture"]
///         expected:
///           stdout: expected_stdout.1
/// ```
///
/// With a template rendered before the run:
///
/// ```yaml
/// template: templates/tool_bench.rs.j2
/// groups:
///   - runs:
///       - template_data:
///           some_value: 1.0
/// ```
pub struct SystemTestConfig {
    /// Grouped runs defined by this benchmark configuration.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// groups:
    ///   - runs_on: "!x86_64-unknown-freebsd"
    ///     runs:
    ///       - args: ["--nocapture"]
    /// ```
    pub groups: Vec<Group>,
    /// Optional Rust source template rendered before a run.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// template: templates/tool_bench.rs.j2
    /// groups:
    ///   - runs:
    ///       - template_data:
    ///           some_value: 1.0
    /// ```
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
            version_compare::compare_to(rust_version.semver.to_string(), version, *cmp)
                .expect("Rust version should be valid")
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
