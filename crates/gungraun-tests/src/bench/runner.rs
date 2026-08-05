//! Orchestration core of the system-test harness.
//!
//! Turns the declarative `.conf.yml` cases discovered and parsed by [`config`][super::config] into
//! real `cargo bench` executions, then hands each captured result to [`assert`][super::assert] for
//! validation. This module owns the whole lifecycle: discovery, partitioning, resume,
//! summary-schema compilation, the per-run setup/teardown and flaky-retry loop, templating, and
//! final cleanup.
//!
//! # Architecture
//!
//! The types form a deliberate three-tier visibility funnel:
//!
//! - [`SystemTestRunner`] is the sole `pub` type - the thin entry point `main` constructs and
//!   `run`s. Everything behind it is an implementation detail.
//! - [`SystemTests`] owns cross-case state (workspace root, target directory, rust version, the
//!   coverage flag, and the resolved case list) and applies the `--filter`, `--partition`, and
//!   `--continue` selections.
//! - [`SystemTest`] is one case: a `.conf.yml`, its bench-target name, and its output directory.
//!   [`ExecContext`] bundles the per-invocation command pieces (cargo args, env, gungraun args,
//!   setup/teardown, tolerance, and the capture flag) passed into a single `cargo bench` call.
//!
//! # Rationale
//!
//! - **`cargo bench` as substrate**: each run is a real `cargo bench --package <pkg> --bench
//!   <name>` subprocess, so the pipeline under test is exactly what users run. The runner only adds
//!   the `GUNGRAUN_RUNNER` and `GUNGRAUN_SAVE_SUMMARY` env knobs and a setup/teardown shell
//!   bracket.
//! - **Backup/restore around `panic::catch_unwind`**: the flaky-retry loop must not leave a
//!   half-written output directory behind, so each run's artifacts are backed up and restored on
//!   failure before a retry.
//! - **Coverage is a separate code path, not a flag**: under `CARGO_LLVM_COV` the instrumented
//!   machine code shifts DHAT's instruction and memory metrics, so `is_coverage_run` adds an extra
//!   normalization pass to stdout before comparison instead of treating the run like a normal one.
//! - **Templating renders into a fixed target name**: a case's Jinja `template` is rendered into
//!   the throwaway `test_bench_template` bench target and reset after each run, so many
//!   parameterized cases share one cargo target without polluting the workspace.
//! - **Resume via a marker file**: `--continue` consults `gungraun-tests.continue` to skip cases
//!   already completed in this output tree, so a re-sharded or re-tried run does not repeat green
//!   cases.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use fs_extra::dir::CopyOptions;
use glob::glob;
use rustc_version::VersionMeta;
use simplematch::DoWild;
use tempfile::{TempDir, tempdir};
use tera::Tera;
use valico::json_schema;
use valico::json_schema::schema::ScopedSchema;

use super::config::{CapturedOutput, Group, PACKAGE, SystemTestConfig};
use super::expected_files::SCHEMA_VERSION;
use super::io::{deserialize_json, deserialize_yaml, get_rust_version, print_info};
use crate::assert::AssertContext;
use crate::config::Partition;
use crate::expected_files::{SCHEMA_PATH, TEMPLATE_DATA};

const TEMPLATE_BENCH_NAME: &str = "test_bench_template";
const TEMPLATE_CONTENT: &str = r#"fn main() {
    panic!("should be replaced by a rendered template");
}
"#;
const CONTINUE_FILE_NAME: &str = "gungraun-tests.continue";
const CARGO_LLVM_COV: &str = "CARGO_LLVM_COV";

struct ExecContext<'a> {
    cargo_args: &'a [String],
    envs: &'a HashMap<String, String>,
    gungraun_args: &'a [String],
    is_capture: bool,
    setup: Option<&'a str>,
    teardown: Option<&'a str>,
    tolerance: Option<f64>,
}

