use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, anyhow, bail};
use fs_extra::dir::CopyOptions;
use glob::glob;
use minijinja::Environment;
use rustc_version::{Channel, VersionMeta};
use simplematch::DoWild;
use tempfile::{TempDir, tempdir};
use valico::json_schema;
use valico::json_schema::schema::ScopedSchema;

pub(super) use super::config::Partition;
use super::config::{CapturedOutput, Group, PACKAGE, SystemTestConfig};
use super::expected_files::SCHEMA_VERSION;
pub(super) use super::expected_files::TEMPLATE_DATA;
use super::io::{deserialize_json, deserialize_yaml, get_rust_version, print_info};

const TEMPLATE_BENCH_NAME: &str = "test_bench_template";
const TEMPLATE_CONTENT: &str = r#"fn main() {
    panic!("should be replaced by a rendered template");
}
"#;
const SCHEMA_PATH: &str = "crates/gungraun-summary/schemas";
const CONTINUE_FILE_NAME: &str = "benchmark-tests.continue";
const CARGO_LLVM_COV: &str = "CARGO_LLVM_COV";

/// Benchmark test case derived from a `.conf.yml` file.
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
    /// Example: `crates/benchmark-tests/benches/test_lib_bench/tools`.
    config_dir: PathBuf,
    /// Original `.conf.yml` file stem.
    ///
    /// This identifies the benchmark configuration and is unique for templated benchmarks too.
    config_name: String,
    /// Root directory for benchmark test output below cargo's target directory.
    ///
    /// Example: `target/gungraun`.
    home_dir: PathBuf,
    /// Directory where this benchmark writes regular output files.
    ///
    /// Example: `target/gungraun/benchmark-tests/test_lib_bench_tools`.
    output_dir: PathBuf,
}

/// Top-level benchmark test runner.
///
/// Owns the metadata used by `bench --filter='test_lib_*'`.
#[derive(Debug)]
pub(super) struct SystemTestRunner {
    /// Resolved workspace, target directory, compiler, and selected benchmark list.
    ///
    /// Contains only the selected benchmarks when `--partition`, `--filter` or `--continue` is
    /// used.
    pub(super) tests: SystemTests,
}

#[derive(Debug, Clone)]
/// Selected system tests and shared execution metadata.
///
/// Example: produced once from cargo metadata before running benchmark tests.
pub(super) struct SystemTests {
    /// Path to the `crates/benchmark-tests/benches` directory.
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
    pub(super) target_directory: PathBuf,
    /// Cargo workspace root.
    ///
    /// Example: repository root containing `Cargo.toml`.
    workspace_root: PathBuf,
}

