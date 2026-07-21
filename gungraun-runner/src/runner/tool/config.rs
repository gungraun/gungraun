//! Tool configuration construction and execution.
//!
//! This module has two main parts:
//!
//! 1. **Construction** — [`ToolConfigBuilder`] resolves user-facing [`ToolSpec`]s, metadata, and
//!    defaults into one or more concrete [`ToolConfig`] instances. See [`ToolConfigBuilder::new`]
//!    for the resolution phase and [`ToolConfigBuilder::build`] for the materialization phase,
//!    which handles tool-specific expansion rules (perf event-set splitting, record sidecars,
//!    etc.).
//! 2. **Execution** — [`ToolConfigs::run`] iterates enabled [`ToolConfig`]s and drives the full
//!    benchmark lifecycle for each: output-path derivation, sandbox setup, assistant launch, delay,
//!    command construction and execution, optional perf-overhead measurement, teardown, and sandbox
//!    reset. The [`ToolConfigs`] collection also provides shared helpers such as
//!    [`analyzers`](ToolConfigs::analyzers), [`output_paths`](ToolConfigs::output_paths), and
//!    [`alpha`](ToolConfigs::alpha).
//!
//! # Why a single tool can produce multiple configs
//!
//! Most tools map 1:1 from a [`ToolSpec`] to a single [`ToolConfig`]. Perf is the exception: `perf
//! stat` can only measure one event set per invocation, so when a user specifies multiple event
//! selectors (e.g. `events: ["cycles", "instructions"]`), the builder emits a separate
//! [`ToolConfig`] for each selector. These are distinguished by a sequential
//! [`part`](ToolConfig::part) number (`1`, `2`, ...), which is appended to output paths (e.g. `p1`,
//! `p2`) so the runs do not overwrite each other's output files.
//!
//! # Record configs
//!
//! For perf, a [`ToolSpec`] can set [`record`](crate::api::PerfSpec::record) to `true`. In that
//! case each stat config gets a paired `perf record` config. The record config inherits most fields
//! from its stat config but changes the [`run_mode`](PerfConfig::run_mode) to
//! [`Direct`](PerfRunMode::Direct) (no batching/repetition) and disables sampling. Only the stat
//! config is marked [`has_analyzer`](ToolConfig::has_analyzer); the record config is a side-car run
//! that produces a raw profile for later inspection.
//!
//! # Design decisions
//!
//! - **Two-phase construction** keeps option-resolution logic (spec parsing, default application,
//!   metadata overrides) separate from the mechanical creation of configs. This makes both phases
//!   easier to test and reason about.
//! - **`has_analyzer` is `true` only for the first config/part** so that even when a tool emits
//!   multiple configs, downstream code sees only one analyzer per tool. This avoids duplicate
//!   parsing and regression checks for the same tool.
//! - **`is_default` propagates only to the first config/part** so captured output and summary
//!   reporting treat the first run as the canonical one.
//! - **Timeout for perf record configs is cleared when sampling is enabled** because the timeout
//!   stores the `sample_duration` intended for the stat measurement, not the record run.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Result, anyhow};

use super::args::{ToolArgsLike, ValgrindArgs};
use super::parser::parser_factory;
use super::path::ToolOutputPath;
use super::regression::ToolRegressionConfig;
use super::run::{RunOptions, ToolCommand};
use crate::api::{
    self, BenchRunMode, EntryPoint, PerfRunMode, RawToolArgs, SanitizeOutput, Tool, ToolSpec,
    ToolSpecs,
};
use crate::runner::args::PerfSampling;
use crate::runner::callgrind::flamegraph::Config as FlamegraphConfig;
use crate::runner::common::{
    Analyzer, Assistant, CapturedOutput, Config, ModulePath, PerfOutputConfig, Sandbox,
};
use crate::runner::format::OutputFormat;
use crate::runner::meta::Metadata;
use crate::runner::perf::args::DEFAULT_PERF_EVENTS;
use crate::runner::perf::run::measure_perf_overhead;
use crate::runner::tasks::ProcessHandler;
use crate::runner::tool::args::ToolArgs;
use crate::runner::{DEFAULT_TOGGLE, cachegrind, callgrind, perf};
use crate::summary::model::BenchmarkSummary;

/// Default minimum percentage of time a PMU counter must be running before the runner keeps its
/// sampled metrics.
///
/// Perf may multiplex (time-share) hardware counters when more events are requested than physical
/// PMU slots are available. `pcnt_running` reports the fraction of the measurement interval that
/// each counter was actually active. The runner drops sampled records whose `pcnt_running` falls
/// below this threshold.
pub const DEFAULT_PERF_MIN_PCNT_RUNNING: f64 = 100.0;
/// Default significance level used for perf regression checks when no alpha is configured.
pub const DEFAULT_PERF_ALPHA: f64 = 0.05;
/// Default patterns for perf metrics that must not be zero.
///
/// Metrics matching these patterns with a zero value cause the entire measurement batch to be
/// discarded. Patterns use `simplematch` glob syntax.
pub const DEFAULT_PERF_NON_ZERO_METRICS: &[&str] = &["task-clock*", "cpu-clock*", "*instructions*"];

/// The DHAT-specific configuration stored in [`ToolConfigOptions::DHAT`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhatConfig {
    /// The wildcard patterns used to matched a function in the call stack of a program point
    pub frames: Vec<String>,
}

/// The runner-resolved perf configuration stored in [`ToolConfigOptions::Perf`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PerfConfig {
    /// The statistical significance threshold used for perf significance handling.
    ///
    /// The runner resolves this to the concrete alpha used when comparing perf results, including
    /// regression checks and significance information shown in perf output.
    pub alpha: f64,
    /// The perf event selector passed through to this concrete perf tool configuration.
    pub events: String,
    /// The minimum percentage of time a PMU counter must be running before sampled metrics are
    /// kept.
    ///
    /// When perf multiplexes hardware counters, `pcnt_running` indicates the fraction of the
    /// measurement interval each counter was active. Records below this threshold are discarded.
    pub min_pcnt_running: f64,
    /// Patterns for perf metrics that must not be zero.
    ///
    /// If a metric matching any of these patterns has a zero value, the entire measurement batch
    /// is discarded. Patterns use `simplematch` glob syntax.
    pub non_zero_metrics: Vec<String>,
    /// How the runner batches benchmark invocations inside each perf measurement.
    pub run_mode: PerfRunMode,
    /// Whether this perf configuration runs `perf stat` in sampled mode.
    pub use_sampling: bool,
}

/// The tool specific flamegraph configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ToolFlamegraphConfig {
    /// The callgrind configuration
    Callgrind(FlamegraphConfig),
    /// If there is no configuration
    None,
}

/// The [`ToolConfig`] containing the basic configuration values to run the benchmark for this tool
#[derive(Debug, Clone, PartialEq)]
pub struct ToolConfig {
    /// The arguments to pass to the Valgrind executable
    pub args: ToolArgs,
    /// The [`EntryPoint`] of this tool
    pub entry_point: EntryPoint,
    /// The tool specific flamegraph configuration
    pub flamegraph_config: ToolFlamegraphConfig,
    /// Whether this config should drive the output analyzer for its tool.
    ///
    /// When a tool produces multiple configs (e.g. perf with multiple event sets), only the first
    /// config/part is marked `true` so downstream code registers one analyzer per tool.
    pub has_analyzer: bool,
    /// If true, this tool is the default tool for the benchmark run
    pub is_default: bool,
    /// If true, this tool is enabled for this benchmark
    pub is_enabled: bool,
    /// The tool-specific resolved options (e.g. [`PerfConfig`] for perf, [`DhatConfig`] for DHAT).
    pub options: ToolConfigOptions,
    /// Sequential part number when a tool emits multiple configs.
    ///
    /// `None` for a single config; `Some(1)`, `Some(2)`, ... when a tool splits into multiple
    /// configs (e.g. perf with multiple event sets). Used to disambiguate output paths.
    pub part: Option<usize>,
    /// The tool specific regression check configuration
    pub regression_config: ToolRegressionConfig,
    /// The resolved output sanitization mode for this tool.
    pub sanitize_output: SanitizeOutput,
    /// Optional timeout for the benchmark invocation.
    ///
    /// For perf, this stores the `sample_duration` when sampling is enabled. It is cleared for
    /// paired record configs because the sample duration applies to the stat measurement, not the
    /// record run.
    ///
    /// Note this is a timeout that is expected to happen and not only a cap that might trigger.
    pub timeout: Option<Duration>,
}

