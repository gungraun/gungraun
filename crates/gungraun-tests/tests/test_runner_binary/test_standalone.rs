//! Subprocess tests for the public non-Cargo runner frontend.
//!
//! These require a native C compiler and Valgrind, like the repository's system tests. The C
//! fixture is compiled once, then copied into consumer directories containing no Cargo manifest
//! or Rust source. Only the parity test launches a separate, ordinary Rust benchmark harness.

// spell-checker: ignore ncwd

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use gungraun_runner::api::{EventKind, Tool};
use gungraun_runner::metrics::model::Metric;
use gungraun_runner::summary::model::{BenchmarkSummary, ToolMetricResults, ToolRegression};
use rstest::rstest;
use tempfile::TempDir;

static FIXTURE: LazyLock<TempDir> = LazyLock::new(|| {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c11", "-O2", "-g"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/standalone/workload.c"))
        .arg("-o")
        .arg(directory.path().join("workload"))
        .output()
        .expect("A native C compiler is required for the standalone runner tests");
    assert_success(&output);
    directory
});

static RUNNER: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Some(path) = std::env::var_os("GUNGRAUN_RUNNER") {
        return PathBuf::from(path).canonicalize().unwrap();
    }

    // Match util::common::Runner's build-on-demand convention, once for this test module.
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["build", "--package", "gungraun-runner", "--release"])
        .output()
        .expect("Building gungraun-runner should succeed");
    assert_success(&output);
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/release/gungraun-runner")
        .canonicalize()
        .unwrap()
});

struct Consumer {
    directory: TempDir,
    root: PathBuf,
}

impl Consumer {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("consumer with spaces");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        fs::copy(FIXTURE.path().join("workload"), root.join("workload")).unwrap();
        Self { directory, root }
    }

    fn command(&self, mode: &str) -> Command {
        let mut command = Command::new(&*RUNNER);
        command
            .env_clear()
            .env("GUNGRAUN_COLOR", "never")
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("CARGO", self.root.join("cargo-must-not-be-needed"))
            .current_dir(self.directory.path())
            .arg(mode)
            .arg("--config")
            .arg(self.root.join("guard.toml"))
            .arg("--allow-aslr");
        command
    }

    fn context_manifest(&self) {
        fs::create_dir_all(self.root.join("working directory")).unwrap();
        fs::write(self.root.join("working directory/marker"), "present").unwrap();
        self.write(
            r#"schema_version = 1
output_dir = "output"
[defaults]
program = "./workload"
cwd = "working directory"
args = ["must be replaced"]
[defaults.env]
DEFAULT_ENV = "default value"
LC_ALL = "C"
LITERAL = "$HOME ${USER} *"
OVERRIDE = "default value"
[defaults.callgrind]
entry_point = "guard_region"
[[cases]]
id = "Quote.Single.Owner"
args = ["context", "argument with spaces", "${LITERAL}", "$(touch SH_INJECTION)", "*"]
[cases.env]
OVERRIDE = "case value"
[cases.callgrind.hard_limits]
Ir = 1000000
"#,
        );
    }

    fn manifest(&self, mode: &str, entry: &str, cap: Option<u64>) {
        self.write(&format!(
            "schema_version = 1\noutput_dir = \"output\"\n[defaults]\nprogram = \
             \"./workload\"\ncwd = \".\"\nargs = [{mode:?}]\n[defaults.callgrind]\nentry_point = \
             {entry:?}\n[[cases]]\nid = \"Quote.Single.Owner\"\n{}",
            cap.map_or_else(String::new, |limit| format!(
                "[cases.callgrind.hard_limits]\nIr = {limit}\n"
            ))
        ));
    }

    fn output_files(&self) -> Vec<PathBuf> {
        files_under(&self.root.join("output"))
    }

    fn run(&self, mode: &str) -> Output {
        self.command(mode).output().unwrap()
    }

    fn summaries(&self) -> Vec<BenchmarkSummary> {
        summaries_under(&self.root.join("output"))
    }

    fn write(&self, manifest: &str) {
        fs::write(self.root.join("guard.toml"), manifest).unwrap();
    }
}

