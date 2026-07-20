//! spell-checker:ignore rmdirs sysdeps multiarch memchr

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs::File;
use std::io::{BufRead, Read, Write as IOWrite, stderr, stdout};
use std::os::unix::process::ExitStatusExt;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::LazyLock;

use benchmark_tests::common::Summary;
use benchmark_tests::serde::runs_on::RunsOn;
use colored::Colorize;
use fs_extra::dir::CopyOptions;
use glob::glob;
use minijinja::Environment;
use once_cell::sync::OnceCell;
use regex::{Captures, Regex};
use rustc_version::{Channel, VersionMeta};
use serde::{Deserialize, Serialize};
use simplematch::DoWild;
use tempfile::{TempDir, tempdir};
use valico::json_schema;
use valico::json_schema::schema::ScopedSchema;

const PACKAGE: &str = "benchmark-tests";
const TEMPLATE_BENCH_NAME: &str = "test_bench_template";
const TEMPLATE_CONTENT: &str = r#"fn main() {
    panic!("should be replaced by a rendered template");
}
"#;
const SCHEMA_PATH: &str = "gungraun-summary/schemas";
const SCHEMA_VERSION: &str = "6";
const CONTINUE_FILE_NAME: &str = "benchmark-tests.continue";
const CARGO_LLVM_COV: &str = "CARGO_LLVM_COV";

static TEMPLATE_DATA: OnceCell<HashMap<String, minijinja::Value>> = OnceCell::new();

// The regex patterns working on the `stdout` must not include the indentation. The indentation can
// be different depending on the `show_grid` option and starts either with 2 spaces (`  `) or if
// `show_grid` is `true` with a pipe character (`|`)
static NUMBERS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            (?<desc>.+?\s*)(?<comp1>[0-9.]+|N/A)\|(?<comp2>[0-9.]+|N/A)
            (?<diff>
                (?<diff_percent>(?<white1>\s*)(?<percent>\(.*\)))
                (?<diff_factor>(?<white2>\s*)(?<factor>\[.*\]))?
            )?",
    )
    .expect("Regex should compile")
});
// Do not match (*********); those placeholder lines should stay unchanged.
static NUMBERS_DIFF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\([^)*]*\)(?:\s+\[[^\]]+\])?$").expect("Regex should compile"));
static UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<prefix>\[)(?<unit>[^\]]+)(?<suffix>\]:\s*)").expect("Regex should compile")
});
static RUNNING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ ]+Running .*$").expect("Regex should compile"));
static PROCESS_DID_NOT_EXIT_SUCCESSFULLY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([ ]+process didn't exit successfully: `)(.*)(` \(exit status: .*\).*)$")
        .expect("Regex should compile")
});

// Performance has regressed: Instructions (133 -> 196) regressed by +47.3684% (>+0.00000%)
// $1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__NUM__>$9
static REGRESSION_SOFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
                ^(Performance\ has\ regressed:\s*[^0-9]+\()
                ([0-9]+)(\s*->\s*)([0-9]+)
                (\)\s*regressed\s*by\s*[+-])
                ([0-9.]+)(%\s*\([><][+-])([0-9.]+)(%\)\s*)
              $",
    )
    .expect("Regex should compile")
});

// * Performance has regressed: Instructions (70021) exceeds limit by 69821 (>200)
// * Performance has regressed: cpu_core/instructions/u [*instructions*] (7002804) exceeds limit by
//   6997804 (>5000)
// * Performance has regressed: task-clock:u [*task-clock*] (601.931 [us]) exceeds limit by 501.931
//   [us] (>100 [us])
// $1<__NUM__>$3<__NUM__>$5<__NUM__>$7
static REGRESSION_HARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            ^(Performance\s*has\s*regressed:\s*[^0-9]+\()
            ([^)]+)
            (\)\s*exceeds\s*limit\s*by\s*)
            ([0-9.]+(?:\s*\[\S+\])?)
            (\s*\([><])
            ([^)]+)
            (\))$",
    )
    .expect("Regex should compile")
});
// Instructions (357182 -> 357704): +0.14614% exceeds limit of +0.00000%
// $1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__PERCENT__>$9
static SUMMARY_REGRESSION_SOFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(\s*[^0-9]+\()([0-9]+)(\s*->\s*)([0-9]+)(\):\s*[+-])",
        r"([0-9.]+)(%\s*exceeds limit of [+-])([0-9.]+)(%\s*)$"
    ))
    .expect("Regex should compile")
});
// * Callgrind: Instructions (70021): 70021 exceeds limit of 200 by 69821
// * Perf: cpu_core/instructions/u [*instructions*] (7002804): 7002804 exceeds limit of 5000 by
//   6997804
// * Perf: task-clock:u [*task-clock*] (602.920 [us]): 602.920 [us] exceeds limit of 100 [us] by
//   502.920 [us]
//
// $1<__NUM__>$3<__NUM__>$5<__LIMIT__>$7<__DIFF__>$9
static SUMMARY_REGRESSION_HARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            ^(\s*[^0-9]+\()
            ([0-9.]+(?:\s*\[[^)]+\])?)
            (\):\s*)
            ([0-9.]+(?:\s*\[\S+\])?)
            (\s*exceeds\s*limit\s*of\s*)
            ([0-9.]+(?:\s*\[\S+\])?)
            (\s*by\s*)
            ([0-9.]+(?:\s*\[\S+\])?)$",
    )
    .expect("Regex should compile")
});
// Command: target/release/deps/test_lib_bench_threads-c2a88f916ff580f9
static COMMAND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Command:)(\s*target/release/deps/test_(lib|bin)_bench_.+-[a-z0-9]+\s*.*)$")
        .expect("Regex should compile")
});
static PID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(p?pid:\s*)([0-9]+)(\s+)?").expect("Regex should compile"));
static DETAILS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Details:").expect("Regex should compile"));
static NOT_DETAILS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(?:\S)|(?:[a-zA-Z]))").expect("Regex should compile"));
// `  ## pid: <__PID__> part: 1 thread: 3   |pid: <__PID__> part: 1 thread: 3`
static FRAGMENT_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(##(?: \S+: \S+)+)(\s*)([|].*)$").expect("Regex should compile")
});
static ABSOLUTE_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\s+|^|')([/][^/]*)+").expect("Regex should compile"));
// Gungraun result: Success. 2 completed without regressions; 0 regressed; 0 filtered;
// 2 benchmarks finished in 0.296s
static SUMMARY_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Gungraun result:.*finished in\s*)([0-9.]+)(s$)").expect("Regex should compile")
});
static THREAD_PANICKED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?<start>thread '.*' )(?<pid>\([0-9]+\))?",
        r"(?<end>\s*panicked at .*)$"
    ))
    .expect("Regex should compile")
});
static ABSOLUTE_PATH_APOSTROPHE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[']([/][^/']+)+[']").expect("Regex should compile"));

fn filter_unit(desc: &str) -> Cow<'_, str> {
    UNIT_RE.replace(desc, |caps: &Captures| {
        format!(
            "{}{}{}",
            &caps["prefix"],
            " ".repeat(caps["unit"].len()),
            &caps["suffix"]
        )
    })
}

/// Benchmark test case derived from a `.conf.yml` file.
///
/// Example: `test_lib_bench_tools.conf.yml` becomes one `Benchmark` value.
#[derive(Debug, Clone)]
struct Benchmark {
    /// Original `.conf.yml` file stem.
    ///
    /// This identifies the benchmark configuration and is unique for templated benchmarks too.
    config_name: String,
    /// Directory containing the benchmark configuration file and expected fixtures.
    ///
    /// Example: `benchmark-tests/benches/test_lib_bench/tools`.
    dir: PathBuf,
    /// Cargo bench target name.
    ///
    /// This is usually the same as `config_name`, but templated benchmarks all use
    /// `test_bench_template`.
    bench_name: String,
    /// Parsed YAML configuration for this benchmark.
    ///
    /// Contains all groups and runs from `test_lib_bench_tools.conf.yml`.
    config: Config,
    /// Directory where this benchmark writes regular output files.
    ///
    /// Example: `target/gungraun/benchmark-tests/test_lib_bench_tools`.
    dest_dir: PathBuf,
    /// Root directory for benchmark test output below cargo's target directory.
    ///
    /// Example: `target/gungraun`.
    home_dir: PathBuf,
}

