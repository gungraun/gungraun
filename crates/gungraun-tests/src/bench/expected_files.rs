//! Expected-file manifests, summary-schema validation, and shared constants.
//!
//! [`ExpectedFilesManifest`] (and its entry types) deserializes the YAML manifest a run points at
//! via [`RunExpectations::files`][super::config::RunExpectations::files], then asserts that the
//! files each `group`/`function`/`id` declares actually exist under the run's output directory -
//! and that every `summary.json` validates against the versioned summary JSON schema compiled once
//! per harness invocation.
//!
//! Like the stream comparison in [`assert`][super::assert], file assertion is dual-mode:
//! `BENCH_OVERWRITE=yes` regenerates the manifest from what the run produced instead of checking
//! it. Note that this does not cover 100% of cases. Existing glob patterns are preserved but new
//! glob patterns are not automatically created and are expected to be added manually.
//!
//! The schema in an [`ExpectedFilesManifest`] is structured as follows:
//!
//! ```yaml
//! home_dir: "path/to/home" # optional path to the gungraun home (env: GUNGRAUN_HOME) directory
//! data: # A list of expectations for each benchmark executed in this system test
//!   - group: # the benchmark group
//!     function: # the benchmark function
//!     id: # optional: the id
//!     expected:
//!       files:  # A list of files which are expected to be created by the benchmark
//!         - callgrind.file_1.out
//!       globs: # A list of files matched by the `pattern`, `count` times
//!         - pattern: "some_glob*"
//!           count: 2
//! ```

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use glob::{Pattern, glob};
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tera::Tera;
use valico::json_schema::schema::ScopedSchema;

use super::io::{deserialize_json, print_error, print_info, serialize_yaml};

pub static TEMPLATE_DATA: OnceCell<HashMap<String, serde_json::Value>> = OnceCell::new();

pub const SCHEMA_PATH: &str = "crates/gungraun-summary/schemas";
pub const SCHEMA_VERSION: &str = "7";

/// Expected files and globs for one benchmark output directory.
///
/// # Examples
///
/// An entry declaring both exact files and a glob with a required match count:
///
/// ```yaml
/// # ...
/// - expected: # `ExpectedFiles`
///     files:
///       - summary.json
///       - callgrind.bench_exit_with.exit_with.log
///     globs:
///       - pattern: "callgrind.bench_exit_with.exit_with.log.#*"
///         count: 1
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpectedFiles {
    /// Exact files expected to exist and be non-empty.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// # ...
    /// - expected: # `ExpectedFiles`
    ///     files:
    ///       - summary.json
    ///       - callgrind.bench_exit_with.exit_with.log
    /// ```
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<PathBuf>,
    /// Glob patterns with required match counts.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// - expected: # `ExpectedFiles`
    ///     globs:
    ///       - pattern: "callgrind.bench_exit_with.exit_with.log.#*"
    ///         count: 1
    /// ```
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub globs: Vec<ExpectedFilesGlob>,
}

/// Expected glob assertion for benchmark output files.
///
/// # Examples
///
/// A glob requiring exactly one matching callgrind outpost file:
///
/// ```yaml
/// # ...
/// expected: # `ExpectedFiles`
///   globs: # `ExpectedFilesGlob`
///     - pattern: "callgrind.bench_exit_with.exit_with.log.#*"
///       count: 1
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Keep the items ordered like this in the output file"
)]
pub struct ExpectedFilesGlob {
    /// Glob pattern relative to an expected run directory.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// globs: # `ExpectedFilesGlob`
    ///   - pattern: "callgrind.bench_exit_with.exit_with.log.#*"
    ///     count: 1
    /// ```
    pub pattern: String,
    /// Required number of files matching `pattern`.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// globs: # `ExpectedFilesGlob`
    ///   - pattern: "callgrind.bench_exit_with.exit_with.log.#*"
    ///     count: 1
    /// ```
    pub count: usize,
}

