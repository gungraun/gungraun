//! The format of Gungraun terminal output
//!
//! All direct print statements should be part of this module and there should be no `println!` or
//! similar statement in any other module of the runner.
use std::borrow::Cow;
use std::fmt::{Display, Write};
use std::path::PathBuf;
use std::time::Duration;

use approx::abs_diff_eq;
use colored::{Color, ColoredString, Colorize};
use either_or_both::EitherOrBoth;
use indexmap::{IndexSet, indexset};

use super::args::NoCapture;
use super::bin_bench::BinBench;
use super::common::{Baselines, BenchmarkSummaries, Config, ModulePath, PerfOutputConfig};
use super::lib_bench::LibBench;
use super::meta::Metadata;
use crate::api::{
    self, CachegrindMetric, CachegrindMetrics, CallgrindMetrics, DhatMetric, DhatMetrics,
    ErrorMetric, EventKind, Tool, ToolOutputFormat, ToolSpec,
};
use crate::metrics::logic::MetricValue;
use crate::metrics::model::{
    AnnotatedMetric, Metric, MetricKind, MetricsDiff, MetricsSummary, PerfQualities,
};
use crate::stats::runner::DiffStats;
use crate::summary::model::{Diffs, ProfileData, ProfileInfo, ToolMetricSummary, ToolRegression};
use crate::units::Unit;
use crate::util::{
    make_relative, to_string_signed_short, to_string_unsigned_short, truncate_str_utf8,
};

const DEFAULT_FILTER_OUTPUT: bool = false;
const DEFAULT_SHOW_GRID: bool = false;
const DEFAULT_SHOW_INTERMEDIATE: bool = false;
const DEFAULT_SHOW_ONLY_COMPARISON: bool = false;
const DEFAULT_TRUNCATE_DESCRIPTION: Option<usize> = Some(50);
/// The width in bytes of the difference (and factor)
pub const DIFF_WIDTH: usize = 9;
/// The width in bytes of the FIELD as in `  FIELD: METRIC | METRIC (DIFF_PCT) [FACTOR]`
pub const FIELD_WIDTH: usize = 36;
/// The `DIFF_WIDTH` - the length of the unit
pub const FLOAT_WIDTH: usize = DIFF_WIDTH - 1;
/// The width in bytes of the "left" side of the separator `|`
pub const LEFT_WIDTH: usize = METRIC_WIDTH + FIELD_WIDTH;
#[expect(clippy::doc_link_with_quotes)]
/// The maximum line width
///
/// indent + left + "|" + metric width + " " + "(" + percentage + ")" + " " + "[" + factor + "]"
pub const MAX_WIDTH: usize = 2 + LEFT_WIDTH + 1 + METRIC_WIDTH + 2 * 11;
/// The width in bytes of the metric
pub const METRIC_WIDTH: usize = 20;
/// The string used to signal that a value is not available
pub const NOT_AVAILABLE: &str = "N/A";
/// Used to indicate that there is no difference between the `new` and `old` metric
pub const NO_CHANGE: &str = "No change";
/// The string used in the difference when there is no difference to show
pub const UNKNOWN: &str = "*********";
/// The string used to signal that the difference is in the tolerance margin
pub const WITHIN_TOLERANCE: &str = "Tolerance";

#[derive(Debug)]
enum IndentKind {
    Normal,
    ToolHeadline,
    ToolSubHeadline,
}

/// The libtest-compatible list format selected via `--format` together with `--list`
///
/// Mirrors the relevant subset of libtest's `--format` values. Gungraun only varies the trailing
/// summary line of `--list`: per-benchmark lines are identical in both formats so they remain
/// parsable by `cargo nextest` and similar libtest-format consumers.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ListFormat {
    /// Print per-benchmark lines followed by a blank line and the `0 tests, N benchmarks` summary
    #[default]
    Pretty,
    /// Print only per-benchmark lines, suppressing the blank line and the summary
    Terse,
}

/// The kind of the output format can be either json or the default terminal output
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormatKind {
    /// The default terminal output
    #[default]
    Default,
    /// Json terminal output
    Json,
    /// Pretty json terminal output
    PrettyJson,
}

/// The first line and header of a binary benchmark run
///
/// For example `module::path id: some args`
#[derive(Debug)]
pub struct BinaryBenchmarkHeader(Header);

/// The header of the comparison between two different benchmarks
#[derive(Debug)]
pub struct ComparisonHeader {
    /// The details to print in addition or instead of the metrics
    pub details: Option<String>,
    /// The function name of the other benchmark
    pub function_name: String,
    /// The id of the other benchmark.
    pub id: String,
    /// The indentation depending on the output format with grid or without
    pub indent: String,
}

/// The first line and header of a benchmark run
#[derive(Debug, PartialEq, Clone, Eq)]
pub struct Header {
    description: Option<String>,
    id: Option<String>,
    module_path: String,
}

/// The first line and header of a library benchmark run
///
/// For example `module::path id: some args`
#[derive(Debug)]
pub struct LibraryBenchmarkHeader(Header);

/// The `OutputFormat` of the Gungraun terminal output
#[expect(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct OutputFormat {
    /// The Cachegrind metrics to show
    pub cachegrind: IndexSet<CachegrindMetric>,
    /// The Callgrind metrics to show
    pub callgrind: IndexSet<EventKind>,
    /// The DHAT metrics to show
    pub dhat: IndexSet<DhatMetric>,
    /// The DRD error metrics to show
    pub drd: IndexSet<ErrorMetric>,
    /// Whether Perf control messages are removed from captured benchmark output.
    ///
    /// This is enabled when Perf is the effective default tool so that its `Events enabled` and
    /// `Events disabled` messages do not clutter benchmark stdout or stderr.
    pub filter_output: bool,
    /// The Helgrind error metrics to show
    pub helgrind: IndexSet<ErrorMetric>,
    /// The [`OutputFormatKind`]
    pub kind: OutputFormatKind,
    /// The Memcheck error metrics to show
    pub memcheck: IndexSet<ErrorMetric>,
    /// Show a grid instead of blank spaces
    pub show_grid: bool,
    /// Show intermediate metrics output or just the total
    pub show_intermediate: bool,
    /// Show only the comparison between different benchmarks when `compare_by_id` is given
    pub show_only_comparison: bool,
    /// Don't show differences within the tolerance margin
    pub tolerance: Option<f64>,
    /// If present truncate the description to this amount of bytes
    pub truncate_description: Option<usize>,
}

/// The formatter of the benchmark summary printed after all benchmarks
#[derive(Debug, Clone)]
pub struct SummaryFormatter {
    /// The [`OutputFormatKind`]
    pub output_format_kind: OutputFormatKind,
}

/// The main implementation of the [`Formatter`] trait
#[derive(Debug, Clone)]
pub struct VerticalFormatter {
    buffer: String,
    indent: String,
    indent_sub_header: String,
    indent_tool_header: String,
    output_format: OutputFormat,
}

/// The trait for the formatter of Gungraun terminal output and metrics
pub trait Formatter {
    /// Clear the buffer
    fn clear(&mut self);

    /// Format the output the whole [`ProfileData`]
    fn format(
        &mut self,
        tool: Tool,
        config: &Config,
        baselines: &Baselines,
        data: &ProfileData,
        is_default_tool: bool,
        perf_config: Option<&PerfOutputConfig>,
    );

    /// Format a line in free form as is
    fn format_line(&mut self, line: &str);

    /// Format the output of a single [`ToolMetricSummary`] of a tool
    fn format_single(
        &mut self,
        baselines: &Baselines,
        info: Option<&EitherOrBoth<ProfileInfo>>,
        metrics_summary: &ToolMetricSummary,
        is_default_tool: bool,
        perf_config: Option<&PerfOutputConfig>,
    );

    /// Print the formatted output of the whole [`ProfileData`]
    fn print(
        &mut self,
        tool: Tool,
        config: &Config,
        baselines: &Baselines,
        data: &ProfileData,
        is_default_tool: bool,
        perf_config: Option<&PerfOutputConfig>,
    ) where
        Self: std::fmt::Display,
    {
        self.format(tool, config, baselines, data, is_default_tool, perf_config);

        print!("{self}");

        self.clear();
    }

    /// Print a comparison between two different benchmarks
    fn print_comparison(
        &mut self,
        function_name: &str,
        id: &str,
        details: Option<&str>,
        summaries: Vec<(Tool, ToolMetricSummary)>,
        perf_config: Option<&PerfOutputConfig>,
    );
}

impl BinaryBenchmarkHeader {
    /// Creates a new `BinaryBenchmarkHeader`.
    pub fn new(meta: &Metadata, bin_bench: &BinBench) -> Self {
        let path = make_relative(&meta.project_root, &bin_bench.command.path);

        let consts_display = bin_bench
            .consts_display
            .as_ref()
            .map(|consts| format!("<{consts}>"));

        let description = if bin_bench.command.args.is_empty() {
            format!(
                "{}({}) -> {}",
                consts_display.as_ref().map_or("", String::as_str),
                bin_bench.display.as_ref().map_or("", String::as_str),
                path.display(),
            )
        } else {
            let command_args: Vec<String> = bin_bench
                .command
                .args
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();
            let command_args = shlex::try_join(command_args.iter().map(String::as_str)).unwrap();

            format!(
                "{}({}) -> {} {}",
                consts_display.as_ref().map_or("", String::as_str),
                bin_bench.display.as_ref().map_or("", String::as_str),
                path.display(),
                command_args
            )
        };

        Self(Header::new(
            &bin_bench.module_path,
            bin_bench.id.clone(),
            Some(description),
            &bin_bench.output_format,
        ))
    }