impl SystemTest {
    fn new(config_path: &Path, _package_dir: &Path, target_dir: &Path) -> anyhow::Result<Self> {
        let config: SystemTestConfig = deserialize_yaml(config_path)?;

        let config_name = config_path.file_name().unwrap().to_string_lossy();
        let config_name = config_name.strip_suffix(".conf.yml").unwrap().to_owned();

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
            config_dir: config_path.parent().unwrap().to_path_buf(),
            home_dir,
        })
    }

    fn clean_benchmark(&self) -> anyhow::Result<()> {
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

    fn backup(&self) -> anyhow::Result<Option<TempDir>> {
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

    fn restore(&self, temp_dir: Option<&TempDir>) -> anyhow::Result<()> {
        self.clean_benchmark()?;

        if let Some(temp_dir) = temp_dir {
            let from = temp_dir.path().join(self.output_dir.file_name().unwrap());
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

    #[expect(clippy::too_many_arguments)]
    fn run_bench(
        &self,
        cargo_args: &[String],
        gungraun_args: &[String],
        envs: &HashMap<String, String>,
        is_capture: bool,
        tolerance: Option<f64>,
        setup: Option<&str>,
        teardown: Option<&str>,
    ) -> anyhow::Result<CapturedOutput> {
        let stdio = if is_capture {
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

        if let Some(setup) = setup {
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

        if is_capture {
            command.args(["--color", "never"]);
        }

        if !gungraun_args.is_empty() {
            command.arg("--");
            command.args(gungraun_args);
        }

        if let Some(tolerance) = tolerance {
            command.arg(format!("--tolerance={tolerance}"));
        }

        let output = command
            .stderr(stdio())
            .stdout(stdio())
            .output()
            .with_context(|| format!("Failed to launch benchmark '{}'", self.config_name))?;

        if let Some(teardown) = teardown {
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
            has_tolerance: tolerance.is_some(),
        })
    }

    #[expect(clippy::too_many_arguments)]
    fn run_template(
        &self,
        template_path: &Path,
        cargo_args: &[String],
        gungraun_args: &[String],
        envs: &HashMap<String, String>,
        template_data: &HashMap<String, minijinja::Value>,
        tests: &SystemTests,
        is_capture: bool,
        tolerance: Option<f64>,
        setup: Option<&str>,
        teardown: Option<&str>,
    ) -> anyhow::Result<CapturedOutput> {
        let mut template_string = String::new();
        let source_path = self.config_dir.join(template_path);
        File::open(&source_path)
            .with_context(|| format!("Failed to open template '{}'", source_path.display()))?
            .read_to_string(&mut template_string)
            .with_context(|| format!("Failed to read template '{}'", source_path.display()))?;

        let mut env = Environment::new();
        env.add_template(&self.bench_name, &template_string)
            .with_context(|| format!("Failed to compile template '{}'", source_path.display()))?;
        let template = env
            .get_template(&self.bench_name)
            .with_context(|| format!("Failed to load template '{}'", self.bench_name))?;

        let destination = tests.get_template();
        let dest = File::create(&destination).with_context(|| {
            format!(
                "Failed to create rendered template '{}'",
                destination.display()
            )
        })?;
        template
            .render_captured_to(template_data, dest)
            .with_context(|| format!("Failed to render template '{}'", source_path.display()))?;

        self.run_bench(
            cargo_args,
            gungraun_args,
            envs,
            is_capture,
            tolerance,
            setup,
            teardown,
        )
    }

    fn run(
        &self,
        num_groups: usize,
        group_index: usize,
        group: &Group,
        tests: &SystemTests,
        schema: &ScopedSchema<'_>,
    ) -> anyhow::Result<()> {
        let target_triple = env!("GR_BUILD_TRIPLE");

        if !group.runs_on.as_ref().is_none_or(|(is_target, target)| {
            if *is_target {
                target == target_triple
            } else {
                target != target_triple
            }
        }) || !group
            .rust_version
            .as_ref()
            .is_none_or(|(cmp, version)| tests.compare_rust_version(*cmp, version))
        {
            return Ok(());
        }

        self.clean_benchmark()?;

        let num_runs = group.runs.len();
        for (index, run) in group
            .runs
            .iter()
            .filter(|r| {
                r.runs_on.as_ref().is_none_or(|(is_target, target)| {
                    if *is_target {
                        target == target_triple
                    } else {
                        target != target_triple
                    }
                }) && r
                    .rust_version
                    .as_ref()
                    .is_none_or(|(cmp, version)| tests.compare_rust_version(*cmp, version))
            })
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

                if !run.gungraun_args.is_empty() {
                    print_info(format!(
                        "Benchmark arguments: {}",
                        run.gungraun_args.join(" ")
                    ));
                }

                let is_capture = run.expected.as_ref().is_some_and(|target_config| {
                    target_config.resolve(target_triple).is_some_and(|e| {
                        e.stdout.is_some()
                            || e.no_stdout
                            || !e.stdout_contains.resolve(target_triple).is_empty()
                            || e.stderr.is_some()
                            || e.no_stderr
                            || !e.stderr_contains.resolve(target_triple).is_empty()
                    })
                });

                let output = if let Some(template) = &self.config.template {
                    let output = self.run_template(
                        template,
                        &run.cargo_args,
                        &run.gungraun_args,
                        &run.envs,
                        &run.template_data,
                        tests,
                        is_capture,
                        run.tolerance,
                        run.setup.as_deref(),
                        run.teardown.as_deref(),
                    )?;
                    Self::reset_template(tests)?;
                    output
                } else {
                    self.run_bench(
                        &run.cargo_args,
                        &run.gungraun_args,
                        &run.envs,
                        is_capture,
                        run.tolerance,
                        run.setup.as_deref(),
                        run.teardown.as_deref(),
                    )?
                };

                if tries < max_tries {
                    let result = panic::catch_unwind(AssertUnwindSafe(|| {
                        run.assert(
                            &self.config_dir,
                            tests.is_coverage_run,
                            &output,
                            schema,
                            &self.home_dir,
                            &self.bench_name,
                            group.expected.as_ref(),
                        )
                    }));
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
                    run.assert(
                        &self.config_dir,
                        tests.is_coverage_run,
                        &output,
                        schema,
                        &self.home_dir,
                        &self.bench_name,
                        group.expected.as_ref(),
                    )?;
                }
            }

            drop(backup_dir);
        }

        Ok(())
    }

    fn reset_template(tests: &SystemTests) -> anyhow::Result<()> {
        let path = tests.get_template();
        let mut file = File::create(&path)
            .with_context(|| format!("Failed to reset template '{}'", path.display()))?;
        file.write_all(TEMPLATE_CONTENT.as_bytes())
            .with_context(|| format!("Failed to write reset template '{}'", path.display()))
    }
}

impl SystemTestRunner {
    pub(super) fn new(
        benches: &[String],
        filter: Option<&str>,
        partition: Option<Partition>,
        resume: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            tests: SystemTests::new(benches, filter, partition, resume)?,
        })
    }

    pub(super) fn run(&self) -> anyhow::Result<()> {
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
        if let Err(error) = std::fs::remove_file(&continue_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to remove continue file '{}'",
                        continue_path.display()
                    )
                });
            }
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
    ) -> anyhow::Result<Self> {
        let meta = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .exec()
            .context("Failed to read cargo metadata")?;

        let package_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let benches_dir = package_dir.join("benches");
        let workspace_root = meta.workspace_root.clone().into_std_path_buf();
        let target_directory = meta.target_directory.into_std_path_buf();

        let config_pattern = format!("{}/**/*.conf.yml", benches_dir.display());
        let config_paths = glob(&config_pattern)
            .with_context(|| format!("Failed to compile benchmark glob '{config_pattern}'"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("Failed to discover benchmarks with '{config_pattern}'"))?;
        let mut cases = config_paths
            .into_iter()
            .filter(|path| {
                let file_name = path.file_name().unwrap().to_string_lossy();
                let name = &file_name.strip_suffix(".conf.yml").unwrap().to_owned();
                if let Some(filter) = filter.as_ref() {
                    filter.dowild(name) && (tests.is_empty() || tests.contains(name))
                } else if tests.is_empty() {
                    true
                } else {
                    tests.contains(name)
                }
            })
            .map(|config_path| SystemTest::new(&config_path, &package_dir, &target_directory))
            .collect::<anyhow::Result<Vec<SystemTest>>>()?;

        cases.sort_by_key(|b| b.config_name.clone());
        if let Some(partition) = partition {
            if cases.is_empty() {
                bail!("The partition did not match any benchmarks");
            }
            let chunk_size = cases.len().div_ceil(partition.total);
            let chunk = cases
                .chunks(chunk_size)
                .nth(partition.part - 1)
                .map(<[SystemTest]>::to_vec);
            cases = chunk.ok_or_else(|| anyhow!("The partition did not match any benchmarks"))?;
        }

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

    fn get_template(&self) -> PathBuf {
        self.benches_dir.join(format!("{TEMPLATE_BENCH_NAME}.rs"))
    }

    fn compare_rust_version(&self, cmp: version_compare::Cmp, version: &str) -> bool {
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

fn build_gungraun_runner() -> anyhow::Result<()> {
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
