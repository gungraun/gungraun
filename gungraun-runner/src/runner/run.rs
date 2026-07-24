//! Main entry point for the gungraun benchmark runner process with [`run`] doing the hard work
//!
//! This module via `gungraun_runner::main` is invoked by the gungraun benchmarking harness (not by
//! the end user directly). The harness spawns the runner as a subprocess and passes basic benchmark
//! metadata via command-line arguments. The actual benchmark configuration
//! ([`LibraryBenchmarkGroups`] or [`BinaryBenchmarkGroups`]) is sent through stdin as a
//! bincode-encoded payload and deserialized by [`receive_benchmark`].
//!
//! The harness formats [`SupportedTools`] through [`std::fmt::Display`] as one of those
//! command-line arguments, and `RunnerArgs::new` parses it through [`FromStr`]. This describes
//! compile-target support only; runtime availability, such as whether an executable is installed or
//! Perf events are permitted, is checked separately by the runner.
//!
//! # Runner flow
//!
//! 1. **CLI parsing**: `Cli::parse` inspects the first argument to decide between showing help,
//!    version, or executing a benchmark run.
//! 2. **Version check**: `compare_versions` validates that the runner version matches the library
//!    version that spawned it, preventing mismatched ABI issues.
//! 3. **Harness argument parsing**: `RunnerArgs::new` reads the remaining arguments passed by the
//!    harness: benchmark kind (`--lib-bench` or `--bin-bench`), paths, target, and module.
//! 4. **Configuration reception**: [`receive_benchmark`] reads the serialized
//!    [`LibraryBenchmarkGroups`] or [`BinaryBenchmarkGroups`] from stdin.
//! 5. **Dispatch**: [`run`] creates a [`Config`] and dispatches to either [`lib_bench::run`] or
//!    [`bin_bench::run`], depending on the benchmark kind.
//! 6. **Post-run**: `PostRun::execute` prints the benchmark summary and returns
//!    [`Error::RegressionError`] if any regressions were detected.

use std::env::ArgsOs;
use std::ffi::OsString;
use std::io::{BufReader, stdin};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use gungraun_common::SupportedTools;
use log::debug;

use super::args::CommandLineArgs;
use super::common::{BenchmarkSummaries, Config, ModulePath};
use super::format::OutputFormatKind;
use super::meta::Metadata;
use super::{bin_bench, lib_bench};
use crate::api::{BinaryBenchmarkGroups, LibraryBenchmarkGroups};
use crate::error::Error;
use crate::summary::model::BenchmarkKind;

#[derive(Debug)]
enum Cli {
    Runner(RunnerArgs),
    ShortHelp,
    LongHelp,
    Version,
}

/// Execute post benchmark run actions like printing the summary line with regressions
#[derive(Debug)]
struct PostRun {
    benchmark_summaries: BenchmarkSummaries,
    nosummary: bool,
    output_format_kind: OutputFormatKind,
}

/// The arguments sent by the gungraun benchmarking harness
///
/// These are not the user arguments of the `cargo bench ... -- ARGS` command.
#[derive(Debug)]
struct RunnerArgs {
    _package_name: String,
    bench_bin: PathBuf,
    bench_file: PathBuf,
    bench_kind: BenchmarkKind,
    module: String,
    num_bytes: usize,
    package_dir: PathBuf,
    supported_tools: SupportedTools,
    target: String,
}

#[derive(Debug)]
struct RunnerArgsIterator(ArgsOs);

impl Cli {
    fn parse() -> Result<Self> {
        let mut args = std::env::args_os().skip(1);

        let next = args.next().map(|s| s.to_string_lossy().into_owned());
        match next.as_deref() {
            Some("-h") => Ok(Self::ShortHelp),
            Some("--help") | None => Ok(Self::LongHelp),
            Some("--version" | "-V") => Ok(Self::Version),
            _ => Ok(Self::Runner(RunnerArgs::new()?)),
        }
    }
}

impl PostRun {
    /// Creates a new `PostRun`.
    fn new(
        nosummary: bool,
        output_format_kind: OutputFormatKind,
        benchmark_summaries: BenchmarkSummaries,
    ) -> Self {
        Self {
            benchmark_summaries,
            nosummary,
            output_format_kind,
        }
    }

    /// Print the summary returning [`Error::RegressionError`] if regressions were present
    ///
    /// The summary is not printed if `nosummary` is true or the [`OutputFormatKind`] is not the
    /// default format (i.e. JSON).
    fn execute(self) -> Result<()> {
        self.benchmark_summaries
            .print(self.nosummary, self.output_format_kind);

        if self.benchmark_summaries.is_regressed() {
            Err(Error::RegressionError(false).into())
        } else {
            Ok(())
        }
    }
}

impl RunnerArgs {
    fn new() -> Result<Self> {
        let runner_version = env!("CARGO_PKG_VERSION").to_owned();

        let mut args_iter = RunnerArgsIterator::new();

        let runner = args_iter.next_path()?;
        debug!("Runner executable: '{}'", runner.display());

        let library_version = args_iter.next_string()?;

        compare_versions(runner_version, library_version)?;

        let bench_kind = match args_iter.next_string()?.as_str() {
            "--lib-bench" => BenchmarkKind::LibraryBenchmark,
            "--bin-bench" => BenchmarkKind::BinaryBenchmark,
            kind => {
                return Err(Error::InitError(format!("Invalid benchmark kind: {kind}")).into());
            }
        };

        let package_dir = args_iter.next_path()?;
        let package_name = args_iter.next_string()?;
        let bench_file = args_iter.next_path()?;
        let module = args_iter.next_string()?;
        let target = args_iter.next_string()?;
        let supported_tools =
            SupportedTools::from_str(&args_iter.next_string()?).map_err(anyhow::Error::msg)?;
        let bench_bin = args_iter.next_path()?;
        let num_bytes = args_iter
            .next_string()?
            .parse::<usize>()
            .map_err(|_| Error::InitError("Failed to parse number of bytes".to_owned()))?;

        Ok(Self {
            bench_bin,
            bench_file,
            bench_kind,
            module,
            num_bytes,
            package_dir,
            _package_name: package_name,
            supported_tools,
            target,
        })
    }
}