/// Builder for constructing one or more [`ToolConfig`]s for a single tool.
///
/// See the [module-level documentation](crate::runner::tool::config) for an overview of the
/// two-phase construction design and why a single tool can produce multiple configs.
#[derive(Debug)]
pub struct ToolConfigBuilder {
    default_args: RawToolArgs,
    entry_point: Option<EntryPoint>,
    flamegraph_config: ToolFlamegraphConfig,
    is_default: bool,
    is_enabled: bool,
    options: Vec<ToolConfigOptions>,
    raw_tool_args: RawToolArgs,
    record: bool,
    record_args: RawToolArgs,
    regression_config: ToolRegressionConfig,
    sanitize_output: SanitizeOutput,
    timeout: Option<Duration>,
    tool: Tool,
    tool_spec: Option<ToolSpec>,
}

/// The active tool variant and its resolved tool-specific configuration.
///
/// Each variant corresponds to a supported profiling tool. The runner uses this enum to determine
/// which tool is active and to access tool-specific settings such as [`PerfConfig`] for perf or
/// [`DhatConfig`] for DHAT. Tools without special configuration (Callgrind, Cachegrind, etc.) are
/// represented as unit variants.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolConfigOptions {
    /// Callgrind variant — no special runner-resolved configuration.
    Callgrind,
    /// Cachegrind variant — no special runner-resolved configuration.
    Cachegrind,
    /// [`PerfConfig`] of the resolved perf tool configuration.
    Perf(PerfConfig),
    /// [`DhatConfig`] of the resolved DHAT tool configuration.
    DHAT(DhatConfig),
    /// Memcheck variant — no special runner-resolved configuration.
    Memcheck,
    /// Helgrind variant — no special runner-resolved configuration.
    Helgrind,
    /// DRD variant — no special runner-resolved configuration.
    DRD,
    /// Massif variant — no special runner-resolved configuration.
    Massif,
    /// BBV variant — no special runner-resolved configuration.
    BBV,
}

/// A collection of [`ToolConfig`]s that owns their execution and provides shared helpers.
///
/// [`ToolConfigs::run`] iterates enabled configs and drives the full benchmark lifecycle for each.
/// The collection also exposes helpers such as [`analyzers`](ToolConfigs::analyzers) and
/// [`output_paths`](ToolConfigs::output_paths) used by downstream code.
#[derive(Debug, Clone)]
pub struct ToolConfigs(pub Vec<ToolConfig>);

impl ToolConfig {
    /// Creates a new `ToolConfig`.
    pub fn new(
        is_enabled: bool,
        args: ToolArgs,
        regression_config: ToolRegressionConfig,
        flamegraph_config: ToolFlamegraphConfig,
        entry_point: EntryPoint,
        is_default: bool,
        sanitize_output: SanitizeOutput,
        part: Option<usize>,
        options: ToolConfigOptions,
        has_analyzer: bool,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            args,
            entry_point,
            flamegraph_config,
            has_analyzer,
            is_default,
            is_enabled,
            options,
            part,
            regression_config,
            sanitize_output,
            timeout,
        }
    }

    /// Returns the [`BenchRunMode`] for the measured main benchmark run.
    ///
    /// Perf-specific [`BenchRunMode`]s are only valid for [`EntryPoint::Default`]. In that case,
    /// the benchmark binary executes through Gungraun's generated harness, which understands
    /// perf-only modes such as [`PerfDynamic`], [`PerfOnce`], ...
    ///
    /// For [`EntryPoint::None`] and [`EntryPoint::Custom`], the selected function is not dispatched
    /// through that generated mode switch, so the runner must fall back to
    /// [`BenchRunMode::Default`].
    ///
    /// Also controls side effects: [`DefaultCalibrate`] and [`Calibrate(N)`] cause a calibration
    /// pass to run before the measurement (see [`ToolCommand::new`]), but the measured run itself
    /// uses [`PerfOnce`] — a single invocation inside the perf fence. `Direct` skips calibration
    /// entirely and also uses [`PerfOnce`].
    ///
    /// Returns [`BenchRunMode::Default`] for non-perf tools.
    ///
    /// [`Calibrate(N)`]: PerfRunMode::Calibrate
    /// [`DefaultCalibrate`]: PerfRunMode::DefaultCalibrate
    /// [`PerfDynamic`]: BenchRunMode::PerfDynamic
    /// [`PerfOnce`]: BenchRunMode::PerfOnce
    pub fn benchmark_run_mode(&self) -> BenchRunMode {
        match &self.options {
            ToolConfigOptions::Perf(perf_config) if self.entry_point == EntryPoint::Default => {
                match perf_config.run_mode {
                    PerfRunMode::DynamicBatch => BenchRunMode::PerfDynamic,
                    PerfRunMode::FixedBatch(count) => BenchRunMode::PerfRepeat(count),
                    PerfRunMode::Direct
                    | PerfRunMode::DefaultCalibrate
                    | PerfRunMode::Calibrate(_) => BenchRunMode::PerfOnce,
                }
            }
            _ => BenchRunMode::Default,
        }
    }

    /// Returns the perf event set of this perf tool configuration if the active tool is perf
    pub fn events(&self) -> Option<&str> {
        match &self.options {
            ToolConfigOptions::Perf(perf_config) => Some(perf_config.events.as_str()),
            _ => None,
        }
    }

    /// Returns `true` if this config represents the optional `perf record` run.
    pub fn is_perf_record(&self) -> bool {
        self.args.is_perf_record()
    }

    /// Returns the output path for this config, applying the tool and part modifiers.
    ///
    /// The base `output_path` is first scoped to this config's tool via
    /// [`ToolOutputPath::to_tool_output`]. If this config has a [`part`](ToolConfig::part)
    /// number, it is appended as a `pN` modifier (e.g. `p1`, `p2`) so that multiple configs for
    /// the same tool do not overwrite each other.
    pub fn output_path(&self, output_path: &ToolOutputPath) -> ToolOutputPath {
        let output_path = output_path.to_tool_output(self.tool());
        if let Some(part) = self.part {
            output_path.with_modifiers([format!("p{part}")])
        } else {
            output_path
        }
    }

    /// Returns the [`Tool`] kind corresponding to this config's active variant.
    pub fn tool(&self) -> Tool {
        match &self.options {
            ToolConfigOptions::Callgrind => Tool::Callgrind,
            ToolConfigOptions::Cachegrind => Tool::Cachegrind,
            ToolConfigOptions::Perf(_) => Tool::Perf,
            ToolConfigOptions::DHAT(_) => Tool::DHAT,
            ToolConfigOptions::Memcheck => Tool::Memcheck,
            ToolConfigOptions::Helgrind => Tool::Helgrind,
            ToolConfigOptions::DRD => Tool::DRD,
            ToolConfigOptions::Massif => Tool::Massif,
            ToolConfigOptions::BBV => Tool::BBV,
        }
    }
}

impl ToolConfigBuilder {
    /// Build the [`ToolConfig`]s.
    ///
    /// See the [module-level documentation](crate::runner::tool::config) for why a single tool can
    /// produce multiple configs (perf event-set splitting), how record configs are paired with stat
    /// configs, and the design decisions behind `has_analyzer` and `is_default` propagation.
    pub fn build(self) -> Result<Vec<ToolConfig>> {
        assert!(!self.options.is_empty());

        let args = Self::build_tool_args(self.tool, &self.raw_tool_args)?;

        if let [options] = &self.options[..] {
            return self.build_tool_configs(args, None, self.is_default, true, options);
        }

        let mut configs = vec![];
        for (option, part) in self.options.iter().zip(1usize..) {
            let new_configs = self.build_tool_configs(
                args.clone(),
                Some(part),
                self.is_default && part == 1,
                part == 1,
                option,
            )?;
            configs.extend(new_configs);
        }

        Ok(configs)
    }

    fn build_record_args(
        tool: Tool,
        default_args: &RawToolArgs,
        record_args: &RawToolArgs,
    ) -> Result<ToolArgs> {
        Ok(ToolArgs::Perf(
            perf::args::PerfRecordArgs::try_from_raw_tool_args(tool, &[default_args, record_args])?
                .into(),
        ))
    }

    fn build_tool_args(tool: Tool, raw_tool_args: &RawToolArgs) -> Result<ToolArgs> {
        Ok(match tool {
            Tool::Callgrind => ToolArgs::Valgrind(
                callgrind::args::CallgrindArgs::try_from_raw_tool_args(tool, &[raw_tool_args])?
                    .into(),
            ),
            Tool::Cachegrind => ToolArgs::Valgrind(
                cachegrind::args::CachegrindArgs::try_from_raw_tool_args(tool, &[raw_tool_args])?
                    .into(),
            ),
            Tool::Perf => ToolArgs::Perf(
                perf::args::PerfStatArgs::try_from_raw_tool_args(tool, &[raw_tool_args])?.into(),
            ),
            _ => ToolArgs::Valgrind(ValgrindArgs::try_from_raw_tool_args(
                tool,
                &[raw_tool_args],
            )?),
        })
    }

