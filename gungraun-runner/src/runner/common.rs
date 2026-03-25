//! This module contains elements which are common to library and binary benchmarks

mod defaults {
    pub const SANDBOX_ENABLED: bool = false;
    pub const SANDBOX_FIXTURES_FOLLOW_SYMLINKS: bool = false;
    pub const COMPARE_BY_ID: bool = false;
}

use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Display;
use std::fs::File;
use std::io::{stderr, stdout, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio as StdStdio};
use std::sync::{atomic, Arc};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use log::{debug, log_enabled, warn, Level};
use tempfile::{tempfile, TempDir};

use super::format::{OutputFormatKind, SummaryFormatter};
use super::meta::Metadata;
use super::summary::{BenchmarkSummary, SummaryOutput};
use crate::api::{
    self, BinaryBenchmarkGroups, EntryPoint, LibraryBenchmarkGroups, Pipe, ValgrindTool,
};
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::bin_bench::{self, BinBench};
use crate::runner::callgrind::flamegraph::{
    BaselineFlamegraphGenerator, Flamegraph, FlamegraphGenerator, LoadBaselineFlamegraphGenerator,
    SaveBaselineFlamegraphGenerator,
};
use crate::runner::callgrind::parser::Sentinel;
use crate::runner::format::{
    self, BinaryBenchmarkHeader, Header, LibraryBenchmarkHeader, OutputFormat,
};
use crate::runner::lib_bench::{self, LibBench};
use crate::runner::summary::{FlamegraphSummary, Profile, ProfileData};
use crate::runner::tasks::ThreadPool;
use crate::runner::tool::config::ToolFlamegraphConfig;
use crate::runner::tool::parser::{Parser, ParserOutput};
use crate::runner::tool::path::{ToolOutputPath, ToolOutputPathKind};
use crate::runner::tool::regression::ToolRegressionConfig;
use crate::util::{copy_directory, make_absolute};

/// Analyzer tuple containing parser, output path, regression config, flamegraph config, and entry
/// point.
pub type Analyzer = (
    Box<dyn Parser>,
    ToolOutputPath,
    ToolRegressionConfig,
    ToolFlamegraphConfig,
    EntryPoint,
);

/// The `Baselines` type
pub type Baselines = (Option<String>, Option<String>);

/// the [`Assistant`] kind
#[derive(Debug, Clone, Copy)]
pub enum AssistantKind {
    /// The `setup` function
    Setup,
    /// The `teardown` function
    Teardown,
}

/// Container for either library or binary benchmark entries within a group.
#[derive(Debug, Clone)]
pub enum Benches {
    /// Collection of library benchmarks.
    LibBenches(Vec<LibBench>),
    /// Collection of binary benchmarks.
    BinBenches(Vec<BinBench>),
}

/// The configuration values for the maximum amount of parallelism
#[derive(Debug, Copy, Clone)]
pub enum MaxParallel {
    /// No maximum for the amount of parallelism (0 or not specified)
    NoMaximum,
    /// No parallelism, run serially (1)
    Serial,
    /// Use this as maximum for the amount of parallelism (N >= 2)
    Count(usize),
}

/// Data processor used for regular benchmark runs without explicit baseline save/load mode.
#[derive(Debug)]
pub struct BaselineDataProcessor {
    /// Analyzer pipeline used to parse and process benchmark outputs.
    pub analyzers: Vec<Analyzer>,
}

/// An `Assistant` corresponds to the `setup` or `teardown` functions in the UI
#[derive(Debug, Clone)]
pub struct Assistant {
    envs: Vec<(OsString, OsString)>,
    group_index: Option<usize>,
    indices: Option<(usize, usize, Option<usize>)>,
    kind: AssistantKind,
    pipe: Option<Pipe>,
    run_parallel: bool,
}
/// Contains benchmark summaries of (binary, library) benchmark runs and their execution time
///
/// Used to print a final summary after all benchmarks.
#[derive(Debug, Default)]
pub struct BenchmarkSummaries {
    /// The amount of filtered benchmarks
    pub num_filtered: usize,
    /// The benchmark summaries
    pub summaries: Vec<BenchmarkSummary>,
    /// The execution time of all benchmarks.
    pub total_time: Option<Duration>,
}

/// The `Config` contains all the information extracted from the UI invocation of the runner
#[derive(Debug, Clone)]
pub struct Config {
    /// The path to the compiled binary with the benchmark harness
    pub bench_bin: PathBuf,
    /// The path to the benchmark file which contains the benchmark harness
    pub bench_file: PathBuf,
    /// The [`Metadata`]
    pub meta: Metadata,
    /// The module path of the benchmark file
    pub module_path: ModulePath,
    /// The package directory of the package in which `gungraun` (not the runner) is used
    pub package_dir: PathBuf,
}

/// A `Group` is the organizational unit and counterpart of the `library_benchmark_group!` macro
#[derive(Debug, Clone)]
pub struct Group {
    /// The benchmarks belonging to this group.
    pub benches: Benches,
    /// Whether summaries with equal benchmark ids are compared and printed.
    pub compare_by_id: bool,
    /// The index of this group in the top-level benchmark list.
    pub index: usize,
    /// The maximum amount of parallel threads to use for the [`ThreadPool`]
    pub max_parallel: MaxParallel,
    /// The module path prefix used for this group in output and diagnostics.
    pub module_path: ModulePath,
    /// Number of benchmarks filtered out before execution.
    pub num_filtered: usize,
    /// Optional setup assistant run before group benchmarks.
    pub setup: Option<Assistant>,
    /// Optional teardown assistant run after group benchmarks.
    pub teardown: Option<Assistant>,
}

/// `Groups` is the top-level organizational unit of the `main!` macro for library benchmarks
#[derive(Debug, Clone)]
pub struct Groups(pub Vec<Group>);

/// Result payload returned by worker jobs when running a benchmark group.
#[derive(Debug)]
pub struct JobResult {
    /// Final benchmark summary produced by the executed job.
    pub benchmark_summary: BenchmarkSummary,
    /// Captured stdout/stderr output associated with this job.
    pub captured_output: CapturedOutput,
    /// Data processor used to parse and finalize tool outputs.
    pub data_processor: Box<dyn BenchmarkDataProcessor>,
    /// Whether regressions in this job should immediately fail the run.
    pub fail_fast: bool,
    /// Header metadata used for formatting benchmark output.
    pub header: Header,
    /// Output formatting configuration used when printing this job result.
    pub output_format: OutputFormat,
}

/// Data processor used when loading and comparing against an existing baseline.
#[derive(Debug)]
pub struct LoadBaselineDataProcessor {
    /// Analyzer pipeline used to parse and process benchmark outputs.
    pub analyzers: Vec<Analyzer>,
}

/// A helper struct similar to a file path but for module paths with the `::` delimiter
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModulePath(String);