    /// Convert the header to a flamegraph title
    pub fn to_title(&self) -> String {
        self.0.to_title()
    }

    /// Returns the description part of the header.
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
    }
}

impl Display for BinaryBenchmarkHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ComparisonHeader {
    /// Creates a new `ComparisonHeader`.
    pub fn new<T, U, V>(
        function_name: T,
        id: U,
        details: Option<V>,
        output_format: &OutputFormat,
    ) -> Self
    where
        T: Into<String>,
        U: Into<String>,
        V: Into<String>,
    {
        Self {
            function_name: function_name.into(),
            id: id.into(),
            details: details.map(Into::into),
            indent: if output_format.show_grid {
                "|-".bright_black().to_string()
            } else {
                "  ".to_owned()
            },
        }
    }

    /// Print the header
    pub fn print(&self) {
        println!("{self}");
    }
}

impl Display for ComparisonHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{} {} {}",
            self.indent,
            "Comparison with".yellow().bold(),
            self.function_name.green(),
            self.id.cyan()
        )?;

        if let Some(details) = &self.details {
            write!(f, ":{}", details.blue().bold())?;
        }

        Ok(())
    }
}

impl Header {
    /// Creates a new `Header`.
    pub fn new<T>(
        module_path: &ModulePath,
        id: T,
        description: Option<String>,
        output_format: &OutputFormat,
    ) -> Self
    where
        T: Into<Option<String>>,
    {
        let truncated = description
            .map(|d| truncate_description(&d, output_format.truncate_description).to_string());

        Self {
            module_path: module_path.to_string(),
            id: id.into(),
            description: truncated,
        }
    }

    /// Creates a new `Header` with a description.
    pub fn without_description<T>(module_path: &ModulePath, id: T) -> Self
    where
        T: Into<Option<String>>,
    {
        Self {
            module_path: module_path.to_string(),
            id: id.into(),
            description: None,
        }
    }

    /// Print the header
    pub fn print(&self) {
        println!("{self}");
    }

    /// Convert the header into a flamegraph title
    pub fn to_title(&self) -> String {
        let mut output = String::new();

        write!(output, "{}", self.module_path).unwrap();
        if let Some(id) = &self.id {
            match &self.description {
                Some(description) if !description.is_empty() => {
                    write!(output, " {id}:{description}").unwrap();
                }
                _ => {
                    write!(output, " {id}").unwrap();
                }
            }
        }
        output
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.module_path.green()))?;

        if let Some(id) = &self.id {
            match &self.description {
                Some(description) if !description.is_empty() => {
                    f.write_fmt(format_args!(" {}:{}", id.cyan(), description.bold().blue()))?;
                }
                _ if !id.is_empty() => {
                    f.write_fmt(format_args!(" {}", id.cyan()))?;
                }
                _ => {}
            }
        } else if let Some(description) = &self.description {
            if !description.is_empty() {
                f.write_fmt(format_args!(" :{}", description.bold().blue()))?;
            }
        } else {
            // do nothing
        }

        Ok(())
    }
}

impl From<BinaryBenchmarkHeader> for Header {
    fn from(value: BinaryBenchmarkHeader) -> Self {
        value.0
    }
}

impl From<LibraryBenchmarkHeader> for Header {
    fn from(value: LibraryBenchmarkHeader) -> Self {
        value.0
    }
}

impl LibraryBenchmarkHeader {
    /// Creates a new `LibraryBenchmarkHeader`.
    pub fn new(lib_bench: &LibBench) -> Self {
        let description = match (
            lib_bench.display.as_ref(),
            lib_bench.consts_display.as_ref(),
        ) {
            (None, None) => None,
            (None, Some(consts)) => Some(format!("<{consts}>")),
            (Some(args), None) => Some(format!("({args})")),
            (Some(args), Some(consts)) => Some(format!("<{consts}>({args})")),
        };

        let header = Header::new(
            &lib_bench.module_path,
            lib_bench.id.clone(),
            description,
            &lib_bench.output_format,
        );

        Self(header)
    }

    /// Convert the header into a flamegraph title
    pub fn to_title(&self) -> String {
        self.0.to_title()
    }

    /// Returns the description part of the header if present.
    pub fn description(&self) -> Option<String> {
        self.0.description.clone()
    }
}

impl Display for LibraryBenchmarkHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl OutputFormat {
    /// Returns `true` if the `OutputFormat` is the default format.
    pub fn is_default(&self) -> bool {
        self.kind == OutputFormatKind::Default
    }

    /// Returns `true` if the `OutputFormat` is json.
    pub fn is_json(&self) -> bool {
        self.kind == OutputFormatKind::Json || self.kind == OutputFormatKind::PrettyJson
    }

    /// Updates the output format from the [`ToolSpec`] if present.
    pub fn update(&mut self, tool_spec: Option<&ToolSpec>) {
        if let Some(tool_spec) = tool_spec
            && let Some(format) = &tool_spec.output_format
        {
            match format {
                ToolOutputFormat::Callgrind(metrics) => {
                    self.callgrind = metrics.iter().fold(IndexSet::new(), |mut acc, m| {
                        acc.extend(IndexSet::from(*m));
                        acc
                    });
                }
                ToolOutputFormat::Cachegrind(metrics) => {
                    self.cachegrind = metrics.iter().fold(IndexSet::new(), |mut acc, m| {
                        acc.extend(IndexSet::from(*m));
                        acc
                    });
                }
                ToolOutputFormat::DHAT(metrics) => {
                    self.dhat = metrics.iter().copied().collect();
                }
                ToolOutputFormat::Memcheck(metrics) => {
                    self.memcheck = metrics.iter().copied().collect();
                }
                ToolOutputFormat::Helgrind(metrics) => {
                    self.helgrind = metrics.iter().copied().collect();
                }
                ToolOutputFormat::DRD(metrics) => {
                    self.drd = metrics.iter().copied().collect();
                }
                ToolOutputFormat::None => {}
            }
        }
    }

    /// Updates the output format with data from command-line arguments in [`Metadata`].
    pub fn update_from_meta(&mut self, meta: &Metadata) {
        if let Some(metrics) = &meta.args.cachegrind_metrics {
            self.cachegrind.clone_from(metrics);
        }
        if let Some(metrics) = &meta.args.callgrind_metrics {
            self.callgrind.clone_from(metrics);
        }
        if let Some(metrics) = &meta.args.dhat_metrics {
            self.dhat.clone_from(metrics);
        }
        if let Some(metrics) = &meta.args.drd_metrics {
            self.drd.clone_from(metrics);
        }
        if let Some(metrics) = &meta.args.helgrind_metrics {
            self.helgrind.clone_from(metrics);
        }
        if let Some(metrics) = &meta.args.memcheck_metrics {
            self.memcheck.clone_from(metrics);
        }

        if meta.args.tolerance.is_some() {
            self.tolerance = meta.args.tolerance;
        }

        if let Some(show_only_comparison) = meta.args.show_only_comparison {
            self.show_only_comparison = show_only_comparison;
        }

        if let Some(show_grid) = meta.args.show_grid {
            self.show_grid = show_grid;
        }

        if let Some(truncate_description) = meta.args.truncate_description {
            self.truncate_description = truncate_description.into();
        }

        if let Some(show_intermediate) = meta.args.show_intermediate {
            self.show_intermediate = show_intermediate;
        }
    }
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self {
            show_only_comparison: DEFAULT_SHOW_ONLY_COMPARISON,
            kind: OutputFormatKind::default(),
            truncate_description: DEFAULT_TRUNCATE_DESCRIPTION,
            show_intermediate: DEFAULT_SHOW_INTERMEDIATE,
            show_grid: DEFAULT_SHOW_GRID,
            tolerance: None,
            callgrind: IndexSet::from(CallgrindMetrics::Default),
            cachegrind: IndexSet::from(CachegrindMetrics::Default),
            dhat: IndexSet::from(DhatMetrics::Default),
            memcheck: indexset![
                ErrorMetric::Errors,
                ErrorMetric::Contexts,
                ErrorMetric::SuppressedErrors,
                ErrorMetric::SuppressedContexts,
            ],
            helgrind: indexset![
                ErrorMetric::Errors,
                ErrorMetric::Contexts,
                ErrorMetric::SuppressedErrors,
                ErrorMetric::SuppressedContexts,
            ],
            drd: indexset![
                ErrorMetric::Errors,
                ErrorMetric::Contexts,
                ErrorMetric::SuppressedErrors,
                ErrorMetric::SuppressedContexts,
            ],
            filter_output: DEFAULT_FILTER_OUTPUT,
        }
    }
}

