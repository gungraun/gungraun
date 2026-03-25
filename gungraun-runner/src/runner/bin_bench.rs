//! The module responsible for running a binary benchmark

mod defaults {
    use crate::api::Stdin;

    pub const ENV_CLEAR: bool = true;
    pub const STDIN: Stdin = Stdin::Pipe;
    pub const WORKSPACE_ROOT_ENV: &str = "_WORKSPACE_ROOT";
}

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Debug;
use std::io::ErrorKind::WouldBlock;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;
use std::{panic, thread};

use anyhow::{anyhow, Context, Result};
use log::{debug, warn};

use super::format::{BinaryBenchmarkHeader, OutputFormat};
use super::meta::Metadata;
use super::summary::{BaselineKind, BaselineName, BenchmarkKind, BenchmarkSummary, SummaryOutput};
use super::tool::config::ToolConfigs;
use super::tool::path::{ToolOutputPath, ToolOutputPathKind};
use super::tool::run::RunOptions;
use crate::api::{
    self, BinaryBenchmarkConfig, BinaryBenchmarkGroups, DelayKind, EntryPoint, Stdin, ValgrindTool,
};
use crate::error::Error;
use crate::runner::common::{
    Assistant, AssistantKind, BaselineDataProcessor, Baselines, BenchmarkDataProcessor,
    BenchmarkSummaries, CapturedOutput, Config, Groups, LoadBaselineDataProcessor, ModulePath,
    Runner, SaveBaselineDataProcessor,
};

#[derive(Debug)]
struct BaselineBenchmark {
    baseline_kind: BaselineKind,
}

/// A `BinBench` represents a single benchmark under the `#[binary_benchmark]` macro
#[derive(Debug, Clone)]
pub struct BinBench {
    /// The [`Command`] to execute under valgrind
    pub command: Command,
    /// The arguments of `consts` parameter as a single string
    pub consts_display: Option<String>,
    /// The default [`ValgrindTool`]. If not changed it is `Callgrind`.
    pub default_tool: ValgrindTool,
    /// The arguments of `args` parameter as a single string
    pub display: Option<String>,
    /// The name of the annotated function
    pub function_name: String,
    /// The id of the benchmark as in `#[bench::id]`
    pub id: Option<String>,
    /// The [`ModulePath`].
    pub module_path: ModulePath,
    /// The [`OutputFormat`]
    pub output_format: OutputFormat,
    /// The [`RunOptions`]
    pub run_options: RunOptions,
    /// The tool configurations for this benchmark run
    pub tools: ToolConfigs,
}

/// The Command derived from the `api::Command`
///
/// If the path is relative we convert it to an absolute path relative to the workspace root.
/// `stdin`, `stdout`, `stderr` of the `api::Command` are part of the `RunOptions` and not part of
/// this `Command`
#[derive(Debug, Clone)]
pub struct Command {
    /// The arguments to pass to the executable
    pub args: Vec<OsString>,
    /// The path to the executable
    pub path: PathBuf,
}

/// The `Delay` which should be applied to the [`Command`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delay {
    /// The kind of delay
    pub kind: DelayKind,
    /// The polling time to check the delay condition
    pub poll: Duration,
    /// The timeout for the delay
    pub timeout: Duration,
}

#[derive(Debug)]
struct LoadBaselineBenchmark {
    baseline: BaselineName,
    loaded_baseline: BaselineName,
}

#[derive(Debug)]
struct SaveBaselineBenchmark {
    baseline: BaselineName,
}

/// Strategy interface for executing binary benchmarks in different baseline modes.
pub trait Benchmark: Debug + Send + Sync {
    /// Returns the pair of baseline names used for this run.
    fn baselines(&self) -> Baselines;

    /// Creates the post-run data processor for the selected tools.
    ///
    /// The processor uses [`ToolConfigs`], the benchmark `project_root`, and the computed
    /// [`ToolOutputPath`] to parse metrics, evaluate regressions, and produce additional
    /// artifacts.
    fn data_processor(
        &self,
        tools: &ToolConfigs,
        project_root: &Path,
        output_path: &ToolOutputPath,
    ) -> Box<dyn BenchmarkDataProcessor>;