/// The `Sandbox` in which benchmarks should be runs
///
/// As soon as the `Sandbox` is dropped the temporary directory is deleted.
#[derive(Debug)]
pub struct Sandbox {
    temp_dir: Option<TempDir>,
}

/// Data processor used when saving current outputs as a baseline.
#[derive(Debug)]
pub struct SaveBaselineDataProcessor {
    /// Analyzer pipeline used to parse and process benchmark outputs.
    pub analyzers: Vec<Analyzer>,
}

/// The main runner to run all library or binary benchmarks
#[derive(Debug)]
pub struct Runner {
    config: Arc<Config>,
    groups: Groups,
    setup: Option<Assistant>,
    teardown: Option<Assistant>,
}

/// Temporary output files used to capture benchmark stdout and stderr.
#[derive(Debug)]
pub struct CapturedOutput {
    /// Temporary file receiving captured stderr output.
    pub stderr: File,
    /// Temporary file receiving captured stdout output.
    pub stdout: File,
}

/// Shared post-processing interface for library and binary benchmark runs.
pub trait BenchmarkDataProcessor: std::fmt::Debug + Send {
    /// Returns the analyzer pipeline used for parsing and artifact generation.
    fn analyzers(&self) -> &[Analyzer];

    /// Copies temporary output files to their final benchmark output location.
    ///
    /// # Errors
    ///
    /// Returns an error if file copying fails.
    fn copy_temp(&self) -> Result<()> {
        self.analyzers()
            .first()
            .map_or(Ok(()), |(_, o, _, _, _)| o.copy_temp())
    }

    /// Creates and initializes the benchmark output directory for this processor.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails.
    fn create_benchmark_directory(&self) -> Result<()> {
        self.analyzers()
            .first()
            .map_or(Ok(()), |(_, o, _, _, _)| o.init())
    }

    /// Processes the benchmark data by parsing output and moving profiling data into place.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing, regression checks, or artifact generation fails.
    fn finalize(
        &mut self,
        benchmark_summary: &mut BenchmarkSummary,
        config: &Config,
        header: &Header,
    ) -> Result<()>;

    /// Generates flamegraph summaries for a parsed benchmark output.
    ///
    /// The provided [`Config`], [`Header`], [`ToolOutputPath`], [`ToolFlamegraphConfig`], and
    /// [`EntryPoint`] determine the flamegraph generator behavior and output paths.
    ///
    /// # Errors
    ///
    /// Returns an error if flamegraph generation fails.
    fn generate_flamegraphs(
        &self,
        config: &Config,
        header: &Header,
        output_path: &ToolOutputPath,
        flamegraph_config: &ToolFlamegraphConfig,
        entry_point: &EntryPoint,
    ) -> Result<Vec<FlamegraphSummary>>;

    /// Returns whether there are [`Analyzer`] and therefore benchmarks to process.
    fn has_benchmarks(&self) -> bool;

    /// Moves new benchmark outputs from the temporary location to the final benchmark directory.
    ///
    /// # Errors
    ///
    /// Returns an error if moving files fails.
    fn move_temp(&self) -> Result<()> {
        self.analyzers()
            .first()
            .map_or(Ok(()), |(_, o, _, _, _)| o.move_temp())
    }

    /// Parses tool outputs and appends processed profiles to the [`BenchmarkSummary`].
    ///
    /// The method compares newly parsed data with `parsed_old` when provided, otherwise it loads
    /// baseline data from analyzer backends. Parsed data is enriched with regression and
    /// flamegraph information before being added to the summary.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails, no new dataset is available, output paths cannot be
    /// resolved, or flamegraph generation fails.
    fn parse(
        &self,
        benchmark_summary: &mut BenchmarkSummary,
        config: &Config,
        header: &Header,
        parsed_old: Option<Vec<Vec<ParserOutput>>>,
    ) -> Result<()> {
        let iter: Box<dyn Iterator<Item = Option<Vec<ParserOutput>>>> =
            if let Some(old) = parsed_old {
                Box::new(old.into_iter().map(Some))
            } else {
                Box::new(std::iter::repeat(None).take(self.analyzers().len()))
            };

        for (
            (parser, output_path, regression_config, flamegraph_config, entry_point),
            parsed_old,
        ) in self.analyzers().iter().zip(iter)
        {
            let tool = output_path.tool;

            let parsed_new = parser.parse().and_then(|parsed| {
                if parsed.is_empty() {
                    Err(anyhow!("A new dataset should always be present"))
                } else {
                    Ok(parsed)
                }
            })?;

            let parsed_old = if let Some(old) = parsed_old {
                old
            } else {
                parser.parse_base()?
            };

            let data = ProfileData::new(parsed_new, (!parsed_old.is_empty()).then_some(parsed_old));

            let mut profile = Profile {
                tool,
                log_paths: output_path.to_log_output().real_paths()?,
                out_paths: output_path.real_paths()?,
                summaries: data,
                flamegraphs: vec![],
            };

            profile.summaries.total.regressions = regression_config.check(&profile.summaries.total);
            profile.flamegraphs = self.generate_flamegraphs(
                config,
                header,
                output_path,
                flamegraph_config,
                entry_point,
            )?;

            benchmark_summary.profiles.push(profile);
        }

        Ok(())
    }

    /// Remove the summary json file.
    ///
    /// It doesn't matter which output path we use for this method. We cleanup the summary file if
    /// it exists no matter if we create a new one or not. It is confusing if there is an summary
    /// file present for an old run and the costs don't match anymore.
    fn remove_summary(&self) -> Result<()> {
        self.analyzers().first().map_or(Ok(()), |(_, o, _, _, _)| {
            let summary_file = SummaryOutput::path(&o.dir);
            if summary_file.exists() {
                std::fs::remove_file(summary_file).map_err(Into::into)
            } else {
                Ok(())
            }
        })
    }

    /// Sanitizes valgrind output files for all tools.
    ///
    /// # Errors
    ///
    /// Returns an error if any sanitization step fails.
    fn sanitize(&self) -> Result<()> {
        self.analyzers()
            .iter()
            .try_for_each(|(_, o, _, _, _)| o.sanitize())
    }

    /// Rotates existing output files.
    ///
    /// This includes all tool-specific files such as log, xtree, and xleak outputs when present.
    ///
    /// # Errors
    ///
    /// Returns an error if shifting any output path fails.
    fn shift(&self) -> Result<()> {
        for (_, output_path, _, _, _) in self.analyzers() {
            output_path.shift()?;
            if matches!(
                output_path.kind,
                ToolOutputPathKind::Out | ToolOutputPathKind::BaseOut(_)
            ) {
                output_path.to_log_output().shift()?;
            }
            if let Some(path) = output_path.to_xtree_output() {
                path.shift()?;
            }
            if let Some(path) = output_path.to_xleak_output() {
                path.shift()?;
            }
        }

        Ok(())
    }
}

