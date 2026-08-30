//! Runner-side logic for the [`summary::model`][super::model] types.
//!
//! This module implements the internal behavior used to build, aggregate, compare, print, and save
//! benchmark summaries.

use std::io::stdout;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use either_or_both::EitherOrBoth;
use itertools::Itertools;
use serde_json::Value;

use crate::api::{ErrorMetric, EventKind, Tool};
use crate::error::Error;
use crate::metrics::model::{Metric, MetricKind, Metrics, MetricsSummary};
use crate::runner::args::NoCapture;
use crate::runner::common::{
    Baselines, CapturedOutput, Config, ModulePath, PerfOutputConfig, PostProcessingConfig,
};
use crate::runner::format::{
    Formatter, OutputFormat, OutputFormatKind, VerticalFormatter, print_no_capture_footer,
    print_no_config, print_regressions,
};
use crate::runner::tool::parser::ParserOutput;
use crate::runner::tool::regression::RegressionMetrics;
use crate::summary::model::{
    BenchmarkKind, BenchmarkSummary, Diffs, FlamegraphSummary, Profile, ProfileData, ProfileInfo,
    ProfilePart, ProfileTotal, Profiles, SCHEMA_VERSION, ToolMetricSummary, ToolMetrics,
    ToolRegression,
};
use crate::summary::output::{SummaryFormat, SummaryOutput};
use crate::util::{factor_diff, make_absolute, make_relative, percentage_diff};

impl BenchmarkSummary {
    /// Creates a new `BenchmarkSummary`.
    ///
    /// Paths below `project_root` are made relative, while paths outside it remain absolute.
    pub fn new(
        kind: BenchmarkKind,
        project_root: PathBuf,
        package_dir: PathBuf,
        benchmark_file: PathBuf,
        benchmark_exe: PathBuf,
        module_path: &ModulePath,
        function_name: &str,
        group: &str,
        id: Option<String>,
        details: Option<String>,
        output_dir: PathBuf,
        baselines: Baselines,
    ) -> Self {
        Self {
            version: SCHEMA_VERSION.to_owned(),
            kind,
            benchmark_file: make_relative(
                &project_root,
                make_absolute(&project_root, benchmark_file),
            ),
            benchmark_exe: make_relative(
                &project_root,
                make_absolute(&project_root, benchmark_exe),
            ),
            module_path: module_path.to_string(),
            function_name: function_name.to_owned(),
            group: group.to_owned(),
            id,
            details,
            profiles: Profiles::default(),
            output_dir: make_relative(&project_root, make_absolute(&project_root, output_dir)),
            package_dir: make_relative(&project_root, make_absolute(&project_root, package_dir)),
            project_root,
            baselines,
        }
    }

    fn print_default(
        &self,
        config: &Config,
        output_format: &OutputFormat,
        mut captured_output: CapturedOutput,
        post_processing_config: &PostProcessingConfig,
    ) -> Result<()> {
        post_processing_config.header.print();

        if self.profiles.is_empty() {
            print_no_config();
            return Ok(());
        }

        if config.meta.args.load_baseline.is_none() {
            match config.meta.args.nocapture {
                NoCapture::True => {
                    captured_output.dump()?;
                }
                NoCapture::False => {}
                NoCapture::Stderr => {
                    captured_output.dump_stderr()?;
                }
                NoCapture::Stdout => {
                    captured_output.dump_stdout()?;
                }
            }

            print_no_capture_footer(config.meta.args.nocapture);
        }

        let has_multiple = self.profiles.has_multiple();
        let baselines = &self.baselines;
        for (index, profile) in self.profiles.iter().enumerate() {
            let is_default = index == 0;
            let mut formatter = VerticalFormatter::new(output_format.clone());
            if !output_format.show_only_comparison
                && (has_multiple || profile.tool != Tool::Callgrind)
            {
                formatter.format_tool_headline(profile.tool);
                formatter.print_buffer();
            }

            formatter.print(
                profile.tool,
                config,
                baselines,
                &profile.summaries,
                is_default,
                post_processing_config.perf_config.as_ref(),
            );
            print_regressions(&profile.summaries.total.regressions);
        }

        Ok(())
    }