    /// Computes the output location for this benchmark run for the default tool
    ///
    /// The path is derived from [`BinBench`], the global `config`, the enclosing
    /// `group_module_path`, and whether a temporary directory for the new valgrind output files
    /// should be used.
    ///
    /// # Errors
    ///
    /// Returns an error if the output path cannot be created or initialized.
    fn default_output_path(
        &self,
        bin_bench: &BinBench,
        config: &Config,
        group_module_path: &ModulePath,
        use_temp_dir: bool,
    ) -> Result<ToolOutputPath>;

    /// Executes a benchmark and returns its [`BenchmarkSummary`].
    ///
    /// The method consumes [`BinBench`], runs according to `config`, optionally uses
    /// [`CapturedOutput`] for captured output, reacts to the shared variable `force_shutdown`,
    /// and writes artifacts to the [`ToolOutputPath`].
    ///
    /// # Errors
    ///
    /// Returns an error if launching or running the benchmark fails
    fn run(
        &self,
        bin_bench: BinBench,
        config: &Config,
        captured_output: Option<CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary>;
}

impl Benchmark for BaselineBenchmark {
    fn default_output_path(
        &self,
        bin_bench: &BinBench,
        config: &Config,
        group_module_path: &ModulePath,
        use_temp_dir: bool,
    ) -> Result<ToolOutputPath> {
        let kind = if bin_bench.default_tool.has_output_file() {
            ToolOutputPathKind::Out
        } else {
            ToolOutputPathKind::Log
        };
        let output_path = ToolOutputPath::new(
            kind,
            bin_bench.default_tool,
            &self.baseline_kind,
            &config.meta.target_dir,
            group_module_path,
            &bin_bench.name(),
            use_temp_dir,
        )?;

        if !use_temp_dir {
            output_path.init()?;
        }

        Ok(output_path)
    }

    fn baselines(&self) -> Baselines {
        match &self.baseline_kind {
            BaselineKind::Old => (None, None),
            BaselineKind::Name(name) => (None, Some(name.to_string())),
        }
    }

    fn run(
        &self,
        bin_bench: BinBench,
        config: &Config,
        captured_output: Option<CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary> {
        let header = BinaryBenchmarkHeader::new(&config.meta, &bin_bench);
        let benchmark_summary = bin_bench.create_benchmark_summary(
            config,
            &output_path,
            &bin_bench.function_name,
            header.description(),
            self.baselines(),
        );

        bin_bench.tools.run(
            benchmark_summary,
            config,
            &bin_bench.command.path,
            &bin_bench.command.args,
            &bin_bench.run_options,
            &output_path,
            &bin_bench.module_path,
            captured_output.as_ref(),
            force_shutdown,
        )
    }

    fn data_processor(
        &self,
        tools: &ToolConfigs,
        project_root: &Path,
        output_path: &ToolOutputPath,
    ) -> Box<dyn BenchmarkDataProcessor> {
        Box::new(BaselineDataProcessor {
            analyzers: tools.analyzers(project_root, output_path),
        })
    }
}

impl BinBench {
    /// Returns whether any configured tool enables fail-fast regression handling.
    pub fn is_fail_fast(&self) -> bool {
        self.tools
            .0
            .iter()
            .any(|c| c.regression_config.is_fail_fast())
    }