#[track_caller]
fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "Unexpected success:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[track_caller]
fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "Failed with {}:\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn current_ir(summary: &BenchmarkSummary) -> u64 {
    assert_eq!(summary.profiles.0.len(), 1);
    let profile = &summary.profiles.0[0];
    assert_eq!(profile.tool, Tool::Callgrind);
    let ToolMetricResults::Callgrind(metrics) = &profile.data.total.metrics else {
        panic!("A native Callgrind metric summary is required");
    };
    let Some(Metric::Int(count)) = metrics.0[&EventKind::Ir].values.as_ref().left() else {
        panic!("A current integer instruction count is required");
    };
    *count
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if root.is_dir() {
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(files_under(&path));
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn has_current_file(files: &[PathBuf], kind: &str) -> bool {
    let extension = format!(".{kind}");
    files.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with("callgrind.")
                    && name.contains(&extension)
                    && !path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("old"))
                    && !name.contains(".old.")
            })
    })
}

fn summaries_under(root: &Path) -> Vec<BenchmarkSummary> {
    files_under(root)
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "summary.json"))
        .map(|path| serde_json::from_slice(&fs::read(path).unwrap()).unwrap())
        .collect()
}

#[test]
fn test_capture_and_absolute_limit_boundaries() {
    let consumer = Consumer::new();
    consumer.manifest("measure", "guard_region", None);
    let capture = consumer.run("run");
    assert_success(&capture);
    let stderr = String::from_utf8_lossy(&capture.stderr).to_lowercase();
    assert!(stderr.contains("capture"), "{stderr}");
    let measured = current_ir(&consumer.summaries()[0]);
    assert!(measured > 0);

    assert_failure(&consumer.run("check"));
    for limit in [measured + 1, measured] {
        consumer.manifest("measure", "guard_region", Some(limit));
        assert_success(&consumer.run("check"));
        let summaries = consumer.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(current_ir(&summaries[0]), measured);
        assert_eq!(
            summaries[0].profiles.0[0].data.total.regressions,
            [] as [ToolRegression; 0]
        );
    }

    consumer.manifest("measure", "guard_region", Some(measured - 1));
    let reviewed_manifest = fs::read(consumer.root.join("guard.toml")).unwrap();
    let exceeded = consumer.run("check");
    assert_eq!(exceeded.status.code(), Some(3));
    assert_eq!(
        reviewed_manifest,
        fs::read(consumer.root.join("guard.toml")).unwrap()
    );
    let summaries = consumer.summaries();
    assert_eq!(current_ir(&summaries[0]), measured);
    assert_eq!(summaries[0].profiles.0[0].data.total.regressions.len(), 1);
    assert!(has_current_file(&consumer.output_files(), "out"));
}

#[rstest]
#[case::nonzero_exit("fail")]
#[case::assertion("assert")]
#[case::signal("signal")]
fn test_child_failure_preserves_current_evidence(#[case] mode: &str) {
    let consumer = Consumer::new();
    consumer.manifest(mode, "guard_region", Some(1_000_000));
    assert_failure(&consumer.run("check"));
    let files = consumer.output_files();
    assert!(
        has_current_file(&files, "out"),
        "Missing current profile: {files:?}"
    );
    assert!(
        has_current_file(&files, "log"),
        "Missing current log: {files:?}"
    );
}