    fn build_tool_configs(
        &self,
        args: ToolArgs,
        part: Option<usize>,
        is_default: bool,
        has_analyzer: bool,
        options: &ToolConfigOptions,
    ) -> Result<Vec<ToolConfig>> {
        let config = ToolConfig::new(
            self.is_enabled,
            args,
            self.regression_config.clone(),
            self.flamegraph_config.clone(),
            self.entry_point.clone().unwrap_or(EntryPoint::None),
            is_default,
            self.sanitize_output,
            part,
            options.clone(),
            has_analyzer,
            self.timeout,
        );

        if self.record {
            let record_args =
                Self::build_record_args(self.tool, &self.default_args, &self.record_args)?;
            let record_config = Self::build_record_config(&config, record_args, options.clone());

            Ok(vec![config, record_config])
        } else {
            Ok(vec![config])
        }
    }

    fn build_record_config(
        stat_config: &ToolConfig,
        record_args: ToolArgs,
        options: ToolConfigOptions,
    ) -> ToolConfig {
        let mut timeout = stat_config.timeout;
        let record_options = if let ToolConfigOptions::Perf(perf_config) = &options {
            if perf_config.use_sampling {
                timeout = None;
            }

            ToolConfigOptions::Perf(PerfConfig {
                run_mode: PerfRunMode::Direct,
                use_sampling: false,
                ..perf_config.clone()
            })
        } else {
            options
        };

        ToolConfig {
            args: record_args,
            is_default: false,
            options: record_options,
            has_analyzer: false,
            timeout,
            ..stat_config.clone()
        }
    }

    /// Build the entry point
    ///
    /// The `default_entry_point` can be different for example for binary benchmarks and library
    /// benchmarks.
    fn entry_point(
        &mut self,
        default_entry_point: &EntryPoint,
        module_path: &ModulePath,
        _id: Option<&String>,
    ) {
        match self.tool {
            Tool::Callgrind => {
                let entry_point = self
                    .tool_spec
                    .as_ref()
                    .and_then(|t| t.entry_point.clone())
                    .unwrap_or_else(|| default_entry_point.clone());

                match &entry_point {
                    EntryPoint::None => {}
                    EntryPoint::Default => {
                        self.raw_tool_args
                            .extend_ignore_flag(&[format!("toggle-collect={DEFAULT_TOGGLE}")]);
                    }
                    EntryPoint::Custom(custom) => {
                        self.raw_tool_args
                            .extend_ignore_flag(&[format!("toggle-collect={custom}")]);
                    }
                }

                self.entry_point = Some(entry_point);
            }
            Tool::DHAT => {
                let entry_point = self
                    .tool_spec
                    .as_ref()
                    .and_then(|t| t.entry_point.clone())
                    .unwrap_or_else(|| default_entry_point.clone());

                if entry_point == EntryPoint::Default {
                    // DHAT does not resolve function calls the same way as callgrind does.
                    // Sometimes the benchmark function matched by the `DEFAULT_TOGGLE` gets inlined
                    // (although annotated with `#[inline(never)]`). So, in addition to the default
                    // toggle we need a fall back to the next best thing which is the function that
                    // calls the benchmark function. It is important to note that this function is
                    // constructed in a way so that it does not contain code that initializes
                    // memory. This "id"-function won't be matched literally but with a wildcard to
                    // address the problem of functions with the same body being condensed into a
                    // single function by the compiler. This also addresses rare cases in which the
                    // id function is taken from another module.
                    if let Some(file) = module_path.components().first() {
                        // This frame glob matches the standalone wrapper mod id function
                        // (`__gungraun_wrapper_id_mod`) and the constructed ones (for example
                        // `__gungraun_wrapper_id_mod_my_benchmark_id`) unambiguously.
                        for option in &mut self.options {
                            if let ToolConfigOptions::DHAT(dhat_config) = option {
                                dhat_config
                                    .frames
                                    .push(format!("{file}::*::__gungraun_wrapper_id_mod*::*"));
                            }
                        }
                    }
                }

                self.entry_point = Some(entry_point);
            }
            Tool::Cachegrind
            | Tool::Memcheck
            | Tool::Helgrind
            | Tool::DRD
            | Tool::Massif
            | Tool::BBV => {}
            Tool::Perf => {
                let entry_point = self
                    .tool_spec
                    .as_ref()
                    .and_then(|t| t.entry_point.clone())
                    .unwrap_or_else(|| default_entry_point.clone());

                self.entry_point = Some(entry_point);
            }
        }
    }

    fn flamegraph_config(&mut self) {
        if let Some(tool_spec) = &self.tool_spec {
            if let Some(flamegraph_config) = &tool_spec.flamegraph_config {
                self.flamegraph_config = flamegraph_config.clone().into();
            }
        }
    }

    fn meta_args(&mut self, meta: &Metadata) {
        if let Some(args) = &meta.args.valgrind_args {
            self.valgrind_args(args);
        }

        let raw_tool_args = match self.tool {
            Tool::Callgrind => &meta.args.callgrind_args,
            Tool::Cachegrind => &meta.args.cachegrind_args,
            Tool::DHAT => &meta.args.dhat_args,
            Tool::Memcheck => &meta.args.memcheck_args,
            Tool::Helgrind => &meta.args.helgrind_args,
            Tool::DRD => &meta.args.drd_args,
            Tool::Massif => &meta.args.massif_args,
            Tool::BBV => &meta.args.bbv_args,
            Tool::Perf => &meta.args.perf_args,
        };

        if let Some(args) = raw_tool_args {
            match self.tool {
                Tool::Perf => self.raw_tool_args.update(args),
                _ => self.raw_tool_args.update_ignore_flag(args),
            }
        }
    }

    fn apply_meta_perf_options(tool: Tool, tool_spec: &mut Option<ToolSpec>, meta: &Metadata) {
        if tool != Tool::Perf
            || (meta.args.perf_events.is_empty()
                && meta.args.perf_record.is_none()
                && meta.args.perf_record_args.is_none()
                && meta.args.perf_sampling.is_none()
                && meta.args.perf_run_mode.is_none())
        {
            return;
        }

        let tool_spec = tool_spec.get_or_insert_with(|| ToolSpec::new(Tool::Perf));

        if let api::ToolSpecOptions::Perf(perf_spec) = &mut tool_spec.options {
            if !meta.args.perf_events.is_empty() {
                perf_spec.events = Some(meta.args.perf_events.clone());
            }
            if let Some(run_mode) = meta.args.perf_run_mode {
                perf_spec.run_mode = Some(run_mode);
            }
            if let Some(record) = meta.args.perf_record {
                perf_spec.record = Some(record);
            }
            if let Some(record_args) = &meta.args.perf_record_args {
                perf_spec.record_args = record_args.clone();
            }
            if let Some(sampling) = meta.args.perf_sampling {
                perf_spec.sample_duration = match sampling {
                    PerfSampling::Disabled => None,
                    PerfSampling::Enabled(duration) => Some(duration),
                };
            }
        }
    }

    /// Resolves tool configuration from parsed specs, metadata, and defaults.
    ///
    /// This constructor applies metadata-level perf overrides first, then resolves either explicit
    /// tool specs or default options per tool kind. It returns a builder ready for [`Self::build`]
    /// which materializes the final [`ToolConfig`] list, including optional paired record configs
    /// for perf runs.
    pub fn new(
        tool: Tool,
        mut tool_spec: Option<ToolSpec>,
        is_default: bool,
        default_args: &HashMap<Tool, RawToolArgs>,
        module_path: &ModulePath,
        id: Option<&String>,
        meta: &Metadata,
        valgrind_args: &RawToolArgs,
        default_entry_point: &EntryPoint,
        perf_mode_override: Option<PerfRunMode>,
    ) -> Result<Self> {
        Self::apply_meta_perf_options(tool, &mut tool_spec, meta);

        let (options, is_enabled, record, record_args, timeout) =
            if let Some(tool_spec) = tool_spec.as_ref() {
                resolve_tool_spec_options(tool_spec, tool, perf_mode_override)?
            } else {
                resolve_default_options(tool, perf_mode_override)?
            };

        assert!(options.iter().all(|o| matches!(
            (tool, &o),
            (Tool::DHAT, ToolConfigOptions::DHAT(_))
                | (Tool::Perf, ToolConfigOptions::Perf(_))
                | (Tool::Callgrind, ToolConfigOptions::Callgrind)
                | (Tool::Cachegrind, ToolConfigOptions::Cachegrind)
                | (Tool::Memcheck, ToolConfigOptions::Memcheck)
                | (Tool::Helgrind, ToolConfigOptions::Helgrind)
                | (Tool::DRD, ToolConfigOptions::DRD)
                | (Tool::Massif, ToolConfigOptions::Massif)
                | (Tool::BBV, ToolConfigOptions::BBV),
        )));

        let mut builder = Self {
            is_enabled,
            tool_spec,
            entry_point: Option::default(),
            flamegraph_config: ToolFlamegraphConfig::None,
            is_default,
            default_args: default_args.get(&tool).cloned().unwrap_or_default(),
            raw_tool_args: RawToolArgs::default(),
            regression_config: ToolRegressionConfig::None,
            tool,
            sanitize_output: SanitizeOutput::No,
            record,
            record_args,
            options,
            timeout,
        };

        // Since the construction sequence is currently always the same, the construction of the
        // `ToolConfig` can happen here in one go instead of having a separate director for it.
        builder.default_args();
        builder.valgrind_args(valgrind_args);
        builder.entry_point(default_entry_point, module_path, id);
        builder.tool_args();
        builder.meta_args(meta);
        builder.flamegraph_config();
        builder.regression_config(meta)?;
        builder.sanitize_output();

        Ok(builder)
    }