    /// Creates a configured binary benchmark from API metadata.
    ///
    /// This constructor derives the final benchmark id by combining `id` with `iter_index` if
    /// present joining them with an underscore (`ID_INDEX`). It resolves effective tool and output
    /// configuration from `config` and `default_tool`, and builds setup/teardown assistants using
    /// the provided index parameters and command details.
    ///
    /// The method returns `Ok(None)` if this benchmark is filtered by the CLI arguments
    /// [`BenchmarkFilter`].
    ///
    /// # Errors
    ///
    /// Returns an error if command or tool configuration is invalid.
    ///
    /// [`BenchmarkFilter`]: crate::runner::args::BenchmarkFilter
    pub fn new(
        id: Option<String>,
        display: Option<String>,
        consts_display: Option<String>,
        module_path: ModulePath,
        function_name: String,
        has_setup: bool,
        has_teardown: bool,
        meta: &Metadata,
        main_index: usize,
        config: BinaryBenchmarkConfig,
        group_index: usize,
        bench_index: usize,
        iter_index: Option<usize>,
        command: api::Command,
        default_tool: ValgrindTool,
    ) -> Result<Option<Self>> {
        let id = if let Some(iter_index) = iter_index {
            id.as_ref().map(|id| format!("{id}_{iter_index}"))
        } else {
            id
        };

        if let Some(filter) = meta.args.filter.as_ref() {
            let is_matched = match id.as_ref() {
                Some(id) => filter.apply(module_path.join(id).as_str()),
                None => filter.apply(module_path.as_str()),
            };
            if !is_matched {
                return Ok(None);
            }
        }

        let default_tool = meta
            .args
            .default_tool
            .unwrap_or_else(|| config.default_tool.unwrap_or(default_tool));

        let api::Command {
            path,
            args,
            stdin,
            stdout,
            stderr,
            delay,
            ..
        } = command;

        let command = Command::new(&module_path, path, args).map_err(|error| {
            Error::ConfigurationError(module_path.clone(), id.clone(), error.to_string())
        })?;

        let mut assistant_envs = config.collect_envs();
        assistant_envs.push((
            OsString::from(defaults::WORKSPACE_ROOT_ENV),
            meta.project_root.clone().into(),
        ));

        let command_envs = config.resolve_envs();

        let mut output_format = config
            .output_format
            .map_or_else(OutputFormat::default, Into::into);
        output_format.kind = meta.args.output_format;

        let tool_configs = ToolConfigs::new(
            &mut output_format,
            config.tools,
            &module_path,
            id.as_ref(),
            meta,
            default_tool,
            &EntryPoint::None,
            &config.valgrind_args,
            &HashMap::default(),
        )
        .map_err(|error| {
            Error::ConfigurationError(module_path.clone(), id.clone(), error.to_string())
        })?;

        let setup = has_setup.then_some(Assistant::new_bench_assistant(
            AssistantKind::Setup,
            main_index,
            (group_index, bench_index, iter_index),
            stdin.as_ref().and_then(|s| {
                if let Stdin::Setup(p) = s {
                    Some(*p)
                } else {
                    None
                }
            }),
            assistant_envs.clone(),
            config.setup_parallel.unwrap_or(false),
        ));
        let teardown = has_teardown.then_some(Assistant::new_bench_assistant(
            AssistantKind::Teardown,
            main_index,
            (group_index, bench_index, iter_index),
            None,
            assistant_envs,
            false,
        ));

        Ok(Some(Self {
            id,
            display,
            consts_display,
            function_name,
            tools: tool_configs,
            run_options: RunOptions {
                env_clear: config.env_clear.unwrap_or(defaults::ENV_CLEAR),
                envs: command_envs,
                stdin: stdin.or(Some(defaults::STDIN)),
                stdout,
                stderr,
                exit_with: config.exit_with,
                current_dir: config.current_dir,
                setup,
                teardown,
                sandbox: config.sandbox,
                delay: delay.map(Into::into),
            },
            module_path,
            command,
            output_format,
            default_tool,
        }))
    }

    fn name(&self) -> String {
        if let Some(bench_id) = &self.id {
            format!("{}.{}", self.function_name, bench_id)
        } else {
            self.function_name.clone()
        }
    }

    fn create_benchmark_summary(
        &self,
        config: &Config,
        output_path: &ToolOutputPath,
        function_name: &str,
        description: Option<String>,
        baselines: Baselines,
    ) -> BenchmarkSummary {
        let summary_output = config
            .meta
            .args
            .save_summary
            .map(|format| SummaryOutput::new(format, &output_path.dir));

        BenchmarkSummary::new(
            BenchmarkKind::BinaryBenchmark,
            config.meta.project_root.clone(),
            config.package_dir.clone(),
            config.bench_file.clone(),
            self.command.path.clone(),
            &self.module_path,
            function_name,
            self.id.clone(),
            description,
            summary_output,
            baselines,
        )
    }
}

impl Command {
    fn new(module_path: &ModulePath, path: PathBuf, args: Vec<OsString>) -> Result<Self> {
        if path.as_os_str().is_empty() {
            return Err(anyhow!("{module_path}: Empty path in command",));
        }

        Ok(Self { args, path })
    }
}

impl Delay {
    /// Creates a new `Delay`.
    pub fn new(poll: Duration, timeout: Duration, kind: DelayKind) -> Self {
        Self {
            kind,
            poll,
            timeout,
        }
    }