#[test]
fn test_command_context_and_public_rust_harness_match() {
    let consumer = Consumer::new();
    consumer.context_manifest();
    assert_success(&consumer.run("check"));
    let standalone = consumer.summaries().remove(0);
    let observed = fs::read(consumer.root.join("working directory/observed.txt")).unwrap();
    let rust_output = consumer.root.join("rust output");
    let output = Command::new(env!("CARGO_BIN_EXE_standalone-rust"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("GUNGRAUN_COLOR", "never")
        .env("GUNGRAUN_RUNNER", &*RUNNER)
        .env("GUNGRAUN_STANDALONE_ROOT", &consumer.root)
        .current_dir(consumer.directory.path())
        .args(["--bench", "--allow-aslr=yes", "--save-summary=json"])
        .arg("--workspace-root")
        .arg(&consumer.root)
        .arg("--home")
        .arg(&rust_output)
        .output()
        .unwrap();
    assert_success(&output);
    let rust = summaries_under(&rust_output).remove(0);
    assert_eq!(current_ir(&standalone), current_ir(&rust));
    assert_eq!(standalone.benchmark_exe, rust.benchmark_exe);
    assert_eq!(
        standalone.profiles.0[0].data.parts[0]
            .tool_run
            .as_ref()
            .left()
            .unwrap()
            .command,
        rust.profiles.0[0].data.parts[0]
            .tool_run
            .as_ref()
            .left()
            .unwrap()
            .command
    );
    assert_eq!(
        observed,
        fs::read(consumer.root.join("working directory/observed.txt")).unwrap()
    );
    assert!(!consumer.root.join("Cargo.toml").exists());
    assert!(
        !consumer
            .root
            .join("working directory/SH_INJECTION")
            .exists()
    );
}

#[test]
fn test_expected_exit_and_defaults_override() {
    let consumer = Consumer::new();
    consumer.manifest("exit-seven", "guard_region", Some(1_000_000));
    assert_failure(&consumer.run("check"));
    let manifest = fs::read_to_string(consumer.root.join("guard.toml")).unwrap();
    consumer.write(&manifest.replace(
        "id = \"Quote.Single.Owner\"",
        "id = \"Quote.Single.Owner\"\nexpected_exit = 7",
    ));
    assert_success(&consumer.run("check"));
    assert!(current_ir(&consumer.summaries()[0]) > 0);
}

#[rstest]
#[case::empty("schema_version = 1\ncases = []\n")]
#[case::missing_cap("schema_version = 1\n[[cases]]\nid = 'one'\nprogram = './workload'\n")]
#[case::zero_cap(
    "schema_version = 1\n[[cases]]\nid = 'one'\nprogram = \
     './workload'\n[cases.callgrind.hard_limits]\nIr = 0\n"
)]
fn test_invalid_guards_fail_before_running(#[case] manifest: &str) {
    let consumer = Consumer::new();
    consumer.write(manifest);
    assert_failure(&consumer.run("check"));
    assert_eq!(consumer.output_files(), [] as [PathBuf; 0]);
}