/// System test case derived from a `.conf.yml` file.
///
/// Example: `test_lib_bench_tools.conf.yml` becomes one `SystemTest` value.
#[derive(Debug, Clone)]
struct SystemTest {
    /// Cargo bench target name.
    ///
    /// This is usually the same as `config_name`, but templated benchmarks all use
    /// `test_bench_template`.
    bench_name: String,
    /// Parsed YAML configuration for this benchmark.
    ///
    /// Contains all groups and runs from `test_lib_bench_tools.conf.yml`.
    config: SystemTestConfig,
    /// Directory containing the benchmark configuration file and expected fixtures.
    ///
    /// Example: `crates/gungraun-tests/benches/test_lib_bench/tools`.
    config_dir: PathBuf,
    /// Original `.conf.yml` file stem.
    ///
    /// This identifies the benchmark configuration and is unique for templated benchmarks too.
    config_name: String,
    /// Root directory for system test output below cargo's target directory.
    ///
    /// Example: `target/gungraun`.
    home_dir: PathBuf,
    /// Directory where this benchmark writes regular output files.
    ///
    /// Example: `target/gungraun/gungraun-tests/test_lib_bench_tools`.
    output_dir: PathBuf,
}

/// Top-level system test runner.
///
/// Owns the metadata used by `bench --filter='test_lib_*'`.
#[derive(Debug)]
pub struct SystemTestRunner {
    /// Resolved workspace, target directory, compiler, and selected benchmark list.
    ///
    /// Contains only the selected benchmarks when `--partition`, `--filter` or `--continue` is
    /// used.
    tests: SystemTests,
}

#[derive(Debug, Clone)]
/// Selected system tests and shared execution metadata.
///
/// Example: produced once from cargo metadata before running system tests.
struct SystemTests {
    /// Path to the `crates/gungraun-tests/benches` directory.
    ///
    /// Example: used to write `test_bench_template.rs`.
    benches_dir: PathBuf,
    /// Benchmarks selected by CLI arguments, filters, partitioning, and resume state.
    ///
    /// Example: only benchmarks matching `--filter='test_lib_*'`.
    cases: Vec<SystemTest>,
    /// Whether the run is a llvm coverage run
    ///
    /// This is usually true when `CARGO_LLVM_COV=1` is set. Coverage instrumentation changes the
    /// benchmarked machine code, so DHAT read/write metrics can differ from normal benchmark
    /// fixtures.
    is_coverage_run: bool,
    /// Rust compiler version metadata used for version-gated runs.
    ///
    /// Example: channel `nightly` or semver `1.86.0`.
    rust_version: VersionMeta,
    /// Cargo target directory from cargo metadata.
    ///
    /// Example: `target` or a custom `CARGO_TARGET_DIR`.
    target_directory: PathBuf,
    /// Cargo workspace root.
    ///
    /// Example: repository root containing `Cargo.toml`.
    workspace_root: PathBuf,
}

impl SystemTest {
    fn new(config_path: &Path, _package_dir: &Path, target_dir: &Path) -> Result<Self> {
        let config: SystemTestConfig = deserialize_yaml(config_path)?;

        let config_name = config_path
            .file_name()
            .with_context(|| {
                format!(
                    "The configuration file '{}' should have a file name",
                    config_path.display()
                )
            })?
            .to_string_lossy();

        let config_name = config_name
            .strip_suffix(".conf.yml")
            .with_context(|| {
                format!(
                    "The configuration file '{}' should end with .conf.yml",
                    config_path.display()
                )
            })?
            .to_owned();

        let bench_name = if config.template.is_some() {
            String::from(TEMPLATE_BENCH_NAME)
        } else {
            config_name.clone()
        };

        let home_dir = target_dir.join("gungraun");

        Ok(Self {
            output_dir: home_dir.join(PACKAGE).join(&bench_name),
            bench_name,
            config_name,
            config,
            config_dir: config_path
                .parent()
                .with_context(|| {
                    format!(
                        "The configuration file '{}' should have a parent directory",
                        config_path.display()
                    )
                })?
                .to_path_buf(),
            home_dir,
        })
    }