    /// Apply the `Delay`
    pub fn apply(&self, current_dir: Option<&Path>) -> Result<()> {
        if let DelayKind::DurationElapse(_) = self.kind {
            self.exec(None)
        } else {
            let (tx, rx) = mpsc::channel::<std::result::Result<(), anyhow::Error>>();

            let delay = self.clone();
            let current_dir = Arc::new(current_dir.map(ToOwned::to_owned));
            let handle = thread::spawn(move || {
                tx.send(delay.exec(current_dir.as_deref()))
                    .map_err(|error| {
                        anyhow!("Command::Delay MPSC channel send error. Error: {error:?}")
                    })
            });

            match rx.recv_timeout(self.timeout) {
                Ok(result) => {
                    // These unwraps are safe
                    handle.join().unwrap().unwrap();
                    result.map(|()| debug!("Command::Delay successfully executed."))
                }
                Err(RecvTimeoutError::Timeout) => {
                    Err(anyhow!("Timeout of '{:?}' reached", self.timeout))
                }
                Err(RecvTimeoutError::Disconnected) => {
                    // The disconnect is caused by a panic in the thread, so the `unwrap_err` is
                    // safe. We propagate the panic as is.
                    panic::resume_unwind(handle.join().unwrap_err())
                }
            }
        }
    }

    fn exec(&self, current_dir: Option<&Path>) -> Result<()> {
        match &self.kind {
            DelayKind::DurationElapse(duration) => {
                thread::sleep(*duration);
            }
            DelayKind::TcpConnect(addr) => {
                while let Err(_err) = TcpStream::connect(addr) {
                    thread::sleep(self.poll);
                }
            }
            DelayKind::UdpResponse(remote, req) => {
                let socket = match remote {
                    SocketAddr::V4(_) => {
                        UdpSocket::bind(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0))
                            .context("Could not bind local IPv4 UDP socket.")?
                    }
                    SocketAddr::V6(_) => {
                        UdpSocket::bind(SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0))
                            .context("Could not bind local IPv6 UDP socket.")?
                    }
                };

                socket.set_read_timeout(Some(self.poll))?;
                socket.set_write_timeout(Some(self.poll))?;

                loop {
                    while let Err(_err) = socket.send_to(req.as_slice(), remote) {
                        thread::sleep(self.poll);
                    }

                    let mut buf = [0; 1];
                    match socket.recv(&mut buf) {
                        Ok(_size) => break,
                        Err(e) => {
                            if e.kind() != WouldBlock {
                                thread::sleep(self.poll);
                            }
                        }
                    }
                }
            }
            DelayKind::PathExists(path) => {
                let path = if let Some(current_dir) = current_dir {
                    Cow::Owned(current_dir.join(path))
                } else {
                    Cow::Borrowed(path)
                };
                while !path.exists() {
                    thread::sleep(self.poll);
                }
            }
        }

        Ok(())
    }
}

impl From<api::Delay> for Delay {
    fn from(value: api::Delay) -> Self {
        let (poll, timeout) = if let DelayKind::DurationElapse(_) = value.kind {
            if value.poll.is_some() {
                warn!("Ignoring poll setting. Not supported for {:?}", value.kind);
            }
            if value.timeout.is_some() {
                warn!(
                    "Ignoring timeout setting. Not supported for {:?}",
                    value.kind
                );
            }
            (Duration::ZERO, Duration::ZERO)
        } else {
            let mut poll = value.poll.unwrap_or_else(|| Duration::from_millis(10));
            let timeout = value.timeout.map_or_else(
                || Duration::from_secs(600),
                |t| {
                    if t < Duration::from_millis(10) {
                        warn!("The minimum timeout setting is 10ms");
                        Duration::from_millis(10)
                    } else {
                        t
                    }
                },
            );

            if poll >= timeout {
                warn!(
                    "Poll duration is equal to or greater than the timeout duration ({poll:?} >= \
                     {timeout:?})."
                );
                poll = timeout.saturating_sub(Duration::from_millis(5));
                warn!("Using poll duration {poll:?} instead");
            }
            (poll, timeout)
        };

        Self {
            poll,
            timeout,
            kind: value.kind,
        }
    }
}