    fn regression_config(&mut self, meta: &Metadata) -> Result<()> {
        let meta_limits = match self.tool {
            Tool::Callgrind => meta.args.callgrind_limits.clone(),
            Tool::Cachegrind => meta.args.cachegrind_limits.clone(),
            Tool::DHAT => meta.args.dhat_limits.clone(),
            Tool::Perf => meta.args.perf_limits.clone(),
            _ => None,
        };

        let mut regression_config = if let Some(tool_spec) = &self.tool_spec {
            meta_limits
                .map(Ok)
                .or_else(|| tool_spec.regression_config.clone().map(TryInto::try_into))
                .transpose()
                .map_err(|error| anyhow!("Invalid limits for {}: {error}", self.tool))?
                .unwrap_or(ToolRegressionConfig::None)
        } else {
            meta_limits.unwrap_or(ToolRegressionConfig::None)
        };

        if let Some(fail_fast) = meta.args.regression_fail_fast {
            match &mut regression_config {
                ToolRegressionConfig::Callgrind(callgrind_regression_config) => {
                    callgrind_regression_config.fail_fast = fail_fast;
                }
                ToolRegressionConfig::Cachegrind(cachegrind_regression_config) => {
                    cachegrind_regression_config.fail_fast = fail_fast;
                }
                ToolRegressionConfig::Dhat(dhat_regression_config) => {
                    dhat_regression_config.fail_fast = fail_fast;
                }
                ToolRegressionConfig::Perf(perf_regression_config) => {
                    perf_regression_config.fail_fast = fail_fast;
                }
                ToolRegressionConfig::None => {}
            }
        }

        self.regression_config = regression_config;

        Ok(())
    }

    fn sanitize_output(&mut self) {
        let apply_default = || {
            if matches!(self.tool, Tool::DHAT) {
                SanitizeOutput::Yes
            } else {
                SanitizeOutput::No
            }
        };

        self.sanitize_output = self.tool_spec.as_ref().map_or_else(apply_default, |t| {
            t.sanitize_output.unwrap_or_else(apply_default)
        });
    }

    fn tool_args(&mut self) {
        if let Some(tool_spec) = self.tool_spec.as_ref() {
            if self.tool == Tool::Perf {
                self.raw_tool_args.update(&tool_spec.raw_tool_args);
            } else {
                self.raw_tool_args
                    .update_ignore_flag(&tool_spec.raw_tool_args);
            }
        }
    }

    fn valgrind_args(&mut self, valgrind_args: &RawToolArgs) {
        if self.tool != Tool::Perf {
            self.raw_tool_args.update_ignore_flag(valgrind_args);
        }
    }

    fn default_args(&mut self) {
        self.raw_tool_args.update(&self.default_args);
    }
}

impl ToolConfigs {
    /// Creates new `ToolConfigs`.
    ///
    /// `default_entry_point` is callgrind specific and specified here because it is different for
    /// library and binary benchmarks.
    ///
    /// `default_args` should only contain command-line arguments which are different for library
    /// and binary benchmarks on a per tool basis. Usually, default arguments are part of the tool
    /// specific arguments struct for example for callgrind [`callgrind::args::CallgrindArgs`] or
    /// cachegrind [`cachegrind::args::CachegrindArgs`].
    ///
    /// `valgrind_args` are from the in-benchmark configuration: `LibraryBenchmarkConfig` or
    /// `BinaryBenchmarkConfig`
    ///
    /// # Errors
    ///
    /// This function will return an error if the configs cannot be created
    pub fn new(
        output_format: &mut OutputFormat,
        mut tool_specs: ToolSpecs,
        module_path: &ModulePath,
        id: Option<&String>,
        meta: &Metadata,
        default_tool: Tool,
        default_entry_point: &EntryPoint,
        valgrind_args: &RawToolArgs,
        default_args: &HashMap<Tool, RawToolArgs>,
        perf_mode_override: Option<PerfRunMode>,
    ) -> Result<Self> {
        let extracted_tool = tool_specs.consume(default_tool);

        output_format.update(extracted_tool.as_ref());
        let default_tool_configs = ToolConfigBuilder::new(
            default_tool,
            extracted_tool,
            true,
            default_args,
            module_path,
            id,
            meta,
            valgrind_args,
            default_entry_point,
            perf_mode_override,
        )?
        .build()?;

        // The tool selection from the command line or env args overwrites the tool selection from
        // the benchmark file. However, any tool configurations from the benchmark files are
        // preserved.
        let meta_tool_specs = if meta.args.tools.is_empty() {
            tool_specs.0
        } else {
            let mut meta_tool_specs = Vec::with_capacity(meta.args.tools.len());
            for tool in &meta.args.tools {
                if let Some(tool_spec) = tool_specs.consume(*tool) {
                    meta_tool_specs.push(tool_spec);
                } else {
                    meta_tool_specs.push(ToolSpec::new(*tool));
                }
            }
            meta_tool_specs
        };

        let mut tool_configs = Self(default_tool_configs);
        let iter = meta_tool_specs.into_iter().map(|tool_spec| {
            output_format.update(Some(&tool_spec));

            ToolConfigBuilder::new(
                tool_spec.tool,
                Some(tool_spec),
                false,
                default_args,
                module_path,
                id,
                meta,
                valgrind_args,
                default_entry_point,
                perf_mode_override,
            )?
            .build()
        });
        tool_configs.extend(iter)?;

        output_format.update_from_meta(meta);
        Ok(tool_configs)
    }

    /// Returns the common perf output configuration shared by all perf configs.
    ///
    /// All [`ToolConfigOptions::Perf`] variants in this collection are expected to have the same
    /// `alpha` and `min_pcnt_running` values.
    ///
    /// Returns `None` if there are no perf configs.
    ///
    /// # Panics
    ///
    /// If not all `alpha` and `min_pcnt_running` values have the exact same value.
    #[expect(clippy::float_cmp)]
    pub fn perf_output_config(&self) -> Option<PerfOutputConfig> {
        let mut values = self.0.iter().filter_map(|t| match &t.options {
            ToolConfigOptions::Perf(perf_config) => {
                Some((perf_config.alpha, perf_config.min_pcnt_running))
            }
            _ => None,
        });

        let (first_alpha, first_min_pcnt_running) = values.next()?;
        assert!(
            values.all(|(alpha, min_pcnt_running)| alpha == first_alpha
                && min_pcnt_running == first_min_pcnt_running),
            "all alpha and min_pcnt_running values should have the exact same value"
        );

        Some((first_alpha, first_min_pcnt_running).into())
    }

    /// Returns `true` if this `tool` is present and enabled
    pub fn has_tool_enabled(&self, tool: Tool) -> bool {
        self.0.iter().any(|t| t.tool() == tool && t.is_enabled)
    }

    /// Returns `true` if there are any [`Tool`]s enabled.
    pub fn has_tools_enabled(&self) -> bool {
        self.0.iter().any(|t| t.is_enabled)
    }

    /// Returns `true` if there are multiple tools configured and are enabled.
    pub fn has_multiple(&self) -> bool {
        self.0
            .iter()
            .filter(|config| config.is_enabled)
            .map(ToolConfig::tool)
            .collect::<HashSet<_>>()
            .len()
            > 1
    }

    /// Returns the parser and configurations for each tool to be able to analyze the outputs.
    ///
    /// Only one (the first) analyzer per tool is returned. This matters, in case of multiple
    /// [`ToolConfig`]s) (for example multiple event sets for perf)
    pub fn analyzers(&self, root_dir: &Path, output_path: &ToolOutputPath) -> Vec<Analyzer> {
        let mut seen = HashSet::new();

        self.0
            .iter()
            .filter(|t| t.is_enabled && t.has_analyzer && seen.insert(t.tool()))
            .map(|t| {
                let tool_path = output_path.to_tool_output(t.tool());
                (
                    parser_factory(t, root_dir.to_path_buf(), &tool_path),
                    tool_path,
                    t.regression_config.clone(),
                    t.flamegraph_config.clone(),
                    t.entry_point.clone(),
                )
            })
            .collect()
    }

    /// Return all [`ToolOutputPath`]s of all enabled tools (once per tool)
    pub fn output_paths(&self, output_path: &ToolOutputPath) -> Vec<ToolOutputPath> {
        let mut seen = HashSet::new();

        self.0
            .iter()
            .filter(|t| t.is_enabled && seen.insert(t.tool()))
            .map(|t| output_path.to_tool_output(t.tool()))
            .collect()
    }