/// Expected-files manifest referenced by [`super::config::RunExpectations`].
///
/// # Examples
///
/// A manifest with an alternate home directory and one expected benchmark:
///
/// ```yaml
/// home_dir: "/tmp/gungraun-home"
/// data:
///   - group: my_group
///     function: bench_exit_with
///     id: exit_with
///     expected:
///       files:
///         - summary.json
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Keep the items ordered like this in the output file"
)]
pub struct ExpectedFilesManifest {
    /// Optional alternate home directory for expected output lookup.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// home_dir: "/tmp/gungraun-home"
    /// data:
    ///   - group: my_group
    ///     function: bench_exit_with
    ///     expected:
    ///       files:
    ///         - summary.json
    /// ```
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_dir: Option<PathBuf>,
    /// Expected output directories and files to assert.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// data:
    ///   - group: my_group
    ///     function: bench_exit_with
    ///     id: exit_with
    ///     expected:
    ///       files:
    ///         - summary.json
    /// ```
    pub data: Vec<ExpectedFilesManifestEntry>,
}

/// Expected files for one benchmark `function.id` or `function` output directory.
///
/// # Examples
///
/// An entry pinning the expected files for one `bench_exit_with` run:
///
/// ```yaml
/// data:
///   - group: my_group
///     function: bench_exit_with
///     id: exit_with
///     expected:
///       files:
///         - summary.json
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Keep the items ordered like this in the output file"
)]
pub struct ExpectedFilesManifestEntry {
    /// The name of the benchmark group (`library_benchmark_group!` or `binary_benchmark_group!`)
    ///
    /// This is used to resolve the directory below the benchmark output root.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// data:
    ///   # `ExpectedFilesManifestEntry`
    ///   - group: my_group
    ///     function: bench_exit_with
    ///     expected:
    ///       files:
    ///         - summary.json
    ///   # ...  more entries
    /// ```
    pub group: String,
    /// The benchmark function (annotated with `#[library_benchmark]` or `#[binary_benchmark]`).
    ///
    /// Used in addition to [`ExpectedFilesManifestEntry::id`] to locate the output directory.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// data:
    ///   - group: my_group
    ///     function: bench_exit_with
    ///     expected:
    ///       files:
    ///         - summary.json
    /// ```
    pub function: String,
    /// Optional benchmark id appended to the function directory name with a dot
    ///
    /// Used in addition to [`ExpectedFilesManifestEntry::function`] to construct the the output
    /// directory. If the id is present both are concatenated with a dot (`function.id`).
    ///
    /// # Examples
    ///
    /// ```yaml
    /// data:
    ///   - group: my_group
    ///     function: bench_exit_with
    ///     id: exit_with
    ///     expected:
    ///       files:
    ///         - summary.json
    /// ```
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// [`ExpectedFiles`] and [`ExpectedFilesGlob`] in the resolved output directory.
    ///
    /// # Examples
    ///
    /// ```yaml
    /// data:
    ///   - group: my_group
    ///     function: bench_exit_with
    ///     expected:
    ///       files:
    ///         - summary.json
    ///       globs:
    ///         - pattern: "callgrind.bench_exit_with.exit_with.log.#*"
    ///           count: 1
    /// ```
    pub expected: ExpectedFiles,
}

