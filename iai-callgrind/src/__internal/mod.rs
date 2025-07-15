//! This module is only used for internal purposes and does not contain any publicly usable
//! structs
#![allow(missing_docs)]

pub mod bin_bench;
pub mod error;
pub mod lib_bench;

// The runner api is not used directly in order to decouple the user interface and
// documentation from the internal usage.
//
// We re-export all structs with the `Internal` prefix to avoid accidental usage. The wrapper
// structs provided by the iai-callgrind module (in `iai_callgrind::bin_bench`, ...) are the
// structs to be used by the iai-callgrind end-user. Almost all of these structs use the
// builder pattern to build the api internal structures. The documentation visible to the user
// can be found in these builders.
//
// As an exception, enums from the runner api are usually used directly and re-exported in
// `lib.rs`.
pub use iai_callgrind_runner::api::{
    BinaryBenchmark as InternalBinaryBenchmark,
    BinaryBenchmarkBench as InternalBinaryBenchmarkBench,
    BinaryBenchmarkConfig as InternalBinaryBenchmarkConfig,
    BinaryBenchmarkGroup as InternalBinaryBenchmarkGroup,
    BinaryBenchmarkGroups as InternalBinaryBenchmarkGroups,
    CachegrindRegressionConfig as InternalCachegrindRegressionConfig,
    CallgrindRegressionConfig as InternalCallgrindRegressionConfig, Command as InternalCommand,
    Delay as InternalDelay, DhatRegressionConfig as InternalDhatRegressionConfig,
    EntryPoint as InternalEntryPoint, ExitWith as InternalExitWith, Fixtures as InternalFixtures,
    FlamegraphConfig as InternalFlamegraphConfig,
    LibraryBenchmark as InternalLibraryBenchmarkBenches,
    LibraryBenchmarkBench as InternalLibraryBenchmarkBench,
    LibraryBenchmarkConfig as InternalLibraryBenchmarkConfig,
    LibraryBenchmarkGroup as InternalLibraryBenchmarkGroup,
    LibraryBenchmarkGroups as InternalLibraryBenchmarkGroups, Metric as InternalMetric,
    OutputFormat as InternalOutputFormat, RawArgs as InternalRawArgs, Sandbox as InternalSandbox,
    Tool as InternalTool, ToolFlamegraphConfig as InternalToolFlamegraphConfig,
    ToolOutputFormat as InternalToolOutputFormat,
    ToolRegressionConfig as InternalToolRegressionConfig, Tools as InternalTools,
};

/// Used in iai-callgrind-macros to store the essential information about a library benchmark
#[derive(Debug, Clone)]
pub struct InternalMacroLibBench {
    pub id_display: Option<&'static str>,
    pub args_display: Option<&'static str>,
    pub func: fn(),
    pub config: Option<fn() -> InternalLibraryBenchmarkConfig>,
}

/// Used in iai-callgrind-macros to store the essential information about a binary benchmark
#[derive(Debug, Clone)]
pub struct InternalMacroBinBench {
    pub id_display: Option<&'static str>,
    pub args_display: Option<&'static str>,
    pub func: fn() -> crate::Command,
    pub setup: Option<fn()>,
    pub teardown: Option<fn()>,
    pub config: Option<fn() -> InternalBinaryBenchmarkConfig>,
}

/// A small internal helper to easily create module paths like `file::group::benchmark::id`
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModulePath(String);

impl ModulePath {
    pub fn new(path: &str) -> Self {
        Self(path.to_owned())
    }

    #[must_use]
    pub fn join(&self, path: &str) -> Self {
        Self(format!("{}::{path}", self.0))
    }
}

impl std::fmt::Display for ModulePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub enum BenchmarkKind {
    BinaryBenchmark,
    LibraryBenchmark,
}

pub struct Runner {
    cmd: std::process::Command,
    module_path: String,
}

impl Runner {
    pub fn new(
        exe: Option<&str>,
        kind: &BenchmarkKind,
        package_dir: &str,
        package_name: &str,
        file: &str,
        module_path: &str,
        bench_bin: String,
    ) -> Self {
        const LIBRARY_VERSION: &str = "0.15.2";

        let mut cmd = std::process::Command::new(exe.unwrap_or("iai-callgrind-runner"));
        cmd.arg(LIBRARY_VERSION);

        match kind {
            BenchmarkKind::BinaryBenchmark => {
                cmd.arg("--bin-bench");
            }
            BenchmarkKind::LibraryBenchmark => {
                cmd.arg("--lib-bench");
            }
        }

        cmd.arg(package_dir);
        cmd.arg(package_name);
        cmd.arg(file);
        cmd.arg(module_path);
        cmd.arg(bench_bin); // The executable benchmark binary

        Self {
            cmd,
            module_path: module_path.to_owned(),
        }
    }

    /// Execute `iai-callgrind-runner` exiting with the exit code of the runner if not `0`
    pub fn exec(mut self, encoded: Vec<u8>) -> Result<(), error::Errors> {
        let mut child = self
            .cmd
            .arg(encoded.len().to_string())
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                let mut errors = error::Errors::default();
                errors.add(error::Error::new(
                    &ModulePath::new(&self.module_path),
                    &format!(
                        "Failed to run benchmarks: {e}.\n\nIs iai-callgrind-runner installed and \
                         iai-callgrind-runner in your $PATH?.\nYou can set the environment \
                         variable IAI_CALLGRIND_RUNNER to the absolute path of the \
                         iai-callgrind-runner executable.\n\nMake sure you have followed the \
                         installation instructions in the guide:\n\
                         https://iai-callgrind.github.io/iai-callgrind/latest/html/installation/iai_callgrind.html",
                    ),
                ));
                errors
            })?;

        let mut stdin = child
            .stdin
            .take()
            .expect("Opening stdin to submit encoded benchmark");
        std::thread::spawn(move || {
            use std::io::Write;
            stdin
                .write_all(&encoded)
                .expect("Writing encoded benchmark to stdin");
        });

        let status = child.wait().expect(
            "Internal error: Waiting for child process to exit should succeed. If the problem \
             persists please submit a bug report.",
        );
        if !status.success() {
            let code = status.code().unwrap_or(1i32);
            std::process::exit(code);
        }

        Ok(())
    }
}
