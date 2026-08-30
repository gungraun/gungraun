//! Summary data model types to build the top-level [`BenchmarkSummary`]
//!
//! Aggregating a [`BenchmarkSummary`] from a benchmark run serves two main purposes:
//!
//! 1. It allows running the benchmark and process the data in completely separate steps.
//! 2. Being able to print a json benchmark summary which at a minimum contains all data of the
//!    terminal output in a machine-readable format.
//!
//! These types define the main consumer-facing structure that is serialized to and deserialized
//! from summary files.

use std::path::PathBuf;

use either_or_both::EitherOrBoth;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind, PerfMetric, Tool};
use crate::metrics::model::{
    AnnotatedMetric, Metric, MetricKind, Metrics, MetricsSummary, PerfQualities,
};
use crate::units::Unit;

/// The version string stored in version summary JSON files.
pub const SCHEMA_VERSION: &str = "7";

/// Describes which baseline a summary compares against.
///
/// # Benchmark Summary
///
/// This struct is not part of the recent summary anymore.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum BaselineKind {
    /// Compare new against `*.old` output files
    Old,
    /// Compare new against a named baseline
    Name(BaselineName),
}

/// Identifies whether a summary describes a library or binary benchmark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum BenchmarkKind {
    /// A library benchmark
    LibraryBenchmark,
    /// A binary benchmark
    BinaryBenchmark,
}

/// A [`Tool`] metric data summary.
///
/// Each variant contains all metric data including the differences to the old or a
/// [`BenchmarkSummary::baselines`] run for a single [`Tool`]. The contained
/// [`MetricsSummary`] is keyed by the metric enum used by that tool.
///
/// The [`ToolMetricSummary::Memcheck`], [`ToolMetricSummary::Helgrind`], and
/// [`ToolMetricSummary::DRD`] variants contain the corresponding error metrics. Massif and BBV are
/// special cases because they do not have a metrics summary and therefore use the
/// [`ToolMetricSummary::None`] variant.
///
/// # Examples
///
/// This is the summary of a Callgrind run which had only [`EventKind::Ir`] (instruction counts)
/// measurement activated. Since there is a [`Diffs`] present, there was an old run and a new run
/// which were compared with each other. Per convention the new run is on the left side of an
/// [`EitherOrBoth::Both`] or [`EitherOrBoth::Left`] and the old run on the right side or a
/// [`EitherOrBoth::Right`].
///
/// ```rust
/// use either_or_both::EitherOrBoth;
/// use gungraun_runner::api::EventKind;
/// use gungraun_runner::metrics::model::{Metric, MetricsDiff, MetricsSummary};
/// use gungraun_runner::summary::model::{Diffs, ToolMetricSummary};
/// use indexmap::IndexMap;
///
/// let callgrind_summary = ToolMetricSummary::Callgrind(MetricsSummary(IndexMap::from([(
///     EventKind::Ir,
///     MetricsDiff {
///         diffs: Some(Diffs {
///             diff_pct: -50.0,
///             factor: -2.0,
///         }),
///         metrics: EitherOrBoth::Both(Metric::Int(1), Metric::Int(2)),
///     },
/// )])));
///
/// match callgrind_summary {
///     ToolMetricSummary::Callgrind(metrics) => {
///         assert!(metrics.0.contains_key(&EventKind::Ir));
///     }
///     _ => {}
/// }
/// ```
///
/// [`Tool`]: crate::api::Tool
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ToolMetricSummary {
    /// If there are no metrics extracted (currently Massif, BBV)
    #[default]
    None,
    /// The [`ErrorMetric`] summary for Memcheck.
    Memcheck(MetricsSummary<ErrorMetric>),
    /// The [`ErrorMetric`] summary for Helgrind.
    Helgrind(MetricsSummary<ErrorMetric>),
    /// The [`ErrorMetric`] summary for DRD.
    DRD(MetricsSummary<ErrorMetric>),
    /// The metric summary of [`DhatMetric`]s
    Dhat(MetricsSummary<DhatMetric>),
    /// The Callgrind summary of [`EventKind`]
    Callgrind(MetricsSummary<EventKind>),
    /// The summary of [`CachegrindMetric`]s
    Cachegrind(MetricsSummary<CachegrindMetric>),
    /// Perf summaries for a single parsed part or direct new/old comparison.
    ///
    /// Unlike the valgrind-based tools, perf does not currently produce a synthetic aggregated
    /// `total` summary across parts in [`ProfileData::new`].
    Perf(MetricsSummary<PerfMetric, AnnotatedMetric<PerfQualities>>),
}