    // Print the json `value` to stdout
    fn print_json(value: &Value, pretty: bool) -> Result<()> {
        let stdout = stdout().lock();
        if pretty {
            serde_json::to_writer_pretty(stdout, &value)
                .with_context(|| "Failed to print json to stdout")
                .map(|()| println!())
        } else {
            serde_json::to_writer(stdout, &value)
                .with_context(|| "Failed to print json to stdout")
                .map(|()| println!())
        }
    }

    /// Save the summary json `value` as a file into the benchmark directory
    fn save_summary(value: &Value, output: &SummaryOutput) -> Result<()> {
        let file = output.create()?;

        let pretty = matches!(output.format, SummaryFormat::PrettyJson);
        let result = if pretty {
            serde_json::to_writer_pretty(file, &value)
        } else {
            serde_json::to_writer(file, &value)
        };

        result
            .with_context(|| format!("Failed to write summary to file: {}", output.path.display()))
    }

    /// If the summary is json output, print it and eventually safe it, if configured to do so
    pub(crate) fn print_and_save(
        &self,
        config: &Config,
        output_format: &OutputFormat,
        captured_output: CapturedOutput,
        post_processing_config: &PostProcessingConfig,
        summary_output: Option<&SummaryOutput>,
    ) -> Result<()> {
        match output_format.kind {
            OutputFormatKind::Default => self
                .print_default(
                    config,
                    output_format,
                    captured_output,
                    post_processing_config,
                )
                .and_then(|()| {
                    if let Some(output) = summary_output {
                        serde_json::to_value(self)
                            .with_context(|| "Failed to serialize summary to json")
                            .and_then(|value| Self::save_summary(&value, output))
                    } else {
                        Ok(())
                    }
                }),
            OutputFormatKind::Json | OutputFormatKind::PrettyJson => serde_json::to_value(self)
                .with_context(|| "Failed to serialize summary to json")
                .and_then(|value| {
                    let pretty = matches!(output_format.kind, OutputFormatKind::PrettyJson);
                    Self::print_json(&value, pretty).and_then(|()| {
                        if let Some(output) = summary_output {
                            Self::save_summary(&value, output)
                        } else {
                            Ok(())
                        }
                    })
                }),
        }
    }

    /// Check if this `BenchmarkSummary` has recorded any performance regressions
    ///
    /// # Errors
    ///
    /// If a regressions is present and are configured to be `fail_fast` an error is returned
    pub fn check_regression(&self, fail_fast: bool) -> Result<()> {
        if self.profiles.is_regressed() && fail_fast {
            return Err(Error::RegressionError(true).into());
        }

        Ok(())
    }

    /// Returns `true` if any [`Profile`] has regressed.
    pub fn is_regressed(&self) -> bool {
        self.profiles.is_regressed()
    }

    /// Compare this summary with another and print the result of the comparison
    pub fn compare_and_print(
        &self,
        id: &str,
        other: &Self,
        output_format: &OutputFormat,
        perf_processing_config: Option<&PerfOutputConfig>,
    ) {
        let mut summaries = vec![];

        for profile in self.profiles.iter() {
            if let Some(other_profile) = other.profiles.iter().find(|s| s.tool == profile.tool)
                && let Some(summary) = ToolMetricSummary::from_self_and_other(
                    &profile.summaries.total.summary,
                    &other_profile.summaries.total.summary,
                )
            {
                summaries.push((profile.tool, summary));
            }
        }

        // There really should always be at least one summary. Also, if the default tool is massif
        // or bbv which (currently) don't have an actual summary.
        if !summaries.is_empty() {
            VerticalFormatter::new(output_format.clone()).print_comparison(
                &self.function_name,
                id,
                self.details.as_deref(),
                summaries,
                perf_processing_config,
            );
        }
    }
}