#[test]
fn test_missing_binary_and_profiler_failures() {
    let consumer = Consumer::new();
    consumer.manifest("measure", "guard_region", Some(1_000_000));
    let nonexistent = consumer.root.join("not-installed");
    let output = consumer
        .command("check")
        .arg("--valgrind-bin")
        .arg(&nonexistent)
        .output()
        .unwrap();
    assert_failure(&output);

    let output = consumer
        .command("check")
        .arg("--valgrind-bin")
        .arg("/usr/bin/false")
        .output()
        .unwrap();
    assert_failure(&output);

    // The fixture's special tool mode writes invalid data, exercising the native parser's failure.
    let output = consumer
        .command("check")
        .arg("--valgrind-bin")
        .arg(consumer.root.join("workload"))
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(
        consumer.output_files().iter().any(|path| {
            fs::read_to_string(path).is_ok_and(|text| text.contains("deliberately invalid"))
        }),
        "Invalid profiler output was not retained: {:?}\n{}",
        consumer.output_files(),
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_file(consumer.root.join("workload")).unwrap();
    assert_failure(&consumer.run("check"));
}

#[test]
fn test_path_lookup_and_output_selection_from_another_directory() {
    let consumer = Consumer::new();
    consumer.manifest("measure", "guard_region", Some(1_000_000));
    let manifest = fs::read_to_string(consumer.root.join("guard.toml")).unwrap();
    consumer.write(&manifest.replace("program = \"./workload\"", "program = \"workload\""));
    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    let search_path = std::env::join_paths(
        std::iter::once(consumer.root.clone()).chain(std::env::split_paths(&inherited_path)),
    )
    .unwrap();
    let output = consumer
        .command("check")
        .env("PATH", search_path)
        .args(["--output-dir", "selected output", "--output-format", "json"])
        .output()
        .unwrap();
    assert_success(&output);
    let stdout: BenchmarkSummary = serde_json::from_slice(&output.stdout).unwrap();
    let summaries = summaries_under(&consumer.root.join("selected output"));
    assert_eq!(summaries.len(), 1);
    assert!(current_ir(&stdout) > 0);
    assert_eq!(current_ir(&stdout), current_ir(&summaries[0]));
    assert_eq!(stdout.benchmark_exe, PathBuf::from("workload"));
    assert!(!consumer.root.join("output").exists());
    assert!(!consumer.directory.path().join("selected output").exists());
}

#[test]
fn test_regressions_do_not_filter_later_cases() {
    let consumer = Consumer::new();
    consumer.manifest("measure", "guard_region", Some(1));
    let mut manifest = fs::read_to_string(consumer.root.join("guard.toml")).unwrap();
    manifest.push_str(
        "\n[[cases]]\nid = \"Quote.Single.Kernel\"\n[cases.callgrind.hard_limits]\nIr = 1000000\n",
    );
    consumer.write(&manifest);
    assert_eq!(consumer.run("check").status.code(), Some(3));
    let summaries = consumer.summaries();
    assert_eq!(summaries.len(), 2);
    assert!(summaries.iter().all(|summary| current_ir(summary) > 0));
    assert_eq!(
        summaries
            .iter()
            .map(|summary| summary.function_name.as_str())
            .collect::<Vec<_>>(),
        ["Quote.Single.Kernel", "Quote.Single.Owner"],
    );
}

#[rstest]
#[case::child_failure("fail")]
#[case::missing_binary("deleted-binary")]
#[case::missing_region("skip")]
fn test_successful_prior_output_cannot_replace_a_failed_current_run(#[case] mode: &str) {
    let consumer = Consumer::new();
    consumer.manifest("measure", "guard_region", Some(1_000_000));
    assert_success(&consumer.run("check"));
    assert!(current_ir(&consumer.summaries()[0]) > 0);

    if mode == "deleted-binary" {
        fs::remove_file(consumer.root.join("workload")).unwrap();
    } else {
        consumer.manifest(mode, "guard_region", Some(1_000_000));
    }
    assert_failure(&consumer.run("check"));
    for summary in consumer.summaries() {
        assert_eq!(
            current_ir(&summary),
            0,
            "A previous successful summary was retained as current"
        );
    }
}

#[test]
fn test_unsupported_filters_and_environment_cannot_disable_the_guard() {
    let consumer = Consumer::new();
    consumer.manifest("measure", "guard_region", Some(1));
    for option in [
        "--filter=missing",
        "missing",
        "--load-baseline=old",
        "--test",
    ] {
        let output = consumer.command("check").arg(option).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(consumer.output_files(), [] as [PathBuf; 0]);
    }

    let output = consumer
        .command("check")
        .envs([
            ("CARGO_PKG_NAME", "not-the-standalone-project"),
            ("CARGO_TARGET_DIR", "not-the-output-directory"),
            ("GUNGRAUN_CALLGRIND_ARGS", "--collect-atstart=no"),
            ("GUNGRAUN_CALLGRIND_LIMITS", "Ir=999999999999"),
            ("GUNGRAUN_DEFAULT_TOOL", "perf"),
            ("GUNGRAUN_FILTER", "missing"),
            ("GUNGRAUN_LIST", "true"),
            ("GUNGRAUN_LOAD_BASELINE", "old"),
            ("GUNGRAUN_SAVE_BASELINE", "reviewed"),
            ("GUNGRAUN_TEST", "true"),
            ("GUNGRAUN_TOOLS", "none"),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(current_ir(&consumer.summaries()[0]) > 0);
}

#[rstest]
#[case::not_entered("skip", "guard_region")]
#[case::nonexistent_symbol("measure", "symbol_that_does_not_exist")]
fn test_zero_or_missing_region_is_not_a_passing_measurement(
    #[case] mode: &str,
    #[case] entry: &str,
) {
    let consumer = Consumer::new();
    consumer.manifest(mode, entry, Some(1_000_000));
    assert_failure(&consumer.run("check"));
    let summaries = consumer.summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(current_ir(&summaries[0]), 0);
}