/// A per-tool collection of raw metric values.
///
/// This enum is used where the summary needs to store metrics keyed by the tool that produced them,
/// without comparison metadata.
///
/// # Benchmark Summary
///
/// This struct is not part of the recent summary anymore.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum ToolMetrics {
    /// If there are no metrics extracted from a tool (currently Massif, BBV)
    #[default]
    None,
    /// The metrics of a dhat benchmark
    Dhat(Metrics<DhatMetric>),
    /// The error metrics from a Memcheck run.
    Memcheck(Metrics<ErrorMetric>),
    /// The error metrics from a Helgrind run.
    Helgrind(Metrics<ErrorMetric>),
    /// The error metrics from a DRD run.
    DRD(Metrics<ErrorMetric>),
    /// The metrics of a Callgrind benchmark
    Callgrind(Metrics<EventKind>),
    /// The metrics of a Cachegrind benchmark
    Cachegrind(Metrics<CachegrindMetric>),
    /// Perf metrics with attached runtime and variability metadata.
    ///
    /// These metrics are summarized per part, but no synthetic aggregate `total` is currently
    /// constructed across parts.
    Perf(Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>),
}

/// A regression detected while evaluating a [`BenchmarkSummary`].
///
/// Soft regressions compare a new value against an older measurement using a percentage threshold.
/// Hard regressions compare a new value against an absolute limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ToolRegression {
    /// A performance regression triggered by a soft limit
    Soft {
        /// The [`MetricKind`] per tool
        metric: MetricKind,
        /// An optional human-readable display label for the regression metric, used in formatted
        /// output.
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<String>,
        /// The unit of the metric values, if present.
        #[serde(skip_serializing_if = "Option::is_none")]
        unit: Option<Unit>,
        /// The [`Metric`] value of the new benchmark run
        new: Metric,
        /// The [`Metric`] value of the old benchmark run
        old: Metric,
        /// The difference between new and old in percent. Serialized as string to preserve
        /// infinity values and avoid null in json.
        #[serde(with = "crate::serde::float_64")]
        #[cfg_attr(feature = "schema", schemars(with = "String"))]
        diff_pct: f64,
        /// The value of the limit which was exceeded to cause a performance regression. Serialized
        /// as string to preserve infinity values and avoid null in json.
        #[serde(with = "crate::serde::float_64")]
        #[cfg_attr(feature = "schema", schemars(with = "String"))]
        limit: f64,
    },
    /// A performance regression triggered by a hard limit
    Hard {
        /// The [`MetricKind`] per tool
        metric: MetricKind,
        /// An optional human-readable display label for the regression metric, used in formatted
        /// output.
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<String>,
        /// The unit of the metric values, if present.
        #[serde(skip_serializing_if = "Option::is_none")]
        unit: Option<Unit>,
        /// The [`Metric`] value of the benchmark run
        new: Metric,
        /// The difference between new and the limit as [`Metric`]
        diff: Metric,
        /// The limit as [`Metric`]
        limit: Metric,
    },
}

/// A baseline file used when comparing a new benchmark result with older data.
///
/// # Benchmark Summary
///
/// This struct is not part of the recent summary anymore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// The kind of the `Baseline`
    pub kind: BaselineKind,
    /// The path to the file which is used to compare against the new output
    pub path: PathBuf,
}

/// The user-visible name of a baseline.
///
/// # Benchmark Summary
///
/// This struct is not part of the recent summary anymore.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BaselineName(pub String);