impl Assistant {
    /// Returns whether this assistant is a setup or teardown [`AssistantKind`]
    pub fn kind(&self) -> AssistantKind {
        self.kind
    }

    /// Returns whether this assistant is configured to run in parallel.
    ///
    /// Assistants that pipe setup output to benchmark input are treated as parallel even if
    /// [`Self::is_parallel`] is not explicitly set.
    pub fn is_parallel(&self) -> bool {
        self.pipe.is_some() || self.run_parallel
    }

    /// The setup or teardown of the `main` macro
    pub fn new_main_assistant(
        kind: AssistantKind,
        envs: Vec<(OsString, OsString)>,
        run_parallel: bool,
    ) -> Self {
        Self {
            kind,
            group_index: None,
            indices: None,
            pipe: None,
            envs,
            run_parallel,
        }
    }

    /// The setup or teardown of a `binary_benchmark_group` or `library_benchmark_group`
    pub fn new_group_assistant(
        kind: AssistantKind,
        group_index: usize,
        envs: Vec<(OsString, OsString)>,
        run_parallel: bool,
    ) -> Self {
        Self {
            kind,
            group_index: Some(group_index),
            indices: None,
            pipe: None,
            envs,
            run_parallel,
        }
    }

    /// The setup or teardown function of a `Bench`
    ///
    /// This is currently only used by binary benchmarks. Library benchmarks use a completely
    /// different logic for setup and teardown functions specified in a `#[bench]`, `#[benches]` and
    /// `#[library_benchmark]` and don't need to be executed via the compiled benchmark.
    pub fn new_bench_assistant(
        kind: AssistantKind,
        group_index: usize,
        indices: (usize, usize, Option<usize>),
        pipe: Option<Pipe>,
        envs: Vec<(OsString, OsString)>,
        run_parallel: bool,
    ) -> Self {
        Self {
            kind,
            group_index: Some(group_index),
            indices: Some(indices),
            pipe,
            envs,
            run_parallel,
        }
    }

    /// The arguments for the benchmark executable
    fn executable_args(&self) -> Vec<OsString> {
        let mut args = vec![OsString::from("--gungraun-run")];

        // The index of the binary or `library_benchmark_group!` in the main! macro
        if let Some(main_index) = &self.group_index {
            args.push(main_index.to_string().into());
        }

        args.push(self.kind.id().into());

        // The `group_index` here is the index in the binary or `library_benchmark_group!`
        if let Some((group_index, bench_index, iter_index)) = &self.indices {
            args.extend([
                group_index.to_string().into(),
                bench_index.to_string().into(),
            ]);
            if let Some(iter_index) = iter_index {
                args.push(iter_index.to_string().into());
            }
        }

        args
    }

    /// Run the `Assistant` by calling the benchmark binary with the needed arguments
    ///
    /// We don't run the assistant if `--load-baseline` was given on the command-line!
    pub fn run(
        &self,
        config: &Config,
        module_path: &ModulePath,
        captured_output: Option<&CapturedOutput>,
        force_parallel: bool,
        current_dir: Option<&Path>,
        nocapture: NoCapture,
    ) -> Result<Option<Child>> {
        if config.meta.args.load_baseline.is_some() {
            return Ok(None);
        }

        let id = self.kind.id();

        let mut command = Command::new(&config.bench_bin);
        command.envs(self.envs.iter().cloned());
        command.args(self.executable_args());

        if let Some(current_dir) = current_dir {
            command.current_dir(current_dir);
        }

        nocapture.apply(&mut command, captured_output)?;

        match &self.pipe {
            Some(Pipe::Stdout) => {
                command.stdout(StdStdio::piped());
            }
            Some(Pipe::Stderr) => {
                command.stderr(StdStdio::piped());
            }
            _ => {}
        }

        if force_parallel || self.is_parallel() {
            debug!("Spawning assistant '{}' in parallel", self.kind.id());
            let child = command
                .spawn()
                .map_err(|error| Error::LaunchError(config.bench_bin.clone(), error.to_string()))?;
            return Ok(Some(child));
        }

        debug!("Running assistant '{}' serially", self.kind.id());
        command
            .output()
            .map_err(|error| Error::LaunchError(config.bench_bin.clone(), error.to_string()))
            .and_then(|output| {
                if output.status.success() {
                    Ok(output)
                } else {
                    Err(Error::new_process_error(
                        module_path.join(&id).to_string(),
                        output,
                        None,
                    ))
                }
            })?;

        Ok(None)
    }
}

impl AssistantKind {
    /// Returns the assistant kind `id` as string.
    pub fn id(&self) -> String {
        match self {
            Self::Setup => "setup",
            Self::Teardown => "teardown",
        }
        .to_owned()
    }
}

impl BenchmarkDataProcessor for BaselineDataProcessor {
    fn finalize(
        &mut self,
        benchmark_summary: &mut BenchmarkSummary,
        config: &Config,
        header: &Header,
    ) -> Result<()> {
        if !self.has_benchmarks() {
            return Ok(());
        }

        self.create_benchmark_directory()
            .and_then(|()| self.remove_summary())
            .and_then(|()| self.shift())
            .and_then(|()| self.sanitize())
            .and_then(|()| self.parse(benchmark_summary, config, header, None))
            .and_then(|()| self.copy_temp())
    }

    fn has_benchmarks(&self) -> bool {
        !self.analyzers.is_empty()
    }

    fn generate_flamegraphs(
        &self,
        config: &Config,
        header: &Header,
        output_path: &ToolOutputPath,
        flamegraph_config: &ToolFlamegraphConfig,
        entry_point: &EntryPoint,
    ) -> Result<Vec<FlamegraphSummary>> {
        if output_path.tool == ValgrindTool::Callgrind {
            if let ToolFlamegraphConfig::Callgrind(flamegraph_config) = &flamegraph_config {
                return BaselineFlamegraphGenerator {
                    baseline_kind: output_path.baseline_kind.clone(),
                }
                .create(
                    &Flamegraph::new(header.to_title(), flamegraph_config.to_owned()),
                    output_path,
                    (*entry_point == EntryPoint::Default)
                        .then(Sentinel::default)
                        .as_ref(),
                    &config.meta.project_root,
                );
            }
        }

        Ok(vec![])
    }

    fn analyzers(&self) -> &[Analyzer] {
        &self.analyzers
    }
}

impl Benches {
    /// Returns the number of benchmarks stored in this container.
    pub fn len(&self) -> usize {
        match self {
            Self::LibBenches(lib_benches) => lib_benches.len(),
            Self::BinBenches(bin_benches) => bin_benches.len(),
        }
    }