    fn clean_benchmark(&self) -> Result<()> {
        if self.output_dir.is_dir() {
            std::fs::remove_dir_all(&self.output_dir).with_context(|| {
                format!(
                    "Failed to remove benchmark directory '{}'",
                    self.output_dir.display()
                )
            })?;
        }
        let alt_dir = self
            .home_dir
            .join(env!("GR_BUILD_TRIPLE"))
            .join(PACKAGE)
            .join(&self.bench_name);
        if alt_dir.is_dir() {
            std::fs::remove_dir_all(&alt_dir).with_context(|| {
                format!(
                    "Failed to remove benchmark directory '{}'",
                    alt_dir.display()
                )
            })?;
        }

        Ok(())
    }

    fn backup(&self) -> Result<Option<TempDir>> {
        if !self.output_dir.is_dir() {
            return Ok(None);
        }

        let temp_dir = tempdir().context("Failed to create temporary backup directory")?;
        fs_extra::copy_items(&[&self.output_dir], temp_dir.path(), &CopyOptions::new())
            .with_context(|| {
                format!(
                    "Failed to back up benchmark directory '{}'",
                    self.output_dir.display()
                )
            })?;
        Ok(Some(temp_dir))
    }

    fn restore(&self, temp_dir: Option<&TempDir>) -> Result<()> {
        self.clean_benchmark()?;

        if let Some(temp_dir) = temp_dir {
            let from = temp_dir
                .path()
                .join(self.output_dir.file_name().with_context(|| {
                    format!(
                        "The output directory '{}' should have a name",
                        self.output_dir.display()
                    )
                })?);
            fs_extra::copy_items(
                &[from],
                self.output_dir
                    .parent()
                    .expect("Parent of benchmark directory should exist"),
                &CopyOptions::new(),
            )
            .with_context(|| {
                format!(
                    "Failed to restore benchmark directory '{}'",
                    self.output_dir.display()
                )
            })?;
        }

        Ok(())
    }

    fn run_bench(&self, ctx: &ExecContext) -> Result<CapturedOutput> {
        let stdio = if ctx.is_capture {
            // SAFETY: Benchmarks are run serially
            unsafe {
                std::env::set_var("GUNGRAUN_COLOR", "never");
            }
            Stdio::piped
        } else {
            // SAFETY: Benchmarks are run serially
            unsafe {
                std::env::set_var("GUNGRAUN_COLOR", "auto");
            }
            Stdio::inherit
        };

        let temp_dir =
            tempdir().context("Failed to create a temporary directory for setup and teardown")?;

        if let Some(setup) = ctx.setup {
            let setup_path = temp_dir.path().join("setup");

            std::fs::write(&setup_path, setup).with_context(|| {
                format!("Failed to write setup script '{}'", setup_path.display())
            })?;

            print_info("Running setup:");
            let status = Command::new("bash")
                .args(["-ex"])
                .arg(setup_path)
                .status()
                .context("Failed to spawn the setup process")?;

            assert!(status.success(), "Running setup failed with {status:?}");
        }

        std::fs::create_dir_all(&self.home_dir).with_context(|| {
            format!(
                "Failed to create gungraun home directory '{}'",
                self.home_dir.display()
            )
        })?;
        std::fs::write(self.home_dir.join(CONTINUE_FILE_NAME), &self.config_name).with_context(
            || {
                format!(
                    "Failed to write continue file '{}'",
                    self.home_dir.join(CONTINUE_FILE_NAME).display()
                )
            },
        )?;

        let mut command = Command::new(env!("CARGO"));
        command.args(["bench", "--package", PACKAGE, "--bench", &self.bench_name]);
        command.args(ctx.cargo_args);

        if !ctx.envs.is_empty() {
            let envs_string = ctx
                .envs
                .iter()
                .map(|(key, value)| format!("  {key}={value}"))
                .collect::<Vec<String>>()
                .join("\n");
            command.envs(ctx.envs);
            print_info(format!("Environment variables:\n{envs_string}"));
        }

        if ctx.is_capture {
            command.args(["--color", "never"]);
        }

        if !ctx.gungraun_args.is_empty() {
            command.arg("--");
            command.args(ctx.gungraun_args);
        }

        if let Some(tolerance) = ctx.tolerance {
            command.arg(format!("--tolerance={tolerance}"));
        }

        let output = command
            .stderr(stdio())
            .stdout(stdio())
            .output()
            .with_context(|| format!("Failed to launch benchmark '{}'", self.config_name))?;

        if let Some(teardown) = ctx.teardown {
            let teardown_path = temp_dir.path().join("teardown");
            std::fs::write(&teardown_path, teardown).with_context(|| {
                format!(
                    "Failed to write teardown script '{}'",
                    teardown_path.display()
                )
            })?;

            print_info("Running teardown:");
            let status = Command::new("bash")
                .args(["-eux"])
                .arg(teardown_path)
                .status()
                .context("Failed to spawn the teardown process")?;

            assert!(status.success(), "Running teardown failed with {status:?}");
        }

        Ok(CapturedOutput {
            output,
            has_tolerance: ctx.tolerance.is_some(),
        })
    }