impl From<api::OutputFormat> for OutputFormat {
    fn from(value: api::OutputFormat) -> Self {
        Self {
            kind: OutputFormatKind::Default,
            truncate_description: value
                .truncate_description
                .unwrap_or(DEFAULT_TRUNCATE_DESCRIPTION),
            show_intermediate: value.show_intermediate.unwrap_or(DEFAULT_SHOW_INTERMEDIATE),
            show_grid: value.show_grid.unwrap_or(DEFAULT_SHOW_GRID),
            tolerance: value.tolerance,
            ..Default::default()
        }
    }
}

impl SummaryFormatter {
    /// Creates a new `SummaryFormatter`.
    pub fn new(output_format_kind: OutputFormatKind) -> Self {
        Self { output_format_kind }
    }

    /// Print the summary
    pub fn print(&self, summaries: &BenchmarkSummaries) {
        if self.output_format_kind == OutputFormatKind::Default {
            let total_benchmarks = summaries.num_benchmarks();
            let total_time = to_string_unsigned_short(
                summaries.total_time.unwrap_or(Duration::ZERO).as_secs_f64(),
            );
            let num_filtered = summaries.num_filtered;

            if summaries.is_regressed() {
                println!("\nRegressions:\n");
                let mut num_regressed = 0;
                for summary in summaries.summaries.iter().filter(|p| p.is_regressed()) {
                    if let Some(id) = &summary.id {
                        println!("  {} {}:", summary.module_path.green(), id.cyan());
                    } else {
                        println!("  {}:", summary.module_path.green());
                    }
                    for (tool, regression) in summary
                        .profiles
                        .iter()
                        .flat_map(|t| t.summaries.total.regressions.iter().map(|r| (t.tool, r)))
                    {
                        match regression {
                            ToolRegression::Soft {
                                metric,
                                display,
                                unit,
                                new,
                                old,
                                diff_pct,
                                limit,
                            } => {
                                let old = format_metric_with_unit(old, unit.as_ref());
                                let new = format_metric_with_unit(new, unit.as_ref());
                                println!(
                                    "    {}: {} ({} -> {}): {:>6}{} exceeds limit of {:>6}{}",
                                    tool.capitalized(),
                                    regression_display_name(metric, display.as_deref()),
                                    old,
                                    new.bold(),
                                    to_string_signed_short(*diff_pct).bright_red().bold(),
                                    "%".bright_red().bold(),
                                    to_string_signed_short(*limit).bright_black(),
                                    "%".bright_black()
                                );
                            }
                            ToolRegression::Hard {
                                metric,
                                display,
                                unit,
                                new,
                                diff,
                                limit,
                            } => {
                                let new = format_metric_with_unit(new, unit.as_ref());
                                let diff = format_metric_with_unit(diff, unit.as_ref());
                                let limit = format_metric_with_unit(limit, unit.as_ref());
                                println!(
                                    "    {0}: {1} ({2}): {2} exceeds limit of {3} by {4}",
                                    tool.capitalized(),
                                    regression_display_name(metric, display.as_deref()),
                                    new.bold(),
                                    limit.bright_black(),
                                    diff.bright_red().bold()
                                );
                            }
                        }
                    }

                    num_regressed += 1;
                }

                let num_not_regressed = total_benchmarks - num_regressed;
                println!(
                    "\nGungraun result: {}. {num_not_regressed} without regressions; \
                     {num_regressed} regressed; {num_filtered} filtered; {total_benchmarks} \
                     benchmarks finished in {total_time:>6}s",
                    "Regressed".bright_red().bold(),
                );
            } else {
                println!(
                    "\nGungraun result: {}. {total_benchmarks} without regressions; 0 regressed; \
                     {num_filtered} filtered; {total_benchmarks} benchmarks finished in \
                     {total_time:>6}s",
                    "Ok".green().bold(),
                );
            }
        }
    }
}

impl VerticalFormatter {
    /// Creates a new `VerticalFormatter` (the default format).
    pub fn new(output_format: OutputFormat) -> Self {
        if output_format.show_grid {
            Self {
                buffer: String::new(),
                indent: "| ".bright_black().to_string(),
                indent_sub_header: "|-".bright_black().to_string(),
                indent_tool_header: "|=".bright_black().to_string(),
                output_format,
            }
        } else {
            Self {
                buffer: String::new(),
                indent: "  ".bright_black().to_string(),
                indent_sub_header: "  ".bright_black().to_string(),
                indent_tool_header: "  ".bright_black().to_string(),
                output_format,
            }
        }
    }

    /// Print the internal buffer as is and clear it afterwards
    pub fn print_buffer(&mut self) {
        print!("{}", self.buffer);
        self.clear();
    }

    /// Write the indentation depending on the chosen output format and [`IndentKind`]
    fn write_indent(&mut self, kind: &IndentKind) {
        match kind {
            IndentKind::Normal => write!(self, "{}", self.indent.clone()).unwrap(),
            IndentKind::ToolHeadline => {
                write!(self, "{}", self.indent_tool_header.clone()).unwrap();
            }
            IndentKind::ToolSubHeadline => {
                write!(self, "{}", self.indent_sub_header.clone()).unwrap();
            }
        }
    }

    fn write_field<VL, VR, F>(
        &mut self,
        field: F,
        values: EitherOrBoth<VL, VR>,
        color: Option<Color>,
        left_align: bool,
        unit: Option<&Unit>,
    ) where
        F: Into<ColoredString>,
        VL: Into<ColoredString>,
        VR: Into<ColoredString>,
    {
        self.write_indent(&IndentKind::Normal);

        let mut field = field.into();

        #[expect(clippy::assigning_clones)]
        let colored = values.bimap(
            |left| {
                let mut left = left.into();
                left.input = left.trim().to_owned();
                match color {
                    Some(color) => left.color(color).bold(),
                    None => left.bold(),
                }
            },
            |right| {
                let mut right = right.into();
                right.input = right.trim().to_owned();
                match color {
                    Some(color) => right.color(color),
                    None => right,
                }
            },
        );

        if let Some(unit) = unit {
            field.input = format!("{} [{}]:", field.trim_end_matches(':'), unit);
        }

        match colored {
            EitherOrBoth::Left(left) => {
                let is_multiline = left.input.len() + field.input.len() + 2 > LEFT_WIDTH;

                if left_align && field.input.len() + 2 > LEFT_WIDTH {
                    writeln!(self, "{field}").unwrap();
                    self.write_indent(&IndentKind::Normal);
                    writeln!(self, "  {left}").unwrap();
                } else if left_align {
                    writeln!(self, "{field} {left}").unwrap();
                } else if is_multiline {
                    writeln!(self, "{field}").unwrap();
                    self.write_indent(&IndentKind::Normal);
                    writeln!(
                        self,
                        "{}{left}",
                        " ".repeat(LEFT_WIDTH.saturating_sub(left.input.len()))
                    )
                    .unwrap();
                } else {
                    writeln!(
                        self,
                        "{field}{}{left}",
                        " ".repeat(
                            LEFT_WIDTH
                                .saturating_sub(left.input.len())
                                .saturating_sub(field.input.len())
                        )
                    )
                    .unwrap();
                }
            }
            EitherOrBoth::Right(right) => {
                let is_multiline = field.input.len() + 2 > LEFT_WIDTH;
                if is_multiline {
                    writeln!(self, "{field}").unwrap();
                    self.write_indent(&IndentKind::Normal);
                    writeln!(self, "{}|{right}", " ".repeat(LEFT_WIDTH)).unwrap();
                } else {
                    writeln!(
                        self,
                        "{field}{}|{right}",
                        " ".repeat(LEFT_WIDTH.saturating_sub(field.input.len()))
                    )
                    .unwrap();
                }
            }
            EitherOrBoth::Both(left, right) => {
                let is_multiline = left.input.len() + field.input.len() + 2 > LEFT_WIDTH;

                if is_multiline {
                    writeln!(self, "{field}").unwrap();
                    self.write_indent(&IndentKind::Normal);

                    if left_align {
                        writeln!(
                            self,
                            "{}{left}{}|{right}",
                            " ".repeat(FIELD_WIDTH),
                            " ".repeat(
                                LEFT_WIDTH
                                    .saturating_sub(FIELD_WIDTH)
                                    .saturating_sub(left.input.len())
                            )
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            self,
                            "{}{left}|{right}",
                            " ".repeat(LEFT_WIDTH.saturating_sub(left.input.len()))
                        )
                        .unwrap();
                    }
                } else if left_align {
                    let padding = LEFT_WIDTH
                        .saturating_sub(field.input.len())
                        .saturating_sub(left.input.len());
                    let left_padding = FIELD_WIDTH
                        .saturating_sub(field.input.len())
                        .saturating_add(2)
                        .min(padding);
                    let right_padding = padding.saturating_sub(left_padding);

                    writeln!(
                        self,
                        "{field}{}{left}{}|{right}",
                        " ".repeat(left_padding),
                        " ".repeat(right_padding)
                    )
                    .unwrap();
                } else {
                    writeln!(
                        self,
                        "{field}{}{left}|{right}",
                        " ".repeat(
                            LEFT_WIDTH
                                .saturating_sub(field.input.len())
                                .saturating_sub(left.input.len())
                        ),
                    )
                    .unwrap();
                }
            }
        }
    }