impl Diffs {
    /// Creates a new `Diffs` calculating the percentage and factor from the `new` and `old`
    /// metrics.
    pub fn new(new: Metric, old: Metric) -> Self {
        Self {
            diff_pct: percentage_diff(new, old),
            factor: factor_diff(new, old),
        }
    }
}

impl FlamegraphSummary {
    /// Creates a new `FlamegraphSummary`.
    pub fn new(event_kind: EventKind) -> Self {
        Self { event_kind }
    }
}

impl Profile {
    /// Returns `true` if one of the summaries has regressed.
    pub fn is_regressed(&self) -> bool {
        self.summaries.is_regressed()
    }
}

impl ProfileData {
    /// Returns `true` if the profile data is empty.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Returns `true` if any `ProfilePart` has a non-empty [`MetricsSummary`]
    pub fn has_data<T>(&self, tool: T) -> bool
    where
        T: Into<Option<Tool>>,
    {
        match tool.into() {
            Some(tool) => !self.parts.iter().all(|p| match (tool, &p.metrics_summary) {
                (Tool::Memcheck, ToolMetricSummary::Memcheck(metrics_summary))
                | (Tool::Helgrind, ToolMetricSummary::Helgrind(metrics_summary))
                | (Tool::DRD, ToolMetricSummary::DRD(metrics_summary)) => {
                    metrics_summary.is_empty()
                }
                (Tool::DHAT, ToolMetricSummary::Dhat(metrics_summary)) => {
                    metrics_summary.is_empty()
                }
                (Tool::Callgrind, ToolMetricSummary::Callgrind(metrics_summary)) => {
                    metrics_summary.is_empty()
                }
                (Tool::Cachegrind, ToolMetricSummary::Cachegrind(metrics_summary)) => {
                    metrics_summary.is_empty()
                }
                (Tool::Perf, ToolMetricSummary::Perf(metrics_summary)) => {
                    metrics_summary.is_empty()
                }
                (Tool::Massif | Tool::BBV, ToolMetricSummary::None) => true,
                (..) => {
                    debug_assert!(false, "tool and metric summary variants must match");
                    false
                }
            }),
            None => !self.parts.iter().all(|p| p.metrics_summary.is_empty()),
        }
    }

    /// Returns `true` if the total and only the total has regressed.
    pub fn is_regressed(&self) -> bool {
        self.total.is_regressed()
    }

    /// Returns `true` if there are multiple parts.
    pub fn has_multiple(&self) -> bool {
        self.parts.len() > 1
    }

    /// Used internally to group the output by pid, then by parts and then by threads
    ///
    /// The grouping simplifies the zipping of the new and old parser output later.
    ///
    /// A simplified example. `(pid, part, thread)`
    ///
    /// ```rust,ignore
    /// let parsed: Vec<(i32, u64, usize)> = [
    ///     (10, 1, 1),
    ///     (10, 1, 2),
    ///     (20, 1, 1)
    /// ];
    ///
    /// let grouped = group(parsed);
    /// assert_eq!(grouped,
    /// vec![
    ///     vec![
    ///         vec![
    ///             (10, 1, 1),
    ///             (10, 1, 2)
    ///         ]
    ///     ],
    ///     vec![
    ///         vec![
    ///             (20, 1, 1)
    ///         ]
    ///     ]
    /// ])
    /// ```
    fn group(parsed: impl Iterator<Item = ParserOutput>) -> Vec<Vec<Vec<ParserOutput>>> {
        let mut grouped = vec![];
        let mut cur_pid = 0_i32;
        let mut cur_part = 0;

        for element in parsed {
            let pid = element.header.pid;
            let part = element.header.part.unwrap_or(0);

            if pid != cur_pid {
                grouped.push(vec![vec![element]]);
                cur_pid = pid;
                cur_part = part;
            } else if part != cur_part {
                let parts = grouped.last_mut().unwrap();
                parts.push(vec![element]);
                cur_part = part;
            } else {
                let parts = grouped.last_mut().unwrap();
                let threads = parts.last_mut().unwrap();
                threads.push(element);
            }
        }
        grouped
    }