    fn run_template(
        &self,
        template_path: &Path,
        template_data: &HashMap<String, serde_json::Value>,
        tests: &SystemTests,
        ctx: &ExecContext,
    ) -> Result<CapturedOutput> {
        let mut template_string = String::new();
        let source_path = self.config_dir.join(template_path);
        File::open(&source_path)
            .with_context(|| format!("Failed to open template '{}'", source_path.display()))?
            .read_to_string(&mut template_string)
            .with_context(|| format!("Failed to read template '{}'", source_path.display()))?;

        let mut tera = Tera::default();
        tera.add_raw_template(&self.bench_name, &template_string)
            .with_context(|| format!("Failed to compile template '{}'", source_path.display()))?;
        let destination = tests.get_template();
        let mut dest = File::create(&destination).with_context(|| {
            format!(
                "Failed to create rendered template '{}'",
                destination.display()
            )
        })?;
        let context = tera::Context::from_serialize(template_data).with_context(|| {
            format!(
                "Failed to serialize template data '{}'",
                source_path.display()
            )
        })?;
        tera.render_to(&self.bench_name, &context, &mut dest)
            .with_context(|| format!("Failed to render template '{}'", source_path.display()))?;

        self.run_bench(ctx)
    }

    fn run(
        &self,
        num_groups: usize,
        group_index: usize,
        group: &Group,
        tests: &SystemTests,
        schema: &ScopedSchema<'_>,
    ) -> Result<()> {
        let target_triple = env!("GR_BUILD_TRIPLE");

        if !group.is_enabled(target_triple, &tests.rust_version) {
            return Ok(());
        }

        self.clean_benchmark()?;

        let num_runs = group.runs.len();
        for (index, run) in group
            .runs
            .iter()
            .filter(|r| r.is_enabled(target_triple, &tests.rust_version))
            .enumerate()
        {
            let max_tries = run.flaky.unwrap_or(0);
            let backup_dir = if max_tries > 0 { self.backup()? } else { None };

            for tries in 0..=max_tries {
                print_info(format!(
                    "Running {}: Group: ({}/{num_groups}), Run: ({}/{})",
                    self.config_name,
                    group_index + 1,
                    index + 1,
                    num_runs
                ));

                for r in run.rmdirs.iter().filter(|r| r.is_dir()) {
                    print_info(format!(
                        "rmdirs is set: Removing directory '{}'",
                        r.display()
                    ));
                    std::fs::remove_dir_all(r).with_context(|| {
                        format!("Failed to remove rmdirs directory '{}'", r.display())
                    })?;
                }

                if !run.cargo_args.is_empty() {
                    print_info(format!("Cargo arguments: {}", run.cargo_args.join(" ")));
                }

                if !run.args.is_empty() {
                    print_info(format!("Benchmark arguments: {}", run.args.join(" ")));
                }

                let is_capture = run.expected.as_ref().is_some_and(|target_config| {
                    target_config
                        .resolve(target_triple)
                        .is_some_and(|r| r.expects_output_capture(target_triple))
                });

                let exec_ctx = ExecContext {
                    cargo_args: &run.cargo_args,
                    envs: &run.envs,
                    gungraun_args: &run.args,
                    is_capture,
                    setup: run.setup.as_deref(),
                    teardown: run.teardown.as_deref(),
                    tolerance: run.tolerance,
                };

                let output = if let Some(template) = &self.config.template {
                    let output =
                        self.run_template(template, &run.template_data, tests, &exec_ctx)?;
                    Self::reset_template(tests)?;
                    output
                } else {
                    self.run_bench(&exec_ctx)?
                };

                let assert_ctx = AssertContext {
                    bench_name: &self.bench_name,
                    config_dir: &self.config_dir,
                    group_expectations: group.expected.as_ref(),
                    home_dir: &self.home_dir,
                    is_coverage_run: tests.is_coverage_run,
                    captured_output: &output,
                    schema,
                };

                if tries < max_tries {
                    let result = panic::catch_unwind(AssertUnwindSafe(|| run.assert(&assert_ctx)));
                    if let Ok(result) = result {
                        result?;
                        break;
                    }
                    print_info(format!(
                        "Flaky test: Re-running {}: ({}/{max_tries})",
                        self.config_name,
                        tries + 1,
                    ));
                    self.restore(backup_dir.as_ref())?;
                } else {
                    run.assert(&assert_ctx)?;
                }
            }

            drop(backup_dir);
        }

        Ok(())
    }