/// A `BenchmarkSummary` which contains all collected data of a single benchmark run
///
/// This is the top-level type most consumers work with after deserializing a Gungraun summary file.
/// Its fields describe the benchmark run itself, while `profiles` contains the collected metric
/// data, differences, and any `ToolRegression` values.
///
/// The `module_path` together with the `id` can serve as a unique identifier of a benchmark run. If
/// the `id` is not present then the unique identifier is just the `module_path`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct BenchmarkSummary {
    /// The baselines if any.
    ///
    /// An absent first baseline indicates that new output was produced. An absent second baseline
    /// indicates the usage of the usual "*.old" output.
    pub baselines: (Option<String>, Option<String>),
    /// The path to the binary which is executed by Gungraun and in turn Valgrind.
    ///
    /// In case of a library benchmark this is the compiled benchmark file. In case of a binary
    /// benchmark this is the path to the executable.
    pub benchmark_exe: PathBuf,
    /// The path to the file containing this benchmark
    pub benchmark_file: PathBuf,
    /// More details describing this benchmark run
    pub details: Option<String>,
    /// The name of the function under test
    pub function_name: String,
    /// The user provided id of this benchmark
    pub id: Option<String>,
    /// Whether this summary describes a library or binary benchmark
    pub kind: BenchmarkKind,
    /// The rust path in the form `bench_file::group::bench`
    pub module_path: String,
    /// The directory containing all generated benchmark artifacts.
    ///
    /// This path, together with the other retained path fields, is relative to `project_root` when
    /// it is located below the project root. Otherwise, it is absolute.
    pub output_dir: PathBuf,
    /// The directory of the package
    pub package_dir: PathBuf,
    /// This is the container with all the benchmark data (metrics, differences, comparisons, ...)
    ///
    /// If there were no errors during the benchmark run, there is at least one [`Profile`]
    /// present.
    pub profiles: Profiles,
    /// The project's root directory
    pub project_root: PathBuf,
    /// The version string of this format.
    ///
    /// This is not semver and only major version numbers are used. There might be text occurrences
    /// of `v6` within this library documentation but v6 is stored as raw number `6` without the
    /// `v` prefix. Only backwards incompatible changes cause an increase of the version
    pub version: String,
}

/// Percentage and factor differences derived from two compared metric values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Diffs {
    /// The percentage of the difference between two `Metrics` serialized as string to preserve
    /// infinity values and avoid `null` in json
    #[serde(with = "crate::serde::float_64")]
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub diff_pct: f64,
    /// The factor of the difference between two `Metrics` serialized as string to preserve
    /// infinity values and avoid `null` in json
    #[serde(with = "crate::serde::float_64")]
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub factor: f64,
}

/// All flamegraph outputs recorded for a benchmark and their totals.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlamegraphSummaries {
    /// The `FlamegraphSummary`s
    pub summaries: Vec<FlamegraphSummary>,
    /// The totals over the `FlamegraphSummary`s
    pub totals: Vec<FlamegraphSummary>,
}

/// A flamegraph associated with a specific [`EventKind`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct FlamegraphSummary {
    /// The `EventKind` of the flamegraph
    pub event_kind: EventKind,
}

/// `Profile` data for one [`Tool`] recorded in a benchmark summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Profile {
    /// Details and information about the created flamegraphs if any
    pub flamegraphs: Vec<FlamegraphSummary>,
    /// The data with the metrics and details about the tool run
    pub summaries: ProfileData,
    /// The Valgrind tool like `DHAT`, `Memcheck` etc.
    pub tool: Tool,
}

/// All [`ProfilePart`]-level and [`ProfileTotal`] data of a single tool run.
///
/// The [`ProfileTotal`] is always present and summarizes all [`ProfilePart`]s. If the tool produced
/// only one part, the total matches that part's metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProfileData {
    /// All [`ProfilePart`]s
    pub parts: Vec<ProfilePart>,
    /// The total over the [`ProfilePart`]s
    pub total: ProfileTotal,
}

/// Metadata describing a single [`ProfilePart`] of a benchmark
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProfileInfo {
    /// The executed command
    pub command: String,
    /// More details for example from the logging output of the tool run
    pub details: Option<String>,
    /// The parent pid of this process if present
    pub parent_pid: Option<i32>,
    /// The part number of this tool run if present (only Callgrind and Perf)
    pub part: Option<u64>,
    /// The pid of the benchmark process
    pub pid: i32,
    /// The thread number of this tool run if present (only Callgrind)
    pub thread: Option<usize>,
}

/// A single part of a tool run with the collected metric data
///
/// A tool run can produce multiple parts, for example one per process when child tracing is
/// enabled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProfilePart {
    /// [`ProfileInfo`] like command, pid, ppid, thread number etc.
    pub details: EitherOrBoth<ProfileInfo>,
    /// The [`ToolMetricSummary`] containing the actual data
    pub metrics_summary: ToolMetricSummary,
}

/// Aggregated metrics, differences and regressions over all parts of a tool run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProfileTotal {
    /// The detected regressions if any
    pub regressions: Vec<ToolRegression>,
    /// The [`ToolMetricSummary`] of the tool containing the collected metric data
    pub summary: ToolMetricSummary,
}

/// Contains all [`Profile`]s with the data for each [`Tool`] run
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Profiles(pub Vec<Profile>);