    /// Creates a new `ProfileData` from parser output grouped by pid, part, and thread.
    ///
    /// A running `total` is built while the grouped summaries are processed. For tools with a
    /// meaningful aggregate summary, `total` starts as the corresponding empty summary and is
    /// updated for every produced [`ProfilePart`].
    ///
    /// Perf is the exception: it does not currently define a synthetic total summary across parts.
    /// Its metadata-bearing metrics are kept on the individual parts only, so the running total is
    /// initialized as [`ToolMetricSummary::None`].
    ///
    /// The summaries created from the new parser outputs and the old parser outputs are grouped by
    /// pid (subprocesses recorded with `--trace-children`), then by part (for example cause by a
    /// `--dump-every-bb=xxx`) and then by thread (caused by `--separate-threads`). Since each of
    /// these components can differ between the new and the old parser output, this complicates the
    /// creation of each [`ProfileData`]. We can't just zip the new and old parser output directly
    /// to get (as far as possible) correct comparisons between the new and old costs. To remedy the
    /// possibly incorrect comparisons, there is always a total created.
    ///
    /// In a first step the parsed outputs are grouped in vectors by pid, then by parts and then by
    /// threads. This solution is not very efficient but there are not too many parsed outputs to be
    /// expected. 100 at most and maybe 2-10 on average, so the tradeoff between performance and
    /// clearer structure of this method looks reasonable.
    ///
    /// Secondly and finally, the groups are processed and summarized in a total.
    pub fn new(parsed_new: Vec<ParserOutput>, parsed_old: Option<Vec<ParserOutput>>) -> Self {
        let mut total = match parsed_new
            .first()
            .expect("At least 1 parsed result should be present")
            .metrics
        {
            // Perf currently has no synthetic total summary; only per-part summaries are kept.
            ToolMetrics::None | ToolMetrics::Perf(_) => ToolMetricSummary::None,
            ToolMetrics::Dhat(_) => ToolMetricSummary::Dhat(MetricsSummary::default()),
            ToolMetrics::Memcheck(_) => ToolMetricSummary::Memcheck(MetricsSummary::default()),
            ToolMetrics::Helgrind(_) => ToolMetricSummary::Helgrind(MetricsSummary::default()),
            ToolMetrics::DRD(_) => ToolMetricSummary::DRD(MetricsSummary::default()),
            ToolMetrics::Callgrind(_) => ToolMetricSummary::Callgrind(MetricsSummary::default()),
            ToolMetrics::Cachegrind(_) => ToolMetricSummary::Cachegrind(MetricsSummary::default()),
        };

        let grouped_new = Self::group(parsed_new.into_iter());
        let grouped_old = Self::group(parsed_old.into_iter().flatten());

        let mut summaries = vec![];

        for e_pids in grouped_new.into_iter().zip_longest(grouped_old) {
            match e_pids {
                itertools::EitherOrBoth::Both(new_parts, old_parts) => {
                    for e_parts in new_parts.into_iter().zip_longest(old_parts) {
                        match e_parts {
                            itertools::EitherOrBoth::Both(new_threads, old_threads) => {
                                for e_threads in new_threads.into_iter().zip_longest(old_threads) {
                                    let summary = match e_threads {
                                        itertools::EitherOrBoth::Both(new, old) => {
                                            ProfilePart::from_new_and_old(new, old)
                                        }
                                        itertools::EitherOrBoth::Left(new) => {
                                            ProfilePart::from_new(new)
                                        }
                                        itertools::EitherOrBoth::Right(old) => {
                                            ProfilePart::from_old(old)
                                        }
                                    };
                                    total.add_mut(&summary.metrics_summary);
                                    summaries.push(summary);
                                }
                            }
                            itertools::EitherOrBoth::Left(left) => {
                                for new in left {
                                    let summary = ProfilePart::from_new(new);
                                    total.add_mut(&summary.metrics_summary);
                                    summaries.push(summary);
                                }
                            }
                            itertools::EitherOrBoth::Right(right) => {
                                for old in right {
                                    let summary = ProfilePart::from_old(old);
                                    total.add_mut(&summary.metrics_summary);
                                    summaries.push(summary);
                                }
                            }
                        }
                    }
                }
                itertools::EitherOrBoth::Left(left) => {
                    for new in left.into_iter().flatten() {
                        let summary = ProfilePart::from_new(new);
                        total.add_mut(&summary.metrics_summary);
                        summaries.push(summary);
                    }
                }
                itertools::EitherOrBoth::Right(right) => {
                    for old in right.into_iter().flatten() {
                        let summary = ProfilePart::from_old(old);
                        total.add_mut(&summary.metrics_summary);
                        summaries.push(summary);
                    }
                }
            }
        }

        Self {
            parts: summaries,
            total: ProfileTotal {
                summary: total,
                regressions: vec![],
            },
        }
    }
}