    fn write_metric<V>(&mut self, field: &str, metrics: &EitherOrBoth<&V>, diffs: Option<Diffs>)
    where
        V: MetricValue + PartialEq,
    {
        match metrics {
            EitherOrBoth::Left(new) => {
                let right = format!(
                    "{NOT_AVAILABLE:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    UNKNOWN.bright_black()
                );
                self.write_field(
                    field,
                    EitherOrBoth::Both(new.to_string_without_unit().as_str(), right.as_str()),
                    None,
                    false,
                    new.unit(),
                );
            }
            EitherOrBoth::Right(old) => {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    old.to_string_without_unit(),
                    UNKNOWN.bright_black()
                );
                self.write_field(
                    field,
                    EitherOrBoth::Both(NOT_AVAILABLE, right.as_str()),
                    None,
                    false,
                    old.unit(),
                );
            }
            EitherOrBoth::Both(new, old) if new == old => {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    old.to_string_without_unit(),
                    NO_CHANGE.bright_black()
                );
                self.write_field(
                    field,
                    EitherOrBoth::Both(new.to_string_without_unit().as_str(), right.as_str()),
                    None,
                    false,
                    merge_units(new.unit(), old.unit()).as_deref(),
                );
            }
            EitherOrBoth::Both(new, old)
                if self.output_format.tolerance.is_some_and(|tolerance| {
                    diffs
                        .map(|diffs| diffs.diff_pct)
                        .expect("A difference should be present")
                        .abs()
                        <= tolerance.abs()
                }) =>
            {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    old.to_string_without_unit(),
                    WITHIN_TOLERANCE.bright_black()
                );
                self.write_field(
                    field,
                    EitherOrBoth::Both(new.to_string_without_unit().as_str(), right.as_str()),
                    None,
                    false,
                    merge_units(new.unit(), old.unit()).as_deref(),
                );
            }
            EitherOrBoth::Both(new, old) if diffs.is_none() => {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    old.to_string_without_unit(),
                    UNKNOWN.bright_black()
                );
                self.write_field(
                    field,
                    EitherOrBoth::Both(new.to_string_without_unit().as_str(), right.as_str()),
                    None,
                    false,
                    merge_units(new.unit(), old.unit()).as_deref(),
                );
            }
            EitherOrBoth::Both(new, old) => {
                let diffs = diffs.expect("checked that diffs are present");
                let pct_string = format_float(diffs.diff_pct, '%');
                let factor_string = format_float(diffs.factor, 'x');

                let right = format!(
                    "{:<METRIC_WIDTH$} ({pct_string:^DIFF_WIDTH$}) [{factor_string:^DIFF_WIDTH$}]",
                    old.to_string_without_unit()
                );
                self.write_field(
                    field,
                    EitherOrBoth::Both(new.to_string_without_unit().as_str(), right.as_str()),
                    None,
                    false,
                    merge_units(new.unit(), old.unit()).as_deref(),
                );
            }
        }
    }

    fn write_perf_metric(
        &mut self,
        field: &str,
        metrics: EitherOrBoth<&AnnotatedMetric<PerfQualities>>,
        diffs: Option<Diffs>,
        perf_config: &PerfOutputConfig,
    ) {
        self.write_metric(field, &metrics, diffs);
        // The second line is only printed if at least one rse is present
        self.write_perf_significance_line(metrics, perf_config);
        // The third line is only printed if at least one samples count is present
        self.write_perf_samples_line(metrics);
    }

    fn write_perf_significance_line(
        &mut self,
        metrics: EitherOrBoth<&AnnotatedMetric<PerfQualities>>,
        perf_config: &PerfOutputConfig,
    ) {
        let field = "  rse% (sig.thr) [sig.fact]".bright_black();
        match metrics.map(|a| (a, a.qualities.rse)) {
            EitherOrBoth::Left((_, Some(rse))) | EitherOrBoth::Both((_, Some(rse)), (_, None)) => {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    NOT_AVAILABLE.bright_black(),
                    UNKNOWN.bright_black()
                );

                self.write_field(
                    field,
                    EitherOrBoth::Both(
                        Metric::Float(rse * 100.0)
                            .to_string_without_unit()
                            .bright_black(),
                        right,
                    ),
                    None,
                    false,
                    None,
                );
            }
            EitherOrBoth::Right((_, Some(rse))) | EitherOrBoth::Both((_, None), (_, Some(rse))) => {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    Metric::Float(rse * 100.0)
                        .to_string_without_unit()
                        .bright_black(),
                    UNKNOWN.bright_black()
                );

                self.write_field(
                    field,
                    EitherOrBoth::Both(NOT_AVAILABLE.to_owned(), right),
                    None,
                    false,
                    None,
                );
            }
            EitherOrBoth::Both((new, Some(new_rse)), (old, Some(old_rse))) => {
                let new_rse_string = Metric::Float(new_rse * 100.0).to_string_without_unit();
                let old_rse_string = Metric::Float(old_rse * 100.0).to_string_without_unit();

                let diff_stats = DiffStats::from_metrics(new, old, perf_config.alpha());

                let right = if let Some(diff_stats) = diff_stats {
                    let significance_threshold = format!(
                        ">{}%",
                        Metric::Float(diff_stats.significance_threshold * 100.0)
                            .to_string_without_unit()
                    );

                    format!(
                        "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$}) [{:^DIFF_WIDTH$}]",
                        old_rse_string.bright_black(),
                        significance_threshold.bright_black(),
                        format_significance_factor(diff_stats.significance_factor)
                    )
                } else {
                    format!(
                        "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                        old_rse_string.bright_black(),
                        UNKNOWN.bright_black()
                    )
                };

                self.write_field(
                    field,
                    EitherOrBoth::Both(new_rse_string.bright_black(), right),
                    None,
                    false,
                    None,
                );
            }
            _ => {}
        }
    }

    fn write_perf_samples_line(&mut self, metrics: EitherOrBoth<&AnnotatedMetric<PerfQualities>>) {
        let field = "  samples".bright_black();
        match metrics.map(|a| a.qualities.n) {
            EitherOrBoth::Left(Some(n)) | EitherOrBoth::Both(Some(n), None) => {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    NOT_AVAILABLE.bright_black(),
                    UNKNOWN.bright_black()
                );

                self.write_field(
                    field,
                    EitherOrBoth::Both(n.to_string().bright_black(), right),
                    None,
                    false,
                    None,
                );
            }
            EitherOrBoth::Right(Some(n)) | EitherOrBoth::Both(None, Some(n)) => {
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$})",
                    n.to_string().bright_black(),
                    UNKNOWN.bright_black()
                );

                self.write_field(
                    field,
                    EitherOrBoth::Both(NOT_AVAILABLE.to_owned(), right),
                    None,
                    false,
                    None,
                );
            }
            EitherOrBoth::Both(Some(new_n), Some(old_n)) => {
                let diffs = Diffs::new(new_n.into(), old_n.into());
                let right = format!(
                    "{:<METRIC_WIDTH$} ({:^DIFF_WIDTH$}) [{:^DIFF_WIDTH$}]",
                    old_n.to_string().bright_black(),
                    format!("{}%", to_string_signed_short(diffs.diff_pct)).bright_black(),
                    format!("{}x", to_string_signed_short(diffs.factor)).bright_black()
                );

                self.write_field(
                    field,
                    EitherOrBoth::Both(new_n.to_string().bright_black(), right),
                    None,
                    false,
                    None,
                );
            }
            _ => {}
        }
    }

    fn write_empty_line(&mut self) {
        let indent = self.indent.trim_end().to_owned();
        if !indent.is_empty() {
            writeln!(self, "{indent}").unwrap();
        }
    }

    fn write_left_indented(&mut self, value: &str) {
        self.write_indent(&IndentKind::Normal);
        writeln!(self, "{}{value}", " ".repeat(FIELD_WIDTH)).unwrap();
    }

    /// Format the baseline
    fn format_baseline(&mut self, baselines: &Baselines) {
        match baselines {
            (None, None) => {}
            (Some(left), Some(right)) if left == right => {
                let right = format!("{right} (old)");
                self.write_field(
                    "Baselines:",
                    EitherOrBoth::Both(left.as_str(), right.as_str()),
                    None,
                    false,
                    None,
                );
            }
            _ => {
                self.write_field(
                    "Baselines:",
                    EitherOrBoth::try_from(baselines.clone())
                        .expect("At least one baseline should be present")
                        .as_ref()
                        .map(String::as_str),
                    None,
                    false,
                    None,
                );
            }
        }
    }

    fn format_details(&mut self, details: &str) {
        let mut details = details.lines();
        if let Some(head_line) = details.next() {
            self.write_indent(&IndentKind::Normal);
            writeln!(self, "{:<FIELD_WIDTH$}{}", "Details:", head_line).unwrap();
            for body_line in details {
                if body_line.is_empty() {
                    self.write_empty_line();
                } else {
                    self.write_left_indented(body_line);
                }
            }
        }
    }

    fn format_metrics<'a, K, V>(&mut self, metrics: impl Iterator<Item = (K, &'a MetricsDiff<V>)>)
    where
        K: Display,
        V: MetricValue + PartialEq + 'a,
    {
        for (metric_kind, diff) in metrics {
            let description = format!("{metric_kind}:");
            self.write_metric(&description, &diff.metrics.as_ref(), diff.diffs);
        }
    }

    fn format_perf_metrics<'a, K>(
        &mut self,
        perf_config: &PerfOutputConfig,
        metrics: impl Iterator<Item = (K, &'a MetricsDiff<AnnotatedMetric<PerfQualities>>)>,
    ) where
        K: Display,
    {
        for (metric_kind, diff) in metrics {
            let description = format!("{metric_kind}:");
            self.write_perf_metric(&description, diff.metrics.as_ref(), diff.diffs, perf_config);
        }
    }

    fn format_tool_total_header(&mut self) {
        self.write_indent(&IndentKind::ToolSubHeadline);
        writeln!(self, "{} {}", "##".yellow(), "Total".bold()).unwrap();
    }

    fn format_multiple_segment_header(&mut self, details: &EitherOrBoth<ProfileInfo>) {
        fn fields(detail: &ProfileInfo) -> String {
            let mut result = String::new();
            write!(result, "pid: {}", detail.pid).unwrap();

            if let Some(ppid) = detail.parent_pid {
                write!(result, " ppid: {ppid}").unwrap();
            }
            if let Some(thread) = detail.thread {
                write!(result, " thread: {thread}").unwrap();
            }
            if let Some(part) = detail.part {
                write!(result, " part: {part}").unwrap();
            }

            result
        }

        self.write_indent(&IndentKind::ToolSubHeadline);
        write!(self, "{} ", "##".yellow()).unwrap();

        let max_left = LEFT_WIDTH - 3;
        match details.as_ref().bimap(
            |new| {
                let left = fields(new);
                let len = left.len();
                (left.bold(), len)
            },
            fields,
        ) {
            EitherOrBoth::Left((left, len)) => {
                if len > max_left {
                    writeln!(self, "{left}\n{}|{NOT_AVAILABLE}", " ".repeat(max_left + 5)).unwrap();
                } else {
                    writeln!(self, "{left}{}|{NOT_AVAILABLE}", " ".repeat(max_left - len)).unwrap();
                }
            }
            EitherOrBoth::Right(right) => {
                writeln!(
                    self,
                    "{}{}|{right}",
                    NOT_AVAILABLE.bold(),
                    " ".repeat(max_left - NOT_AVAILABLE.len())
                )
                .unwrap();
            }
            EitherOrBoth::Both((left, len), right) => {
                if len > max_left {
                    writeln!(self, "{left}\n{}|{right}", " ".repeat(max_left + 5)).unwrap();
                } else {
                    writeln!(self, "{left}{}|{right}", " ".repeat(max_left - len)).unwrap();
                }
            }
        }
    }

    fn format_command(&mut self, config: &Config, command: &EitherOrBoth<&String>) {
        let bench_bin_path = config.bench_bin.display().to_string();
        let paths = command
            .both_and_then(|l, r| {
                if l == r {
                    EitherOrBoth::Left(l)
                } else {
                    EitherOrBoth::Both(l, r)
                }
            })
            .map(|command| {
                if command.starts_with(&bench_bin_path) {
                    make_relative(&config.meta.project_root, &config.bench_bin)
                        .display()
                        .to_string()
                } else {
                    make_relative(&config.meta.project_root, PathBuf::from(command))
                        .display()
                        .to_string()
                }
            });

        self.write_field("Command:", paths, Some(Color::Blue), true, None);
    }

    /// Format the tool headline shown for all tools
    pub fn format_tool_headline(&mut self, tool: Tool) {
        self.write_indent(&IndentKind::ToolHeadline);

        let id = tool.id();
        writeln!(
            self,
            "{} {} {}",
            "=======".bright_black(),
            id.to_ascii_uppercase(),
            "=".repeat(MAX_WIDTH.saturating_sub(id.len() + 9))
                .bright_black(),
        )
        .unwrap();
    }

    fn format_perf_config(&mut self, perf_config: Option<&PerfOutputConfig>) {
        self.write_indent(&IndentKind::Normal);

        let PerfOutputConfig {
            alpha,
            min_pcnt_running,
        } = perf_config.copied().unwrap_or_default();

        let values = format!(
            ">> alpha: {}, min_pcnt_running: {}",
            Metric::from(alpha).to_string_without_unit(),
            Metric::from(min_pcnt_running).to_string_without_unit()
        );

        writeln!(self, "{}", values.bright_black()).unwrap();
    }

    fn format_single_error_metric(
        &mut self,
        summary: &MetricsSummary<ErrorMetric>,
        output_format: &IndexSet<ErrorMetric>,
        info: Option<&EitherOrBoth<ProfileInfo>>,
    ) {
        self.format_metrics(
            output_format
                .clone()
                .iter()
                .filter_map(|e| summary.diff_by_kind(e).map(|d| (e, d))),
        );

        // We only check for `new` errors
        if let Some(info) = info
            && summary.diff_by_kind(&ErrorMetric::Errors).is_some_and(|e| {
                e.metrics
                    .as_ref()
                    .left()
                    .is_some_and(|l| *l > Metric::Int(0))
            })
            && let Some(new) = info.as_ref().left()
            && let Some(details) = new.details.as_ref()
        {
            self.format_details(details);
        }
    }
}

