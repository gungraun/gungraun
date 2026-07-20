//! The `lib_bench` module
//!
//! This module runs all the library benchmarks

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Debug;
use std::marker::{Send, Sync};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Result;

use super::format::{LibraryBenchmarkHeader, ListFormat, OutputFormat};
use super::meta::Metadata;
use super::tool::config::ToolConfigs;
use super::tool::path::{ToolOutputPath, ToolOutputPathKind};
use super::tool::run::RunOptions;
use crate::api::{
    BenchRunMode, EntryPoint, LibraryBenchmarkConfig, LibraryBenchmarkGroups, RawToolArgs, Tool,
};
use crate::error::Error;
use crate::runner::args;
use crate::runner::common::{
    BaselineAndSaveDataProcessor, BaselineDataProcessor, Baselines, BenchmarkDataProcessor,
    BenchmarkSummaries, CapturedOutput, Config, Groups, LoadBaselineDataProcessor, ModulePath,
    Runner, SaveBaselineDataProcessor,
};
use crate::runner::tool::config::ToolConfig;
use crate::summary::model::{
    BaselineKind, BaselineName, BenchmarkKind, BenchmarkSummary, SummaryOutput,
};

/// Implements [`Benchmark`] to compare a [`LibBench`] against one baseline and save the new run as
/// another baseline.
#[derive(Debug)]
pub struct BaselineAndSaveBenchmark {
    baseline: BaselineName,
    save_baseline: BaselineName,
}

/// Implements [`Benchmark`] to run a [`LibBench`] and compare against an earlier [`BenchmarkKind`]
#[derive(Debug)]
pub struct BaselineBenchmark {
    baseline_kind: BaselineKind,
}

/// A `LibBench` represents a single benchmark under the `#[library_benchmark]` attribute macro
///
/// It needs an implementation of `Benchmark` to be run.
#[derive(Debug, Clone)]
pub struct LibBench {
    /// The index of the `#[bench]` in the `#[library_benchmark]`
    pub bench_index: usize,
    /// The arguments of the `consts` parameter as a single string
    pub consts_display: Option<String>,
    /// The default [`Tool`]. If not changed it is `Callgrind`.
    pub default_tool: Tool,
    /// The arguments of `args` attribute as a single string
    pub display: Option<String>,
    /// The name of the annotated function
    pub function_name: String,
    /// The index of the `#[library_benchmark]` in the `library_benchmark_group!`
    pub group_index: usize,
    /// The id of the benchmark as in `#[bench::id]`
    pub id: Option<String>,
    /// The index of the element in the iterator of `#[benches::id(iter = ITERATOR)]` if present
    pub iter_index: Option<usize>,
    /// The [`ModulePath`].
    ///
    /// This is an artificial path for display purposes and does not reflect the real module path
    /// of the benchmark in the benchmark file
    pub module_path: ModulePath,
    /// The [`OutputFormat`]
    pub output_format: OutputFormat,
    /// The [`RunOptions`]
    pub run_options: RunOptions,
    /// The tool configurations for this benchmark run
    pub tools: ToolConfigs,
}

/// Implements [`Benchmark`] to load a [`LibBench`] baseline run and compare against another
/// baseline
///
/// This benchmark runner does not run valgrind or execute anything.
#[derive(Debug)]
pub struct LoadBaselineBenchmark {
    baseline: BaselineName,
    loaded_baseline: BaselineName,
}

/// Implements [`Benchmark`] to save a [`LibBench`] run as baseline. If present compare against a
/// former baseline with the same name
#[derive(Debug)]
pub struct SaveBaselineBenchmark {
    baseline: BaselineName,
}

/// Strategy interface for executing library benchmarks in different baseline modes.
///
/// Despite having the same name, this trait differs from `bin_bench::Benchmark` and is
/// designed to run a `LibBench` only.
pub trait Benchmark: Debug + Send + Sync {
    /// Returns the pair of loaded and active baseline names used for this run.
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

    /// Computes the output location for this benchmark run.
    ///
    /// The path is derived from [`LibBench`], the global [`Config`], the enclosing
    /// `group_module_path`, and whether temporary directories should be used.
    ///
    /// # Errors
    ///
    /// Returns an error if the output path cannot be created or initialized.
    fn default_output_path(
        &self,
        lib_bench: &LibBench,
        config: &Config,
        group_module_path: &ModulePath,
        use_temp_dir: bool,
    ) -> Result<ToolOutputPath>;