impl From<ParserOutput> for ProfileInfo {
    fn from(value: ParserOutput) -> Self {
        Self {
            command: value.header.command,
            pid: value.header.pid,
            parent_pid: value.header.parent_pid,
            details: (!value.details.is_empty()).then(|| value.details.join("\n")),
            part: value.header.part,
            thread: value.header.thread,
        }
    }
}

impl ProfilePart {
    /// Returns `true` if an error checking valgrind tool (like `Memcheck`) has errors detected.
    pub fn new_has_errors(&self) -> bool {
        match &self.metrics_summary {
            ToolMetricSummary::None
            | ToolMetricSummary::Dhat(_)
            | ToolMetricSummary::Cachegrind(_)
            | ToolMetricSummary::Callgrind(_)
            | ToolMetricSummary::Perf(_) => false,
            ToolMetricSummary::Memcheck(metrics)
            | ToolMetricSummary::Helgrind(metrics)
            | ToolMetricSummary::DRD(metrics) => metrics
                .diff_by_kind(&ErrorMetric::Errors)
                .is_some_and(|e| e.metrics.has_left_and(|new| new > Metric::Int(0))),
        }
    }

    /// Creates a new part from `new` parser output.
    pub fn from_new(new: ParserOutput) -> Self {
        let metrics_summary = ToolMetricSummary::from_new_metrics(&new.metrics);
        Self {
            details: EitherOrBoth::Left(new.into()),
            metrics_summary,
        }
    }

    /// Creates a new part from `old` parser output.
    pub fn from_old(old: ParserOutput) -> Self {
        let metrics_summary = ToolMetricSummary::from_old_metrics(&old.metrics);
        Self {
            details: EitherOrBoth::Right(old.into()),
            metrics_summary,
        }
    }

    /// Creates a new `ProfilePart` from new and old [`ParserOutput`].
    ///
    /// # Panics
    ///
    /// Treat new and old with different metric kinds as programming error and not as runtime error
    /// and panic
    pub fn from_new_and_old(new: ParserOutput, old: ParserOutput) -> Self {
        let metrics_summary =
            ToolMetricSummary::try_from_new_and_old_metrics(&new.metrics, &old.metrics)
                .expect("New and old metrics should have a matching kind");
        Self {
            details: EitherOrBoth::Both(new.into(), old.into()),
            metrics_summary,
        }
    }
}

impl ProfileTotal {
    /// Returns `true` if there are any regressions.
    pub fn is_regressed(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Returns `true` if there is a summary.
    pub fn is_some(&self) -> bool {
        self.summary.is_some()
    }

    /// Returns `true` if there is no summary.
    pub fn is_none(&self) -> bool {
        self.summary.is_none()
    }
}

impl Profiles {
    /// Creates a new collection of [`Profile`]s.
    pub fn new(values: Vec<Profile>) -> Self {
        Self(values)
    }

    /// Returns `true` when no tool produced a profile for the benchmark.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return an iterator over the contained [`Profile`]s
    pub fn iter(&self) -> impl Iterator<Item = &Profile> {
        self.0.iter()
    }

    /// Add a new [`Profile`] to this collection
    pub fn push(&mut self, summary: Profile) {
        self.0.push(summary);
    }