    /// Appends a library benchmark when this container holds library benchmarks.
    pub fn push_lib_bench(&mut self, lib_bench: LibBench) {
        match self {
            Self::LibBenches(lib_benches) => lib_benches.push(lib_bench),
            Self::BinBenches(_) => {}
        }
    }

    /// Appends a binary benchmark when this container holds binary benchmarks.
    pub fn push_bin_bench(&mut self, bin_bench: BinBench) {
        match self {
            Self::LibBenches(_) => {}
            Self::BinBenches(bin_benches) => bin_benches.push(bin_bench),
        }
    }

    /// Returns `true` if this container has no benchmarks.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::LibBenches(lib_benches) => lib_benches.is_empty(),
            Self::BinBenches(bin_benches) => bin_benches.is_empty(),
        }
    }
}

impl BenchmarkSummaries {
    /// Add a [`BenchmarkSummary`]
    pub fn add_summary(&mut self, summary: BenchmarkSummary) {
        self.summaries.push(summary);
    }

    /// Add another `BenchmarkSummary`
    ///
    /// Ignores the execution time.
    pub fn add_other(&mut self, other: Self) {
        other.summaries.into_iter().for_each(|s| {
            self.add_summary(s);
        });
    }

    /// Returns `true` if any regressions were encountered.
    pub fn is_regressed(&self) -> bool {
        self.summaries.iter().any(BenchmarkSummary::is_regressed)
    }

    /// Set the total execution from `start` to `now`
    pub fn elapsed(&mut self, start: Instant) {
        self.total_time = Some(start.elapsed());
    }

    /// Returns the number of total benchmarks.
    pub fn num_benchmarks(&self) -> usize {
        self.summaries.len()
    }

    /// Print the summary if not prevented by command-line arguments
    ///
    /// If `nosummary` is true or [`OutputFormatKind`] is any kind of `JSON` format the summary is
    /// not printed.
    pub fn print(&self, nosummary: bool, output_format_kind: OutputFormatKind) {
        if !nosummary {
            SummaryFormatter::new(output_format_kind).print(self);
        }
    }
}

impl Group {
    /// Prints all benchmarks in this group for `--list` and returns their count.
    pub fn list(self) -> u64 {
        let mut sum = 0u64;
        match self.benches {
            Benches::LibBenches(lib_benches) => {
                for bench in lib_benches {
                    sum += 1;
                    format::print_list_benchmark(&bench.module_path, bench.id.as_ref());
                }
            }
            Benches::BinBenches(bin_benches) => {
                for bench in bin_benches {
                    sum += 1;
                    format::print_list_benchmark(&bench.module_path, bench.id.as_ref());
                }
            }
        }

        sum
    }

    fn start_bin_benches(
        config: &Arc<Config>,
        module_path: &Arc<ModulePath>,
        thread_pool: &mut ThreadPool<Result<JobResult>>,
        benches: Vec<BinBench>,
    ) {
        for bench in benches {
            let fail_fast = bench.is_fail_fast();
            let benchmark: Arc<dyn bin_bench::Benchmark> = bin_bench::benchmark_factory(config);
            let header = BinaryBenchmarkHeader::new(&config.meta, &bench);

            let config = Arc::clone(config);
            let module_path = Arc::clone(module_path);

            let output_format = bench.output_format.clone();

            thread_pool.execute(move |force_shutdown| {
                let captured_output = CapturedOutput::new()?;
                let output_path =
                    benchmark.default_output_path(&bench, &config, &module_path, true)?;

                let data_processor =
                    benchmark.data_processor(&bench.tools, &config.meta.project_root, &output_path);

                match benchmark.run(
                    bench,
                    &config,
                    Some(captured_output.try_clone()?),
                    &force_shutdown,
                    output_path.clone(),
                ) {
                    Ok(benchmark_summary) => Ok(JobResult {
                        benchmark_summary,
                        fail_fast,
                        header: header.into(),
                        output_format,
                        captured_output,
                        data_processor,
                    }),
                    Err(error) => Err(anyhow::Error::from(Error::JobError(
                        Box::new(error),
                        header.into(),
                        captured_output,
                        Box::new(output_path),
                    ))),
                }
            });
        }
    }

    fn start_lib_benches(
        main_index: usize,
        config: &Arc<Config>,
        module_path: &Arc<ModulePath>,
        thread_pool: &mut ThreadPool<Result<JobResult>>,
        benches: Vec<LibBench>,
    ) {
        for bench in benches {
            let fail_fast = bench.is_fail_fast();
            let benchmark: Arc<dyn lib_bench::Benchmark> = lib_bench::benchmark_factory(config);
            let header = LibraryBenchmarkHeader::new(&bench);

            let config = Arc::clone(config);
            let module_path = Arc::clone(module_path);

            let output_format = bench.output_format.clone();

            thread_pool.execute(move |force_shutdown| {
                let captured_output = CapturedOutput::new()?;
                let output_path =
                    benchmark.default_output_path(&bench, &config, &module_path, true)?;

                let data_processor =
                    benchmark.data_processor(&bench.tools, &config.meta.project_root, &output_path);

                match benchmark.run(
                    bench,
                    &config,
                    main_index,
                    Some(captured_output.try_clone()?),
                    &force_shutdown,
                    output_path.clone(),
                ) {
                    Ok(benchmark_summary) => Ok(JobResult {
                        benchmark_summary,
                        fail_fast,
                        header: header.into(),
                        output_format,
                        captured_output,
                        data_processor,
                    }),
                    Err(error) => Err(anyhow::Error::from(Error::JobError(
                        Box::new(error),
                        header.into(),
                        captured_output,
                        Box::new(output_path),
                    ))),
                }
            });
        }
    }