    fn reset_template(tests: &SystemTests) -> Result<()> {
        let path = tests.get_template();
        let mut file = File::create(&path)
            .with_context(|| format!("Failed to reset template '{}'", path.display()))?;
        file.write_all(TEMPLATE_CONTENT.as_bytes())
            .with_context(|| format!("Failed to write reset template '{}'", path.display()))
    }
}

impl SystemTestRunner {
    pub fn new(
        benches: &[String],
        filter: Option<&str>,
        partition: Option<Partition>,
        resume: bool,
    ) -> Result<Self> {
        let tests = SystemTests::new(benches, filter, partition, resume)?;

        let mut map = HashMap::new();
        map.insert(
            "target_dir_sanitized".to_owned(),
            serde_json::Value::String(
                tests
                    .target_directory
                    .display()
                    .to_string()
                    .replace('/', "_"),
            ),
        );

        TEMPLATE_DATA
            .set(map)
            .map_err(|_| anyhow!("Failed to initialize template data"))?;

        Ok(Self { tests })
    }

    pub fn run(&self) -> Result<()> {
        // We need the `summary.json` files to verify that not all costs are zero. Extracting this
        // info from the summary is much easier than doing it from the output.
        // SAFETY: Benchmarks are run serially
        unsafe {
            std::env::set_var("GUNGRAUN_SAVE_SUMMARY", "json");
        }
        // SAFETY: Benchmarks are run serially
        unsafe {
            std::env::set_var(
                "GUNGRAUN_RUNNER",
                self.tests.target_directory.join("release/gungraun-runner"),
            );
        }

        let schema: serde_json::Value = deserialize_json(
            &self
                .tests
                .workspace_root
                .join(SCHEMA_PATH)
                .join(format!("summary.v{SCHEMA_VERSION}.schema.json")),
        )?;
        let mut scope = json_schema::Scope::new();
        let compiled = scope
            .compile_and_return(schema, false)
            .map_err(|error| anyhow!("Failed to compile summary schema: {error:?}"))?;

        build_gungraun_runner()?;

        for test in &self.tests.cases {
            let num_groups = test.config.groups.len();
            for (index, group) in test.config.groups.iter().enumerate() {
                test.run(num_groups, index, group, &self.tests, &compiled)?;
            }
        }

        let continue_path = self
            .tests
            .target_directory
            .join("gungraun")
            .join(CONTINUE_FILE_NAME);
        if let Err(error) = std::fs::remove_file(&continue_path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(error).with_context(|| {
                format!(
                    "Failed to remove continue file '{}'",
                    continue_path.display()
                )
            });
        }

        Ok(())
    }
}