impl ExpectedFilesManifest {
    /// Regenerates an expected-files manifest from a benchmark's current output files.
    ///
    /// `output_dir` is the benchmark output root, such as
    /// `<target-dir>/gungraun/gungraun-tests/test_...`. `manifest` is the manifest path relative to
    /// the system-test directory as given in the system test configuration file, and
    /// `manifest_path` is the absolute path to the manifest file to replace.
    ///
    /// This supports the `BENCH_OVERWRITE=yes` fixture-update workflow, keeping checked-in
    /// expectations aligned with intentional output changes. Existing glob expectations are
    /// retained when they still apply, but new globs are not inferred. Any needed glob coverage
    /// needs to be added manually to the resulting files. Generated file and retained glob entries
    /// are sorted, and an existing `home_dir` is preserved rather than added. The resulting
    /// manifest is formatted with `npx prettier` when available.
    ///
    /// # Panics
    ///
    /// Panics when benchmark output cannot be enumerated as expected, a retained glob is invalid,
    /// the output layout is unexpected, or the manifest cannot be created or serialized.
    pub fn overwrite(
        self,
        output_dir: &Path,
        old_manifest_content: &str,
        manifest: &str,
        manifest_path: &Path,
    ) -> Result<()> {
        let discovered_files = glob(&format!("{}/**/*", output_dir.display()))
            .expect("The glob pattern should be valid")
            .map(Result::unwrap)
            .filter(|p| !p.is_dir())
            .map(|p| {
                let file = p.file_name().expect("A file name should be present");
                let benchmark_directory =
                    p.parent().expect("A benchmark directory should be present");
                let (function, id) = (
                    benchmark_directory
                        .file_stem()
                        .expect("A file stem should be present"),
                    benchmark_directory.extension(),
                );

                let group = benchmark_directory
                    .parent()
                    .expect("A group should be present");

                assert_eq!(group.parent(), Some(output_dir));

                (
                    (
                        group
                            .file_name()
                            .expect("group should have a file name")
                            .to_string_lossy()
                            .to_string(),
                        function.to_string_lossy().to_string(),
                        id.map(|i| i.to_string_lossy().to_string()),
                    ),
                    PathBuf::from(file),
                )
            })
            .fold(IndexMap::new(), |mut acc, (key, value)| {
                acc.entry(key)
                    .and_modify(|v: &mut Vec<_>| v.push(value.clone()))
                    .or_insert_with(|| vec![value]);
                acc
            });

        let mut existing_entries = self.data;
        let regenerated_entries = discovered_files.into_iter().fold(
            Vec::new(),
            |mut acc, ((group, function, id), files)| {
                let mut run = ExpectedFilesManifestEntry {
                    group,
                    function,
                    id,
                    expected: ExpectedFiles {
                        files,
                        globs: vec![],
                    },
                };

                let existing_index = existing_entries.iter().position(|e| e.matches_other(&run));
                if let Some(index) = existing_index {
                    // The order of the old data doesn't matter since we use the order of the
                    // new data
                    let existing = existing_entries.swap_remove(index);
                    // Multiple globs can match the same files, so we have to collect the
                    // matched files first before removing them from `run.expected.files`
                    let mut matched = HashSet::new();
                    for ExpectedFilesGlob { pattern, .. } in &existing.expected.globs {
                        let glob = Pattern::new(pattern).expect("The pattern should be valid");

                        let num_matches = run
                            .expected
                            .files
                            .iter()
                            .filter(|f| glob.matches_path(f))
                            .inspect(|f| {
                                matched.insert((*f).to_owned());
                            })
                            .count();

                        if num_matches > 0 {
                            let new_glob = ExpectedFilesGlob {
                                pattern: pattern.clone(),
                                count: num_matches,
                            };
                            run.expected.globs.push(new_glob);
                        }
                    }

                    run.expected.files.retain(|f| !matched.contains(f));

                    run.expected.globs.sort_unstable();
                    run.expected.globs.dedup();
                }

                acc.push(run);
                acc
            },
        );

        let updated_manifest = Self {
            data: regenerated_entries,
            home_dir: self.home_dir,
        };

        serialize_yaml(manifest_path, &updated_manifest)?;

        let status = std::process::Command::new("npx")
            .args(["-y", "prettier", "-w"])
            .arg(manifest_path)
            .stdout(Stdio::null())
            .status();

        let new_manifest_content = fs::read_to_string(manifest_path)
            .with_context(|| format!("Failed to read '{}'", manifest_path.display()))?;

        if old_manifest_content == new_manifest_content {
            if status.is_ok_and(|s| s.success()) {
                print_info(format!(
                    "Overwriting expected-files manifest '{manifest}' did not change the manifest",
                ));
            } else {
                print_info(format!(
                    "Overwriting expected-files manifest '{manifest}' did not change the manifest"
                ));
                print_info("Running `npx prettier` failed. Continuing ...");
            }
        } else {
            print!(
                "{}",
                pretty_assertions::StrComparison::new(&old_manifest_content, &new_manifest_content)
            );

            if status.is_ok_and(|s| s.success()) {
                print_info(format!(
                    "Overwriting expected-files manifest '{manifest}' successful. Formatting with \
                     `npx prettier` succeeded.",
                ));
            } else {
                print_info(format!(
                    "Overwriting expected-files manifest '{manifest}' successful"
                ));
                print_info(
                    "Running `npx prettier` failed. This file needs to be manually formatted. \
                     Continuing ...",
                );
            }
        }

        Ok(())
    }
}

impl ExpectedFilesManifestEntry {
    /// Returns whether this entry targets the same benchmark as `other`.
    ///
    /// This deviates from the `PartialEq` implementation. Two entries match when their `group`,
    /// `function`, and `id` triples are equal.
    fn matches_other(&self, other: &Self) -> bool {
        self.group == other.group && self.function == other.function && self.id == other.id
    }