    /// Runs all benchmarks in this group and returns the [`BenchmarkSummaries`].
    ///
    /// Benchmarks are executed in a thread pool, finalized through their data processors, and
    /// optionally compared by id when configured.
    ///
    /// # Errors
    ///
    /// Returns an error if benchmark execution, output finalization, printing, or regression checks
    /// fail.
    pub fn run(self, config: &Arc<Config>) -> Result<BenchmarkSummaries> {
        let mut benchmark_summaries = BenchmarkSummaries::default();

        let compare_by_id = self.compare_by_id;
        let num_threads = match self.max_parallel {
            MaxParallel::NoMaximum => config.meta.args.parallel,
            MaxParallel::Serial => 1,
            MaxParallel::Count(num) => config.meta.args.parallel.min(num),
        };
        let num_benches = self.benches.len();
        let module_path = Arc::new(self.module_path.clone());
        let main_index = self.index;

        let mut thread_pool = ThreadPool::<Result<JobResult>>::new(num_threads)?;

        match self.benches {
            Benches::LibBenches(lib_benches) => {
                Self::start_lib_benches(
                    main_index,
                    config,
                    &module_path,
                    &mut thread_pool,
                    lib_benches,
                );
            }
            Benches::BinBenches(bin_benches) => {
                Self::start_bin_benches(config, &module_path, &mut thread_pool, bin_benches);
            }
        }

        let mut comparison_summaries: HashMap<String, Vec<BenchmarkSummary>> =
            HashMap::with_capacity(num_benches);
        let force_shutdown = thread_pool.clone_force_shutdown();
        let mut error = None;
        for result in thread_pool {
            let JobResult {
                mut benchmark_summary,
                fail_fast,
                header,
                output_format,
                captured_output,
                mut data_processor,
            } = match result {
                // On error, wait for all threads to finish and/or shutdown their running processes
                // and don't rely on the thread pool drop to finish before the runner exits.
                _ if error.is_some() => {
                    continue;
                }
                Ok(result) => result,
                Err(e) => {
                    force_shutdown.store(true, atomic::Ordering::Release);
                    if let Some(Error::JobError(_, _, _, path)) = e.downcast_ref::<Error>() {
                        // Avoid an error within the error situation. The worst that can happen is
                        // that we silently fail here if the log files won't be available in the
                        // benchmark directory.
                        let _ = path
                            .init()
                            .and_then(|()| path.clear_temp_files(true))
                            .and_then(|()| path.copy_temp());
                    }
                    error = Some(e);
                    continue;
                }
            };

            if !data_processor.has_benchmarks() {
                continue;
            }

            if let Err(e) = data_processor
                .finalize(&mut benchmark_summary, config, &header)
                .and_then(|()| {
                    benchmark_summary.print_and_save(
                        config,
                        &header,
                        &output_format,
                        captured_output,
                    )
                })
                .and_then(|()| benchmark_summary.check_regression(fail_fast))
            {
                force_shutdown.store(true, atomic::Ordering::Release);
                error = Some(e);
                continue;
            }

            benchmark_summaries.add_summary(benchmark_summary.clone());
            if compare_by_id && output_format.is_default() {
                if let Some(id) = &benchmark_summary.id {
                    if let Some(sums) = comparison_summaries.get_mut(id) {
                        for sum in sums.iter() {
                            sum.compare_and_print(id, &benchmark_summary, &output_format);
                        }
                        sums.push(benchmark_summary);
                    } else {
                        comparison_summaries.insert(id.clone(), vec![benchmark_summary]);
                    }
                }
            }
        }

        // At this point the thread pool iterator either processed all jobs or aborted them on
        // error, so the threads are idle. On return, whether successful or not, the thread pool
        // will be dropped and all threads are properly joined and closed.
        if let Some(error) = error {
            return Err(error);
        }

        Ok(benchmark_summaries)
    }
}

impl BenchmarkDataProcessor for LoadBaselineDataProcessor {
    fn finalize(
        &mut self,
        benchmark_summary: &mut BenchmarkSummary,
        config: &Config,
        header: &Header,
    ) -> Result<()> {
        self.parse(benchmark_summary, config, header, None)
    }

    fn has_benchmarks(&self) -> bool {
        !self.analyzers.is_empty()
    }

    fn analyzers(&self) -> &[Analyzer] {
        &self.analyzers
    }

    fn generate_flamegraphs(
        &self,
        config: &Config,
        header: &Header,
        output_path: &ToolOutputPath,
        flamegraph_config: &ToolFlamegraphConfig,
        entry_point: &EntryPoint,
    ) -> Result<Vec<FlamegraphSummary>> {
        if output_path.tool == ValgrindTool::Callgrind {
            if let ToolFlamegraphConfig::Callgrind(flamegraph_config) = &flamegraph_config {
                let loaded_baseline = output_path.loaded_baseline_name().expect(
                    "The loaded baseline of an output path of a loaded baseline should have a name",
                );
                let baseline = output_path.baseline_name().cloned().expect(
                    "The baseline of an output path of a loaded baseline should have a name",
                );

                return LoadBaselineFlamegraphGenerator {
                    baseline,
                    loaded_baseline,
                }
                .create(
                    &Flamegraph::new(header.to_title(), flamegraph_config.to_owned()),
                    output_path,
                    (*entry_point == EntryPoint::Default)
                        .then(Sentinel::default)
                        .as_ref(),
                    &config.meta.project_root,
                );
            }
        }

        Ok(vec![])
    }
}

