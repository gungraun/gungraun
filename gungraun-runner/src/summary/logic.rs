//! Runner-side logic for the [`summary::model`][super::model] types.
//!
//! This module implements the internal behavior used to build, aggregate, compare, print, and save
//! benchmark summaries.

use std::fmt::Display;
use std::fs::File;
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use either_or_both::EitherOrBoth;
use glob::glob;
use itertools::Itertools;
use serde_json::Value;

use crate::api::{ErrorMetric, EventKind, Tool};
use crate::error::Error;
use crate::metrics::model::{Metric, MetricKind, MetricsSummary};
use crate::runner::args::NoCapture;
use crate::runner::common::{Baselines, CapturedOutput, Config, ModulePath};
use crate::runner::format::{
    Formatter, Header, OutputFormat, OutputFormatKind, VerticalFormatter, print_no_capture_footer,
    print_regressions,
};
use crate::runner::tool::parser::ParserOutput;
use crate::runner::tool::regression::RegressionMetrics;
use crate::summary::model::{
    BaselineName, BenchmarkKind, BenchmarkSummary, Diffs, FlamegraphSummary, Profile, ProfileData,
    ProfileInfo, ProfilePart, ProfileTotal, Profiles, SCHEMA_VERSION, SummaryFormat, SummaryOutput,
    ToolMetricSummary, ToolMetrics, ToolRegression,
};
use crate::util::{factor_diff, make_absolute, percentage_diff};

impl Display for BaselineName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BaselineName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for char in s.chars() {
            if !(char.is_ascii_alphanumeric() || char == '_') {
                return Err(format!(
                    "A baseline name can only consist of ascii characters which are alphanumeric \
                     or '_' but found: '{char}'"
                ));
            }
        }
        Ok(Self(s.to_owned()))
    }
}

impl BenchmarkSummary {
    /// Creates a new `BenchmarkSummary`.
    ///
    /// Relative paths are made absolute with the `project_root` as base directory.
    pub fn new(
        kind: BenchmarkKind,
        project_root: PathBuf,
        package_dir: PathBuf,
        benchmark_file: PathBuf,
        benchmark_exe: PathBuf,
        module_path: &ModulePath,
        function_name: &str,
        id: Option<String>,
        details: Option<String>,
        output: Option<SummaryOutput>,
        baselines: Baselines,
    ) -> Self {
        Self {
            version: SCHEMA_VERSION.to_owned(),
            kind,
            benchmark_file: make_absolute(&project_root, benchmark_file),
            benchmark_exe: make_absolute(&project_root, benchmark_exe),
            module_path: module_path.to_string(),
            function_name: function_name.to_owned(),
            id,
            details,
            profiles: Profiles::default(),
            summary_output: output,
            project_root,
            package_dir,
            baselines,
        }
    }

    fn print_default(
        &self,
        config: &Config,
        header: &Header,
        output_format: &OutputFormat,
        mut captured_output: CapturedOutput,
        alpha: Option<f64>,
    ) -> Result<()> {
        header.print();

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
                alpha,
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
    pub fn print_and_save(
        &self,
        config: &Config,
        header: &Header,
        output_format: &OutputFormat,
        captured_output: CapturedOutput,
        alpha: Option<f64>,
    ) -> Result<()> {
        match output_format.kind {
            OutputFormatKind::Default => self
                .print_default(config, header, output_format, captured_output, alpha)
                .and_then(|()| {
                    if let Some(output) = &self.summary_output {
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
                        if let Some(output) = &self.summary_output {
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
        alpha: Option<f64>,
    ) {
        let mut summaries = vec![];

        for profile in self.profiles.iter() {
            if let Some(other_profile) = other.profiles.iter().find(|s| s.tool == profile.tool) {
                if let Some(summary) = ToolMetricSummary::from_self_and_other(
                    &profile.summaries.total.summary,
                    &other_profile.summaries.total.summary,
                ) {
                    summaries.push((profile.tool, summary));
                }
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
                alpha,
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
        Self {
            event_kind,
            regular_path: Option::default(),
            base_path: Option::default(),
            diff_path: Option::default(),
        }
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
            ToolMetrics::ErrorTool(_) => ToolMetricSummary::ErrorTool(MetricsSummary::default()),
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
            path: value.path,
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
            ToolMetricSummary::ErrorTool(metrics) => metrics
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

impl SummaryOutput {
    /// Creates a new `SummaryOutput` with `dir` as base dir and an extension fitting the.
    /// [`SummaryFormat`]
    pub fn new(format: SummaryFormat, dir: &Path) -> Self {
        Self {
            format,
            path: Self::path(dir),
        }
    }

    /// Initialize this `SummaryOutput` removing old summary files
    pub fn init(&self) -> Result<()> {
        for entry in glob(self.path.with_extension("*").to_string_lossy().as_ref())
            .expect("Glob pattern should be valid")
        {
            let entry = entry?;
            std::fs::remove_file(entry.as_path()).with_context(|| {
                format!(
                    "Failed removing summary file '{}'",
                    entry.as_path().display()
                )
            })?;
        }

        Ok(())
    }

    /// Try to create an empty summary file returning the [`File`] object
    pub fn create(&self) -> Result<File> {
        File::create(&self.path).with_context(|| "Failed to create json summary file")
    }

    /// Returns the path to this summary file.
    pub fn path(dir: &Path) -> PathBuf {
        dir.join("summary.json")
    }
}

impl ToolMetricSummary {
    /// Sum up another summary metrics to these metrics
    pub fn add_mut(&mut self, other: &Self) {
        match (self, other) {
            (Self::ErrorTool(this), Self::ErrorTool(other)) => {
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
            ToolMetrics::ErrorTool(metrics) => {
                Self::ErrorTool(MetricsSummary::new(EitherOrBoth::Left(metrics.clone())))
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
            ToolMetrics::ErrorTool(metrics) => {
                Self::ErrorTool(MetricsSummary::new(EitherOrBoth::Right(metrics.clone())))
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
            (ToolMetrics::ErrorTool(new_metrics), ToolMetrics::ErrorTool(old_metrics)) => {
                Ok(Self::ErrorTool(MetricsSummary::new(EitherOrBoth::Both(
                    new_metrics.clone(),
                    old_metrics.clone(),
                ))))
            }
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
            (Self::ErrorTool(metrics), Self::ErrorTool(other_metrics)) => {
                let costs = metrics.extract_costs();
                let other_costs = other_metrics.extract_costs();

                if let (
                    EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _),
                    EitherOrBoth::Left(other_new) | EitherOrBoth::Both(other_new, _),
                ) = (costs, other_costs)
                {
                    Some(Self::ErrorTool(MetricsSummary::new(EitherOrBoth::Both(
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
    use crate::units::Unit;

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