    /// Returns `true` if any [`Profile`] has regressed.
    pub fn is_regressed(&self) -> bool {
        self.iter().any(Profile::is_regressed)
    }

    /// Returns `true` if there are multiple [`Profile`]s.
    pub fn has_multiple(&self) -> bool {
        self.0.len() > 1
    }
}

impl IntoIterator for Profiles {
    type Item = Profile;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl ToolMetrics {
    /// Associates `metrics` with the error-reporting `tool` that produced them.
    pub fn from_error_metric(tool: Tool, metrics: Metrics<ErrorMetric>) -> Self {
        match tool {
            Tool::Memcheck => Self::Memcheck(metrics),
            Tool::Helgrind => Self::Helgrind(metrics),
            Tool::DRD => Self::DRD(metrics),
            _ => unreachable!("{tool} does not report error metrics"),
        }
    }
}

impl ToolMetricSummary {
    /// Returns `true` if this summary is a typed variant with no metric diffs present.
    ///
    /// `ToolMetricSummary::None` is not considered empty and returns `false`.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::None => false,
            Self::Memcheck(summary) | Self::Helgrind(summary) | Self::DRD(summary) => {
                summary.is_empty()
            }
            Self::Dhat(summary) => summary.is_empty(),
            Self::Callgrind(summary) => summary.is_empty(),
            Self::Cachegrind(summary) => summary.is_empty(),
            Self::Perf(summary) => summary.is_empty(),
        }
    }

    /// Sum up another summary metrics to these metrics
    pub fn add_mut(&mut self, other: &Self) {
        match (self, other) {
            (Self::Memcheck(this), Self::Memcheck(other))
            | (Self::Helgrind(this), Self::Helgrind(other))
            | (Self::DRD(this), Self::DRD(other)) => {
                this.add(other);
            }
            (Self::Dhat(this), Self::Dhat(other)) => {
                this.add(other);
            }
            (Self::Callgrind(this), Self::Callgrind(other)) => {
                this.add(other);
            }
            (Self::Cachegrind(this), Self::Cachegrind(other)) => {
                this.add(other);
            }
            (Self::Perf(this), Self::Perf(other)) => {
                this.add(other);
            }
            _ => {}
        }
    }