    /// Extends this collection of tools with the contents of an iterator.
    pub fn extend<I>(&mut self, iter: I) -> Result<()>
    where
        I: Iterator<Item = Result<Vec<ToolConfig>>>,
    {
        for a in iter {
            self.0.extend(a?);
        }

        Ok(())
    }

    /// Run all enabled [`ToolConfig`]s and return the [`BenchmarkSummary`].
    ///
    /// Each [`ToolConfig`] is executed in isolation and sequentially so that tools do not interfere
    /// with each other: [`ToolOutputPath`]s are scoped per tool and [`ToolConfig::part`], the
    /// [`Sandbox`] is set up and reset for each run, and setup/teardown [`Assistant`]s run only
    /// for the active `ToolConfig`. Only the default tool captures output for the
    /// [`BenchmarkSummary`]; other enabled tools are run silently. Perf batch overhead is measured
    /// only when [`PerfRunMode::FixedBatch`] or [`PerfRunMode::DynamicBatch`] is in use. Disabled
    /// configs are skipped entirely.
    pub fn run<'args, F>(
        self,
        benchmark_summary: BenchmarkSummary,
        config: &Config,
        executable: &Path,
        executable_args: F,
        run_options: &RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        captured_output: Option<&CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
    ) -> Result<BenchmarkSummary>
    where
        F: Fn(&ToolConfig, Option<BenchRunMode>) -> Cow<'args, [OsString]>,
    {
        for tool_config in self.0.iter().filter(|t| t.is_enabled) {
            let output_path = tool_config.output_path(output_path);

            // We're implicitly applying the default here: In the absence of a user provided sandbox
            // we don't run the benchmarks in a sandbox.
            let sandbox = run_options
                .sandbox
                .as_ref()
                .map(|sandbox| Sandbox::setup(sandbox, &config.meta))
                .transpose()?;

            let mut process_handler = ProcessHandler::new(
                force_shutdown.clone(),
                module_path.clone(),
                run_options
                    .setup
                    .as_ref()
                    .is_some_and(Assistant::is_parallel),
                Duration::from_millis(50),
                sandbox.as_ref().and_then(Sandbox::path),
            );

            let command = ToolCommand::new(tool_config, &config.meta, &output_path, run_options)?;
            let nocapture = command.nocapture;
            let captured_output = if tool_config.is_default {
                captured_output
            } else {
                None
            };

            run_options.setup.as_ref().map_or(Ok(()), |setup| {
                process_handler.start_assistant(
                    true,
                    setup,
                    config,
                    module_path,
                    captured_output,
                    nocapture,
                )
            })?;

            if let Some(delay) = run_options.delay.as_ref() {
                if let Err(delay_error) = delay.apply(sandbox.as_ref().and_then(Sandbox::path)) {
                    if let Some(Err(_)) = process_handler.wait_for_setup() {
                        return Err(delay_error);
                    }
                }
            }

            process_handler
                .start_bench(
                    command,
                    tool_config,
                    executable,
                    &executable_args,
                    run_options,
                    &output_path,
                    module_path,
                    captured_output,
                    config.meta.args.tool_runner_dest.as_deref(),
                )
                .and_then(|()| process_handler.wait_or_shutdown(tool_config.timeout))?;

            if let ToolConfigOptions::Perf(options) = &tool_config.options {
                if matches!(
                    options.run_mode,
                    PerfRunMode::DynamicBatch | PerfRunMode::FixedBatch(_)
                ) {
                    measure_perf_overhead(
                        &config.meta,
                        tool_config,
                        executable,
                        &executable_args,
                        run_options,
                        &output_path,
                        sandbox.as_ref().and_then(|s| s.path()),
                        config.meta.args.tool_runner_dest.as_deref(),
                    )?;
                }
            }

            if let Some(teardown) = run_options.teardown.as_ref() {
                process_handler
                    .start_assistant(
                        true,
                        teardown,
                        config,
                        module_path,
                        captured_output,
                        nocapture,
                    )
                    .and_then(|()| process_handler.wait_for_teardown().transpose())?;
            }

            if let Some(sandbox) = sandbox {
                sandbox.reset()?;
            }
        }

        Ok(benchmark_summary)
    }
}

impl From<Option<FlamegraphConfig>> for ToolFlamegraphConfig {
    fn from(value: Option<FlamegraphConfig>) -> Self {
        match value {
            Some(config) => Self::Callgrind(config),
            None => Self::None,
        }
    }
}

impl From<api::ToolFlamegraphConfig> for ToolFlamegraphConfig {
    fn from(value: api::ToolFlamegraphConfig) -> Self {
        match value {
            api::ToolFlamegraphConfig::Callgrind(flamegraph_config) => {
                Self::Callgrind(flamegraph_config.into())
            }
            api::ToolFlamegraphConfig::None => Self::None,
        }
    }
}

/// Resolve the [`ToolConfigOptions`] and related state from an explicit [`ToolSpec`].
///
/// Return a tuple with `(options, is_enabled, record, record_args, timeout)`.
///
/// # Errors
///
/// Returns an error if any perf validation fails (invalid alpha, `min_pcnt_running`, sample
/// duration, zero calibration duration, ...).
#[expect(clippy::type_complexity)]
fn resolve_tool_spec_options(
    tool_spec: &ToolSpec,
    tool: Tool,
    perf_mode_override: Option<PerfRunMode>,
) -> Result<(
    Vec<ToolConfigOptions>,
    bool,
    bool,
    RawToolArgs,
    Option<Duration>,
)> {
    let is_enabled = tool_spec.enable.unwrap_or(true);

    let (options, record, record_args, timeout) = match &tool_spec.options {
        api::ToolSpecOptions::Perf(perf_spec) => {
            let alpha = resolve_perf_alpha(perf_spec.alpha).map_err(anyhow::Error::msg)?;
            let min_pcnt_running = resolve_perf_min_pcnt_running(perf_spec.min_pcnt_running)?;
            let non_zero_metrics =
                resolve_perf_non_zero_metrics(perf_spec.non_zero_metrics.as_deref());
            let run_mode = resolve_perf_run_mode(perf_spec.run_mode, perf_mode_override)?;

            let options = perf_spec.events.as_ref().map_or_else(
                || {
                    vec![ToolConfigOptions::Perf(PerfConfig {
                        alpha,
                        events: DEFAULT_PERF_EVENTS.into(),
                        non_zero_metrics: non_zero_metrics.clone(),
                        run_mode,
                        use_sampling: perf_spec.sample_duration.is_some(),
                        min_pcnt_running,
                    })]
                },
                |events| {
                    events
                        .iter()
                        .map(|e| {
                            ToolConfigOptions::Perf(PerfConfig {
                                alpha,
                                events: e.clone(),
                                non_zero_metrics: non_zero_metrics.clone(),
                                run_mode,
                                use_sampling: perf_spec.sample_duration.is_some(),
                                min_pcnt_running,
                            })
                        })
                        .collect()
                },
            );

            (
                options,
                perf_spec.record.unwrap_or_default(),
                perf_spec.record_args.clone(),
                validate_perf_sample_duration(perf_spec.sample_duration)?,
            )
        }
        api::ToolSpecOptions::Dhat(dhat_spec) => (
            vec![ToolConfigOptions::DHAT(DhatConfig {
                frames: dhat_spec
                    .frames
                    .as_ref()
                    .map_or_else(Vec::default, Clone::clone),
            })],
            false,
            RawToolArgs::default(),
            None,
        ),
        api::ToolSpecOptions::None => {
            let options = match tool {
                Tool::Callgrind => ToolConfigOptions::Callgrind,
                Tool::Cachegrind => ToolConfigOptions::Cachegrind,
                Tool::Memcheck => ToolConfigOptions::Memcheck,
                Tool::Helgrind => ToolConfigOptions::Helgrind,
                Tool::DRD => ToolConfigOptions::DRD,
                Tool::Massif => ToolConfigOptions::Massif,
                Tool::BBV => ToolConfigOptions::BBV,
                _ => unreachable!(),
            };
            (vec![options], false, RawToolArgs::default(), None)
        }
    };

    Ok((options, is_enabled, record, record_args, timeout))
}