impl SystemTests {
    fn new(
        tests: &[String],
        filter: Option<&str>,
        partition: Option<Partition>,
        resume: bool,
    ) -> Result<Self> {
        let meta = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .exec()
            .context("Failed to read cargo metadata")?;

        let package_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let benches_dir = package_dir.join("benches");
        let workspace_root = meta.workspace_root.clone().into_std_path_buf();
        let target_directory = meta.target_directory.into_std_path_buf();

        let cases = Self::discover_config_paths(&benches_dir)
            .and_then(|paths| {
                Self::paths_to_cases(paths, filter, tests, &package_dir, &target_directory)
            })
            .map(Self::sort)
            .and_then(|cases| Self::apply_partition(cases, partition.as_ref()))
            .and_then(|cases| Self::apply_resume(cases, resume, &target_directory))?;

        print_info("Benchmarks to run:");
        for b in &cases {
            println!("  {}", b.config_name);
        }

        let rust_version =
            get_rust_version().ok_or_else(|| anyhow!("Failed to determine Rust version"))?;

        Ok(Self {
            workspace_root,
            target_directory,
            cases,
            benches_dir,
            rust_version,
            is_coverage_run: std::env::var(CARGO_LLVM_COV).is_ok_and(|v| v == "1"),
        })
    }

    fn apply_partition(
        cases: Vec<SystemTest>,
        partition: Option<&Partition>,
    ) -> Result<Vec<SystemTest>> {
        if let Some(partition) = partition {
            if cases.is_empty() {
                bail!("The partition did not match any benchmarks");
            }
            let chunk_size = cases.len().div_ceil(partition.total);
            let chunk = cases
                .chunks(chunk_size)
                .nth(partition.part - 1)
                .map(<[SystemTest]>::to_vec);
            chunk.ok_or_else(|| anyhow!("The partition did not match any benchmarks"))
        } else {
            Ok(cases)
        }
    }

    fn apply_resume(
        mut cases: Vec<SystemTest>,
        resume: bool,
        target_directory: &Path,
    ) -> Result<Vec<SystemTest>> {
        // We do not check the exact command, so it is possible to resume at any point. The only
        // condition is that the benchmark name must be part of the new command.
        if resume {
            let test =
                std::fs::read_to_string(target_directory.join("gungraun").join(CONTINUE_FILE_NAME))
                    .context("Failed to read the continue file")?;
            let test = test.trim();
            print_info(format!("Continue with {test}"));

            let index = cases
                .iter()
                .position(|b| b.config_name == test)
                .ok_or_else(|| anyhow!("Benchmark '{test}' from continue file was not found"))?;

            cases.drain(..index);
        }

        Ok(cases)
    }

    fn discover_config_paths(benches_dir: &Path) -> Result<Vec<PathBuf>> {
        let config_pattern = format!("{}/**/*.conf.yml", benches_dir.display());
        glob(&config_pattern)
            .with_context(|| format!("Failed to compile benchmark glob '{config_pattern}'"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("Failed to discover benchmarks with '{config_pattern}'"))
    }

    fn sort(mut cases: Vec<SystemTest>) -> Vec<SystemTest> {
        cases.sort_by_key(|b| b.config_name.clone());
        cases
    }

    fn paths_to_cases(
        config_paths: Vec<PathBuf>,
        filter: Option<&str>,
        tests: &[String],
        package_dir: &Path,
        target_directory: &Path,
    ) -> Result<Vec<SystemTest>> {
        config_paths
            .into_iter()
            .filter(|path| {
                let file_name = path
                    .file_name()
                    .expect("The configuration file glob pattern should match a file name")
                    .to_string_lossy();
                let name = &file_name
                    .strip_suffix(".conf.yml")
                    .expect(
                        "The configuration file glob pattern should match files which end with \
                         .conf.yml",
                    )
                    .to_owned();
                if let Some(filter) = filter.as_ref() {
                    filter.dowild(name) && (tests.is_empty() || tests.contains(name))
                } else if tests.is_empty() {
                    true
                } else {
                    tests.contains(name)
                }
            })
            .map(|config_path| SystemTest::new(&config_path, package_dir, target_directory))
            .collect::<Result<Vec<SystemTest>>>()
    }

    fn get_template(&self) -> PathBuf {
        self.benches_dir.join(format!("{TEMPLATE_BENCH_NAME}.rs"))
    }
}

fn build_gungraun_runner() -> Result<()> {
    print_info("Building gungraun-runner");
    let status = Command::new(env!("CARGO"))
        .args(["build", "--package", "gungraun-runner", "--release"])
        .status()
        .context("Failed to spawn gungraun-runner build")?;
    if !status.success() {
        bail!("Building gungraun-runner failed with {status}");
    }
    Ok(())
}
