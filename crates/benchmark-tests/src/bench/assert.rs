use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs::{self, File};
use std::io::{Read, Write as IOWrite, stderr, stdout};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use benchmark_tests::common::Summary;
use glob::glob;
use tempfile::tempdir;
use valico::json_schema::schema::ScopedSchema;

use super::config::{CapturedOutput, GroupExpectations, PACKAGE, RunExpectations};
use super::expected_files::ExpectedFilesManifest;
use super::io::{deserialize_yaml_str, print_info};

pub struct AssertContext<'a> {
    pub bench_name: &'a str,
    pub config_dir: &'a Path,
    pub group_expectations: Option<&'a GroupExpectations>,
    pub home_dir: &'a Path,
    pub is_coverage_run: bool,
    pub output: &'a CapturedOutput,
    pub schema: &'a ScopedSchema<'a>,
}

impl super::config::CapturedOutput {
    pub fn assert(
        &self,
        config_dir: &Path,
        is_coverage_run: bool,
        expected: &RunExpectations,
        target_triple: &str,
    ) -> Result<()> {
        let output = &self.output;

        print_info("STDERR:");
        stderr()
            .write_all(&output.stderr)
            .context("Failed to write captured stderr")?;
        print_info("STDOUT:");
        stdout()
            .write_all(&output.stdout)
            .context("Failed to write captured stdout")?;

        if expected.no_stderr {
            let filtered = Self::filter_stderr(&output.stderr);
            if filtered.is_empty() {
                print_info("Verifying stderr successful: Expected no stderr");
            } else {
                panic!("Assertion of stderr failed: Expected no stderr");
            }
        } else if !expected.stderr_contains.resolve(target_triple).is_empty() {
            for expected in expected.stderr_contains.resolve(target_triple) {
                let output_stderr: String = String::from_utf8_lossy(&output.stderr).into();
                if output_stderr.contains(expected) {
                    print_info(format!("Verifying stderr contains '{expected}' succeeded"));
                } else {
                    panic!("Assertion of stderr failed: Expected stderr to contain '{expected}'");
                }
            }
        } else if let Some(stderr) = expected
            .stderr
            .as_ref()
            .and_then(|s| s.resolve(target_triple))
        {
            let mut expected_stderr: Vec<u8> = Vec::new();
            File::open(config_dir.join(stderr))
                .with_context(|| format!("File should exist: '{}'", stderr.display()))?
                .read_to_end(&mut expected_stderr)
                .with_context(|| format!("Failed to read '{}'", stderr.display()))?;

            let filtered = Self::filter_stderr(&output.stderr);
            let expected_string: String = String::from_utf8_lossy(&expected_stderr).into();

            if option_env!("BENCH_OVERWRITE").map_or(false, |s| s.eq_ignore_ascii_case("yes")) {
                if filtered == expected_string {
                    print_info(format!(
                        "Skip overwrite since verifying stderr '{}' was successful",
                        stderr.display()
                    ));
                } else {
                    print!(
                        "{}",
                        pretty_assertions::StrComparison::new(&filtered, &expected_string)
                    );

                    File::create(config_dir.join(stderr))
                        .with_context(|| {
                            format!("Failed to create expected stderr '{}'", stderr.display())
                        })?
                        .write_all(filtered.as_bytes())
                        .with_context(|| {
                            format!("Failed to write expected stderr '{}'", stderr.display())
                        })?;

                    print_info(format!(
                        "Overwriting stderr '{}' successful",
                        stderr.display()
                    ));
                }
            } else {
                assert!(
                    filtered == expected_string,
                    "Assertion of stderr '{}' failed: {}",
                    stderr.display(),
                    pretty_assertions::StrComparison::new(&filtered, &expected_string)
                );

                print_info(format!(
                    "Verifying stderr '{}' successful",
                    stderr.display()
                ));
            }
        } else {
            // do nothing
        }

        if expected.no_stdout {
            let filtered = self.filter_stdout(&output.stdout);
            if filtered.is_empty() {
                print_info("Verifying stdout successful: Expected no stdout");
            } else {
                panic!("Assertion of stdout failed: Expected no stdout");
            }
        } else if !expected.stdout_contains.resolve(target_triple).is_empty() {
            for expected in expected.stdout_contains.resolve(target_triple) {
                let output_stdout: String = String::from_utf8_lossy(&output.stdout).into();
                if output_stdout.contains(expected) {
                    print_info(format!("Verifying stdout contains '{expected}' succeeded"));
                } else {
                    panic!("Assertion of stdout failed: Expected stdout to contain '{expected}'");
                }
            }
        } else if let Some(stdout) = expected
            .stdout
            .as_ref()
            .and_then(|s| s.resolve(target_triple))
        {
            let mut expected_stdout: Vec<u8> = Vec::new();
            File::open(config_dir.join(stdout))
                .with_context(|| format!("File should exist: '{}'", stdout.display()))?
                .read_to_end(&mut expected_stdout)
                .with_context(|| format!("Failed to read '{}'", stdout.display()))?;

            let mut filtered = self.filter_stdout(&output.stdout);
            let mut expected_string = String::from_utf8_lossy(&expected_stdout).into_owned();

            if option_env!("BENCH_OVERWRITE").map_or(false, |s| s.eq_ignore_ascii_case("yes")) {
                if filtered == expected_string {
                    print_info(format!(
                        "Skip overwrite since verifying stdout '{}' was successful",
                        stdout.display()
                    ));
                } else {
                    print!(
                        "{}",
                        pretty_assertions::StrComparison::new(&filtered, &expected_string)
                    );

                    File::create(config_dir.join(stdout))
                        .with_context(|| {
                            format!("Failed to create expected stdout '{}'", stdout.display())
                        })?
                        .write_all(filtered.as_bytes())
                        .with_context(|| {
                            format!("Failed to write expected stdout '{}'", stdout.display())
                        })?;

                    print_info(format!(
                        "Overwriting stdout '{}' successful",
                        stdout.display()
                    ));
                }
            } else {
                if is_coverage_run {
                    filtered = Self::normalize_coverage_stdout(&filtered);
                    expected_string = Self::normalize_coverage_stdout(&expected_string);
                }

                assert!(
                    filtered == expected_string,
                    "Assertion of stdout failed: {}",
                    pretty_assertions::StrComparison::new(&filtered, &expected_string)
                );
                print_info(format!(
                    "Verifying stdout '{}' successful",
                    stdout.display()
                ));
            }
        } else {
            // do nothing
        }

        Ok(())
    }