/// Resolve the default [`ToolConfigOptions`] and related state when no [`ToolSpec`] is provided.
///
/// Return a tuple with `(options, is_enabled, record, record_args, timeout)`.
///
/// # Errors
///
/// Returns an error if the default perf run mode is [`PerfRunMode::Calibrate`] with a zero
/// duration.
#[expect(clippy::type_complexity)]
fn resolve_default_options(
    tool: Tool,
    perf_mode_override: Option<PerfRunMode>,
) -> Result<(
    Vec<ToolConfigOptions>,
    bool,
    bool,
    RawToolArgs,
    Option<Duration>,
)> {
    let options = match tool {
        Tool::DHAT => vec![ToolConfigOptions::DHAT(DhatConfig {
            frames: Vec::default(),
        })],
        Tool::Perf => vec![ToolConfigOptions::Perf(PerfConfig {
            alpha: DEFAULT_PERF_ALPHA,
            events: DEFAULT_PERF_EVENTS.into(),
            non_zero_metrics: DEFAULT_PERF_NON_ZERO_METRICS
                .iter()
                .map(ToString::to_string)
                .collect(),
            run_mode: resolve_perf_run_mode(None, perf_mode_override)?,
            use_sampling: false,
            min_pcnt_running: DEFAULT_PERF_MIN_PCNT_RUNNING,
        })],
        Tool::Callgrind => vec![ToolConfigOptions::Callgrind],
        Tool::Cachegrind => vec![ToolConfigOptions::Cachegrind],
        Tool::Memcheck => vec![ToolConfigOptions::Memcheck],
        Tool::Helgrind => vec![ToolConfigOptions::Helgrind],
        Tool::DRD => vec![ToolConfigOptions::DRD],
        Tool::Massif => vec![ToolConfigOptions::Massif],
        Tool::BBV => vec![ToolConfigOptions::BBV],
    };

    Ok((options, true, false, RawToolArgs::default(), None))
}

/// Resolves the configured perf significance level (`alpha`) to a concrete value.
///
/// Returns the provided `alpha` when it is within the valid open interval `(0.0, 1.0)`. If no value
/// is provided, this falls back to [`DEFAULT_PERF_ALPHA`].
///
/// # Errors
///
/// Returns an error if `alpha` is provided but is not strictly between `0.0` and `1.0`. This
/// includes `0.0`, `1.0`, negative values, values greater than `1.0`, and `NaN`.
pub fn resolve_perf_alpha(alpha: Option<f64>) -> std::result::Result<f64, String> {
    if let Some(alpha) = alpha {
        if alpha > 0.0 && alpha < 1.0 {
            Ok(alpha)
        } else {
            Err(format!(
                "Invalid alpha value '{alpha}': alpha is required to be 0.0 < alpha < 1.0"
            ))
        }
    } else {
        Ok(DEFAULT_PERF_ALPHA)
    }
}

/// Resolves the configured perf `min_pcnt_running` to a concrete value.
///
/// Returns the provided `min_pcnt_running` if it is valid. If no value is provided, the default
/// value [`DEFAULT_PERF_MIN_PCNT_RUNNING`] is applied.
///
/// # Errors
///
/// Returns an error if `min_pcnt_running` is provided but is not finite or is outside the inclusive
/// range `0.0..=100.0`. This includes negative values, values greater than `100.0`, and `NaN`.
fn resolve_perf_min_pcnt_running(min_pcnt_running: Option<f64>) -> Result<f64> {
    if let Some(min_pcnt_running) = min_pcnt_running {
        if min_pcnt_running.is_finite() && (0.0..=100.0).contains(&min_pcnt_running) {
            Ok(min_pcnt_running)
        } else {
            Err(anyhow!(
                "Invalid min_pcnt_running value '{min_pcnt_running}': min_pcnt_running is \
                 required to be 0.0 <= min_pcnt_running <= 100.0"
            ))
        }
    } else {
        Ok(DEFAULT_PERF_MIN_PCNT_RUNNING)
    }
}

/// Resolve the `non_zero_metrics` patterns, falling back to the default list if not set.
///
/// Empty strings are filtered out from both the default constants and user-provided values to
/// avoid accidentally matching all metrics.
fn resolve_perf_non_zero_metrics(non_zero_metrics: Option<&[String]>) -> Vec<String> {
    non_zero_metrics.as_ref().map_or_else(
        || {
            DEFAULT_PERF_NON_ZERO_METRICS
                .iter()
                .filter(|n| !n.trim().is_empty())
                .map(ToString::to_string)
                .collect()
        },
        |metrics| {
            metrics
                .iter()
                .filter(|n| !n.trim().is_empty())
                .map(ToString::to_string)
                .collect()
        },
    )
}

/// Resolve the [`PerfRunMode`] and if the `run_mode_override` is present use the override
///
/// If neither `run_mode` and `run_mode_override` are present, use the default `run_mode`.
///
/// # Errors
///
/// Returns an error if the [`PerfRunMode::Calibrate`] duration is zero
fn resolve_perf_run_mode(
    run_mode: Option<PerfRunMode>,
    run_mode_override: Option<PerfRunMode>,
) -> Result<PerfRunMode> {
    let run_mode = run_mode_override.unwrap_or_else(|| run_mode.unwrap_or_default());
    if let PerfRunMode::Calibrate(duration) = run_mode {
        if duration.is_zero() {
            return Err(anyhow!("perf run mode calibration duration was zero"));
        }
    }

    Ok(run_mode)
}