impl Groups {
    /// Builds benchmark groups from binary benchmark metadata.
    ///
    /// The resulting groups include expanded benchmark entries, setup/teardown assistants, and
    /// applied configuration inheritance.
    ///
    /// # Errors
    ///
    /// Returns an error if any benchmark entry cannot be configured.
    #[allow(clippy::too_many_lines)]
    pub fn from_binary_benchmark(
        module_path: &ModulePath,
        benchmark_groups: BinaryBenchmarkGroups,
        meta: &Metadata,
    ) -> Result<Self> {
        let global_config = benchmark_groups.config;
        let default_tool = benchmark_groups.default_tool;

        let mut groups = vec![];
        for (main_index, binary_benchmark_group) in benchmark_groups.groups.into_iter().enumerate()
        {
            let group_module_path = module_path.join(&binary_benchmark_group.id);
            let group_config = global_config
                .clone()
                .update_from_all([binary_benchmark_group.config.as_ref()]);

            let setup = binary_benchmark_group
                .has_setup
                .then_some(Assistant::new_group_assistant(
                    AssistantKind::Setup,
                    main_index,
                    group_config.collect_envs(),
                    false,
                ));
            let teardown =
                binary_benchmark_group
                    .has_teardown
                    .then_some(Assistant::new_group_assistant(
                        AssistantKind::Teardown,
                        main_index,
                        group_config.collect_envs(),
                        false,
                    ));

            let mut group = Group {
                index: main_index,
                benches: Benches::BinBenches(vec![]),
                compare_by_id: binary_benchmark_group
                    .compare_by_id
                    .unwrap_or(defaults::COMPARE_BY_ID),
                max_parallel: binary_benchmark_group.max_parallel.into(),
                module_path: group_module_path,
                num_filtered: 0,
                setup,
                teardown,
            };

            for (group_index, binary_benchmark_benches) in binary_benchmark_group
                .binary_benchmarks
                .into_iter()
                .enumerate()
            {
                for (bench_index, binary_benchmark_bench) in
                    binary_benchmark_benches.benches.into_iter().enumerate()
                {
                    let module_path = group
                        .module_path
                        .join(&binary_benchmark_bench.function_name);

                    match &binary_benchmark_bench.command {
                        api::CommandKind::Default(command) => {
                            let config = group_config.clone().update_from_all([
                                binary_benchmark_benches.config.as_ref(),
                                binary_benchmark_bench.config.as_ref(),
                                Some(&command.config),
                            ]);

                            let bin_bench = BinBench::new(
                                binary_benchmark_bench.id,
                                binary_benchmark_bench.args,
                                binary_benchmark_bench.consts_display,
                                module_path,
                                binary_benchmark_bench.function_name,
                                binary_benchmark_bench.has_setup,
                                binary_benchmark_bench.has_teardown,
                                meta,
                                main_index,
                                config,
                                group_index,
                                bench_index,
                                None,
                                *command.clone(),
                                default_tool,
                            )?;
                            if let Some(bin_bench) = bin_bench {
                                group.benches.push_bin_bench(bin_bench);
                            } else {
                                group.num_filtered += 1;
                            }
                        }
                        api::CommandKind::Iter(commands) => {
                            match (commands.len(), &binary_benchmark_bench.id) {
                                (0, Some(id)) => {
                                    warn!(
                                        "The iterator of {module_path} with id '{id}' was empty."
                                    );
                                }
                                (0, None) => {
                                    warn!("The iterator of {module_path} was empty.");
                                }
                                _ => {
                                    for (iter_index, command) in commands.iter().enumerate() {
                                        let config = group_config.clone().update_from_all([
                                            binary_benchmark_benches.config.as_ref(),
                                            binary_benchmark_bench.config.as_ref(),
                                            Some(&command.config),
                                        ]);

                                        let bin_bench = BinBench::new(
                                            binary_benchmark_bench.id.clone(),
                                            binary_benchmark_bench.args.clone(),
                                            binary_benchmark_bench.consts_display.clone(),
                                            module_path.clone(),
                                            binary_benchmark_bench.function_name.clone(),
                                            binary_benchmark_bench.has_setup,
                                            binary_benchmark_bench.has_teardown,
                                            meta,
                                            main_index,
                                            config,
                                            group_index,
                                            bench_index,
                                            Some(iter_index),
                                            command.clone(),
                                            default_tool,
                                        )?;
                                        if let Some(bin_bench) = bin_bench {
                                            group.benches.push_bin_bench(bin_bench);
                                        } else {
                                            group.num_filtered += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            groups.push(group);
        }
        Ok(Self(groups))
    }

    /// Builds benchmark groups from library benchmark metadata.
    ///
    /// The resulting groups include expanded benchmark entries for iterator benchmarks,
    /// setup/teardown assistants, and applied configuration inheritance.
    ///
    /// # Errors
    ///
    /// Returns an error if any benchmark entry cannot be configured.
    #[allow(clippy::too_many_lines)]
    pub fn from_library_benchmark(
        module_path: &ModulePath,
        benchmark_groups: LibraryBenchmarkGroups,
        meta: &Metadata,
    ) -> Result<Self> {
        let global_config = benchmark_groups.config;
        let default_tool = benchmark_groups.default_tool;

        let mut groups = vec![];
        for (main_index, library_benchmark_group) in benchmark_groups.groups.into_iter().enumerate()
        {
            let group_module_path = module_path.join(&library_benchmark_group.id);
            let group_config = global_config
                .clone()
                .update_from_all([library_benchmark_group.config.as_ref()]);

            let setup =
                library_benchmark_group
                    .has_setup
                    .then_some(Assistant::new_group_assistant(
                        AssistantKind::Setup,
                        main_index,
                        group_config.collect_envs(),
                        false,
                    ));
            let teardown =
                library_benchmark_group
                    .has_teardown
                    .then_some(Assistant::new_group_assistant(
                        AssistantKind::Teardown,
                        main_index,
                        group_config.collect_envs(),
                        false,
                    ));

            let mut group = Group {
                index: main_index,
                benches: Benches::LibBenches(vec![]),
                compare_by_id: library_benchmark_group
                    .compare_by_id
                    .unwrap_or(defaults::COMPARE_BY_ID),
                max_parallel: library_benchmark_group.max_parallel.into(),
                module_path: group_module_path,
                num_filtered: 0,
                setup,
                teardown,
            };

            for (group_index, library_benchmark_benches) in library_benchmark_group
                .library_benchmarks
                .into_iter()
                .enumerate()
            {
                for (bench_index, library_benchmark_bench) in
                    library_benchmark_benches.benches.into_iter().enumerate()
                {
                    let config = group_config.clone().update_from_all([
                        library_benchmark_benches.config.as_ref(),
                        library_benchmark_bench.config.as_ref(),
                    ]);

                    let module_path = group
                        .module_path
                        .join(&library_benchmark_bench.function_name);

                    if let Some(iter_count) = library_benchmark_bench.iter_count {
                        match (iter_count, &library_benchmark_bench.id) {
                            (0, Some(id)) => {
                                warn!("The iterator of {module_path} with id '{id}' was empty.");
                            }
                            (0, None) => {
                                warn!("The iterator of {module_path} was empty.");
                            }
                            _ => {
                                for iter_index in 0..iter_count {
                                    let lib_bench = LibBench::new(
                                        library_benchmark_bench.id.clone(),
                                        library_benchmark_bench.args.clone(),
                                        library_benchmark_bench.consts_display.clone(),
                                        module_path.clone(),
                                        library_benchmark_bench.function_name.clone(),
                                        meta,
                                        config.clone(),
                                        group_index,
                                        bench_index,
                                        Some(iter_index),
                                        default_tool,
                                    )?;
                                    if let Some(lib_bench) = lib_bench {
                                        group.benches.push_lib_bench(lib_bench);
                                    } else {
                                        group.num_filtered += 1;
                                    }
                                }
                            }
                        }
                    } else {
                        let lib_bench = LibBench::new(
                            library_benchmark_bench.id,
                            library_benchmark_bench.args,
                            library_benchmark_bench.consts_display,
                            module_path,
                            library_benchmark_bench.function_name,
                            meta,
                            config,
                            group_index,
                            bench_index,
                            None,
                            default_tool,
                        )?;
                        if let Some(lib_bench) = lib_bench {
                            group.benches.push_lib_bench(lib_bench);
                        } else {
                            group.num_filtered += 1;
                        }
                    }
                }
            }

            groups.push(group);
        }

        Ok(Self(groups))
    }

    /// Returns whether at least one group contains at least one benchmark.
    pub fn has_benchmarks(&self) -> bool {
        self.0.iter().any(|g| !g.benches.is_empty())
    }

    /// Prints all groups in list format and a final benchmark summary.
    pub fn list(self) {
        let mut sum = 0u64;
        for group in self.0 {
            sum += group.list();
        }

        format::print_benchmark_list_summary(sum);
    }

    /// Returns the total number of filtered benchmarks across all groups.
    pub fn num_filtered(&self) -> usize {
        self.0.iter().fold(0, |acc, group| acc + group.num_filtered)
    }

    /// Runs all groups in order, including per-group setup and teardown assistants.
    ///
    /// # Errors
    ///
    /// Returns an error if any assistant or group run fails.
    pub fn run(self, config: &Arc<Config>) -> Result<BenchmarkSummaries> {
        let mut benchmark_summaries = BenchmarkSummaries::default();

        for group in self.0 {
            if let Some(setup) = &group.setup {
                setup.run(
                    config,
                    &group.module_path,
                    None,
                    false,
                    None,
                    config.meta.args.nocapture,
                )?;
            }

            let module_path = group.module_path.clone();
            let teardown = group.teardown.clone();

            let summaries = group.run(config)?;

            if let Some(teardown) = teardown {
                teardown.run(
                    config,
                    &module_path,
                    None,
                    false,
                    None,
                    config.meta.args.nocapture,
                )?;
            }

            benchmark_summaries.add_other(summaries);
        }

        Ok(benchmark_summaries)
    }
}

impl From<Option<usize>> for MaxParallel {
    fn from(value: Option<usize>) -> Self {
        match value {
            None | Some(0) => Self::NoMaximum,
            Some(1) => Self::Serial,
            Some(num) => Self::Count(num),
        }
    }
}

impl ModulePath {
    /// Creates a new `ModulePath`.
    ///
    /// There is no validity check if the path contains valid characters or not and the path is
    /// created as is.
    pub fn new(path: &str) -> Self {
        Self(path.to_owned())
    }

    /// Join this module path with another string (unchecked)
    #[must_use]
    pub fn join(&self, path: &str) -> Self {
        let new = format!("{}::{path}", self.0);
        Self(new)
    }

    /// Returns the module path as string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the first segment of the module path if any.
    pub fn first(&self) -> Option<Self> {
        self.0
            .split_once("::")
            .map(|(first, _)| Self::new(first))
            .or_else(|| (!self.0.is_empty()).then_some(self.clone()))
    }

    /// Returns the last segment of the module path if any.
    pub fn last(&self) -> Option<Self> {
        self.0.rsplit_once("::").map(|(_, last)| Self::new(last))
    }

    /// Returns the parent module path if present.
    pub fn parent(&self) -> Option<Self> {
        self.0
            .rsplit_once("::")
            .map(|(prefix, _)| Self::new(prefix))
    }

    /// Returns a vector which contains all segments of the module path without the delimiter.
    pub fn components(&self) -> Vec<&str> {
        self.0.split("::").collect()
    }
}

impl Display for ModulePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Runner {
    /// Returns whether this runner has at least one benchmark to execute.
    pub fn has_benchmarks(&self) -> bool {
        self.groups.has_benchmarks()
    }

    /// Returns the number of benchmarks filtered out before execution.
    pub fn num_filtered(&self) -> usize {
        self.groups.num_filtered()
    }

    /// Creates a runner from library benchmark metadata and runtime configuration.
    ///
    /// This wires top-level setup and teardown assistants and builds benchmark groups.
    ///
    /// # Errors
    ///
    /// Returns an error if group construction fails.
    pub fn from_library_benchmark(
        benchmark_groups: LibraryBenchmarkGroups,
        config: Config,
    ) -> Result<Self> {
        let setup = benchmark_groups
            .has_setup
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Setup,
                benchmark_groups.config.collect_envs(),
                false,
            ));
        let teardown = benchmark_groups
            .has_teardown
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Teardown,
                benchmark_groups.config.collect_envs(),
                false,
            ));

        let groups =
            Groups::from_library_benchmark(&config.module_path, benchmark_groups, &config.meta)?;

        Ok(Self {
            config: Arc::new(config),
            groups,
            setup,
            teardown,
        })
    }

    /// Creates a runner from binary benchmark metadata and runtime configuration.
    ///
    /// This wires top-level setup and teardown assistants and builds benchmark groups.
    ///
    /// # Errors
    ///
    /// Returns an error if group construction fails.
    pub fn from_binary_benchmark(
        benchmark_groups: BinaryBenchmarkGroups,
        config: Config,
    ) -> Result<Self> {
        let setup = benchmark_groups
            .has_setup
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Setup,
                benchmark_groups.config.collect_envs(),
                false,
            ));
        let teardown = benchmark_groups
            .has_teardown
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Teardown,
                benchmark_groups.config.collect_envs(),
                false,
            ));

        let groups =
            Groups::from_binary_benchmark(&config.module_path, benchmark_groups, &config.meta)?;

        Ok(Self {
            config: Arc::new(config),
            groups,
            setup,
            teardown,
        })
    }