impl Display for VerticalFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.buffer)
    }
}

impl Formatter for VerticalFormatter {
    fn format_single(
        &mut self,
        baselines: &Baselines,
        info: Option<&EitherOrBoth<ProfileInfo>>,
        metrics_summary: &ToolMetricSummary,
        is_default_tool: bool,
        perf_config: Option<&PerfOutputConfig>,
    ) {
        if is_default_tool {
            self.format_baseline(baselines);
        }

        if metrics_summary.is_empty() {
            self.write_indent(&IndentKind::Normal);
            writeln!(self, "{}", "Empty data".bright_black()).unwrap();
            return;
        }

        match metrics_summary {
            ToolMetricSummary::None => {
                if let Some(info) = info
                    && let Some(new) = info.as_ref().left()
                    && let Some(details) = &new.details
                {
                    self.format_details(details);
                }
            }
            ToolMetricSummary::Memcheck(summary) => {
                let format = self.output_format.memcheck.clone();
                self.format_single_error_metric(summary, &format, info);
            }
            ToolMetricSummary::Helgrind(summary) => {
                let format = self.output_format.helgrind.clone();
                self.format_single_error_metric(summary, &format, info);
            }
            ToolMetricSummary::DRD(summary) => {
                let format = self.output_format.drd.clone();
                self.format_single_error_metric(summary, &format, info);
            }
            ToolMetricSummary::Dhat(summary) => self.format_metrics(
                self.output_format
                    .dhat
                    .clone()
                    .iter()
                    .filter_map(|e| summary.diff_by_kind(e).map(|d| (e, d))),
            ),
            ToolMetricSummary::Callgrind(summary) => {
                self.format_metrics(
                    self.output_format
                        .callgrind
                        .clone()
                        .iter()
                        .filter_map(|e| summary.diff_by_kind(e).map(|d| (e, d))),
                );
            }
            ToolMetricSummary::Cachegrind(summary) => {
                self.format_metrics(
                    self.output_format
                        .cachegrind
                        .clone()
                        .iter()
                        .filter_map(|e| summary.diff_by_kind(e).map(|d| (e, d))),
                );
            }
            ToolMetricSummary::Perf(summary) => {
                let default_perf_config = PerfOutputConfig::default();
                self.format_perf_metrics(
                    perf_config.unwrap_or(&default_perf_config),
                    summary
                        .all_diffs()
                        .map(|(perf_metric, diff)| (perf_metric.display(), diff)),
                );
            }
        }
    }

    fn format(
        &mut self,
        tool: Tool,
        config: &Config,
        baselines: &Baselines,
        data: &ProfileData,
        is_default_tool: bool,
        perf_config: Option<&PerfOutputConfig>,
    ) {
        if matches!(tool, Tool::Perf) && data.has_data(Tool::Perf) {
            self.format_perf_config(perf_config);
        }

        if self.output_format.show_only_comparison {
            // no usual data to show
        } else if data.has_multiple()
            && (self.output_format.show_intermediate || tool == Tool::Perf)
        {
            let mut first = true;
            for part in &data.parts {
                self.format_multiple_segment_header(&part.details);
                if tool != Tool::Perf {
                    self.format_command(config, &part.details.as_ref().map(|i| &i.command));
                }

                if first {
                    self.format_single(
                        baselines,
                        Some(&part.details),
                        &part.metrics_summary,
                        is_default_tool,
                        perf_config,
                    );
                    first = false;
                } else {
                    self.format_single(
                        &(None, None),
                        Some(&part.details),
                        &part.metrics_summary,
                        is_default_tool,
                        perf_config,
                    );
                }
            }

            if data.total.is_some() {
                self.format_tool_total_header();
                self.format_single(
                    &(None, None),
                    None,
                    &data.total.summary,
                    is_default_tool,
                    perf_config,
                );
            }
        } else if data.total.is_some() {
            self.format_single(
                baselines,
                None,
                &data.total.summary,
                is_default_tool,
                perf_config,
            );
        } else if !data.is_empty() && tool == Tool::Perf {
            self.format_single(
                baselines,
                None,
                &data.parts[0].metrics_summary,
                is_default_tool,
                perf_config,
            );
        } else if data.total.is_none() && !data.parts.is_empty() {
            // Since there is no total, show_all is partly ignored, and we show all data in a little
            // bit more aggregated form without the multiple files headlines. This affects currently
            // the output of `Massif`, `BBV` and `perf`.
            for part in &data.parts {
                self.format_command(config, &part.details.as_ref().map(|i| &i.command));

                if let Some(new) = part.details.as_ref().left()
                    && let Some(details) = &new.details
                {
                    self.format_details(details);
                }
            }
        } else {
            // no data to show
        }
    }