/// Validate that [`crate::api::PerfSpec::sample_duration`] is nonzero
///
/// # Errors
///
/// Returns an error if the `sample_duration` is zero
fn validate_perf_sample_duration(sample_duration: Option<Duration>) -> Result<Option<Duration>> {
    if let Some(sample_duration) = sample_duration {
        if sample_duration.is_zero() {
            return Err(anyhow!("perf sample duration was zero"));
        }
    }

    Ok(sample_duration)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use rstest::rstest;

    use super::*;
    use crate::api::ToolSpecOptions;
    use crate::fixtures::api::dhat_spec_f;
    use crate::fixtures::perf::{perf_config_f, perf_spec_f};
    use crate::fixtures::{metadata_f, tool_config_builder_f, tool_config_f, tool_spec_f};

    #[rstest]
    #[case::default(None, DEFAULT_PERF_MIN_PCNT_RUNNING)]
    #[case::zero(Some(0.0), 0.0)]
    #[case::fifty(Some(50.0), 50.0)]
    #[case::hundred(Some(100.0), 100.0)]
    fn test_resolve_perf_min_pcnt_running(#[case] input: Option<f64>, #[case] expected: f64) {
        let actual = resolve_perf_min_pcnt_running(input).unwrap();
        #[expect(clippy::float_cmp)]
        {
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_resolve_tool_spec_options_perf_events_expand_into_multiple_configs() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .enable(true)
            .options(ToolSpecOptions::Perf(
                perf_spec_f()
                    .alpha(0.2)
                    .events(vec!["cycles", "instructions"])
                    .min_pcnt_running(12.5)
                    .non_zero_metrics(vec!["a", "b"])
                    .record(true)
                    .run_mode(PerfRunMode::Direct)
                    .sample_duration(Duration::from_millis(5))
                    .fx(),
            ))
            .fx();

        let (options, is_enabled, record, record_args, timeout) =
            resolve_tool_spec_options(&spec, Tool::Perf, None).unwrap();

        assert!(is_enabled);
        assert!(record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, Some(Duration::from_millis(5)));
        assert_eq!(
            options,
            vec![
                ToolConfigOptions::Perf(
                    perf_config_f()
                        .alpha(0.2)
                        .events("cycles")
                        .min_pcnt_running(12.5)
                        .non_zero_metrics(vec!["a", "b"])
                        .run_mode(PerfRunMode::Direct)
                        .use_sampling(true)
                        .fx(),
                ),
                ToolConfigOptions::Perf(
                    perf_config_f()
                        .alpha(0.2)
                        .events("instructions")
                        .min_pcnt_running(12.5)
                        .non_zero_metrics(vec!["a", "b"])
                        .run_mode(PerfRunMode::Direct)
                        .use_sampling(true)
                        .fx(),
                ),
            ]
        );
    }

    #[test]
    fn test_resolve_tool_spec_options_perf_events_none_uses_default_events() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .options(ToolSpecOptions::Perf(perf_spec_f().fx()))
            .fx();

        let (options, is_enabled, record, record_args, timeout) =
            resolve_tool_spec_options(&spec, Tool::Perf, None).unwrap();

        assert!(is_enabled);
        assert!(!record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, None);
        assert_eq!(
            options,
            vec![ToolConfigOptions::Perf(
                perf_config_f()
                    .alpha(DEFAULT_PERF_ALPHA)
                    .events(DEFAULT_PERF_EVENTS)
                    .min_pcnt_running(DEFAULT_PERF_MIN_PCNT_RUNNING)
                    .non_zero_metrics(DEFAULT_PERF_NON_ZERO_METRICS.to_vec())
                    .run_mode(PerfRunMode::Direct)
                    .use_sampling(false)
                    .fx(),
            )]
        );
    }

    #[test]
    fn test_resolve_tool_spec_options_perf_enable_false_disables_tool() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .enable(false)
            .options(ToolSpecOptions::Perf(perf_spec_f().fx()))
            .fx();

        let (_, is_enabled, record, record_args, timeout) =
            resolve_tool_spec_options(&spec, Tool::Perf, None).unwrap();

        assert!(!is_enabled);
        assert!(!record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, None);
    }

    #[test]
    fn test_resolve_tool_spec_options_dhat_preserves_frames() {
        let spec = tool_spec_f()
            .tool(Tool::DHAT)
            .options(ToolSpecOptions::Dhat(
                dhat_spec_f().frames(vec!["a", "b"]).fx(),
            ))
            .fx();

        let (options, is_enabled, record, record_args, timeout) =
            resolve_tool_spec_options(&spec, Tool::DHAT, None).unwrap();

        assert!(is_enabled);
        assert!(!record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, None);
        assert_eq!(
            options,
            vec![ToolConfigOptions::DHAT(DhatConfig {
                frames: vec!["a".into(), "b".into()],
            })]
        );
    }

    #[test]
    fn test_resolve_tool_spec_options_none_for_callgrind_returns_callgrind() {
        let spec = tool_spec_f().tool(Tool::Callgrind).fx();

        let (options, is_enabled, record, record_args, timeout) =
            resolve_tool_spec_options(&spec, Tool::Callgrind, None).unwrap();

        assert!(is_enabled);
        assert!(!record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, None);
        assert_eq!(options, vec![ToolConfigOptions::Callgrind]);
    }

    #[test]
    fn test_resolve_tool_spec_options_invalid_alpha_returns_error() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .options(ToolSpecOptions::Perf(perf_spec_f().alpha(0.0).fx()))
            .fx();

        resolve_tool_spec_options(&spec, Tool::Perf, None).unwrap_err();
    }

    #[test]
    fn test_resolve_tool_spec_options_invalid_min_pcnt_running_returns_error() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .options(ToolSpecOptions::Perf(
                perf_spec_f().alpha(0.2).min_pcnt_running(-1.0).fx(),
            ))
            .fx();

        resolve_tool_spec_options(&spec, Tool::Perf, None).unwrap_err();
    }

    #[test]
    fn test_resolve_tool_spec_options_zero_sample_duration_returns_error() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .options(ToolSpecOptions::Perf(
                perf_spec_f()
                    .alpha(0.2)
                    .min_pcnt_running(50.0)
                    .sample_duration(Duration::ZERO)
                    .fx(),
            ))
            .fx();

        resolve_tool_spec_options(&spec, Tool::Perf, None).unwrap_err();
    }

    #[test]
    fn test_cli_perf_sampling_enables_default_duration() {
        let builder = tool_config_builder_f()
            .tool(Tool::Perf)
            .raw_command_line_args(["--perf-sampling=yes"])
            .fx();

        let configs = builder.build().unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].timeout, Some(Duration::from_secs(2)));
        let ToolConfigOptions::Perf(perf_config) = &configs[0].options else {
            unreachable!("expected perf config")
        };
        assert!(perf_config.use_sampling);
    }

    #[test]
    fn test_cli_perf_sampling_no_overrides_spec_sample_duration() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .options(ToolSpecOptions::Perf(
                perf_spec_f().sample_duration(Duration::from_secs(5)).fx(),
            ))
            .fx();
        let builder = tool_config_builder_f()
            .tool(Tool::Perf)
            .tool_spec(spec)
            .raw_command_line_args(["--perf-sampling=no"])
            .fx();

        let configs = builder.build().unwrap();
        assert_eq!(configs[0].timeout, None);
        let ToolConfigOptions::Perf(perf_config) = &configs[0].options else {
            unreachable!("expected perf config")
        };
        assert!(!perf_config.use_sampling);
    }

    #[test]
    fn test_absent_cli_perf_sampling_preserves_spec_sample_duration() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .options(ToolSpecOptions::Perf(
                perf_spec_f().sample_duration(Duration::from_secs(5)).fx(),
            ))
            .fx();
        let builder = tool_config_builder_f()
            .tool(Tool::Perf)
            .tool_spec(spec)
            .fx();

        let configs = builder.build().unwrap();
        assert_eq!(configs[0].timeout, Some(Duration::from_secs(5)));
        let ToolConfigOptions::Perf(perf_config) = &configs[0].options else {
            unreachable!("expected perf config")
        };
        assert!(perf_config.use_sampling);
    }

    #[test]
    fn test_cli_perf_sampling_zero_duration_returns_error() {
        let result = ToolConfigBuilder::new(
            Tool::Perf,
            None,
            true,
            &HashMap::default(),
            &ModulePath::new("foo::bar"),
            None,
            &metadata_f()
                .raw_command_line_args(["--perf-sampling=0"])
                .fx(),
            &RawToolArgs::default(),
            &EntryPoint::Default,
            None,
        );

        let error = result.expect_err("zero sampling duration must be rejected");
        assert!(error.to_string().contains("perf sample duration was zero"));
    }

    #[test]
    fn test_resolve_tool_spec_options_zero_calibration_duration_via_override_returns_error() {
        let spec = tool_spec_f()
            .tool(Tool::Perf)
            .options(ToolSpecOptions::Perf(perf_spec_f().fx()))
            .fx();

        resolve_tool_spec_options(
            &spec,
            Tool::Perf,
            Some(PerfRunMode::Calibrate(Duration::ZERO)),
        )
        .unwrap_err();
    }

    #[test]
    fn test_resolve_default_options_perf_returns_expected_defaults() {
        let (options, is_enabled, record, record_args, timeout) =
            resolve_default_options(Tool::Perf, None).unwrap();

        assert!(is_enabled);
        assert!(!record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, None);
        assert_eq!(
            options,
            vec![ToolConfigOptions::Perf(
                perf_config_f()
                    .alpha(DEFAULT_PERF_ALPHA)
                    .events(DEFAULT_PERF_EVENTS)
                    .min_pcnt_running(DEFAULT_PERF_MIN_PCNT_RUNNING)
                    .non_zero_metrics(DEFAULT_PERF_NON_ZERO_METRICS.to_vec())
                    .run_mode(PerfRunMode::Direct)
                    .use_sampling(false)
                    .fx(),
            )]
        );
    }

    #[test]
    fn test_resolve_default_options_callgrind_returns_expected_defaults() {
        let (options, is_enabled, record, record_args, timeout) =
            resolve_default_options(Tool::Callgrind, None).unwrap();

        assert!(is_enabled);
        assert!(!record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, None);
        assert_eq!(options, vec![ToolConfigOptions::Callgrind]);
    }

    #[test]
    fn test_resolve_default_options_dhat_returns_empty_frames() {
        let (options, is_enabled, record, record_args, timeout) =
            resolve_default_options(Tool::DHAT, None).unwrap();

        assert!(is_enabled);
        assert!(!record);
        assert_eq!(record_args, RawToolArgs::default());
        assert_eq!(timeout, None);
        assert_eq!(
            options,
            vec![ToolConfigOptions::DHAT(DhatConfig { frames: Vec::new() })]
        );
    }

    #[test]
    #[expect(clippy::float_cmp)]
    fn test_resolve_default_options_perf_override_changes_only_run_mode() {
        let base = resolve_default_options(Tool::Perf, None).unwrap();
        let overridden =
            resolve_default_options(Tool::Perf, Some(PerfRunMode::DynamicBatch)).unwrap();

        assert_eq!(base.1, overridden.1);
        assert_eq!(base.2, overridden.2);
        assert_eq!(base.3, overridden.3);
        assert_eq!(base.4, overridden.4);

        let [ToolConfigOptions::Perf(base_perf)] = &base.0[..] else {
            unreachable!("expected one perf option")
        };
        let [ToolConfigOptions::Perf(overridden_perf)] = &overridden.0[..] else {
            unreachable!("expected one perf option")
        };

        assert_eq!(base_perf.alpha, overridden_perf.alpha);
        assert_eq!(base_perf.events, overridden_perf.events);
        assert_eq!(base_perf.min_pcnt_running, overridden_perf.min_pcnt_running);
        assert_eq!(base_perf.non_zero_metrics, overridden_perf.non_zero_metrics);
        assert_eq!(base_perf.use_sampling, overridden_perf.use_sampling);
        assert_eq!(overridden_perf.run_mode, PerfRunMode::DynamicBatch);
        assert_eq!(base_perf.run_mode, PerfRunMode::Direct);
    }

    #[test]
    fn test_resolve_default_options_non_perf_ignores_perf_override() {
        let base = resolve_default_options(Tool::Callgrind, None).unwrap();
        let overridden = resolve_default_options(
            Tool::Callgrind,
            Some(PerfRunMode::Calibrate(Duration::from_secs(1))),
        )
        .unwrap();

        assert_eq!(base, overridden);
    }

    #[test]
    fn test_resolve_default_options_perf_calibrate_zero_duration_returns_error() {
        resolve_default_options(Tool::Perf, Some(PerfRunMode::Calibrate(Duration::ZERO)))
            .unwrap_err();
    }

    #[rstest]
    #[case::negative(
        Some(-0.1), "Invalid min_pcnt_running value '-0.1'".to_owned()
    )]
    #[case::barely_above_range(
        Some(100.000_000_1), "Invalid min_pcnt_running value '100.0000001'".to_owned()
    )]
    #[case::nan(Some(f64::NAN), "Invalid min_pcnt_running value 'NaN'".to_owned())]
    #[case::positive_infinity(
        Some(f64::INFINITY), "Invalid min_pcnt_running value 'inf'".to_owned()
    )]
    #[case::negative_infinity(
        Some(f64::NEG_INFINITY), "Invalid min_pcnt_running value '-inf'".to_owned()
    )]
    fn test_resolve_perf_min_pcnt_running_when_invalid_then_error(
        #[case] input: Option<f64>,
        #[case] expected: String,
    ) {
        let actual = resolve_perf_min_pcnt_running(input).unwrap_err();
        assert!(actual.to_string().contains(&expected));
    }

    #[rstest]
    #[case::callgrind(Tool::Callgrind)]
    #[case::cachegrind(Tool::Cachegrind)]
    #[case::perf(Tool::Perf)]
    #[case::memcheck(Tool::Memcheck)]
    fn test_build_tool_args_parses_empty_args_for_tool(#[case] tool: Tool) {
        let args = ToolConfigBuilder::build_tool_args(tool, &RawToolArgs::default()).unwrap();
        match tool {
            Tool::Perf => assert!(matches!(args, ToolArgs::Perf(_))),
            _ => assert!(matches!(args, ToolArgs::Valgrind(_))),
        }
    }

    #[test]
    fn test_build_record_args_parses_empty_args() {
        let args = ToolConfigBuilder::build_record_args(
            Tool::Perf,
            &RawToolArgs::default(),
            &RawToolArgs::default(),
        )
        .unwrap();
        assert!(matches!(args, ToolArgs::Perf(_)));
        assert!(args.is_perf_record());
    }

    #[rstest]
    #[case::no_part(None)]
    #[case::part_one(Some(1))]
    #[case::part_two(Some(2))]
    fn test_build_tool_configs(#[case] part: Option<usize>) {
        let builder = tool_config_builder_f().tool(Tool::Perf).fx();

        let args = ToolConfigBuilder::build_tool_args(Tool::Perf, &RawToolArgs::default()).unwrap();
        let options = ToolConfigOptions::Perf(PerfConfig::default());
        let config = builder
            .build_tool_configs(args, part, false, false, &options)
            .unwrap();

        assert_eq!(config.len(), 1);

        let first = config.first().unwrap();

        assert_eq!(first.part, part);
        assert!(!first.is_default);
        assert!(!first.has_analyzer);
    }

    #[test]
    fn test_build_record_config_inherits_all_top_level_fields() {
        let base = tool_config_f()
            .tool(Tool::Perf)
            .is_enabled(false)
            .entry_point(EntryPoint::Default)
            .sanitize_output(SanitizeOutput::Yes)
            .part(1)
            .options(ToolConfigOptions::Perf(
                perf_config_f()
                    .alpha(0.5)
                    .events("cycles,instructions")
                    .min_pcnt_running(50.0)
                    .non_zero_metrics(vec!["metric1"])
                    .run_mode(PerfRunMode::DynamicBatch)
                    .use_sampling(true)
                    .fx(),
            ))
            .timeout(Duration::from_secs(30))
            .fx();

        let record_args = ToolConfigBuilder::build_record_args(
            Tool::Perf,
            &RawToolArgs::default(),
            &RawToolArgs::default(),
        )
        .unwrap();
        let record_config = ToolConfigBuilder::build_record_config(
            &base,
            record_args.clone(),
            base.options.clone(),
        );

        assert_eq!(record_config.is_enabled, base.is_enabled);
        assert_eq!(record_config.entry_point, base.entry_point);
        assert_eq!(record_config.regression_config, base.regression_config);
        assert_eq!(record_config.flamegraph_config, base.flamegraph_config);
        assert_eq!(record_config.sanitize_output, base.sanitize_output);
        assert_eq!(record_config.part, base.part);
        assert_eq!(record_config.args, record_args);
        assert!(!record_config.is_default);
        assert!(!record_config.has_analyzer);
    }

    #[test]
    #[expect(clippy::float_cmp)]
    fn test_build_record_config_changes_only_perf_run_mode_and_sampling() {
        let base = tool_config_f()
            .tool(Tool::Perf)
            .entry_point(EntryPoint::Default)
            .sanitize_output(SanitizeOutput::No)
            .options(ToolConfigOptions::Perf(
                perf_config_f()
                    .alpha(0.5)
                    .events("cycles,instructions")
                    .min_pcnt_running(50.0)
                    .non_zero_metrics(vec!["metric1"])
                    .run_mode(PerfRunMode::DynamicBatch)
                    .use_sampling(false)
                    .fx(),
            ))
            .timeout(Duration::from_secs(30))
            .fx();

        let record_args = ToolConfigBuilder::build_record_args(
            Tool::Perf,
            &RawToolArgs::default(),
            &RawToolArgs::default(),
        )
        .unwrap();
        let record_config =
            ToolConfigBuilder::build_record_config(&base, record_args, base.options.clone());

        if let ToolConfigOptions::Perf(perf) = &record_config.options {
            assert_eq!(perf.run_mode, PerfRunMode::Direct);
            assert!(!perf.use_sampling);
            assert_eq!(perf.alpha, 0.5);
            assert_eq!(perf.events, "cycles,instructions");
            assert_eq!(perf.min_pcnt_running, 50.0);
            assert_eq!(perf.non_zero_metrics, vec!["metric1".to_owned()]);
        } else {
            panic!("Expected Perf options");
        }
        assert_eq!(record_config.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn build_record_config_clears_timeout_when_sampling() {
        let base = tool_config_f()
            .tool(Tool::Perf)
            .entry_point(EntryPoint::Default)
            .sanitize_output(SanitizeOutput::No)
            .options(ToolConfigOptions::Perf(
                perf_config_f()
                    .alpha(DEFAULT_PERF_ALPHA)
                    .events(DEFAULT_PERF_EVENTS)
                    .min_pcnt_running(DEFAULT_PERF_MIN_PCNT_RUNNING)
                    .non_zero_metrics(DEFAULT_PERF_NON_ZERO_METRICS.to_vec())
                    .run_mode(PerfRunMode::DynamicBatch)
                    .use_sampling(true)
                    .fx(),
            ))
            .timeout(Duration::from_secs(30))
            .fx();

        let record_args = ToolConfigBuilder::build_record_args(
            Tool::Perf,
            &RawToolArgs::default(),
            &RawToolArgs::default(),
        )
        .unwrap();
        let record_config =
            ToolConfigBuilder::build_record_config(&base, record_args, base.options.clone());

        assert_eq!(record_config.timeout, None);
    }

    #[test]
    fn test_build_multi_event_perf_order_is_stat_then_record() {
        let builder = tool_config_builder_f()
            .tool(Tool::Perf)
            .tool_spec(
                tool_spec_f()
                    .tool(Tool::Perf)
                    .enable(true)
                    .options(ToolSpecOptions::Perf(
                        perf_spec_f()
                            .events(vec!["cycles", "instructions"])
                            .record(true)
                            .fx(),
                    ))
                    .fx(),
            )
            .fx();

        let configs = builder.build().unwrap();
        assert_eq!(configs.len(), 4);

        assert!(matches!(configs[0].options, ToolConfigOptions::Perf(_)));
        assert!(!configs[0].is_perf_record());
        assert_eq!(configs[0].part, Some(1));
        assert!(configs[0].is_default);
        assert!(configs[0].has_analyzer);

        assert!(configs[1].is_perf_record());
        assert_eq!(configs[1].part, Some(1));
        assert!(!configs[1].is_default);
        assert!(!configs[1].has_analyzer);

        assert!(matches!(configs[2].options, ToolConfigOptions::Perf(_)));
        assert!(!configs[2].is_perf_record());
        assert_eq!(configs[2].part, Some(2));
        assert!(!configs[2].is_default);
        assert!(!configs[2].has_analyzer);

        assert!(configs[3].is_perf_record());
        assert_eq!(configs[3].part, Some(2));
        assert!(!configs[3].is_default);
        assert!(!configs[3].has_analyzer);
    }

    #[test]
    fn test_build_non_perf_tool_has_no_record() {
        let builder = tool_config_builder_f().tool(Tool::Callgrind).fx();

        let configs = builder.build().unwrap();
        assert_eq!(configs.len(), 1);
        assert!(!configs[0].is_perf_record());
    }

    #[test]
    fn test_build_malformed_tool_args_returns_error() {
        let builder = tool_config_builder_f()
            .tool(Tool::Callgrind)
            .valgrind_args(RawToolArgs::from_iter(&["--fair-sched=invalid"]))
            .fx();

        builder.build().unwrap_err();
    }
}