impl Benchmark for LoadBaselineBenchmark {
    fn default_output_path(
        &self,
        bin_bench: &BinBench,
        config: &Config,
        group_module_path: &ModulePath,
        _use_temp_dir: bool,
    ) -> Result<ToolOutputPath> {
        let kind = if bin_bench.default_tool.has_output_file() {
            ToolOutputPathKind::BaseOut(self.loaded_baseline.to_string())
        } else {
            ToolOutputPathKind::BaseLog(self.loaded_baseline.to_string())
        };
        ToolOutputPath::new(
            kind,
            bin_bench.default_tool,
            &BaselineKind::Name(self.baseline.clone()),
            &config.meta.target_dir,
            group_module_path,
            &bin_bench.name(),
            false, // We use a hard coded override to be sure
        )
    }

    fn baselines(&self) -> Baselines {
        (
            Some(self.loaded_baseline.to_string()),
            Some(self.baseline.to_string()),
        )
    }

    fn run(
        &self,
        bin_bench: BinBench,
        config: &Config,
        _captured_output: Option<CapturedOutput>,
        _force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary> {
        let header = BinaryBenchmarkHeader::new(&config.meta, &bin_bench);
        Ok(bin_bench.create_benchmark_summary(
            config,
            &output_path,
            &bin_bench.function_name,
            header.description(),
            self.baselines(),
        ))
    }

    fn data_processor(
        &self,
        tools: &ToolConfigs,
        project_root: &Path,
        output_path: &ToolOutputPath,
    ) -> Box<dyn BenchmarkDataProcessor> {
        Box::new(LoadBaselineDataProcessor {
            analyzers: tools.analyzers(project_root, output_path),
        })
    }
}

impl Benchmark for SaveBaselineBenchmark {
    fn default_output_path(
        &self,
        bin_bench: &BinBench,
        config: &Config,
        group_module_path: &ModulePath,
        use_temp_dir: bool,
    ) -> Result<ToolOutputPath> {
        let kind = if bin_bench.default_tool.has_output_file() {
            ToolOutputPathKind::BaseOut(self.baseline.to_string())
        } else {
            ToolOutputPathKind::BaseLog(self.baseline.to_string())
        };
        let output_path = ToolOutputPath::new(
            kind,
            bin_bench.default_tool,
            &BaselineKind::Name(self.baseline.clone()),
            &config.meta.target_dir,
            group_module_path,
            &bin_bench.name(),
            use_temp_dir,
        )?;

        if !use_temp_dir {
            output_path.init()?;
        }

        Ok(output_path)
    }

    fn baselines(&self) -> Baselines {
        (
            Some(self.baseline.to_string()),
            Some(self.baseline.to_string()),
        )
    }

