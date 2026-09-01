//! This module is only used for internal purposes and does not contain any publicly usable
//! structs
#![allow(missing_docs)]

pub mod bin_bench;
pub mod error;
pub mod lib_bench;
pub mod perf;

pub mod stats {
    pub use gungraun_runner::api::{calibrate_linear, logistic};
}

use gungraun_common::supported_tools;
// The runner api is not used directly in order to decouple the user interface and
// documentation from the internal usage.
//
// We re-export all structs with the `Internal` prefix to avoid accidental usage. The wrapper
// structs provided by the gungraun module (in `gungraun::bin_bench`, ...) are the
// structs to be used by the gungraun end-user. Almost all of these structs use the
// builder pattern to build the api internal structures. The documentation visible to the user
// can be found in these builders.
//
// As an exception, enums from the runner api are usually used directly and re-exported in
// `lib.rs`.
pub use gungraun_runner::api::{
    BenchRunMode as InternalBenchRunMode, BinaryBenchmark as InternalBinaryBenchmark,
    BinaryBenchmarkBench as InternalBinaryBenchmarkBench,
    BinaryBenchmarkConfig as InternalBinaryBenchmarkConfig,
    BinaryBenchmarkGroup as InternalBinaryBenchmarkGroup,
    BinaryBenchmarkGroups as InternalBinaryBenchmarkGroups,
    CachegrindRegressionConfig as InternalCachegrindRegressionConfig,
    CallgrindRegressionConfig as InternalCallgrindRegressionConfig, Command as InternalCommand,
    CommandKind as InternalCommandKind, Delay as InternalDelay,
    DhatRegressionConfig as InternalDhatRegressionConfig, DhatSpec as InternalDhatSpec,
    EntryPoint as InternalEntryPoint, ExitWith as InternalExitWith, Fixtures as InternalFixtures,
    FlamegraphConfig as InternalFlamegraphConfig,
    LibraryBenchmark as InternalLibraryBenchmarkBenches,
    LibraryBenchmarkBench as InternalLibraryBenchmarkBench,
    LibraryBenchmarkConfig as InternalLibraryBenchmarkConfig,
    LibraryBenchmarkGroup as InternalLibraryBenchmarkGroup,
    LibraryBenchmarkGroups as InternalLibraryBenchmarkGroups, OutputFormat as InternalOutputFormat,
    PERF_ACK_FD_READ, PERF_ACK_FD_WRITE, PERF_CTL_FD_READ, PERF_CTL_FD_WRITE, PERF_LOG_FD,
    PERF_REPETITIONS_MARKER, PerfRegressionConfig as InternalPerfRegressionConfig,
    PerfRunMode as InternalPerfRunMode, PerfSpec as InternalPerfSpec,
    RawToolArgs as InternalRawArgs, Sandbox as InternalSandbox,
    ToolFlamegraphConfig as InternalToolFlamegraphConfig,
    ToolOutputFormat as InternalToolOutputFormat,
    ToolRegressionConfig as InternalToolRegressionConfig, ToolSpec as InternalToolSpec,
    ToolSpecOptions as InternalToolSpecOptions, ToolSpecs as InternalToolSpecs,
};

#[derive(Debug)]
pub enum BenchmarkKind {
    BinaryBenchmark,
    LibraryBenchmark,
}

#[derive(Debug, Clone, Copy, Eq)]
pub enum InternalBinAssistantKind {
    Iter(fn(Option<usize>)),
    Default(fn()),
    None,
}

#[derive(Debug, Clone)]
pub enum InternalBinFunctionKind {
    Iter(fn() -> Vec<crate::Command>),
    Default(fn() -> crate::Command),
}

#[derive(Debug, Clone, Copy)]
pub enum InternalLibFunctionKind {
    Iter(fn(InternalBenchRunMode, Option<usize>) -> usize),
    Default(fn(InternalBenchRunMode)),
}

/// Used in gungraun-macros to store the essential information about a binary benchmark
#[derive(Debug, Clone)]
pub struct InternalMacroBinBench {
    /// The arguments as a display string for output purposes
    pub args_display: Option<&'static str>,
    /// The configuration at `#[bench]` level
    pub config: Option<fn() -> InternalBinaryBenchmarkConfig>,
    /// The const generic arguments as a display string for output purposes
    pub consts_display: Option<&'static str>,
    /// The kind of function (Default or Iter)
    pub func: InternalBinFunctionKind,
    /// The id of the benchmark as a display string
    pub id_display: Option<&'static str>,
    /// The setup function kind
    pub setup: InternalBinAssistantKind,
    /// The teardown function kind
    pub teardown: InternalBinAssistantKind,
}

/// Used in gungraun-macros to store the essential information about a library benchmark
#[derive(Debug, Clone)]
pub struct InternalMacroLibBench {
    /// The arguments as a display string for output purposes
    pub args_display: Option<&'static str>,
    /// The configuration at `#[bench]` level
    pub config: Option<fn() -> InternalLibraryBenchmarkConfig>,
    /// The const generic arguments as a display string for output purposes
    pub consts_display: Option<&'static str>,
    /// The kind of function (Default or Iter)
    pub func: InternalLibFunctionKind,
    /// The id of the benchmark as a display string
    pub id_display: Option<&'static str>,
}

/// A small internal helper to easily create module paths like `file::group::benchmark::id`
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModulePath(String);

#[derive(Debug)]
pub struct Runner {
    cmd: std::process::Command,
    module_path: String,
}

impl InternalBinAssistantKind {
    pub fn is_some(&self) -> bool {
        !matches!(*self, Self::None)
    }
}

impl PartialEq for InternalBinAssistantKind {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Iter(_), Self::Iter(_))
                | (Self::Default(_), Self::Default(_))
                | (Self::None, Self::None)
        )
    }
}

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
        const LIBRARY_VERSION: &str = "0.19.4";

        let mut cmd = std::process::Command::new(exe.unwrap_or("gungraun-runner"));
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
        cmd.arg(env!("__GUNGRAUN_BUILD_TARGET"));
        cmd.arg(supported_tools().to_string());
        cmd.arg(bench_bin); // The executable benchmark binary

        Self {
            cmd,
            module_path: module_path.to_owned(),
        }
    }

    /// Execute `gungraun-runner` exiting with the exit code of the runner if not `0`
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
                        "Failed to run benchmarks: {e}.\n\nIs gungraun-runner installed and \
                        gungraun-runner in your $PATH?.\nYou can set the environment variable \
                        GUNGRAUN_RUNNER to the absolute path of the gungraun-runner \
                        executable.\n\nMake sure you have followed the installation \
                        instructions in the guide:\n\
                        https://gungraun.github.io/gungraun/latest/html/installation/gungraun.html",
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