    pub fn assert_exit(&self, exit_code: Option<i32>) {
        match exit_code {
            Some(expected) => match self.output.status.code() {
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

impl super::config::Run {
    pub fn assert(&self, ctx: &AssertContext) -> Result<()> {
        let target_triple = env!("GR_BUILD_TRIPLE");
        let expected = self
            .expected
            .as_ref()
            .and_then(|e| e.resolve(target_triple));

        if let Some(expected) = expected {
            if expected.expects_output_capture(target_triple) {
                ctx.output
                    .assert(ctx.config_dir, ctx.is_coverage_run, expected, target_triple)?;
            }
            ctx.output.assert_exit(
                expected
                    .exit_code
                    .as_ref()
                    .and_then(|e| e.resolve(target_triple)),
            );

            // a run-local script takes precedence over a group script if present
            if let Some(script) = expected.script.as_ref().map_or_else(
                || {
                    ctx.group_expectations
                        .and_then(|g| g.script.as_ref().and_then(|s| s.resolve(target_triple)))
                },
                |s| s.resolve(target_triple),
            ) {
                let temp_dir = tempdir()
                    .context("Failed to create a temporary directory for the assertion script")?;

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
            }

            if let Some(manifest) = expected
                .files
                .as_ref()
                .and_then(|f| f.resolve(target_triple))
            {
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
                    return Ok(());
                }

                let mut dirs_by_group = HashMap::new();
                let mut visited_dirs = HashMap::new();

                for manifest_entry in expected_files_manifest.data {
                    dirs_by_group
                        .entry(manifest_entry.group.clone())
                        .or_insert_with(|| {
                            glob(&format!(
                                "{}/{}/*/",
                                output_dir.display(),
                                manifest_entry.group
                            ))
                            .unwrap()
                            .map(Result::unwrap)
                            .collect::<HashSet<PathBuf>>()
                        });

                    let expected_dir = manifest_entry.assert(&output_dir, ctx.schema)?;
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
            } else if expected.no_files {
                let package_dir = ctx.home_dir.join(PACKAGE);
                let output_dir = package_dir.join(ctx.bench_name);

                if output_dir.exists() {
                    let files = glob(&format!("{}/**/*", output_dir.display()))
                        .unwrap()
                        .map(Result::unwrap)
                        .fold(String::new(), |mut acc, p| {
                            let display = p.strip_prefix(&package_dir).unwrap().display();
                            let _ = writeln!(acc, "  {display}");
                            acc
                        });
                    panic!(
                        "The benchmark directory '{}' was not expected to exist but \
                         found:\n{files}",
                        output_dir.display()
                    );
                } else {
                    print_info(format!(
                        "Verifying the benchmark directory '{}' not exists was successful",
                        output_dir.display()
                    ));
                }
            } else {
                // do nothing
            }
        }

        if expected
            .as_ref()
            .is_some_and(|expected| !expected.zero_metrics)
        {
            let output_dir = ctx.home_dir.join(PACKAGE).join(ctx.bench_name);
            // These checks heavily depends on the creation of the `summary.json` files, but we
            // create them by default.
            for path in glob(&format!("{}/**/summary.json", output_dir.display()))
                .unwrap()
                .map(Result::unwrap)
            {
                let summary = Summary::new(&path)
                    .with_context(|| format!("Failed to read summary '{}'", path.display()))?;
                summary.assert_costs_not_all_zero();
                print_info("Verifying costs not all zero successful");
            }
        }

        Ok(())
    }
}