    /// Resolves the benchmark output directory this entry's expectations apply to.
    ///
    /// Renders `function` as a Tera template against [`TEMPLATE_DATA`], then joins
    /// `output_dir / group / function`, appending `.{id}` when `id` is set.
    ///
    /// # Panics
    ///
    /// Panics if the `function` template cannot be added to Tera, if [`TEMPLATE_DATA`] has not been
    /// initialized, or if rendering the template fails.
    ///
    /// # Errors
    ///
    /// Returns an error if the Tera context cannot be serialized from [`TEMPLATE_DATA`].
    fn expected_dir(&self, output_dir: &Path) -> Result<PathBuf> {
        let mut tera = Tera::default();
        tera.add_raw_template("function", &self.function)
            .expect("Adding the raw tera template should succeed");

        let context = tera::Context::from_serialize(
            TEMPLATE_DATA
                .get()
                .expect("The template data should be initialized"),
        )?;
        let function = tera
            .render("function", &context)
            .expect("Rendering the tera template should succeed");

        if let Some(id) = &self.id {
            Ok(output_dir
                .join(&self.group)
                .join(format!("{function}.{id}")))
        } else {
            Ok(output_dir.join(&self.group).join(&function))
        }
    }

    /// Asserts that this entry's expected files exist under `output_dir` and validate.
    ///
    /// Resolves the expected directory via [`Self::expected_dir`], then checks that every declared
    /// file is present and non-empty, that each glob matches its required `count`, that any
    /// `summary.json` validates against `schema` and reports [`SCHEMA_VERSION`], and that no
    /// further files remain. Returns the resolved `expected_dir` on success.
    ///
    /// # Panics
    ///
    /// Panics via `assert*` or if a glob's match count differs, `summary.json` is not an object or
    /// lacks a `version`, the version is not [`SCHEMA_VERSION`], or unexpected extra files remain.
    ///
    /// # Errors
    ///
    /// Returns an error if file metadata cannot be read or `summary.json` cannot be deserialized.
    pub fn assert(&self, output_dir: &Path, schema: &ScopedSchema) -> Result<PathBuf> {
        let expected_dir = self.expected_dir(output_dir)?;

        print_info(format!(
            "Running assertions in directory '{}'",
            expected_dir.display()
        ));

        assert!(
            expected_dir.exists(),
            "Expected benchmark directory '{}' to exist",
            expected_dir.display()
        );

        let mut discovered_files = glob(&format!("{}/*", expected_dir.display()))
            .expect("Glob pattern should compile")
            .map(Result::unwrap)
            .collect::<HashSet<PathBuf>>();

        let mut summary = None;
        for file in self.expected.files.iter().map(|f| expected_dir.join(f)) {
            if let Some(file_name) = file.file_name()
                && file_name == "summary.json"
            {
                summary = Some(file.clone());
            }
            // Gungraun does not produce empty files and if so we treat it as an error
            assert!(
                discovered_files.remove(&file),
                "Expected file '{}' does not exist",
                file.display()
            );
            assert_ne!(
                std::fs::metadata(&file)?.len(),
                0,
                "Expected file '{}' was empty",
                file.display()
            );
        }

        for ExpectedFilesGlob { pattern, count } in &self.expected.globs {
            let pattern = &expected_dir.join(pattern).display().to_string();
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

            for file in files {
                if let Some(file_name) = file.file_name()
                    && file_name == "summary.json"
                {
                    summary = Some(file.clone());
                }
                discovered_files.remove(&file);
            }
        }

        if let Some(summary) = summary {
            print_info(format!("Validating summary '{}'", summary.display()));
            let value: serde_json::Value = deserialize_json(&summary)?;

            let result = schema.validate(&value);
            if !result.is_valid() {
                for error in result.errors {
                    print_error(format!("{}: Validation error: {error}", summary.display()));
                }
            }
            let (_, value) = value
                .as_object()
                .expect("The summary should be a json object")
                .get_key_value("version")
                .expect("The summary should have a version");
            assert_eq!(
                value, SCHEMA_VERSION,
                "summary json schema version mismatch"
            );
        }

        assert!(
            discovered_files.is_empty(),
            "Expected no other files in directory '{}' but found: {:#?}",
            expected_dir.display(),
            discovered_files
        );

        Ok(expected_dir)
    }
}