    /// Run all benchmarks in all groups
    pub fn run(self) -> Result<BenchmarkSummaries> {
        let num_filtered = self.num_filtered();

        if self.has_benchmarks() {
            let start = Instant::now();

            if let Some(setup) = &self.setup {
                setup.run(
                    &self.config,
                    &self.config.module_path,
                    None,
                    false,
                    None,
                    self.config.meta.args.nocapture,
                )?;
            }

            let mut summaries = self.groups.run(&self.config)?;

            if let Some(teardown) = &self.teardown {
                teardown.run(
                    &self.config,
                    &self.config.module_path,
                    None,
                    false,
                    None,
                    self.config.meta.args.nocapture,
                )?;
            }

            summaries.elapsed(start);
            summaries.num_filtered = num_filtered;
            Ok(summaries)
        } else {
            Ok(BenchmarkSummaries {
                num_filtered,
                ..Default::default()
            })
        }
    }
}

impl Sandbox {
    /// Setup the `Sandbox` if enabled
    ///
    /// If enabled, create a temporary directory which has a standardized length. Then copy fixtures
    /// into the temporary directory.
    pub fn setup(inner: &api::Sandbox, meta: &Metadata) -> Result<Self> {
        let enabled = inner.enabled.unwrap_or(defaults::SANDBOX_ENABLED);
        let follow_symlinks = inner
            .follow_symlinks
            .unwrap_or(defaults::SANDBOX_FIXTURES_FOLLOW_SYMLINKS);

        let current_dir = std::env::current_dir().map_err(|error| {
            Error::SandboxError(format!("Failed to detect current directory: {error}"))
        })?;

        let temp_dir = if enabled {
            debug!("Creating sandbox");

            let temp_dir = tempfile::tempdir().map_err(|error| {
                Error::SandboxError(format!("Failed creating temporary directory: {error}"))
            })?;

            for fixture in &inner.fixtures {
                if fixture.is_relative() {
                    let absolute_path = make_absolute(&meta.project_root, fixture);
                    copy_directory(&absolute_path, temp_dir.path(), follow_symlinks)?;
                } else {
                    copy_directory(fixture, temp_dir.path(), follow_symlinks)?;
                }
            }

            Some(temp_dir)
        } else {
            debug!(
                "Sandbox disabled: Running benchmarks in current directory '{}'",
                current_dir.display()
            );
            None
        };

        Ok(Self { temp_dir })
    }