    fn print_comparison(
        &mut self,
        function_name: &str,
        id: &str,
        details: Option<&str>,
        summaries: Vec<(Tool, ToolMetricSummary)>,
        perf_config: Option<&PerfOutputConfig>,
    ) {
        if self.output_format.is_default() {
            ComparisonHeader::new(function_name, id, details, &self.output_format).print();

            let is_multiple = summaries.len() > 1;
            for (tool, summary) in summaries
                .iter()
                .filter(|(_, s)| *s != ToolMetricSummary::None)
            {
                if is_multiple || *tool != Tool::Callgrind {
                    self.format_line(&format!(
                        "{}{} {}\n",
                        self.indent_sub_header,
                        "-------".bright_black(),
                        tool.to_string().to_uppercase()
                    ));
                }
                self.format_single(&(None, None), None, summary, false, perf_config);
            }
            self.print_buffer();
        }
    }

    fn clear(&mut self) {
        self.buffer.clear();
    }

    fn format_line(&mut self, line: &str) {
        self.buffer.push_str(line);
    }
}

impl Write for VerticalFormatter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.buffer.push_str(s);
        Ok(())
    }
}

/// Format a floating point number with `unit`
pub fn format_float(float: f64, unit: char) -> ColoredString {
    let signed_short = to_string_signed_short(float);
    if float.is_infinite() {
        if float.is_sign_positive() {
            format!("{signed_short:+^DIFF_WIDTH$}").bright_red().bold()
        } else {
            format!("{signed_short:-^DIFF_WIDTH$}")
                .bright_green()
                .bold()
        }
    } else if float.is_sign_positive() {
        format!("{signed_short:>+FLOAT_WIDTH$}{unit}")
            .bright_red()
            .bold()
    } else {
        format!("{signed_short:>+FLOAT_WIDTH$}{unit}")
            .bright_green()
            .bold()
    }
}

fn format_metric_with_unit(metric: &Metric, unit: Option<&Unit>) -> String {
    if let Some(unit) = unit {
        format!("{metric} [{unit}]")
    } else {
        metric.to_string()
    }
}

fn format_significance_factor(float: f64) -> ColoredString {
    let unsigned_short = to_string_unsigned_short(float);
    if !float.is_finite() {
        format!("{:^DIFF_WIDTH$}", "Invalid").bright_red().bold()
    } else if float < 1.0 && !abs_diff_eq!(float, 1.0) {
        format!("{unsigned_short:>+FLOAT_WIDTH$}x").bright_black()
    } else {
        format!("{unsigned_short:>+FLOAT_WIDTH$}x").blue().bold()
    }
}

fn merge_units<'a>(new: Option<&'a Unit>, old: Option<&'a Unit>) -> Option<Cow<'a, Unit>> {
    match (new, old) {
        (None, None) => None,
        (None, Some(unit)) | (Some(unit), None) => Some(Cow::Borrowed(unit)),
        (Some(new_unit), Some(old_unit)) if new_unit == old_unit => Some(Cow::Borrowed(new_unit)),
        // This is a safety net. The metrics in a diff should all have the same unit
        (Some(new_unit), Some(old_unit)) => {
            Some(Cow::Owned(Unit::Unknown(format!("{new_unit}/{old_unit}"))))
        }
    }
}

/// Returns the formatted string if `NoCapture` is not `False`.
pub fn no_capture_footer(nocapture: NoCapture) -> Option<String> {
    match nocapture {
        NoCapture::True => Some(format!(
            "{} {}",
            "-".yellow(),
            "end of stdout/stderr".yellow()
        )),
        NoCapture::False => None,
        NoCapture::Stderr => Some(format!("{} {}", "-".yellow(), "end of stderr".yellow())),
        NoCapture::Stdout => Some(format!("{} {}", "-".yellow(), "end of stdout".yellow())),
    }
}

/// Print the summary of the --list argument
///
/// When `format` is [`ListFormat::Terse`] the trailing blank line and `0 tests, N benchmarks`
/// summary are suppressed so the output consists solely of per-benchmark lines, matching the
/// libtest terse listing format that `cargo nextest` expects.
pub fn print_benchmark_list_summary(sum: u64, format: ListFormat) {
    if format == ListFormat::Terse {
        return;
    }
    if sum != 0 {
        println!();
    }
    println!("0 tests, {sum} benchmarks");
}

/// Print a single benchmark for the --list argument
pub fn print_list_benchmark(module_path: &ModulePath, id: Option<&String>) {
    match id {
        Some(id) => {
            println!("{module_path}::{id}: benchmark");
        }
        None => {
            println!("{module_path}: benchmark");
        }
    }
}

/// Print the appropriate footer for the [`NoCapture`] option
pub fn print_no_capture_footer(nocapture: NoCapture) {
    if let Some(footer) = no_capture_footer(nocapture) {
        println!("{footer}");
    }
}

/// Prints the status used when no configured tool is supported and enabled for a benchmark.
pub fn print_no_config() {
    println!(
        "  {}",
        "skipped: no supported configured tool".bright_black()
    );
}

/// Print detected regressions to `stderr`
pub fn print_regressions(regressions: &[ToolRegression]) {
    let mut first = true;

    for regression in regressions {
        if first {
            println!();
            first = false;
        }
        match regression {
            ToolRegression::Soft {
                metric,
                display,
                unit,
                new,
                old,
                diff_pct,
                limit,
            } => {
                let display = regression_display_name(metric, display.as_deref());
                let old = format_metric_with_unit(old, unit.as_ref());
                let new = format_metric_with_unit(new, unit.as_ref());

                if limit.is_sign_positive() {
                    eprintln!(
                        "Performance has {0}: {1} ({2} -> {3}) regressed by {4:>+6} (>{5:>+6})",
                        "regressed".bold().bright_red(),
                        display,
                        old,
                        new.bold(),
                        format!("{}%", to_string_signed_short(*diff_pct))
                            .bold()
                            .bright_red(),
                        format!("{}%", to_string_signed_short(*limit)).bright_black()
                    );
                } else {
                    eprintln!(
                        "Performance has {0}: {1} ({2} -> {3}) regressed by {4:>+6} (<{5:>+6})",
                        "regressed".bold().bright_red(),
                        display,
                        old,
                        new.bold(),
                        format!("{}%", to_string_signed_short(*diff_pct))
                            .bold()
                            .bright_red(),
                        format!("{}%", to_string_signed_short(*limit)).bright_black()
                    );
                }
            }
            ToolRegression::Hard {
                metric,
                display,
                unit,
                new,
                diff,
                limit,
            } => {
                let display = regression_display_name(metric, display.as_deref());
                let new = format_metric_with_unit(new, unit.as_ref());
                let diff = format_metric_with_unit(diff, unit.as_ref());
                let limit = format_metric_with_unit(limit, unit.as_ref());

                eprintln!(
                    "Performance has {0}: {1} ({2}) exceeds limit by {3} (>{4})",
                    "regressed".bold().bright_red(),
                    display,
                    new.bold(),
                    diff.bold().bright_red(),
                    limit.bright_black(),
                );
            }
        }
    }
}

fn regression_display_name<'a>(metric: &MetricKind, display: Option<&'a str>) -> Cow<'a, str> {
    display.map_or_else(
        || {
            let name = match metric {
                MetricKind::None => None,
                MetricKind::Callgrind(event_kind) => Some(event_kind.to_string()),
                MetricKind::Cachegrind(cachegrind_metric) => Some(cachegrind_metric.to_string()),
                MetricKind::Dhat(dhat_metric) => Some(dhat_metric.to_string()),
                MetricKind::Memcheck(error_metric)
                | MetricKind::Helgrind(error_metric)
                | MetricKind::DRD(error_metric) => Some(error_metric.to_string()),
                MetricKind::Perf(perf_metric) => Some(perf_metric.to_string()),
            }
            .unwrap_or_default();

            Cow::Owned(name)
        },
        Cow::Borrowed,
    )
}

fn truncate_description(description: &str, truncate_description: Option<usize>) -> Cow<'_, str> {
    if let Some(num) = truncate_description {
        let new_description = truncate_str_utf8(description, num);
        if new_description.len() < description.len() {
            Cow::Owned(format!("{new_description}..."))
        } else {
            Cow::Borrowed(description)
        }
    } else {
        Cow::Borrowed(description)
    }
}

#[cfg(test)]
mod tests {
    use indexmap::indexmap;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::metrics::model::{Metrics, MetricsSummary};