    fn run(
        &self,
        bin_bench: BinBench,
        config: &Config,
        captured_output: Option<CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary> {
        let header = BinaryBenchmarkHeader::new(&config.meta, &bin_bench);
        let benchmark_summary = bin_bench.create_benchmark_summary(
            config,
            &output_path,
            &bin_bench.function_name,
            header.description(),
            self.baselines(),
        );

        bin_bench.tools.run(
            benchmark_summary,
            config,
            &bin_bench.command.path,
            &bin_bench.command.args,
            &bin_bench.run_options,
            &output_path,
            &bin_bench.module_path,
            captured_output.as_ref(),
            force_shutdown,
        )
    }

    fn data_processor(
        &self,
        tools: &ToolConfigs,
        project_root: &Path,
        output_path: &ToolOutputPath,
    ) -> Box<dyn BenchmarkDataProcessor> {
        Box::new(SaveBaselineDataProcessor {
            analyzers: tools.analyzers(project_root, output_path),
        })
    }
}

/// Creates the binary benchmark executor [`Benchmark`] matching the current baseline mode.
///
/// # Panics
///
/// Panics when `--load-baseline` is active but no comparison baseline is configured.
pub fn benchmark_factory(config: &Config) -> Arc<dyn Benchmark> {
    if let Some(baseline_name) = &config.meta.args.save_baseline {
        Arc::new(SaveBaselineBenchmark {
            baseline: baseline_name.clone(),
        })
    } else if let Some(baseline_name) = &config.meta.args.load_baseline {
        Arc::new(LoadBaselineBenchmark {
            loaded_baseline: baseline_name.clone(),
            baseline: config
                .meta
                .args
                .baseline
                .as_ref()
                .expect("A baseline should be present")
                .clone(),
        })
    } else {
        Arc::new(BaselineBenchmark {
            baseline_kind: config
                .meta
                .args
                .baseline
                .as_ref()
                .map_or(BaselineKind::Old, |name| BaselineKind::Name(name.clone())),
        })
    }
}

/// Print a list of all benchmarks with a short summary
pub fn list(benchmark_groups: BinaryBenchmarkGroups, config: &Config) -> Result<()> {
    Groups::from_binary_benchmark(&config.module_path, benchmark_groups, &config.meta)
        .map(Groups::list)
}

/// The top-level method which should be used to initiate running all benchmarks
pub fn run(benchmark_groups: BinaryBenchmarkGroups, config: Config) -> Result<BenchmarkSummaries> {
    Runner::from_binary_benchmark(benchmark_groups, config).and_then(Runner::run)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::net::TcpListener;

    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use tempfile::tempdir;

    use super::*;

    fn api_delay_fixture<T, U>(poll: T, timeout: U, kind: DelayKind) -> api::Delay
    where
        T: Into<Option<u64>>,
        U: Into<Option<u64>>,
    {
        api::Delay {
            poll: poll.into().map(Duration::from_millis),
            timeout: timeout.into().map(Duration::from_millis),
            kind,
        }
    }

    #[rstest]
    #[case::duration_elapse_when_no_poll_no_timeout(
        api_delay_fixture(None, None, DelayKind::DurationElapse(Duration::from_millis(100))),
        Duration::ZERO,
        Duration::ZERO
    )]
    #[case::duration_elapse_when_poll_no_timeout(
        api_delay_fixture(10, None, DelayKind::DurationElapse(Duration::from_millis(100))),
        Duration::ZERO,
        Duration::ZERO
    )]
    #[case::duration_elapse_when_no_poll_but_timeout(
        api_delay_fixture(None, 10, DelayKind::DurationElapse(Duration::from_millis(100))),
        Duration::ZERO,
        Duration::ZERO
    )]
    #[case::duration_elapse_when_poll_and_timeout(
        api_delay_fixture(10, 100, DelayKind::DurationElapse(Duration::from_millis(100))),
        Duration::ZERO,
        Duration::ZERO
    )]
    #[case::path_when_no_poll_no_timeout(
        api_delay_fixture(None, None, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(10),
        Duration::from_secs(600)
    )]
    #[case::path_when_poll_no_timeout(
        api_delay_fixture(20, None, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(20),
        Duration::from_secs(600)
    )]
    #[case::path_when_no_poll_but_timeout(
        api_delay_fixture(None, 200, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(10),
        Duration::from_millis(200)
    )]
    #[case::path_when_poll_and_timeout(
        api_delay_fixture(20, 200, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(20),
        Duration::from_millis(200)
    )]
    #[case::path_when_poll_equal_to_timeout(
        api_delay_fixture(200, 200, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(195),
        Duration::from_millis(200)
    )]
    #[case::path_when_poll_higher_than_timeout(
        api_delay_fixture(201, 200, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(195),
        Duration::from_millis(200)
    )]
    #[case::path_when_poll_equal_to_timeout_smaller_than_10(
        api_delay_fixture(10, 9, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(5),
        Duration::from_millis(10)
    )]
    #[case::path_when_poll_lower_than_timeout_smaller_than_10(
        api_delay_fixture(7, 9, DelayKind::PathExists(PathBuf::from("/some/path"))),
        Duration::from_millis(7),
        Duration::from_millis(10)
    )]
    fn test_from_api_delay_for_delay(
        #[case] delay: api::Delay,
        #[case] poll: Duration,
        #[case] timeout: Duration,
    ) {
        let expected = Delay::new(poll, timeout, delay.kind.clone());
        assert_eq!(Delay::from(delay), expected);
    }

    #[test]
    fn test_delay_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("file.pid");

        let delay = Delay {
            poll: Duration::from_millis(50),
            timeout: Duration::from_millis(200),
            kind: DelayKind::PathExists(file_path.clone()),
        };
        let handle = thread::spawn(move || {
            delay.apply(None).unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        File::create(file_path).unwrap();

        handle.join().unwrap();
        drop(dir);
    }

    #[test]
    fn test_delay_path_with_current_dir() {
        let dir = tempdir().unwrap();
        let file_path = PathBuf::from("file.pid");

        let delay = Delay {
            poll: Duration::from_millis(50),
            timeout: Duration::from_millis(200),
            kind: DelayKind::PathExists(file_path.clone()),
        };

        let dir_path = dir.path().to_owned();
        let handle = thread::spawn(move || {
            delay.apply(Some(&dir_path)).unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        File::create(dir.path().join(file_path)).unwrap();

        handle.join().unwrap();
        drop(dir);
    }

    #[test]
    fn test_delay_tcp_connect() {
        let addr = "127.0.0.1:32000".parse::<SocketAddr>().unwrap();
        let _listener = TcpListener::bind(addr).unwrap();

        let delay = Delay {
            poll: Duration::from_millis(20),
            timeout: Duration::from_secs(1),
            kind: DelayKind::TcpConnect(addr),
        };
        delay.apply(None).unwrap();
    }

    #[test]
    fn test_delay_tcp_connect_poll() {
        let addr = "127.0.0.1:32001".parse::<SocketAddr>().unwrap();

        let check_addr = addr;
        let handle = thread::spawn(move || {
            let delay = Delay {
                poll: Duration::from_millis(20),
                timeout: Duration::from_secs(1),
                kind: DelayKind::TcpConnect(check_addr),
            };
            delay.apply(None).unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        let _listener = TcpListener::bind(addr).unwrap();

        handle.join().unwrap();
    }

    #[test]
    fn test_delay_tcp_connect_timeout() {
        let addr = "127.0.0.1:32002".parse::<SocketAddr>().unwrap();
        let delay = Delay {
            poll: Duration::from_millis(20),
            timeout: Duration::from_secs(1),
            kind: DelayKind::TcpConnect(addr),
        };

        let result = delay.apply(None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Timeout of '1s' reached");
    }

    #[test]
    fn test_delay_udp_response() {
        let addr = "127.0.0.1:34000".parse::<SocketAddr>().unwrap();

        thread::spawn(move || -> ! {
            let server = UdpSocket::bind(addr).unwrap();
            server
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            server
                .set_write_timeout(Some(Duration::from_millis(100)))
                .unwrap();

            loop {
                let mut buf = [0; 1];

                match server.recv_from(&mut buf) {
                    Ok((_size, from)) => {
                        server.send_to(&[2], from).unwrap();
                    }
                    Err(_e) => {}
                }
            }
        });

        let delay = Delay {
            poll: Duration::from_millis(20),
            timeout: Duration::from_millis(100),
            kind: DelayKind::UdpResponse(addr, vec![1]),
        };

        delay.apply(None).unwrap();
    }

    #[test]
    fn test_delay_udp_response_poll() {
        let addr = "127.0.0.1:34001".parse::<SocketAddr>().unwrap();

        thread::spawn(move || {
            let delay = Delay {
                poll: Duration::from_millis(20),
                timeout: Duration::from_millis(100),
                kind: DelayKind::UdpResponse(addr, vec![1]),
            };
            delay.apply(None).unwrap();
        });

        let server = UdpSocket::bind(addr).unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        server
            .set_write_timeout(Some(Duration::from_millis(100)))
            .unwrap();

        loop {
            let mut buf = [0; 1];

            thread::sleep(Duration::from_millis(70));

            match server.recv_from(&mut buf) {
                Ok((_size, from)) => {
                    server.send_to(&[2], from).unwrap();
                    break;
                }
                Err(_e) => {}
            }
        }
    }

    #[test]
    fn test_delay_udp_response_timeout() {
        let addr = "127.0.0.1:34002".parse::<SocketAddr>().unwrap();
        let delay = Delay {
            poll: Duration::from_millis(20),
            timeout: Duration::from_millis(100),
            kind: DelayKind::UdpResponse(addr, vec![1]),
        };
        let result = delay.apply(None);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Timeout of '100ms' reached"
        );
    }
}