    /// Delete the temporary directory if present
    pub fn reset(self) -> Result<()> {
        if let Some(temp_dir) = self.temp_dir {
            if log_enabled!(Level::Debug) {
                debug!("Removing temporary workspace");
                if let Err(error) = temp_dir.close() {
                    debug!("Error trying to delete temporary workspace: {error}");
                }
            } else {
                _ = temp_dir.close();
            }
        }

        Ok(())
    }

    /// Returns the sandbox path when sandboxing is enabled.
    pub fn path(&self) -> Option<&Path> {
        self.temp_dir.as_ref().map(tempfile::TempDir::path)
    }
}

impl BenchmarkDataProcessor for SaveBaselineDataProcessor {
    fn finalize(
        &mut self,
        benchmark_summary: &mut BenchmarkSummary,
        config: &Config,
        header: &Header,
    ) -> Result<()> {
        if !self.has_benchmarks() {
            return Ok(());
        }

        self.create_benchmark_directory()?;

        let parsed_old = self
            .analyzers()
            .iter()
            .map(|(parser, _, _, _, _)| parser.parse_base())
            .collect::<Result<Vec<Vec<ParserOutput>>>>()?;

        self.remove_summary()
            .and_then(|()| self.shift())
            .and_then(|()| self.move_temp())
            .and_then(|()| self.sanitize())
            .and_then(|()| self.parse(benchmark_summary, config, header, Some(parsed_old)))
            .and_then(|()| self.copy_temp()) // for the flamegraphs which are created in the
                                             // temporary directory
    }

    fn has_benchmarks(&self) -> bool {
        !self.analyzers.is_empty()
    }

    fn analyzers(&self) -> &[Analyzer] {
        &self.analyzers
    }

    fn generate_flamegraphs(
        &self,
        config: &Config,
        header: &Header,
        output_path: &ToolOutputPath,
        flamegraph_config: &ToolFlamegraphConfig,
        entry_point: &EntryPoint,
    ) -> Result<Vec<FlamegraphSummary>> {
        if output_path.tool == ValgrindTool::Callgrind {
            if let ToolFlamegraphConfig::Callgrind(flamegraph_config) = &flamegraph_config {
                let baseline = output_path.baseline_name().cloned().expect(
                    "The baseline of an output path of a saved baseline should have a name",
                );
                return SaveBaselineFlamegraphGenerator { baseline }.create(
                    &Flamegraph::new(header.to_title(), flamegraph_config.to_owned()),
                    output_path,
                    (*entry_point == EntryPoint::Default)
                        .then(Sentinel::default)
                        .as_ref(),
                    &config.meta.project_root,
                );
            }
        }

        Ok(vec![])
    }
}

impl CapturedOutput {
    /// Creates new temporary files for capturing stdout and stderr.
    ///
    /// # Errors
    ///
    /// Returns an error if creating temporary files fails.
    pub fn new() -> Result<Self> {
        tempfile()
            .and_then(|stdout| tempfile().map(|stderr| Self { stderr, stdout }))
            .with_context(|| "Creating captured output failed")
    }

    /// Creates a duplicate `CapturedOutput` handle backed by cloned file descriptors.
    ///
    /// # Errors
    ///
    /// Returns an error if cloning either stream handle fails.
    pub fn try_clone(&self) -> Result<Self> {
        self.stdout
            .try_clone()
            .and_then(|stdout| {
                self.stderr
                    .try_clone()
                    .map(|stderr| Self { stderr, stdout })
            })
            .with_context(|| "Cloning captured output failed")
    }

    /// Flushes and rewinds both captured output files to the beginning.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing or seeking either stream fails.
    pub fn reset(&mut self) -> Result<()> {
        self.stdout
            .flush()
            .and_then(|()| self.stdout.rewind())
            .and_then(|()| self.stderr.flush())
            .and_then(|()| self.stderr.rewind())
            .with_context(|| "Resetting captured output failed")
    }

    /// Returns cloned stdout and stderr file handles as [`std::process::Stdio`].
    ///
    /// # Errors
    ///
    /// Returns an error if stream cloning fails.
    pub fn into_stdio(&self) -> Result<(StdStdio, StdStdio)> {
        self.try_clone()
            .map(|cloned| (cloned.stdout.into(), cloned.stderr.into()))
    }

    /// Writes captured stdout and stderr to the standard stdout/stderr.
    ///
    /// # Errors
    ///
    /// Returns an error if stream reset or copying data to terminal output fails.
    pub fn dump(&mut self) -> Result<()> {
        self.reset().and_then(|()| {
            let mut stdout_lock = stdout().lock();
            let mut stderr_lock = stderr().lock();

            std::io::copy(&mut self.stdout, &mut stdout_lock)
                .and_then(|_| std::io::copy(&mut self.stderr, &mut stderr_lock))
                .map(|_| ())
                .with_context(|| "Dumping captured output failed")
        })
    }

    /// Writes captured stdout and stderr to terminal output without changing the `self` state.
    ///
    /// # Errors
    ///
    /// Returns an error if cloning, resetting, or copying stream data fails.
    pub fn dump_cloned(&self) -> Result<()> {
        let mut captured_output = self.try_clone()?;
        captured_output.reset().and_then(|()| {
            let mut stdout_lock = stdout().lock();
            let mut stderr_lock = stderr().lock();

            std::io::copy(&mut captured_output.stdout, &mut stdout_lock)
                .and_then(|_| std::io::copy(&mut captured_output.stderr, &mut stderr_lock))
                .map(|_| ())
                .with_context(|| "Dumping cloned captured output failed")
        })
    }

    /// Writes captured stdout to the standard stderr stream.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing, seeking, or copying stderr data fails.
    pub fn dump_stderr(&mut self) -> Result<()> {
        self.stderr
            .flush()
            .and_then(|()| self.stderr.rewind())
            .and_then(|()| {
                let mut stderr_lock = stderr().lock();
                std::io::copy(&mut self.stderr, &mut stderr_lock).map(|_| ())
            })
            .with_context(|| "Dumping stderr failed")
    }

    /// Writes captured stdout to the standard stdout stream.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing, seeking, or copying stdout data fails.
    pub fn dump_stdout(&mut self) -> Result<()> {
        self.stdout
            .flush()
            .and_then(|()| self.stdout.rewind())
            .and_then(|()| {
                let mut stdout_lock = stdout().lock();
                std::io::copy(&mut self.stdout, &mut stdout_lock).map(|_| ())
            })
            .with_context(|| "Dumping stdout failed")
    }
}

impl From<ModulePath> for String {
    fn from(value: ModulePath) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty("", None)]
    #[case::single("first", Some("first"))]
    #[case::two("first::second", Some("first"))]
    #[case::three("first::second::third", Some("first"))]
    fn test_module_path_first(#[case] module_path: &str, #[case] expected: Option<&str>) {
        let expected = expected.map(ModulePath::new);
        let actual = ModulePath::new(module_path).first();

        assert_eq!(actual, expected);
    }
}