    const FIELD_34: &str = "Some Field1234567890Some Field123:";
    const FIELD_35: &str = "Some Field1234567890Some Field1234:";
    const FIELD_50: &str = "Some Field1234567890Some Field1234567890123456789:";
    const FIELD_53: &str = "Some Field1234567890Some Field1234567890Some Field12:";
    const FIELD_54: &str = "Some Field1234567890Some Field1234567890Some Field123:";
    const FIELD_55: &str = "Some Field1234567890Some Field1234567890Some Field1234:";
    const FIELD_SOME_5: &str = "Some:";
    const TWENTY_DIGITS: &str = "12345678901234567890";
    const TWENTY_ONE_DIGITS: &str = "123456789012345678901";

    #[rstest]
    #[case::simple("some::module", Some("id"), Some("1, 2"), "some::module id:1, 2")]
    #[case::id_but_no_description("some::module", Some("id"), None, "some::module id")]
    #[case::id_but_empty_description("some::module", Some("id"), Some(""), "some::module id")]
    #[case::no_id_but_description("some::module", None, Some("1, 2, 3"), "some::module :1, 2, 3")]
    #[case::no_id_no_description("some::module", None, None, "some::module")]
    #[case::no_id_empty_description("some::module", None, Some(""), "some::module")]
    #[case::length_is_greater_than_default(
        "some::module",
        Some("id"),
        Some("012345678901234567890123456789012345678901234567890123456789"),
        "some::module id:012345678901234567890123456789012345678901234567890123456789"
    )]
    fn test_header_display_when_no_truncate(
        #[case] module_path: &str,
        #[case] id: Option<&str>,
        #[case] description: Option<&str>,
        #[case] expected: &str,
    ) {
        colored::control::set_override(false);

        let output_format = OutputFormat {
            truncate_description: None,
            ..Default::default()
        };
        let header = Header::new(
            &ModulePath::new(module_path),
            id.map(ToOwned::to_owned),
            description.map(ToOwned::to_owned),
            &output_format,
        );

        assert_eq!(header.to_string(), expected);
    }

    #[rstest]
    #[case::truncate_0(
        "some::module",
        Some("id"),
        Some("1, 2, 3"),
        Some(0),
        "some::module id:..."
    )]
    #[case::truncate_0_when_length_is_0(
        "some::module",
        Some("id"),
        Some(""),
        Some(0),
        "some::module id"
    )]
    #[case::truncate_0_when_length_is_1(
        "some::module",
        Some("id"),
        Some("1"),
        Some(0),
        "some::module id:..."
    )]
    #[case::truncate_1(
        "some::module",
        Some("id"),
        Some("1, 2, 3"),
        Some(1),
        "some::module id:1..."
    )]
    #[case::truncate_1_when_length_is_0(
        "some::module",
        Some("id"),
        Some(""),
        Some(1),
        "some::module id"
    )]
    #[case::truncate_1_when_length_is_1(
        "some::module",
        Some("id"),
        Some("1"),
        Some(1),
        "some::module id:1"
    )]
    #[case::truncate_1_when_length_is_2(
        "some::module",
        Some("id"),
        Some("1,"),
        Some(1),
        "some::module id:1..."
    )]
    #[case::truncate_3(
        "some::module",
        Some("id"),
        Some("1, 2, 3"),
        Some(3),
        "some::module id:1, ..."
    )]
    #[case::truncate_3_when_length_is_2(
        "some::module",
        Some("id"),
        Some("1,"),
        Some(3),
        "some::module id:1,"
    )]
    #[case::truncate_3_when_length_is_3(
        "some::module",
        Some("id"),
        Some("1, "),
        Some(3),
        "some::module id:1, "
    )]
    #[case::truncate_3_when_length_is_4(
        "some::module",
        Some("id"),
        Some("1, 2"),
        Some(3),
        "some::module id:1, ..."
    )]
    #[case::truncate_is_smaller_than_length(
        "some::module",
        Some("id"),
        Some("1, 2, 3, 4, 5"),
        Some(4),
        "some::module id:1, 2..."
    )]
    #[case::truncate_is_one_smaller_than_length(
        "some::module",
        Some("id"),
        Some("1, 2, 3"),
        Some(6),
        "some::module id:1, 2, ..."
    )]
    #[case::truncate_is_one_greater_than_length(
        "some::module",
        Some("id"),
        Some("1, 2, 3"),
        Some(8),
        "some::module id:1, 2, 3"
    )]
    #[case::truncate_is_far_greater_than_length(
        "some::module",
        Some("id"),
        Some("1, 2, 3"),
        Some(100),
        "some::module id:1, 2, 3"
    )]
    #[case::truncate_is_equal_to_length(
        "some::module",
        Some("id"),
        Some("1, 2, 3"),
        Some(7),
        "some::module id:1, 2, 3"
    )]
    #[case::description_is_empty(
        "some::module",
        Some("id"),
        Some(""),
        Some(100),
        "some::module id"
    )]
    fn test_header_display_when_truncate(
        #[case] module_path: &str,
        #[case] id: Option<&str>,
        #[case] description: Option<&str>,
        #[case] truncate_description: Option<usize>,
        #[case] expected: &str,
    ) {
        colored::control::set_override(false);

        let output_format = OutputFormat {
            truncate_description,
            ..Default::default()
        };

        let header = Header::new(
            &ModulePath::new(module_path),
            id.map(ToOwned::to_owned),
            description.map(ToOwned::to_owned),
            &output_format,
        );

        assert_eq!(header.to_string(), expected);
    }

    #[rstest]
    #[case::new_costs_0(EventKind::Ir, 0, None, "*********", None)]
    #[case::old_costs_0(EventKind::Ir, 1, Some(0), "+++inf+++", Some("+++inf+++"))]
    #[case::all_costs_0(EventKind::Ir, 0, Some(0), "No change", None)]
    #[case::new_costs_u64_max(EventKind::Ir, u64::MAX, None, "*********", None)]
    #[case::old_costs_u64_max(
    EventKind::Ir,
    u64::MAX / 10,
    Some(u64::MAX),
    "-90.0000%",
    Some("-10.0000x")
)]
    #[case::all_costs_u64_max(EventKind::Ir, u64::MAX, Some(u64::MAX), "No change", None)]
    #[case::no_change_when_not_0(EventKind::Ir, 1000, Some(1000), "No change", None)]
    #[case::neg_change_when_not_0(EventKind::Ir, 2000, Some(3000), "-33.3333%", Some("-1.50000x"))]
    #[case::pos_change_when_not_0(EventKind::Ir, 2000, Some(1000), "+100.000%", Some("+2.00000x"))]
    #[case::pos_inf(EventKind::Ir, 2000, Some(0), "+++inf+++", Some("+++inf+++"))]
    #[case::neg_inf(EventKind::Ir, 0, Some(2000), "-100.000%", Some("---inf---"))]
    fn test_format_vertical_when_new_costs_are_present(
        #[case] event_kind: EventKind,
        #[case] new: u64,
        #[case] old: Option<u64>,
        #[case] diff_pct: &str,
        #[case] diff_fact: Option<&str>,
    ) {
        colored::control::set_override(false);

        let costs = match old {
            Some(old) => EitherOrBoth::Both(
                Metrics(indexmap! {event_kind => Metric::Int(new)}),
                Metrics(indexmap! {event_kind => Metric::Int(old)}),
            ),
            None => EitherOrBoth::Left(Metrics(indexmap! {event_kind => Metric::Int(new)})),
        };
        let metrics_summary = MetricsSummary::new(costs);
        let mut formatter = VerticalFormatter::new(OutputFormat::default());
        formatter.format_metrics(metrics_summary.all_diffs());

        let expected = format!(
            "  {:<36}{new:>METRIC_WIDTH$}|{:<METRIC_WIDTH$} ({diff_pct}){}\n",
            format!("{event_kind}:"),
            old.map_or_else(|| NOT_AVAILABLE.to_owned(), |o| o.to_string()),
            diff_fact.map_or_else(String::new, |f| format!(" [{f}]"))
        );

        assert_eq!(formatter.buffer, expected);
    }

    #[rstest]
    #[case::no_change(2000, Some(2000), 50.0, "No change", None)]
    #[case::new_costs_0_no_old(0, None, 50.0, "*********", None)]
    #[case::old_costs_0(1, Some(0), 50.0, "+++inf+++", Some("+++inf+++"))]
    #[case::all_costs_0(0, Some(0), 50.0, "No change", None)]
    #[case::all_0(0, Some(0), 0.0, "No change", None)]
    #[case::neg_change_when_tolerance_0(2000, Some(3000), 0.0, "-33.3333%", Some("-1.50000x"))]
    #[case::pos_change_when_tolerance_0(2000, Some(1000), 0.0, "+100.000%", Some("+2.00000x"))]
    #[case::neg_change_when_within_tolerance(2000, Some(3000), 50.0, "Tolerance", None)]
    #[case::neg_change_when_within_tolerance_exact(
        2000,
        Some(3000),
        1.0 / 3.0 * 100.0,
        "Tolerance",
        None
    )]
    #[case::pos_change_when_within_tolerance(3000, Some(2000), 50.0, "Tolerance", None)]
    #[case::pos_change_when_neg_tolerance(3000, Some(2000), -50.0, "Tolerance", None)]
    #[case::pos_change_when_tolerance_is_nan(
        2000,
        Some(1000),
        f64::NAN,
        "+100.000%",
        Some("+2.00000x")
    )]
    fn test_format_vertical_when_tolerance_is_set(
        #[case] new: u64,
        #[case] old: Option<u64>,
        #[case] tolerance: f64,
        #[case] diff_pct: &str,
        #[case] diff_fact: Option<&str>,
    ) {
        colored::control::set_override(false);

        let expected = format!(
            "  {:<FIELD_WIDTH$}{new:>METRIC_WIDTH$}|{:<METRIC_WIDTH$} ({diff_pct}){}\n",
            format!("{}:", EventKind::Ir),
            old.map_or_else(|| NOT_AVAILABLE.to_owned(), |o| o.to_string()),
            diff_fact.map_or_else(String::new, |f| format!(" [{f}]"))
        );

        let output_format = OutputFormat {
            tolerance: Some(tolerance),
            ..Default::default()
        };

        let costs = match old {
            Some(old) => EitherOrBoth::Both(
                Metrics(indexmap! {EventKind::Ir => Metric::Int(new)}),
                Metrics(indexmap! {EventKind::Ir => Metric::Int(old)}),
            ),
            None => EitherOrBoth::Left(Metrics(indexmap! {EventKind::Ir => Metric::Int(new)})),
        };
        let metrics_summary = MetricsSummary::new(costs);
        let mut formatter = VerticalFormatter::new(output_format);
        formatter.format_metrics(metrics_summary.all_diffs());

        assert_eq!(formatter.buffer, expected);
    }

    #[rstest]
    #[case::normal_no_grid(IndentKind::Normal, false, "  ")]
    #[case::tool_header_no_grid(IndentKind::ToolHeadline, false, "  ")]
    #[case::tool_sub_header_no_grid(IndentKind::ToolSubHeadline, false, "  ")]
    #[case::normal_with_grid(IndentKind::Normal, true, "| ")]
    #[case::tool_header_with_grid(IndentKind::ToolHeadline, true, "|=")]
    #[case::tool_sub_header_with_grid(IndentKind::ToolSubHeadline, true, "|-")]
    fn test_vertical_formatter_write_indent(
        #[case] kind: IndentKind,
        #[case] show_grid: bool,
        #[case] expected: &str,
    ) {
        colored::control::set_override(false);

        let output_format = OutputFormat {
            show_grid,
            ..Default::default()
        };

        let mut formatter = VerticalFormatter::new(output_format);
        formatter.write_indent(&kind);
        assert_eq!(formatter.buffer, expected);
    }

    #[rstest]
    #[case::left(
        FIELD_SOME_5,
        EitherOrBoth::Left("left"),
        None,
        false,
        format!("  {FIELD_SOME_5}{}left\n", " ".repeat(LEFT_WIDTH - 5 - 4))
    )]
    #[case::left_when_barely_fit(
        FIELD_34,
        EitherOrBoth::Left(TWENTY_DIGITS),
        None,
        false,
        format!("  {FIELD_34}  {TWENTY_DIGITS}\n")
    )]
    #[case::left_when_field_barely_fit(
        FIELD_53,
        EitherOrBoth::Left("0"),
        None,
        false,
        format!("  {FIELD_53}  0\n")
    )]
    #[case::left_when_barely_not_fit_then_multi_line(
        FIELD_35,
        EitherOrBoth::Left(TWENTY_DIGITS),
        None,
        false,
        format!("  {FIELD_35}\n  {}{TWENTY_DIGITS}\n", " ".repeat(FIELD_WIDTH))
    )]
    #[case::left_when_not_fit_and_left_align_then_no_multi_line(
        FIELD_35,
        EitherOrBoth::Left(TWENTY_DIGITS),
        None,
        true,
        format!("  {FIELD_35} {TWENTY_DIGITS}\n")
    )]
    #[case::left_when_left_align(
        FIELD_SOME_5,
        EitherOrBoth::Left("left"),
        None,
        true,
        format!("  {FIELD_SOME_5} left\n")
    )]
    #[case::left_when_field_barely_fit_and_left_align(
        FIELD_54,
        EitherOrBoth::Left("0"),
        None,
        true,
        format!("  {FIELD_54} 0\n")
    )]
    #[case::left_when_field_barely_not_fit_and_left_align_then_multiline(
        FIELD_55,
        EitherOrBoth::Left("0"),
        None,
        true,
        format!("  {FIELD_55}\n    0\n")
    )]
    #[case::left_with_unit(
        FIELD_SOME_5,
        EitherOrBoth::Left("left"),
        Unit::Seconds,
        false,
        format!("  Some [s]:{}left\n", " ".repeat(LEFT_WIDTH - 9 - 4))
    )]
    #[case::right(
        FIELD_SOME_5,
        EitherOrBoth::Right("right"),
        None,
        false,
        format!("  {FIELD_SOME_5}{}|right\n", " ".repeat(LEFT_WIDTH - 5))
    )]
    #[case::right_when_barely_fit(
        FIELD_54,
        EitherOrBoth::Right("right"),
        None,
        false,
        format!("  {FIELD_54}  |right\n")
    )]
    #[case::right_when_barely_not_fit_then_multi_line(
        FIELD_55,
        EitherOrBoth::Right("right"),
        None,
        false,
        format!("  {FIELD_55}\n  {}|right\n", " ".repeat(LEFT_WIDTH))
    )]
    #[case::right_when_left_align_no_effect(
        FIELD_54,
        EitherOrBoth::Right("right"),
        None,
        true,
        format!("  {FIELD_54}  |right\n")
    )]
    #[case::right_with_unit(
        FIELD_SOME_5,
        EitherOrBoth::Right("right"),
        Unit::Seconds,
        false,
        format!("  Some [s]:{}|right\n", " ".repeat(LEFT_WIDTH - 9))
    )]
    #[case::both(
        FIELD_SOME_5,
        EitherOrBoth::Both("left", "right"),
        None,
        false,
        format!("  {FIELD_SOME_5}{}left|right\n", " ".repeat(LEFT_WIDTH - 5 - 4))
    )]
    #[case::both_when_barely_fit(
        FIELD_34,
        EitherOrBoth::Both(TWENTY_DIGITS, TWENTY_DIGITS),
        None,
        false,
        format!(
            "  {FIELD_34}{}{TWENTY_DIGITS}|{TWENTY_DIGITS}\n",
            " ".repeat(LEFT_WIDTH - 34 - 20)
        )
    )]
    #[case::both_when_21_digits_then_no_multi_line(
        FIELD_SOME_5,
        EitherOrBoth::Both(TWENTY_ONE_DIGITS, TWENTY_ONE_DIGITS),
        None,
        false,
        format!(
            "  {FIELD_SOME_5}{}{TWENTY_ONE_DIGITS}|{TWENTY_ONE_DIGITS}\n",
            " ".repeat(LEFT_WIDTH - 5 - 21)
        )
    )]
    #[case::both_when_barely_not_fit_then_multi_line(
        FIELD_35,
        EitherOrBoth::Both(TWENTY_DIGITS, "right"),
        None,
        false,
        format!("  {FIELD_35}\n  {}{TWENTY_DIGITS}|right\n", " ".repeat(LEFT_WIDTH - 20))
    )]
    #[case::both_with_unit(
        FIELD_SOME_5,
        EitherOrBoth::Both("left", "right"),
        Unit::Seconds,
        false,
        format!("  Some [s]:{}left|right\n", " ".repeat(LEFT_WIDTH - 9 - 4))
    )]
    #[case::both_when_left_align(
        FIELD_SOME_5,
        EitherOrBoth::Both("left", "right"),
        None,
        true,
        format!("  {FIELD_SOME_5}  {}left{}|right\n", " ".repeat(FIELD_WIDTH - 5), " ".repeat(LEFT_WIDTH - FIELD_WIDTH - 2 - 4))
    )]
    #[case::both_when_left_align_barely_fit(
        FIELD_50,
        EitherOrBoth::Both("left", "right"),
        None,
        true,
        format!("  {FIELD_50}  left|right\n")
    )]
    #[case::both_when_left_align_with_some_space(
        FIELD_50,
        EitherOrBoth::Both("l", "right"),
        None,
        true,
        format!("  {FIELD_50}  l   |right\n")
    )]
    #[case::both_when_left_align_and_multi_line(
        FIELD_54,
        EitherOrBoth::Both("left", "right"),
        None,
        true,
        format!("  {FIELD_54}\n  {}left{}|right\n", " ".repeat(FIELD_WIDTH), " ".repeat(LEFT_WIDTH - FIELD_WIDTH - 4))
    )]
    fn test_vertical_formatter_write_field<E, U, V>(
        #[case] field: &str,
        #[case] values: EitherOrBoth<V>,
        #[case] unit: U,
        #[case] left_align: bool,
        #[case] expected: E,
    ) where
        E: Into<String>,
        U: Into<Option<Unit>>,
        V: Into<ColoredString>,
    {
        colored::control::set_override(false);

        let output_format = OutputFormat::default();
        let mut formatter = VerticalFormatter::new(output_format);
        let unit = unit.into();

        formatter.write_field(field, values, None, left_align, unit.as_ref());

        assert_eq!(formatter.buffer, expected.into());
    }
}