impl RunnerArgsIterator {
    fn new() -> Self {
        Self(std::env::args_os())
    }

    fn next(&mut self) -> Result<OsString> {
        self.0
            .next()
            .ok_or_else(|| Error::InitError("Unexpected number of arguments".to_owned()).into())
    }

    fn next_string(&mut self) -> Result<String> {
        self.next()?
            .to_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::InitError("Invalid utf-8 string".to_owned()).into())
    }

    fn next_path(&mut self) -> Result<PathBuf> {
        Ok(PathBuf::from(self.next()?))
    }
}

fn compare_versions<R, L>(runner_version: R, library_version: L) -> Result<()>
where
    R: AsRef<str>,
    L: AsRef<str>,
{
    match version_compare::compare(runner_version.as_ref(), library_version.as_ref()) {
        Ok(cmp) => match cmp {
            version_compare::Cmp::Lt | version_compare::Cmp::Gt => {
                return Err(Error::VersionMismatch(
                    cmp,
                    runner_version.as_ref().to_owned(),
                    library_version.as_ref().to_owned(),
                )
                .into());
            }
            // version_compare::compare only returns Cmp::Lt, Cmp::Gt and Cmp::Eq so the versions
            // are equal here
            _ => {}
        },
        // gungraun versions before 0.3.0 don't submit the version
        Err(()) => {
            return Err(Error::VersionMismatch(
                version_compare::Cmp::Ne,
                runner_version.as_ref().to_owned(),
                library_version.as_ref().to_owned(),
            )
            .into());
        }
    }

    Ok(())
}

/// Method to read, decode and deserialize the data sent by gungraun
///
/// gungraun uses elements from the [`crate::api`], so the runner can understand which elements
/// can be received by this method
///
/// With bincode update to 2 the length checked was dropped and `_num_bytes` is ignored.
pub fn receive_benchmark<T>(_num_bytes: usize) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    bincode_next::serde::decode_from_reader(
        BufReader::new(stdin().lock()),
        bincode_next::config::legacy(),
    )
    .with_context(|| "Failed to decode configuration")
}

/// Run this benchmark
pub fn run() -> Result<()> {
    // The term width env var is not intended for public usage.
    let term_width = std::env::var(super::envs::GUNGRAUN_TERM_WIDTH)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok());

    let runner_args = match (Cli::parse()?, term_width) {
        (Cli::Runner(runner_args), _) => runner_args,
        (Cli::ShortHelp, Some(width)) => {
            return CommandLineArgs::command()
                .term_width(width)
                .print_help()
                .map_err(Into::into);
        }
        (Cli::ShortHelp, None) => {
            return CommandLineArgs::command().print_help().map_err(Into::into);
        }
        (Cli::LongHelp, Some(width)) => {
            return CommandLineArgs::command()
                .term_width(width)
                .print_long_help()
                .map_err(Into::into);
        }
        (Cli::LongHelp, None) => {
            return CommandLineArgs::command()
                .print_long_help()
                .map_err(Into::into);
        }
        (Cli::Version, _) => {
            CommandLineArgs::parse_from(["--version"]);
            return Ok(());
        }
    };

    let RunnerArgs {
        bench_kind,
        package_dir,
        bench_file,
        module,
        bench_bin,
        num_bytes,
        supported_tools,
        target,
        ..
    } = runner_args;

    let post_run = match bench_kind {
        BenchmarkKind::LibraryBenchmark => {
            let benchmark_groups: LibraryBenchmarkGroups = receive_benchmark(num_bytes)?;
            let meta = Metadata::new(
                &benchmark_groups.command_line_args,
                &target,
                supported_tools,
            )?;

            let config = Config {
                package_dir,
                bench_file,
                module_path: ModulePath::new(&module),
                bench_bin,
                meta,
            };

            let CommandLineArgs {
                output_format,
                list,
                nosummary,
                format,
                ignored,
                ..
            } = config.meta.args;

            if list {
                return lib_bench::list(benchmark_groups, &config, format, ignored);
            }

            lib_bench::run(benchmark_groups, config)
                .map(|summaries| PostRun::new(nosummary, output_format, summaries))?
        }
        BenchmarkKind::BinaryBenchmark => {
            let benchmark_groups: BinaryBenchmarkGroups = receive_benchmark(num_bytes)?;
            let meta = Metadata::new(
                &benchmark_groups.command_line_args,
                &target,
                supported_tools,
            )?;

            let config = Config {
                package_dir,
                bench_file,
                module_path: ModulePath::new(&module),
                bench_bin,
                meta,
            };

            let CommandLineArgs {
                output_format,
                list,
                nosummary,
                format,
                ignored,
                ..
            } = config.meta.args;

            if list {
                return bin_bench::list(benchmark_groups, &config, format, ignored);
            }

            bin_bench::run(benchmark_groups, config)
                .map(|summaries| PostRun::new(nosummary, output_format, summaries))?
        }
    };

    post_run.execute()
}