    /// Executes a benchmark and returns its populated summary.
    ///
    /// The method consumes [`LibBench`], uses `main_index` to address the benchmark harness entry,
    /// optionally captures output into [`CapturedOutput`], reacts to `force_shutdown`, and writes
    /// artifacts under [`ToolOutputPath`].
    ///
    /// # Errors
    ///
    /// Returns an error if launching, running, or post-processing the benchmark fails.
    fn run(
        &self,
        lib_bench: LibBench,
        config: &Config,
        main_index: usize,
        captured_output: Option<CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary>;
}

impl Benchmark for BaselineAndSaveBenchmark {
    fn default_output_path(
        &self,
        lib_bench: &LibBench,
        config: &Config,
        group_module_path: &ModulePath,
        use_temp_dir: bool,
    ) -> Result<ToolOutputPath> {
        let kind = if lib_bench.default_tool.has_output_file() {
            ToolOutputPathKind::BaseOut(self.save_baseline.to_string())
        } else {
            ToolOutputPathKind::BaseLog(self.save_baseline.to_string())
        };

        let output_path = ToolOutputPath::new(
            kind,
            lib_bench.default_tool,
            &BaselineKind::Name(self.baseline.clone()),
            &config.meta.target_dir,
            group_module_path,
            &lib_bench.name(),
            use_temp_dir,
        )?;

        if !use_temp_dir {
            output_path.init()?;
        }

        Ok(output_path)
    }

    fn baselines(&self) -> Baselines {
        (
            Some(self.save_baseline.to_string()),
            Some(self.baseline.to_string()),
        )
    }