    /// Creates a new summary from `new` [`ToolMetrics`].
    pub fn from_new_metrics(metrics: &ToolMetrics) -> Self {
        match metrics {
            ToolMetrics::None => Self::None,
            ToolMetrics::Dhat(metrics) => {
                Self::Dhat(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
            }
            ToolMetrics::Memcheck(metrics) => {
                Self::Memcheck(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
            }
            ToolMetrics::Helgrind(metrics) => {
                Self::Helgrind(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
            }
            ToolMetrics::DRD(metrics) => {
                Self::DRD(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
            }
            ToolMetrics::Callgrind(metrics) => {
                Self::Callgrind(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
            }
            ToolMetrics::Cachegrind(metrics) => {
                Self::Cachegrind(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
            }
            ToolMetrics::Perf(metrics) => {
                Self::Perf(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
            }
        }
    }

    /// Creates a new summary from `old` [`ToolMetrics`].
    pub fn from_old_metrics(metrics: &ToolMetrics) -> Self {
        match metrics {
            ToolMetrics::None => Self::None,
            ToolMetrics::Dhat(metrics) => {
                Self::Dhat(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
            }
            ToolMetrics::Memcheck(metrics) => {
                Self::Memcheck(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
            }
            ToolMetrics::Helgrind(metrics) => {
                Self::Helgrind(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
            }
            ToolMetrics::DRD(metrics) => {
                Self::DRD(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
            }
            ToolMetrics::Callgrind(metrics) => {
                Self::Callgrind(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
            }
            ToolMetrics::Cachegrind(metrics) => {
                Self::Cachegrind(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
            }
            ToolMetrics::Perf(metrics) => {
                Self::Perf(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
            }
        }
    }

    /// Creates a new summary from `new` and `old` [`ToolMetrics`].
    ///
    /// Returns the `ToolMetricSummary` if the `MetricsKind` are the same kind, else return with.
    /// error
    pub fn try_from_new_and_old_metrics(
        new_metrics: &ToolMetrics,
        old_metrics: &ToolMetrics,
    ) -> Result<Self> {
        match (new_metrics, old_metrics) {
            (ToolMetrics::None, ToolMetrics::None) => Ok(Self::None),
            (ToolMetrics::Dhat(new_metrics), ToolMetrics::Dhat(old_metrics)) => Ok(Self::Dhat(
                MetricsSummary::new(EitherOrBoth::Both(new_metrics.clone(), old_metrics.clone())),
            )),
            (ToolMetrics::Memcheck(new_metrics), ToolMetrics::Memcheck(old_metrics)) => {
                Ok(Self::Memcheck(MetricsSummary::new(EitherOrBoth::Both(
                    new_metrics.clone(),
                    old_metrics.clone(),
                ))))
            }
            (ToolMetrics::Helgrind(new_metrics), ToolMetrics::Helgrind(old_metrics)) => {
                Ok(Self::Helgrind(MetricsSummary::new(EitherOrBoth::Both(
                    new_metrics.clone(),
                    old_metrics.clone(),
                ))))
            }
            (ToolMetrics::DRD(new_metrics), ToolMetrics::DRD(old_metrics)) => Ok(Self::DRD(
                MetricsSummary::new(EitherOrBoth::Both(new_metrics.clone(), old_metrics.clone())),
            )),
            (ToolMetrics::Callgrind(new_metrics), ToolMetrics::Callgrind(old_metrics)) => {
                Ok(Self::Callgrind(MetricsSummary::new(EitherOrBoth::Both(
                    new_metrics.clone(),
                    old_metrics.clone(),
                ))))
            }
            (ToolMetrics::Cachegrind(new_metrics), ToolMetrics::Cachegrind(old_metrics)) => {
                Ok(Self::Cachegrind(MetricsSummary::new(EitherOrBoth::Both(
                    new_metrics.clone(),
                    old_metrics.clone(),
                ))))
            }
            (ToolMetrics::Perf(new_metrics), ToolMetrics::Perf(old_metrics)) => Ok(Self::Perf(
                MetricsSummary::new(EitherOrBoth::Both(new_metrics.clone(), old_metrics.clone())),
            )),
            _ => Err(anyhow!("Cannot create summary from incompatible costs")),
        }
    }

    /// Creates a new summary from this summary and another [`ToolMetricSummary`].
    pub fn from_self_and_other(this: &Self, other: &Self) -> Option<Self> {
        match (this, other) {
            (Self::None, Self::None) => Some(Self::None),
            (Self::Callgrind(metrics), Self::Callgrind(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::Callgrind(MetricsSummary::new(EitherOrBoth::Both(
                        new, other_new,
                    ))))
                } else {
                    None
                }
            }
            (Self::Memcheck(metrics), Self::Memcheck(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::Memcheck(MetricsSummary::new(EitherOrBoth::Both(
                        new, other_new,
                    ))))
                } else {
                    None
                }
            }
            (Self::Helgrind(metrics), Self::Helgrind(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::Helgrind(MetricsSummary::new(EitherOrBoth::Both(
                        new, other_new,
                    ))))
                } else {
                    None
                }
            }
            (Self::DRD(metrics), Self::DRD(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::DRD(MetricsSummary::new(EitherOrBoth::Both(
                        new, other_new,
                    ))))
                } else {
                    None
                }
            }
            (Self::Dhat(metrics), Self::Dhat(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::Dhat(MetricsSummary::new(EitherOrBoth::Both(
                        new, other_new,
                    ))))
                } else {
                    None
                }
            }
            (Self::Cachegrind(metrics), Self::Cachegrind(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::Cachegrind(MetricsSummary::new(EitherOrBoth::Both(
                        new, other_new,
                    ))))
                } else {
                    None
                }
            }
            (Self::Perf(metrics), Self::Perf(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::Perf(MetricsSummary::new(EitherOrBoth::Both(
                        new, other_new,
                    ))))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns `true` if this summary has metrics.
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    /// Returns `true` if this summary doesn't have metrics (currently massif, bbv).
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToolRegression {
    /// Creates a new `ToolRegression`.
    pub fn with<T>(apply: fn(T) -> MetricKind, regressions: RegressionMetrics<T>) -> Self {
        match regressions {
            RegressionMetrics::Soft(metric, display, unit, new, old, diff_pct, limit) => {
                Self::Soft {
                    metric: apply(metric),
                    display,
                    unit,
                    new,
                    old,
                    diff_pct,
                    limit,
                }
            }
            RegressionMetrics::Hard(metric, display, unit, new, diff, limit) => Self::Hard {
                metric: apply(metric),
                display,
                unit,
                new,
                diff,
                limit,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::api::PerfMetric;
    use crate::runner::common::ModulePath;
    use crate::units::Unit;

    #[test]
    fn test_when_serializing_new_summary_then_paths_under_project_root_are_relative() {
        let summary = BenchmarkSummary::new(
            BenchmarkKind::LibraryBenchmark,
            PathBuf::from("/project"),
            PathBuf::from("/project/crates/example"),
            PathBuf::from("crates/example/benches/benchmark.rs"),
            PathBuf::from("target/release/benchmark"),
            &ModulePath::new("example::benchmark"),
            "benchmark",
            "example",
            None,
            None,
            PathBuf::from("/project/target/gungraun/example"),
            (None, None),
        );

        let value = serde_json::to_value(summary).unwrap();

        assert_eq!(value["project_root"], "/project");
        assert_eq!(value["output_dir"], "target/gungraun/example");
        assert_eq!(value["package_dir"], "crates/example");
        assert_eq!(
            value["benchmark_file"],
            "crates/example/benches/benchmark.rs"
        );
        assert_eq!(value["benchmark_exe"], "target/release/benchmark");
        assert!(value.get("summary_output").is_none());
    }

    #[test]
    fn test_when_serializing_new_summary_then_paths_outside_project_root_remain_absolute() {
        let summary = BenchmarkSummary::new(
            BenchmarkKind::LibraryBenchmark,
            PathBuf::from("/project"),
            PathBuf::from("/tmp/package"),
            PathBuf::from("/tmp/benchmark.rs"),
            PathBuf::from("/tmp/benchmark"),
            &ModulePath::new("example::benchmark"),
            "benchmark",
            "example",
            None,
            None,
            PathBuf::from("/tmp/gungraun"),
            (None, None),
        );

        let value = serde_json::to_value(summary).unwrap();

        assert_eq!(value["output_dir"], "/tmp/gungraun");
        assert_eq!(value["package_dir"], "/tmp/package");
        assert_eq!(value["benchmark_file"], "/tmp/benchmark.rs");
        assert_eq!(value["benchmark_exe"], "/tmp/benchmark");
        for key in [
            "output_dir",
            "package_dir",
            "benchmark_file",
            "benchmark_exe",
        ] {
            assert!(!value[key].as_str().unwrap().starts_with(".."));
        }
    }

    #[test]
    fn test_tool_regression_with_preserves_unit() {
        let regression = ToolRegression::with(
            MetricKind::Perf,
            RegressionMetrics::Soft(
                PerfMetric("foo".to_owned()),
                Some("foo [s]".to_owned()),
                Some(Unit::Seconds),
                Metric::Int(5),
                Metric::Int(4),
                25.0,
                10.0,
            ),
        );

        assert_eq!(
            regression,
            ToolRegression::Soft {
                metric: MetricKind::Perf(PerfMetric("foo".to_owned())),
                display: Some("foo [s]".to_owned()),
                unit: Some(Unit::Seconds),
                new: Metric::Int(5),
                old: Metric::Int(4),
                diff_pct: 25.0,
                limit: 10.0,
            }
        );
    }
}