/// Captured result of one cargo bench invocation.
///
/// Example:
/// * stdout/stderr from `cargo bench --package benchmark-tests --bench test_lib_bench_tools`.
#[derive(Debug)]
struct BenchmarkOutput {
    /// Process output returned by `std::process::Command::output`.
    ///
    /// Example: includes the benchmark process exit status and captured stderr.
    output: Output,
    /// Whether the run used an explicit tolerance argument.
    ///
    /// Example: `true` when `--tolerance=0.01` was forwarded to the benchmark.
    is_tolerance: bool,
}

/// Top-level benchmark test runner.
///
/// Owns the metadata used by `bench --filter='test_lib_*'`.
#[derive(Debug)]
pub struct BenchmarkRunner {
    /// Resolved workspace, target directory, compiler, and selected benchmark list.
    ///
    /// Contains only the selected benchmarks when `--partition`, `--filter` or `--continue` is
    /// used.
    metadata: Metadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// YAML group containing multiple benchmark runs under shared conditions.
///
/// A group can gate all runs to Linux only.
pub struct GroupConfig {
    /// TODO: DOCS
    expected: GroupExpected,
    /// Optional target triple include or exclude condition for the whole group.
    ///
    /// Example: `x86_64-unknown-linux-gnu`.
    #[serde(default, with = "benchmark_tests::serde::runs_on")]
    runs_on: Option<RunsOn>,
    /// Optional Rust compiler version or channel condition for the whole group.
    ///
    /// Example: `>=1.86.0` or `=nightly`.
    #[serde(default, with = "benchmark_tests::serde::rust_version")]
    rust_version: Option<benchmark_tests::serde::rust_version::VersionComparator>,
    /// Runs executed for this group after group-level filters match.
    ///
    /// Example: two runs comparing default output and `--show-grid=true` output.
    runs: Vec<RunConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GroupExpected {
    /// TODO: DOCS
    script: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// YAML configuration loaded from one benchmark `.conf.yml` file.
///
/// Example: `test_lib_bench_tools.conf.yml` with an optional template and groups.
struct Config {
    /// Optional Rust source template rendered before a run.
    ///
    /// Example: `templates/tool_bench.rs.j2`.
    template: Option<PathBuf>,
    /// Grouped runs defined by this benchmark configuration.
    ///
    /// Example: groups for default and filtered benchmark invocations.
    groups: Vec<GroupConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
/// Expected files for one benchmark output directory.
///
/// Example: requires `summary.json` and one `callgrind.out.*` glob match.
struct Expected {
    /// Exact files expected to exist and be non-empty.
    ///
    /// Example: `["summary.json"]`.
    #[serde(default)]
    files: Vec<PathBuf>,
    /// Glob patterns with required match counts.
    ///
    /// Example: `callgrind.out.*` with `count = 1`.
    #[serde(default)]
    globs: Vec<ExpectedGlob>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Expected side effects and process result for a run.
///
/// Example: compare stdout against `expected.stdout` and require exit code `0`.
struct ExpectedConfig {
    /// Path to an expected files manifest relative to the benchmark config directory.
    ///
    /// Example: `expected/files.yml`.
    #[serde(default)]
    files: Option<PathBuf>,
    /// Path to expected stdout relative to the benchmark config directory.
    ///
    /// Example: `expected.stdout`.
    #[serde(default)]
    stdout: Option<PathBuf>,
    /// Path to expected stderr relative to the benchmark config directory.
    ///
    /// Example: `expected.stderr`.
    #[serde(default)]
    stderr: Option<PathBuf>,
    /// Expected process exit code.
    ///
    /// Example: `101` for a benchmark expected to panic.
    #[serde(default)]
    exit_code: Option<i32>,
    /// Whether all-zero metrics are allowed in generated summaries.
    ///
    /// Example: `true` for a run that intentionally does not collect costs.
    #[serde(default)]
    zero_metrics: bool,
    /// Whether no benchmark output directory is expected.
    ///
    /// Example: `true` for an early argument validation failure.
    #[serde(default)]
    no_files: bool,
    /// Whether filtered stdout must be empty.
    ///
    /// Example: `true` for a quiet successful run.
    #[serde(default)]
    no_stdout: bool,
    /// Whether filtered stderr must be empty.
    ///
    /// Example: `true` when cargo should not emit benchmark diagnostics.
    #[serde(default)]
    no_stderr: bool,
    /// Run a bash script in the `HOME/PACKAGE_DIR/BENCH_NAME` directory
    ///
    /// For example this is the directory of the `test_something` benchmark in which the script is
    /// executed: `project_root/target/benchmark-tests/test_something`
    #[serde(default)]
    script: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
/// Expected glob assertion for benchmark output files.
///
/// Example: require exactly one `callgrind.out.*` file.
struct ExpectedGlob {
    /// Glob pattern relative to an expected run directory.
    ///
    /// Example: `callgrind.out.*`.
    pattern: String,
    /// Required number of files matching `pattern`.
    ///
    /// Example: `1`.
    count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
/// Expected files for one benchmark function output directory.
///
/// Example: group `library_benchmark`, function `bench_sort`, id `small`.
struct ExpectedRun {
    /// Benchmark group directory below the benchmark output root.
    ///
    /// Example: `library_benchmark`.
    group: String,
    /// Benchmark function name or template string used to locate the output directory.
    ///
    /// Example: `bench_sort`.
    function: String,
    /// Optional benchmark id appended to the function directory name.
    ///
    /// Example: `small` for `bench_sort.small`.
    id: Option<String>,
    /// Expected files and glob counts for the resolved function directory.
    ///
    /// Example: require `summary.json` and one callgrind output file.
    expected: Expected,
}

#[derive(Debug, Serialize, Deserialize)]
/// Expected files manifest referenced by an `ExpectedConfig`.
///
/// Example: a YAML file listing all expected benchmark function output directories.
struct ExpectedRuns {
    /// Optional alternate home directory for expected output lookup.
    ///
    /// Example: a target-triple-specific output root.
    #[serde(default)]
    home_dir: Option<PathBuf>,
    /// Expected output directories and files to assert.
    ///
    /// Example: entries for all functions generated by a templated benchmark.
    data: Vec<ExpectedRun>,
}

#[derive(Debug, Clone)]
/// Workspace and benchmark selection metadata.
///
/// Example: produced once from cargo metadata before running benchmark tests.
struct Metadata {
    /// Cargo workspace root.
    ///
    /// Example: repository root containing `Cargo.toml`.
    workspace_root: PathBuf,
    /// Cargo target directory from cargo metadata.
    ///
    /// Example: `target` or a custom `CARGO_TARGET_DIR`.
    target_directory: PathBuf,
    /// Benchmarks selected by CLI arguments, filters, partitioning, and resume state.
    ///
    /// Example: only benchmarks matching `--filter='test_lib_*'`.
    benchmarks: Vec<Benchmark>,
    /// Path to the `benchmark-tests/benches` directory.
    ///
    /// Example: used to write `test_bench_template.rs`.
    benches_dir: PathBuf,
    /// Rust compiler version metadata used for version-gated runs.
    ///
    /// Example: channel `nightly` or semver `1.86.0`.
    rust_version: VersionMeta,
    /// Whether the run is a llvm coverage run
    ///
    /// This is usually true when CARGO_LLVM_COV=1 is set. Coverage instrumentation changes the
    /// benchmarked machine code, so DHAT read/write metrics can differ from normal benchmark
    /// fixtures.
    is_coverage_run: bool,
}

#[derive(Debug, Clone, Copy)]
/// Selected partition of the benchmark list.
///
/// Example: `part = 2`, `total = 4` runs the second quarter of benchmarks.
pub struct Partition {
    /// One-based partition number to run.
    ///
    /// Example: `2` in `--partition=2/4`.
    part: usize,
    /// Total number of partitions.
    ///
    /// Example: `4` in `--partition=2/4`.
    total: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// YAML configuration for one benchmark invocation.
///
/// Example: one run can pass extra cargo args, benchmark args, envs, and expectations.
struct RunConfig {
    /// Extra cargo arguments passed before `--`.
    ///
    /// Example: `["--features", "client-requests"]`.
    #[serde(default)]
    cargo_args: Vec<String>,
    /// Benchmark binary arguments passed after `--`.
    ///
    /// Example: `["--show-grid=true"]`.
    #[serde(default)]
    args: Vec<String>,
    /// Data passed to the benchmark source template renderer.
    ///
    /// Example: `{ "tool": "callgrind" }`.
    #[serde(default)]
    template_data: HashMap<String, minijinja::Value>,
    /// Expected process output, exit status, and generated files.
    ///
    /// Example: compare stdout with `expected.stdout` and validate `summary.json`.
    #[serde(default)]
    expected: Option<ExpectedConfig>,
    /// Optional target triple include or exclude condition for this run.
    ///
    /// Example: skip a run on `aarch64-apple-darwin`.
    #[serde(default, with = "benchmark_tests::serde::runs_on")]
    runs_on: Option<RunsOn>,
    /// Directories removed before this run starts.
    ///
    /// Example: `target/gungraun/benchmark-tests/test_lib_bench_tools`.
    #[serde(default)]
    rmdirs: Vec<PathBuf>,
    /// Optional Rust compiler version or channel condition for this run.
    ///
    /// Example: `>=1.86.0` or `!=nightly`.
    #[serde(default, with = "benchmark_tests::serde::rust_version")]
    rust_version: Option<benchmark_tests::serde::rust_version::VersionComparator>,
    /// Number of retries allowed for flaky assertion failures.
    ///
    /// Example: `2` allows up to two retries after the first failed attempt.
    #[serde(default)]
    flaky: Option<usize>,
    /// Environment variables set for the cargo bench command.
    ///
    /// Example: `{ "RUSTFLAGS": "-C target-feature=-avx2" }`.
    #[serde(default)]
    envs: HashMap<String, String>,
    /// Optional benchmark tolerance forwarded as `--tolerance=<value>`.
    ///
    /// Example: `0.01`.
    #[serde(default)]
    tolerance: Option<f64>,
    /// Shell snippet executed before the benchmark command.
    ///
    /// Example: `mkdir -p /tmp/gungraun-fixture`.
    #[serde(default)]
    setup: Option<String>,
    /// Shell snippet executed after the benchmark command.
    ///
    /// Example: `rm -rf /tmp/gungraun-fixture`.
    #[serde(default)]
    teardown: Option<String>,
}

impl Benchmark {
    pub fn new(path: &Path, _package_dir: &Path, target_dir: &Path) -> Self {
        let config: Config = serde_yaml::from_reader(File::open(path).expect("File should exist"))
            .map_err(|error| format!("Failed to deserialize '{}': {error}", path.display()))
            .expect("File should be deserializable");

        let config_name = path.file_name().unwrap().to_string_lossy();
        let config_name = config_name.strip_suffix(".conf.yml").unwrap().to_owned();
        let bench_name = if config.template.is_some() {
            String::from(TEMPLATE_BENCH_NAME)
        } else {
            config_name.clone()
        };

        Benchmark {
            home_dir: target_dir.join("gungraun"),
            dest_dir: target_dir.join("gungraun").join(PACKAGE).join(&bench_name),
            bench_name,
            config_name,
            config,
            dir: path.parent().unwrap().to_path_buf(),
        }
    }

    pub fn clean_benchmark(&self) {
        if self.dest_dir.is_dir() {
            std::fs::remove_dir_all(&self.dest_dir).unwrap();
        }
        let alt_dir = self
            .home_dir
            .join(env!("GR_BUILD_TRIPLE"))
            .join(PACKAGE)
            .join(&self.bench_name);
        if alt_dir.is_dir() {
            std::fs::remove_dir_all(&alt_dir).unwrap();
        }
    }

    pub fn backup(&self) -> Option<TempDir> {
        if self.dest_dir.is_dir() {
            let dir = tempdir().expect("Creating temporary directory should succeed");
            fs_extra::copy_items(&[&self.dest_dir], dir.path(), &CopyOptions::new()).unwrap();
            Some(dir)
        } else {
            None
        }
    }

    pub fn restore(&self, temp_dir: Option<&TempDir>) {
        self.clean_benchmark();

        if let Some(temp_dir) = temp_dir {
            let from = temp_dir.path().join(self.dest_dir.file_name().unwrap());
            fs_extra::copy_items(
                &[from],
                self.dest_dir
                    .parent()
                    .expect("Parent of benchmark directory should exist"),
                &CopyOptions::new(),
            )
            .expect("Restoring backup should succeed");
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub fn run_bench(
        &self,
        cargo_args: &[String],
        args: &[String],
        envs: &HashMap<String, String>,
        capture: bool,
        tolerance: Option<f64>,
        setup: Option<&str>,
        teardown: Option<&str>,
    ) -> BenchmarkOutput {
        let stdio = if capture {
            // SAFETY: Benchmarks are run serially
            unsafe { std::env::set_var("GUNGRAUN_COLOR", "never") };
            Stdio::piped
        } else {
            // SAFETY: Benchmarks are run serially
            unsafe { std::env::set_var("GUNGRAUN_COLOR", "auto") };
            Stdio::inherit
        };

        let dir = tempdir().expect(
            "Creating a temporary directory for setup and teardown
            should succeed",
        );

        if let Some(setup) = setup {
            let setup_path = dir.path().join("setup");
            std::fs::write(&setup_path, setup)
                .expect("Preparing the file with the setup content should succeed");
            print_info("Running setup:");
            let status = Command::new("bash")
                .args(["-ex"])
                .arg(setup_path)
                .status()
                .expect(
                    "Spawning
                    the setup process should succeed",
                );

            if !status.success() {
                panic!("Running setup failed with {status:?}");
            }
        }

        std::fs::create_dir_all(&self.home_dir)
            .expect("Creating the gungraun home directory should succeed");
        std::fs::write(self.home_dir.join(CONTINUE_FILE_NAME), &self.config_name)
            .expect("Writing to the continue file should succeed");

        let mut command = Command::new(env!("CARGO"));
        command.args(["bench", "--package", PACKAGE, "--bench", &self.bench_name]);
        command.args(cargo_args);

        if !envs.is_empty() {
            let envs_string = envs
                .iter()
                .map(|(key, value)| format!("  {key}={value}"))
                .collect::<Vec<String>>()
                .join("\n");
            command.envs(envs);
            print_info(format!("Environment variables:\n{envs_string}"));
        }
        if capture {
            command.args(["--color", "never"]);
        }
        if !args.is_empty() {
            command.arg("--");
            command.args(args);
        }
        if let Some(tolerance) = tolerance {
            command.arg(format!("--tolerance={tolerance}"));
        }

        let output = command
            .stderr(stdio())
            .stdout(stdio())
            .output()
            .expect("Launching benchmark should succeed");

        if let Some(teardown) = teardown {
            let teardown_path = dir.path().join("teardown");
            std::fs::write(&teardown_path, teardown)
                .expect("Preparing the file with the teardown content should succeed");

            print_info("Running teardown:");
            let status = Command::new("bash")
                .args(["-eux"])
                .arg(teardown_path)
                .status()
                .expect("Spawning the teardown process should succeed");

            if !status.success() {
                panic!("Running teardown failed with {status:?}");
            }
        }

        BenchmarkOutput {
            output,
            is_tolerance: tolerance.is_some(),
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub fn run_template(
        &self,
        template_path: &Path,
        cargo_args: &[String],
        args: &[String],
        envs: &HashMap<String, String>,
        template_data: &HashMap<String, minijinja::Value>,
        meta: &Metadata,
        capture: bool,
        tolerance: Option<f64>,
        setup: Option<&str>,
        teardown: Option<&str>,
    ) -> BenchmarkOutput {
        let mut template_string = String::new();
        File::open(self.dir.join(template_path))
            .expect("File should exist")
            .read_to_string(&mut template_string)
            .expect("Reading to string should succeed");

        let mut env = Environment::new();
        env.add_template(&self.bench_name, &template_string)
            .unwrap();
        let template = env.get_template(&self.bench_name).unwrap();

        let dest = File::create(meta.get_template()).unwrap();
        template.render_captured_to(template_data, dest).unwrap();

        self.run_bench(cargo_args, args, envs, capture, tolerance, setup, teardown)
    }

    pub fn run(
        &self,
        num_groups: usize,
        group_index: usize,
        group: &GroupConfig,
        meta: &Metadata,
        schema: &ScopedSchema<'_>,
    ) {
        if !group.runs_on.as_ref().is_none_or(|(is_target, target)| {
            if *is_target {
                target == env!("GR_BUILD_TRIPLE")
            } else {
                target != env!("GR_BUILD_TRIPLE")
            }
        }) || !group
            .rust_version
            .as_ref()
            .is_none_or(|(cmp, version)| meta.compare_rust_version(*cmp, version))
        {
            return;
        }

        self.clean_benchmark();

        let num_runs = group.runs.len();
        for (index, run) in group
            .runs
            .iter()
            .filter(|r| {
                r.runs_on.as_ref().is_none_or(|(is_target, target)| {
                    if *is_target {
                        target == env!("GR_BUILD_TRIPLE")
                    } else {
                        target != env!("GR_BUILD_TRIPLE")
                    }
                }) && r
                    .rust_version
                    .as_ref()
                    .is_none_or(|(cmp, version)| meta.compare_rust_version(*cmp, version))
            })
            .enumerate()
        {
            let max_tries = run.flaky.unwrap_or(0);
            let backup_dir = if max_tries > 0 { self.backup() } else { None };

            for tries in 0..=max_tries {
                print_info(format!(
                    "Running {}: Group: ({}/{num_groups}), Run: ({}/{})",
                    self.config_name,
                    group_index + 1,
                    index + 1,
                    num_runs
                ));

                for r in run.rmdirs.iter().filter(|r| r.is_dir()) {
                    print_info(format!("Removing directory: {}", r.display()));
                    std::fs::remove_dir_all(r).unwrap();
                }

                if !run.cargo_args.is_empty() {
                    print_info(format!("Cargo arguments: {}", run.cargo_args.join(" ")))
                }

                if !run.args.is_empty() {
                    print_info(format!("Benchmark arguments: {}", run.args.join(" ")))
                }

                let capture = run
                    .expected
                    .as_ref()
                    .is_some_and(|e| e.stdout.is_some() || e.stderr.is_some());

                let output = if let Some(template) = &self.config.template {
                    let output = self.run_template(
                        template,
                        &run.cargo_args,
                        &run.args,
                        &run.envs,
                        &run.template_data,
                        meta,
                        capture,
                        run.tolerance,
                        run.setup.as_deref(),
                        run.teardown.as_deref(),
                    );
                    self.reset_template(meta);
                    output
                } else {
                    self.run_bench(
                        &run.cargo_args,
                        &run.args,
                        &run.envs,
                        capture,
                        run.tolerance,
                        run.setup.as_deref(),
                        run.teardown.as_deref(),
                    )
                };

                if tries < max_tries {
                    if panic::catch_unwind(AssertUnwindSafe(|| {
                        run.assert(
                            &self.dir,
                            meta,
                            output,
                            schema,
                            &self.home_dir,
                            &self.bench_name,
                            &group.expected,
                        )
                    }))
                    .is_ok()
                    {
                        break;
                    } else {
                        print_info(format!(
                            "Flaky test: Re-running {}: ({}/{max_tries})",
                            self.config_name,
                            tries + 1,
                        ));
                        self.restore(backup_dir.as_ref());
                    }
                } else {
                    run.assert(
                        &self.dir,
                        meta,
                        output,
                        schema,
                        &self.home_dir,
                        &self.bench_name,
                        &group.expected,
                    )
                }
            }

            drop(backup_dir);
        }
    }

    fn reset_template(&self, meta: &Metadata) {
        let mut file = File::create(meta.get_template()).unwrap();
        file.write_all(TEMPLATE_CONTENT.as_bytes()).unwrap();
    }
}

impl BenchmarkOutput {
    fn assert(&self, bench_dir: &Path, meta: &Metadata, expected: &ExpectedConfig) {
        let output = &self.output;

        print_info("STDERR:");
        stderr().write_all(&output.stderr).unwrap();
        print_info("STDOUT:");
        stdout().write_all(&output.stdout).unwrap();

        if expected.no_stderr {
            let filtered = self.filter_stderr(&output.stderr);
            if filtered.is_empty() {
                print_info("Verifying stderr successful: Expected no stderr");
            } else {
                panic!("Assertion of stderr failed: Expected no stderr");
            }
        } else if let Some(stderr) = &expected.stderr {
            let mut expected_stderr: Vec<u8> = Vec::new();
            File::open(bench_dir.join(stderr))
                .expect("File should exist")
                .read_to_end(&mut expected_stderr)
                .expect("Reading file should succeed");

            let filtered = self.filter_stderr(&output.stderr);
            let expected_string: String = String::from_utf8_lossy(&expected_stderr).into();

            if option_env!("BENCH_OVERWRITE").map_or(false, |s| s.eq_ignore_ascii_case("yes")) {
                if filtered != expected_string {
                    print!(
                        "{}",
                        pretty_assertions::StrComparison::new(&filtered, &expected_string)
                    );

                    File::create(bench_dir.join(stderr))
                        .expect("Opening expected stderr for writing should succeed")
                        .write_all(filtered.as_bytes())
                        .expect("Writing to expected stderr should succeed");

                    print_info("Overwriting stderr successful");
                } else {
                    print_info("Skip overwrite since verifying stderr was successful");
                }
            } else {
                if filtered != expected_string {
                    panic!(
                        "Assertion of stderr failed: {}",
                        pretty_assertions::StrComparison::new(&filtered, &expected_string)
                    );
                }

                print_info("Verifying stderr successful");
            }
        }

        if expected.no_stdout {
            let filtered = self.filter_stdout(&output.stdout);
            if filtered.is_empty() {
                print_info("Verifying stdout successful: Expected no stdout");
            } else {
                panic!("Assertion of stdout failed: Expected no stdout");
            }
        } else if let Some(stdout) = &expected.stdout {
            let mut expected_stdout: Vec<u8> = Vec::new();
            File::open(bench_dir.join(stdout))
                .expect("File should exist")
                .read_to_end(&mut expected_stdout)
                .expect("Reading file should succeed");

            let mut filtered = self.filter_stdout(&output.stdout);
            let mut expected_string = String::from_utf8_lossy(&expected_stdout).into();

            if option_env!("BENCH_OVERWRITE").map_or(false, |s| s.eq_ignore_ascii_case("yes")) {
                if filtered != expected_string {
                    print!(
                        "{}",
                        pretty_assertions::StrComparison::new(&filtered, &expected_string)
                    );

                    File::create(bench_dir.join(stdout))
                        .expect("Opening expected stdout for writing should succeed")
                        .write_all(filtered.as_bytes())
                        .expect("Writing to expected stdout should succeed");

                    print_info("Overwriting stdout successful");
                } else {
                    print_info("Skip overwrite since verifying stdout was successful");
                }
            } else {
                if meta.is_coverage_run {
                    filtered = Self::normalize_coverage_stdout(filtered);
                    expected_string = Self::normalize_coverage_stdout(expected_string);
                }

                if filtered != expected_string {
                    panic!(
                        "Assertion of stdout failed: {}",
                        pretty_assertions::StrComparison::new(&filtered, &expected_string)
                    );
                }
                print_info("Verifying stdout successful");
            }
        }
    }

    fn normalize_coverage_stdout(stdout: String) -> String {
        let mut result = String::new();
        for line in stdout.lines() {
            let (indent, line) = if line.starts_with("  ") || line.starts_with("|") {
                (&line[0..2], &line[2..])
            } else {
                (&line[0..0], line)
            };

            let line = if line.starts_with("Reads bytes") || line.starts_with("Writes bytes") {
                NUMBERS_DIFF_RE.replace(line, "(         )")
            } else {
                Cow::Borrowed(line)
            };

            writeln!(result, "{indent}{line}").unwrap();
        }
        result
    }

    fn filter_stderr(&self, stderr: &[u8]) -> String {
        let mut result = String::new();
        let mut start = false;
        let mut first = false;
        for line in stderr.lines().map(Result::unwrap) {
            if !start {
                if RUNNING_RE.is_match(&line) {
                    start = true;
                    first = true;
                }
                continue;
            } else if first {
                if line.trim().is_empty() {
                    first = false;
                    continue;
                }
                first = false;
            }

            let line = if let Some(caps) = THREAD_PANICKED.captures(&line) {
                let mut new = String::with_capacity(line.len());
                new.push_str(caps.name("start").unwrap().as_str());
                if caps.name("pid").is_some() {
                    new.push_str("(<__PID__>)");
                }

                new.push_str(caps.name("end").unwrap().as_str());

                new
            } else {
                line
            };

            let line = PROCESS_DID_NOT_EXIT_SUCCESSFULLY_RE.replace(&line, "$1<__PATH__>$3");
            let line = ABSOLUTE_PATH_APOSTROPHE_RE.replace(&line, "'<__ABS_PATH__>$1'");
            let line = REGRESSION_SOFT_RE
                .replace(&line, "$1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__NUM__>$9");
            let line = REGRESSION_HARD_RE.replace(&line, "$1<__NUM__>$3<__DIFF__>$5<__LIMIT__>$7");
            writeln!(result, "{line}").unwrap();
        }
        result
    }

    fn filter_stdout(&self, stdout: &[u8]) -> String {
        let mut result = String::new();
        let mut details = false;
        for line in stdout.lines().map(Result::unwrap) {
            let (indent, line) = if line.starts_with("  ") || line.starts_with("|") {
                (&line[0..2], &line[2..])
            } else {
                (&line[0..0], line.as_str())
            };

            let line = if let Some(caps) = THREAD_PANICKED.captures(line) {
                let mut new = String::with_capacity(line.len());
                new.push_str(caps.name("start").unwrap().as_str());
                if caps.name("pid").is_some() {
                    new.push_str("(<__PID__>)");
                }

                new.push_str(caps.name("end").unwrap().as_str());

                new
            } else {
                line.to_owned()
            };

            let line = line.as_str();

            // The `  Details: ...` can contain platform, toolchain specific information about a
            // tool run and make the benchmark tests flaky. So, we filter the details. The
            // (multiline) details usually look like this in the original output:
            //
            // ```
            //   Command:            target/release/deps/test_lib_bench_tools-85f9071c66a70881
            //   Details:            # Thread 1
            //                       #   Total intervals: 0 (Interval Size 100000000)
            //                       #   Total instructions: 459813
            //                       #   Total reps: 499
            //                       #   Unique reps: 5
            //                       #   Total fldcw instructions: 0
            //   Command:            target/release/sort
            //   Details:            # Thread 1
            //                       #   Total intervals: 1 (Interval Size 100000000)
            //                       #   Total instructions: 104432528
            //                       #   Total reps: 457
            //                       #   Unique reps: 4
            //                       #   Total fldcw instructions: 0
            // ```
            //
            // and are transformed into: (The benchmark `Command` is also filtered. See below.)
            //
            // ```
            //   Command: <__COMMAND__>
            //   Details: <__DETAILS__>
            //   Command:            target/release/sort
            //   Details: <__DETAILS__>
            // ```
            if details {
                if NOT_DETAILS_RE.is_match(line) {
                    details = false;
                } else {
                    continue;
                }
            } else if DETAILS_RE.is_match(line) {
                writeln!(result, "{indent}Details: <__DETAILS__>").unwrap();
                details = true;
                continue;
            }

            if let Some(caps) = NUMBERS_RE.captures(line) {
                let mut string = String::new();
                let desc = filter_unit(caps.name("desc").unwrap().as_str());
                let comp1 = {
                    let cap = caps.name("comp1").unwrap().as_str();
                    if cap.parse::<f64>().is_ok() {
                        " ".repeat(cap.len())
                    } else {
                        cap.to_owned()
                    }
                };
                let comp2 = {
                    let cap = caps.name("comp2").unwrap().as_str();
                    if cap.parse::<f64>().is_ok() {
                        " ".repeat(cap.len())
                    } else {
                        cap.to_owned()
                    }
                };
                write!(string, "{desc}{comp1}|{comp2}").unwrap();

                // RAM Hits (and EstimatedCycles, L1, LL Hits) events are unreliable across
                // different systems/toolchains and deviate by a few counts up or down. So to keep
                // the output comparison more reliable we change this line from (for example)
                //
                //   RAM Hits:             179|209             (-14.3541%) [-1.16760x]
                //   RAM Hits:             179|179             (No Change)
                //
                // to
                //
                //   RAM Hits:             179|209             (         )
                //
                // and
                //
                //   RAM Hits:             179|N/A             (*********)
                //
                // to
                //
                //   RAM Hits:                |N/A             (*********)

                // Callgrind/Cachegrind
                if desc.starts_with("RAM Hits")
                    || desc.starts_with("Estimated Cycles")
                    || desc.starts_with("LL Hits")
                    || desc.starts_with("L1 Hits")
                    || desc.starts_with("SysTime")
                    || desc.starts_with("SysCpuTime")
                    // Error tools like Memcheck
                    || desc.starts_with("Suppressed Errors")
                    || desc.starts_with("Suppressed Contexts")
                    // DHAT
                    || desc.starts_with("At t-gmax bytes")
                    || desc.starts_with("At t-gmax blocks")
                {
                    if caps.name("diff_percent").is_some() {
                        let white1 = caps.name("white1").unwrap().as_str();
                        let percent = caps.name("percent").unwrap().as_str();
                        if percent == "(*********)" {
                            write!(string, "{white1}{percent}").unwrap();
                        } else {
                            write!(string, "{white1}(         )").unwrap();
                        }
                    }
                } else {
                    if caps.name("diff_percent").is_some() {
                        let white1 = caps.name("white1").unwrap().as_str();
                        let percent = caps.name("percent").unwrap().as_str();
                        let num = &percent[1..percent.len() - 2];
                        let pos = num.find(['+', '-', '>']);

                        match pos {
                            Some(pos)
                                if num[pos + 1..].parse::<f64>().is_ok()
                                    || percent == "(---inf---)"
                                    || percent == "(+++inf+++)" =>
                            {
                                write!(string, "{white1}(        %)").unwrap();
                            }
                            Some(_) | None if self.is_tolerance && percent == "(No change)" => {
                                write!(string, "{white1}(Tolerance)").unwrap();
                            }
                            Some(_) | None => {
                                write!(string, "{white1}{percent}").unwrap();
                            }
                        }
                    }
                    if caps.name("diff_factor").is_some() {
                        let white2 = caps.name("white2").unwrap().as_str();
                        let factor = caps.name("factor").unwrap().as_str();
                        let num = &factor[1..factor.len() - 2];
                        let pos = num.find(['+', '-', ' ']);

                        match pos {
                            Some(pos)
                                if num[pos + 1..].parse::<f64>().is_ok()
                                    || factor == "[---inf---]"
                                    || factor == "[+++inf+++]" =>
                            {
                                write!(string, "{white2}[        x]").unwrap();
                            }
                            Some(_) | None => {
                                write!(string, "{white2}{factor}").unwrap();
                            }
                        }
                    }
                }
                writeln!(result, "{indent}{string}").unwrap();
            } else {
                let line = if COMMAND_RE.is_match(line) {
                    // Filter the benchmark command of library benchmarks because it has a random
                    // hash in it's name
                    COMMAND_RE.replace(line, "$1 <__COMMAND__>")
                } else {
                    // Replace absolute paths
                    ABSOLUTE_PATH_RE.replace_all(line, "$1<__ABS_PATH__>$2")
                };

                // Filter the pids and parent pids
                let line = PID_RE.replace_all(&line, |caps: &Captures| {
                    format!("{}<__PID__>{}", &caps[1], caps.get(3).map_or("", |_| " "))
                });

                // Fix the spaces after replacement of pids
                let line = FRAGMENT_HEADER_RE.replace_all(&line, |caps: &Captures| {
                    let caps_1 = &caps[1];
                    let caps_3 = &caps[3];
                    if caps_1.len() < 40 {
                        format!(
                            "{caps_1}{}{caps_3}",
                            " ".repeat(gungraun_runner::runner::format::LEFT_WIDTH - caps_1.len())
                        )
                    } else {
                        format!("{caps_1} {caps_3}")
                    }
                });
                let line = SUMMARY_REGRESSION_SOFT_RE.replace_all(
                    &line,
                    "$1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__PERCENT__>$9",
                );

                let line = SUMMARY_REGRESSION_HARD_RE
                    .replace_all(&line, "$1<__NUM__>$3<__NUM__>$5<__LIMIT__>$7<__DIFF__>$9");

                let line = SUMMARY_LINE_RE.replace_all(&line, "$1<__SECONDS__>$3");
                writeln!(result, "{indent}{line}").unwrap();
            }
        }

        result
    }

    fn assert_exit(&self, exit_code: Option<i32>) {
        match exit_code {
            Some(expected) => match self.output.status.code() {
                Some(code) => {
                    assert_eq!(
                        expected, code,
                        "Expected benchmark to exit with code '{expected}' but exited with code \
                         '{code}'"
                    );
                    print_info(format!(
                        "Verifying exit code was successful: Process exited with '{code}'"
                    ));
                }
                None => panic!(
                    "Expected benchmark to exit with code '{expected}' but exited with signal '{}'",
                    self.output.status.signal().unwrap()
                ),
            },
            None => assert!(
                self.output.status.success(),
                "Expected benchmark to exit with success"
            ),
        }
    }
}

impl BenchmarkRunner {
    pub fn new(
        benches: &[String],
        filter: Option<String>,
        partition: Option<Partition>,
        resume: bool,
    ) -> Self {
        Self {
            metadata: Metadata::new(benches, filter, partition, resume),
        }
    }

    pub fn run(&self) -> Result<(), String> {
        // We need the `summary.json` files to verify that not all costs are zero. Extracting this
        // info from the summary is much easier than doing it from the output.
        // SAFETY: Benchmarks are run serially
        unsafe { std::env::set_var("GUNGRAUN_SAVE_SUMMARY", "json") };
        // SAFETY: Benchmarks are run serially
        unsafe {
            std::env::set_var(
                "GUNGRAUN_RUNNER",
                self.metadata
                    .target_directory
                    .join("release/gungraun-runner"),
            )
        };

        let schema: serde_json::Value = serde_json::from_reader(
            File::open(
                self.metadata
                    .workspace_root
                    .join(SCHEMA_PATH)
                    .join(format!("summary.v{SCHEMA_VERSION}.schema.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let mut scope = json_schema::Scope::new();
        let compiled = scope.compile_and_return(schema, false).unwrap();

        build_gungraun_runner();

        for bench in &self.metadata.benchmarks {
            let num_groups = bench.config.groups.len();
            for (index, group) in bench.config.groups.iter().enumerate() {
                bench.run(num_groups, index, group, &self.metadata, &compiled);
            }
        }

        let _ = std::fs::remove_file(
            self.metadata
                .target_directory
                .join("gungraun")
                .join(CONTINUE_FILE_NAME),
        );

        Ok(())
    }
}

impl ExpectedRun {
    pub fn assert(&self, base_dir: &Path, schema: &ScopedSchema) {
        let mut env = Environment::default();
        env.add_template("function", &self.function).unwrap();
        let template = env.get_template("function").unwrap();
        let function = template.render(TEMPLATE_DATA.get().unwrap()).unwrap();

        let dir = if let Some(id) = &self.id {
            base_dir.join(&self.group).join(format!("{function}.{id}"))
        } else {
            base_dir.join(&self.group).join(&function)
        };
        print_info(format!(
            "Running assertions in directory '{}'",
            dir.display()
        ));

        assert!(
            dir.exists(),
            "Expected benchmark directory '{}' to exist",
            dir.display()
        );

        let mut real_files = glob(&format!("{}/*", dir.display()))
            .expect("Glob pattern should compile")
            .map(Result::unwrap)
            .collect::<HashSet<PathBuf>>();

        let mut summary = None;
        for file in self.expected.files.iter().map(|f| dir.join(f)) {
            if let Some(file_name) = file.file_name() {
                if file_name == "summary.json" {
                    summary = Some(file.clone());
                }
            }
            // Gungraun does not produce empty files and if so we treat it as an error
            assert!(
                real_files.remove(&file),
                "Expected file '{}' does not exist",
                file.display()
            );
            assert_ne!(
                std::fs::metadata(&file).unwrap().len(),
                0,
                "Expected file '{}' was empty",
                file.display()
            );
        }

        for ExpectedGlob { pattern, count } in self.expected.globs.iter() {
            let pattern = &dir.join(pattern).display().to_string();
            let files = glob(pattern)
                .expect("Glob pattern should compile")
                .map(Result::unwrap)
                .collect::<Vec<PathBuf>>();

            assert_eq!(
                files.len(),
                *count,
                "Expected file count for glob '{pattern}' was {} but found {} files",
                *count,
                files.len()
            );

            for file in files.into_iter() {
                if let Some(file_name) = file.file_name() {
                    if file_name == "summary.json" {
                        summary = Some(file.clone());
                    }
                }
                real_files.remove(&file);
            }
        }

        if let Some(summary) = summary {
            print_info(format!("Validating summary {}", summary.display()));
            let instance: serde_json::Value =
                serde_json::from_reader(File::open(&summary).unwrap()).unwrap();
            let result = schema.validate(&instance);
            if !result.is_valid() {
                for error in result.errors {
                    print_error(format!("{}: Validation error: {error}", summary.display()))
                }
            }
            let (_, value) = instance
                .as_object()
                .unwrap()
                .get_key_value("version")
                .unwrap();
            assert_eq!(
                value, SCHEMA_VERSION,
                "summary json schema version mismatch"
            );
        }

        assert!(
            real_files.is_empty(),
            "Expected no other files in directory '{}' but found: {:#?}",
            dir.display(),
            real_files
        );
    }
}

impl Metadata {
    pub fn new(
        benches: &[String],
        filter: Option<String>,
        partition: Option<Partition>,
        resume: bool,
    ) -> Self {
        let meta = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .exec()
            .unwrap();

        let package_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let benches_dir = package_dir.join("benches");
        let workspace_root = meta.workspace_root.clone().into_std_path_buf();
        let target_directory = meta.target_directory.clone().into_std_path_buf();

        let mut benchmarks = glob(&format!("{}/**/*.conf.yml", benches_dir.display()))
            .unwrap()
            .map(Result::unwrap)
            .filter(|path| {
                let file_name = path.file_name().unwrap().to_string_lossy();
                let name = &file_name.strip_suffix(".conf.yml").unwrap().to_string();
                if let Some(filter) = filter.as_ref() {
                    filter.as_str().dowild(name) && (benches.is_empty() || benches.contains(name))
                } else if benches.is_empty() {
                    true
                } else {
                    benches.contains(name)
                }
            })
            .map(|path| Benchmark::new(&path, &package_dir, &target_directory))
            .collect::<Vec<Benchmark>>();

        benchmarks.sort_by_key(|b| b.config_name.clone());
        if let Some(partition) = partition {
            let chunk_size = benchmarks.len().div_ceil(partition.total);
            let chunk = benchmarks
                .chunks(chunk_size)
                .nth(partition.part - 1)
                .map(|c| c.to_vec());
            benchmarks = chunk.expect("The partition should map to a chunk of all benchmarks");
        }

        // We do not check the exact command, so it is possible to resume at any point. The only
        // condition is that the benchmark name must be part of the new command.
        if resume {
            let benchmark =
                std::fs::read_to_string(target_directory.join("gungraun").join(CONTINUE_FILE_NAME))
                    .expect("The continue file should exist");
            let benchmark = benchmark.trim();
            print_info(format!("Continue with {benchmark}"));

            let index = benchmarks
                .iter()
                .position(|b| b.config_name == benchmark)
                .unwrap_or_else(|| {
                    panic!("Benchmark '{benchmark}' from continue file was not found")
                });

            benchmarks.drain(..index);
        }

        print_info("Benchmarks to run:");
        benchmarks
            .iter()
            .for_each(|b| println!("  {}", b.config_name));

        let rust_version = get_rust_version().expect("Rust version should be present");

        Self {
            workspace_root,
            target_directory,
            benchmarks,
            benches_dir,
            rust_version,
            is_coverage_run: std::env::var(CARGO_LLVM_COV).is_ok_and(|v| v == "1"),
        }
    }

    pub fn get_template(&self) -> PathBuf {
        self.benches_dir.join(format!("{TEMPLATE_BENCH_NAME}.rs"))
    }

    pub fn compare_rust_version(&self, cmp: version_compare::Cmp, version: &str) -> bool {
        if version.starts_with(|p: char| p.is_ascii_digit()) {
            version_compare::compare_to(self.rust_version.semver.to_string(), version, cmp).unwrap()
        } else {
            let channel = match version {
                "nightly" => Channel::Nightly,
                "stable" => Channel::Stable,
                "dev" => Channel::Dev,
                "beta" => Channel::Beta,
                _ => panic!("Invalid version string: {version}"),
            };
            match cmp {
                version_compare::Cmp::Eq => self.rust_version.channel == channel,
                version_compare::Cmp::Ne => self.rust_version.channel != channel,
                _ => panic!(
                    "Invalid comparator for channel: {version}. Only '=' and '!=' are allowed."
                ),
            }
        }
    }
}

impl RunConfig {
    #[allow(clippy::too_many_arguments)]
    fn assert(
        &self,
        bench_dir: &Path,
        meta: &Metadata,
        output: BenchmarkOutput,
        schema: &ScopedSchema<'_>,
        home_dir: &Path,
        bench_name: &str,
        group_expected: &GroupExpected,
    ) {
        if let Some(expected) = &self.expected {
            if expected.stdout.is_some()
                || expected.no_stdout
                || expected.stderr.is_some()
                || expected.no_stderr
            {
                output.assert(bench_dir, meta, expected);
            }
            output.assert_exit(expected.exit_code);

            if let Some(script) = expected.script.as_ref().or(group_expected.script.as_ref()) {
                let dir = tempdir().expect(
                    "Creating a temporary directory for the assertion script should succeed",
                );

                let base_dir = home_dir.join(PACKAGE).join(bench_name);

                let assert_path = dir.path().join("assert");
                std::fs::write(&assert_path, script)
                    .expect("Preparing the file with the script content should succeed");
                print_info("Running assertion script:");
                let status = Command::new("bash")
                    .current_dir(base_dir)
                    .args(["-ex"])
                    .arg(assert_path)
                    .status()
                    .expect("Spawning the assertion script should succeed");

                if !status.success() {
                    panic!("Running assertion script failed with {status:?}");
                }
            }

            if let Some(files) = &expected.files {
                let expected_runs: ExpectedRuns = serde_yaml::from_reader(
                    File::open(bench_dir.join(files)).expect("File should exist"),
                )
                .map_err(|error| format!("Failed to deserialize '{}': {error}", files.display()))
                .expect("File should be deserializable");

                let dest_dir = if let Some(home_dir) = expected_runs.home_dir {
                    home_dir.join(PACKAGE).join(bench_name)
                } else {
                    home_dir.join(PACKAGE).join(bench_name)
                };

                for expected in expected_runs.data {
                    expected.assert(&dest_dir, schema);
                }
            } else if expected.no_files {
                let package_dir = home_dir.join(PACKAGE);
                let base_dir = package_dir.join(bench_name);

                if base_dir.exists() {
                    let list = glob(&format!("{}/**/*", base_dir.display()))
                        .unwrap()
                        .map(Result::unwrap)
                        .fold(String::new(), |mut acc, p| {
                            let display = p.strip_prefix(&package_dir).unwrap().display();
                            acc.push_str(&format!("  {display}\n"));
                            acc
                        });
                    panic!(
                        "The benchmark directory '{}' was not expected to exist but found:\n{list}",
                        base_dir.display()
                    );
                } else {
                    print_info(format!(
                        "Verifying the benchmark directory '{}' not exists was successful",
                        base_dir.display()
                    ));
                }
            } else {
                // do nothing
            }
        }

        if self
            .expected
            .as_ref()
            .is_some_and(|expected| !expected.zero_metrics)
        {
            let base_dir = home_dir.join(PACKAGE).join(bench_name);
            // These checks heavily depends on the creation of the `summary.json` files, but we
            // create them by default.
            for path in glob(&format!("{}/**/summary.json", base_dir.display()))
                .unwrap()
                .map(Result::unwrap)
            {
                let summary = Summary::new(&path).unwrap();
                summary.assert_costs_not_all_zero();
                print_info("Verifying costs not all zero successful");
            }
        }
    }
}

fn build_gungraun_runner() {
    print_info("Building gungraun-runner");
    let status = Command::new(env!("CARGO"))
        .args(["build", "--package", "gungraun-runner", "--release"])
        .status()
        .unwrap();
    assert!(status.success());
}

fn print_error<T>(message: T)
where
    T: AsRef<str>,
{
    eprintln!(
        "{}: {}: {}",
        "bench".purple().bold(),
        "Error".red().bold(),
        message.as_ref()
    );
}

fn print_info<T>(message: T)
where
    T: AsRef<str>,
{
    eprintln!("{}: {}", "bench".purple().bold(), message.as_ref());
}

fn get_rust_version() -> Option<VersionMeta> {
    rustc_version::version_meta().ok()
}

fn main() {
    let mut benches = Vec::default();
    let mut filter = Option::default();
    let mut partition = Option::default();
    let mut resume = false;
    for arg in std::env::args().skip(1) {
        match arg.split_once("=") {
            Some(("--filter", value)) => filter = Some(value.to_owned()),
            Some(("--partition", value)) => {
                if let Some((part_str, total_str)) = value.split_once("/") {
                    let part = part_str
                        .parse::<usize>()
                        .expect("The partition nominator should be a valid number");
                    let total = total_str
                        .parse::<usize>()
                        .expect("The partition nominator should be a valid number");
                    assert!(
                        total > 0,
                        "The total or a partition should be greater than zero"
                    );
                    assert!(
                        part > 0 && part <= total,
                        "The part of a partition should be within bounds: 0 < x <= total"
                    );
                    partition = Some(Partition { part, total })
                } else {
                    panic!("Invalid partition: {value}");
                }
            }
            Some(_) => panic!("Invalid argument: {arg}"),
            None if arg == "--continue" => resume = true,
            None => benches.push(arg),
        }
    }

    let runner = BenchmarkRunner::new(&benches, filter, partition, resume);

    let mut map = HashMap::new();
    map.insert(
        "target_dir_sanitized".to_owned(),
        minijinja::Value::from_serialize(
            runner
                .metadata
                .target_directory
                .display()
                .to_string()
                .replace('/', "_"),
        ),
    );

    TEMPLATE_DATA.set(map).unwrap();

    if let Err(error) = runner.run() {
        print_error(error);
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::lib_bench("Command: target/release/deps/test_lib_bench_threads-c2a88f916ff580f9")]
    #[case::bin_bench("Command: target/release/deps/test_bin_bench_threads-c2a88f916ff580f9")]
    fn test_command_re(#[case] haystack: &str) {
        assert!(COMMAND_RE.is_match(haystack));
    }

    #[rstest]
    #[case::just_root(" /", " <__ABS_PATH__>/")]
    #[case::with_single_component(" /some", " <__ABS_PATH__>/some")]
    #[case::with_two_components(" /some/final", " <__ABS_PATH__>/final")]
    #[case::with_mixed_characters(" /wi-th_/123/final", " <__ABS_PATH__>/final")]
    #[case::with_text_before(
        "some text before /wi-th_/123/final",
        "some text before <__ABS_PATH__>/final"
    )]
    #[case::with_text_after(
        " /wi-th_/123/final some text after",
        " <__ABS_PATH__>/final some text after"
    )]
    fn test_absolute_path_re(#[case] haystack: &str, #[case] replaced: &str) {
        assert_eq!(
            ABSOLUTE_PATH_RE.replace_all(haystack, "$1<__ABS_PATH__>$2"),
            replaced
        );
    }

    #[rstest]
    #[case::valgrind(
        "Instructions:                                       1234|1234                 \
         (12345678%) [1234.1234x]"
    )]
    #[case::valgrind_na(
        "Instructions:                                        123|N/A                  (*********)"
    )]
    #[case::number_in_event(
        "L1 Hits:                                             123|1                    \
         (1.234567%) [2.3456789x]"
    )]
    #[case::valgrind_with_special(
        "Total read+write:                                 123456|12345                \
         (1.234567%) [123.45678%]"
    )]
    #[case::perf_without_unit(
        "cpu_core/instructions/u:                             N/A|1234                 (*********)"
    )]
    #[case::perf_with_unit(
        "task-clock/u [us]:                                 0.123|1.234                \
         (123456.8%) [1234.1234x]"
    )]
    #[case::perf_rse(
        "  rse% (sig.thr) [sig.fact]                        2.345|3.4567890            \
         (9.876543%) [234.56789x]"
    )]
    #[case::perf_samples(
        "  samples                                     1000000000|20000000000000000000 \
         (3456.123%) [1234.1234x]"
    )]
    fn test_numbers_re(#[case] haystack: &str) {
        assert!(NUMBERS_RE.is_match(haystack));
    }

    #[rstest]
    #[case::perf_with_unit("task-clock/u [us]:", "task-clock/u [  ]:")]
    #[case::perf_without_unit("cpu_core/instructions/u:", "cpu_core/instructions/u:")]
    #[case::non_unit_brackets("rse% (sig.thr) [sig.fact]", "rse% (sig.thr) [sig.fact]")]
    fn test_filter_unit(#[case] haystack: &str, #[case] replaced: &str) {
        assert_eq!(filter_unit(haystack), replaced);
    }

    #[rstest]
    #[case::instructions_positive_when_0_allowed(
        "Performance has regressed: Instructions (133 -> 196) regressed by +47.3684% (>+0.00000%)",
        "Performance has regressed: Instructions (<__NUM__> -> <__NUM__>) regressed by \
         +<__PERCENT__>% (>+<__NUM__>%)"
    )]
    fn test_regression_re(#[case] haystack: &str, #[case] replaced: &str) {
        assert_eq!(
            REGRESSION_SOFT_RE.replace(
                haystack,
                "$1<__NUM__>$3<__NUM__>$5<__PERCENT__>$7<__NUM__>$9"
            ),
            replaced
        );
    }

    #[rstest]
    #[case::callgrind(
        "Performance has regressed: Instructions (70021) exceeds limit by 69821 (>200)"
    )]
    #[case::perf_no_unit(
        "Performance has regressed: cpu_core/instructions/u [*instructions*] (7002804) exceeds \
         limit by 6997804 (>5000)"
    )]
    #[case::perf_with_unit(
        "Performance has regressed: task-clock:u [*task-clock*] (601.931 [us]) exceeds limit by \
         501.931 [us] (>100 [us])"
    )]
    fn test_regression_hard_re(#[case] haystack: &str) {
        assert!(REGRESSION_HARD_RE.is_match(haystack));
    }

    #[rstest]
    #[case::callgrind("Callgrind: Instructions (70021): 70021 exceeds limit of 200 by 69821")]
    #[case::perf_no_unit(
        "Perf: cpu_core/instructions/u [*instructions*] (7002804): 7002804 exceeds limit of 5000 \
         by 6997804"
    )]
    #[case::perf_with_unit(
        "Perf: task-clock:u [*task-clock*] (632.461 [us]): 632.461 [us] exceeds limit of 100 [us] \
         by 532.461 [us]"
    )]
    fn test_summary_hard_regression_re(#[case] haystack: &str) {
        assert!(SUMMARY_REGRESSION_HARD_RE.is_match(haystack));
    }

    #[rstest]
    #[case::one_path_component("Expected '/root/exit-with'", &["'/root/exit-with'", "/exit-with"])]
    #[case::two_path_component(
        "Expected '/some/root/exit-with'",
        &["'/some/root/exit-with'", "/exit-with"]
    )]
    #[case::multiple_path_component(
        "Expected '/some/root/and/more/path/components/exit-with'",
        &["'/some/root/and/more/path/components/exit-with'", "/exit-with"]
    )]
    #[case::two_apostrophes(
        "Expected '/root/exit-with' to exit with '1' but it succeeded",
        &["'/root/exit-with'", "/exit-with"]
    )]
    fn test_absolute_path_apostrophe_re(#[case] haystack: &str, #[case] matches: &[&str]) {
        if !matches.is_empty() {
            let caps = ABSOLUTE_PATH_APOSTROPHE_RE
                .captures(haystack)
                .expect("The regex should succeed to match");

            let caps_debug_string = format!("{caps:?}");
            assert_eq!(caps.len(), matches.len(), "{caps_debug_string}");

            for (cap, mat) in caps.iter().zip(matches) {
                assert_eq!(
                    cap.expect("The capture should be present").as_str(),
                    *mat,
                    "{caps_debug_string}"
                );
            }
        } else {
            assert!(!ABSOLUTE_PATH_APOSTROPHE_RE.is_match(haystack))
        }
    }
}