    fn run(
        &self,
        lib_bench: LibBench,
        config: &Config,
        main_index: usize,
        captured_output: Option<CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary> {
        let header = LibraryBenchmarkHeader::new(&lib_bench);
        let benchmark_summary = lib_bench.create_benchmark_summary(
            config,
            &output_path,
            &lib_bench.function_name,
            header.description(),
            self.baselines(),
        );

        let LibBench {
            bench_index,
            group_index,
            iter_index,
            module_path,
            run_options,
            tools,
            ..
        } = lib_bench;

        tools.run(
            benchmark_summary,
            config,
            &config.bench_bin,
            |tool_config, run_mode| {
                Cow::Owned(LibBench::bench_args(
                    tool_config,
                    run_mode,
                    main_index,
                    group_index,
                    bench_index,
                    iter_index,
                ))
            },
            &run_options,
            &output_path,
            &module_path,
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
        Box::new(BaselineAndSaveDataProcessor {
            analyzers: tools.analyzers(project_root, output_path),
        })
    }
}

impl Benchmark for BaselineBenchmark {
    fn default_output_path(
        &self,
        lib_bench: &LibBench,
        config: &Config,
        group_module_path: &ModulePath,
        use_temp_dir: bool,
    ) -> Result<ToolOutputPath> {
        let kind = if lib_bench.default_tool.has_output_file() {
            ToolOutputPathKind::Out
        } else {
            ToolOutputPathKind::Log
        };
        let output_path = ToolOutputPath::new(
            kind,
            lib_bench.default_tool,
            &self.baseline_kind,
            &config.meta.target_dir,
            group_module_path,
            &lib_bench.name(),
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
        lib_bench: LibBench,
        config: &Config,
        main_index: usize,
        captured_output: Option<CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary> {
        let header = LibraryBenchmarkHeader::new(&lib_bench);
        let benchmark_summary = lib_bench.create_benchmark_summary(
            config,
            &output_path,
            &lib_bench.function_name,
            header.description(),
            self.baselines(),
        );

        let LibBench {
            bench_index,
            group_index,
            iter_index,
            module_path,
            run_options,
            tools,
            ..
        } = lib_bench;

        tools.run(
            benchmark_summary,
            config,
            &config.bench_bin,
            |tool_config, run_mode| {
                Cow::Owned(LibBench::bench_args(
                    tool_config,
                    run_mode,
                    main_index,
                    group_index,
                    bench_index,
                    iter_index,
                ))
            },
            &run_options,
            &output_path,
            &module_path,
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

impl LibBench {
    /// Returns whether any configured tool enables fail-fast regression handling.
    pub fn is_fail_fast(&self) -> bool {
        self.tools
            .0
            .iter()
            .any(|c| c.regression_config.is_fail_fast())
    }

    /// Creates a new library benchmark.
    pub fn new(
        id: Option<String>,
        display: Option<String>,
        consts_display: Option<String>,
        module_path: ModulePath,
        function_name: String,
        meta: &Metadata,
        config: LibraryBenchmarkConfig,
        group_index: usize,
        bench_index: usize,
        iter_index: Option<usize>,
        default_tool: Tool,
    ) -> Result<Option<Self>> {
        let id = if let Some(iter_index) = iter_index {
            id.as_ref().map(|s| format!("{s}_{iter_index}"))
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

        let mut envs = config.resolve_envs();
        // meta envs are already resolved
        envs.extend(meta.args.envs.iter().flatten().cloned());

        // Setting --delay=-1 for perf is only required for library benchmarks. This is applied no
        // matter if the default entry point is used or not. The user can still override this
        // setting if required.
        let mut default_args = HashMap::from([(Tool::Perf, RawToolArgs::new(["--delay", "-1"]))]);

        // The Cachegrind client requests are not inserted into the benchmark function if the
        // default tool is not Cachegrind, so setting --instr-at-start to `no` is only required if
        // the default tool sent by the benchmark harness (not with command-line arguments) is
        // Cachegrind. Also, we only need to set this in library benchmarks, so it's best to use
        // `default_args` to add this command-line argument.
        let default_tool = if let Some(meta_default_tool) = meta.args.default_tool {
            meta_default_tool
        } else {
            if default_tool == Tool::Cachegrind {
                default_args.insert(
                    Tool::Cachegrind,
                    RawToolArgs::new_ignore_flag(["--instr-at-start=no"]),
                );
            }
            config.default_tool.unwrap_or(default_tool)
        };

        let mut output_format = config
            .output_format
            .map_or_else(OutputFormat::default, Into::into);
        output_format.kind = meta.args.output_format;

        let tool_configs = ToolConfigs::new(
            &mut output_format,
            config.tool_specs,
            &module_path,
            id.as_ref(),
            meta,
            default_tool,
            &EntryPoint::Default,
            &config.valgrind_args,
            &default_args,
            None,
        )
        .map_err(|error| {
            Error::ConfigurationError(module_path.clone(), id.clone(), error.to_string())
        })?;

        Ok(Some(Self {
            group_index,
            bench_index,
            iter_index,
            id,
            function_name,
            display,
            consts_display,
            run_options: RunOptions {
                env_clear: meta
                    .args
                    .env_clear
                    .unwrap_or_else(|| config.env_clear.unwrap_or(args::defaults::ENV_CLEAR)),
                envs,
                sandbox: config.sandbox,
                current_dir: config.current_dir,
                ..Default::default()
            },
            tools: tool_configs,
            module_path,
            output_format,
            default_tool,
        }))
    }

    /// The name of this `LibBench` consisting of the name of the benchmark function and if present,
    /// the id of the bench attribute (`#[bench::ID(...)]`)
    ///
    /// The name is used to identify a benchmark run within the same [`Group`] and has therefore to
    /// be unique within the same [`Group`]
    ///
    /// [`Group`]: [crate::runner::common::Group]
    fn name(&self) -> String {
        if let Some(bench_id) = &self.id {
            format!("{}.{}", self.function_name, bench_id)
        } else {
            self.function_name.clone()
        }
    }

    /// The arguments for the `bench_bin` to actually run the benchmark function
    fn bench_args(
        config: &ToolConfig,
        run_mode: Option<BenchRunMode>,
        main_index: usize,
        group_index: usize,
        bench_index: usize,
        iter_index: Option<usize>,
    ) -> Vec<OsString> {
        // The string has a fixed length to have an equal argument parser in the benchmark binary
        // for all benchmarks.
        let index_to_string = |index| format!("{index:05}");

        let mut args = vec![
            OsString::from("--gungraun-run".to_owned()),
            OsString::from(run_mode.unwrap_or_else(|| config.benchmark_run_mode()).id()),
            OsString::from(index_to_string(main_index)),
            OsString::from(index_to_string(group_index)),
            OsString::from(index_to_string(bench_index)),
        ];

        if let Some(iter_index) = iter_index {
            args.push(OsString::from(index_to_string(iter_index)));
        }

        args
    }

    /// This method creates the initial [`BenchmarkSummary`]
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
            BenchmarkKind::LibraryBenchmark,
            config.meta.project_root.clone(),
            config.package_dir.clone(),
            config.bench_file.clone(),
            config.bench_bin.clone(),
            &self.module_path,
            function_name,
            self.id.clone(),
            description,
            summary_output,
            baselines,
        )
    }
}

impl Benchmark for LoadBaselineBenchmark {
    fn default_output_path(
        &self,
        lib_bench: &LibBench,
        config: &Config,
        group_module_path: &ModulePath,
        _use_temp_dir: bool,
    ) -> Result<ToolOutputPath> {
        let kind = if lib_bench.default_tool.has_output_file() {
            ToolOutputPathKind::BaseOut(self.loaded_baseline.to_string())
        } else {
            ToolOutputPathKind::BaseLog(self.loaded_baseline.to_string())
        };
        ToolOutputPath::new(
            kind,
            lib_bench.default_tool,
            &BaselineKind::Name(self.baseline.clone()),
            &config.meta.target_dir,
            group_module_path,
            &lib_bench.name(),
            false,
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
        lib_bench: LibBench,
        config: &Config,
        _main_index: usize,
        _captured_output: Option<CapturedOutput>,
        _force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary> {
        let header = LibraryBenchmarkHeader::new(&lib_bench);
        Ok(lib_bench.create_benchmark_summary(
            config,
            &output_path,
            &lib_bench.function_name,
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
        lib_bench: &LibBench,
        config: &Config,
        group_module_path: &ModulePath,
        use_temp_dir: bool,
    ) -> Result<ToolOutputPath> {
        let kind = if lib_bench.default_tool.has_output_file() {
            ToolOutputPathKind::BaseOut(self.baseline.to_string())
        } else {
            ToolOutputPathKind::BaseLog(self.baseline.to_string())
        };

        let output_path = ToolOutputPath::new(
            kind,
            lib_bench.default_tool,
            &BaselineKind::Name(self.baseline.clone()),
            &config.meta.target_dir,
            group_module_path,
            &lib_bench.name(),
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
        lib_bench: LibBench,
        config: &Config,
        main_index: usize,
        captured_output: Option<CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
        output_path: ToolOutputPath,
    ) -> Result<BenchmarkSummary> {
        let header = LibraryBenchmarkHeader::new(&lib_bench);
        let benchmark_summary = lib_bench.create_benchmark_summary(
            config,
            &output_path,
            &lib_bench.function_name,
            header.description(),
            self.baselines(),
        );

        let LibBench {
            bench_index,
            group_index,
            iter_index,
            module_path,
            run_options,
            tools,
            ..
        } = lib_bench;

        tools.run(
            benchmark_summary,
            config,
            &config.bench_bin,
            |tool_config, run_mode| {
                Cow::Owned(LibBench::bench_args(
                    tool_config,
                    run_mode,
                    main_index,
                    group_index,
                    bench_index,
                    iter_index,
                ))
            },
            &run_options,
            &output_path,
            &module_path,
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

/// Creates the library benchmark executor [`Benchmark`] matching the current baseline mode.
///
/// # Panics
///
/// Panics when `--load-baseline` is active but no comparison baseline is configured.
pub fn benchmark_factory(config: &Config) -> Arc<dyn Benchmark> {
    if let (Some(save_baseline), Some(baseline)) =
        (&config.meta.args.save_baseline, &config.meta.args.baseline)
    {
        Arc::new(BaselineAndSaveBenchmark {
            baseline: baseline.clone(),
            save_baseline: save_baseline.clone(),
        })
    } else if let Some(baseline_name) = &config.meta.args.save_baseline {
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
pub fn list(
    benchmark_groups: LibraryBenchmarkGroups,
    config: &Config,
    format: ListFormat,
    ignored: bool,
) -> Result<()> {
    Groups::from_library_benchmark(&config.module_path, benchmark_groups, &config.meta)
        .map(|groups| groups.list(format, ignored))
}

/// The top-level method which should be used to initiate running all benchmarks
pub fn run(benchmark_groups: LibraryBenchmarkGroups, config: Config) -> Result<BenchmarkSummaries> {
    Runner::from_library_benchmark(benchmark_groups, config).and_then(Runner::run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SanitizeOutput;
    use crate::fixtures::tool_config_f;

    #[test]
    fn test_baseline_and_save_benchmark_uses_different_display_baselines() {
        let benchmark = BaselineAndSaveBenchmark {
            baseline: BaselineName("main".to_owned()),
            save_baseline: BaselineName("pr_1234".to_owned()),
        };

        assert_eq!(
            benchmark.baselines(),
            (Some("pr_1234".to_owned()), Some("main".to_owned()))
        );
    }

    #[test]
    fn bench_args_uses_run_mode_override() {
        let config = tool_config_f()
            .tool(Tool::Perf)
            .entry_point(EntryPoint::Default)
            .sanitize_output(SanitizeOutput::No)
            .fixture();

        let args = LibBench::bench_args(&config, Some(BenchRunMode::PerfCalibrate), 1, 2, 3, None);

        assert_eq!(args[1], OsString::from(BenchRunMode::PerfCalibrate.id()));
    }
}
