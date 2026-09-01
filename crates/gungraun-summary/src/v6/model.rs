//! Frozen version 6 summary data model.
//!
//! These types are copied from `gungraun-runner` as it produced version 6 summaries at the
//! latest `gungraun-summary` v6 release, including the `ErrorTool` variants used by Memcheck,
//! Helgrind and DRD. They are owned by this snapshot so parsing version 6 summaries keeps working
//! even if newer summary versions change the shared model in `gungraun-runner`.

use std::hash::Hash;
use std::path::PathBuf;

use either_or_both::EitherOrBoth;
use gungraun_runner::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind};
use gungraun_runner::metrics::model::Metric;
use indexmap::IndexMap;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::util::float_64;

/// The version string stored in version 6 summary JSON files.
pub const SCHEMA_VERSION: &str = "6";

/// Identifies whether a summary describes a library or binary benchmark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum BenchmarkKind {
    /// A library benchmark
    LibraryBenchmark,
    /// A binary benchmark
    BinaryBenchmark,
}

/// Identifies a metric kind by tool
///
/// This enum appears in places where a summary needs to describe a metric without separately
/// carrying the tool family that owns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum MetricKind {
    /// The `None` kind if there are no metrics for a tool (i.e. BBV and Massif)
    None,
    /// The Callgrind metric kind
    Callgrind(EventKind),
    /// The Cachegrind metric kind
    Cachegrind(CachegrindMetric),
    /// The DHAT metric kind
    Dhat(DhatMetric),
    /// The Memcheck metric kind
    Memcheck(ErrorMetric),
    /// The Helgrind metric kind
    Helgrind(ErrorMetric),
    /// The DRD metric kind
    DRD(ErrorMetric),
}

/// Identifies the format of a summary file written by Gungraun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum SummaryFormat {
    /// The format in a space optimal json representation without newlines
    Json,
    /// The format in pretty printed json
    PrettyJson,
}

/// A [`ValgrindTool`] metric data summary.
///
/// Each variant contains all metric data including the differences to the old or a
/// [`BenchmarkSummary::baselines`] run for a single [`ValgrindTool`]. The contained
/// [`MetricsSummary`] is keyed by the metric enum used by that tool.
///
/// The [`ToolMetricSummary::ErrorTool`] variant is used by Memcheck, Helgrind and DRD. Massif and
/// BBV are special cases because they do not have a metrics summary and therefore use the
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
/// use gungraun_summary::v6::{
///     Diffs, EventKind, Metric, MetricsDiff, MetricsSummary, ToolMetricSummary,
/// };
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ToolMetricSummary {
    /// If there are no metrics extracted (currently Massif, BBV)
    #[default]
    None,
    /// The summary of tools which report errors (Memcheck, Helgrind, DRD) ([`ErrorMetric`])
    ErrorTool(MetricsSummary<ErrorMetric>),
    /// The metric summary of [`DhatMetric`]s
    Dhat(MetricsSummary<DhatMetric>),
    /// The Callgrind summary of [`EventKind`]
    Callgrind(MetricsSummary<EventKind>),
    /// The summary of [`CachegrindMetric`]s
    Cachegrind(MetricsSummary<CachegrindMetric>),
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
        /// The [`Metric`] value of the new benchmark run
        new: Metric,
        /// The [`Metric`] value of the old benchmark run
        old: Metric,
        /// The difference between new and old in percent. Serialized as string to preserve
        /// infinity values and avoid null in json.
        #[serde(with = "float_64")]
        #[cfg_attr(feature = "schema", schemars(with = "String"))]
        diff_pct: f64,
        /// The value of the limit which was exceeded to cause a performance regression. Serialized
        /// as string to preserve infinity values and avoid null in json.
        #[serde(with = "float_64")]
        #[cfg_attr(feature = "schema", schemars(with = "String"))]
        limit: f64,
    },
    /// A performance regression triggered by a hard limit
    Hard {
        /// The [`MetricKind`] per tool
        metric: MetricKind,
        /// The [`Metric`] value of the benchmark run
        new: Metric,
        /// The difference between new and the limit as [`Metric`]
        diff: Metric,
        /// The limit as [`Metric`]
        limit: Metric,
    },
}

/// The Valgrind tools which can be run in a benchmark
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ValgrindTool {
    /// Callgrind: a call-graph generating cache and branch prediction profiler
    /// <https://valgrind.org/docs/manual/cl-manual.html>
    Callgrind,
    /// Cachegrind: a high-precision tracing profiler
    /// <https://valgrind.org/docs/manual/cg-manual.html>
    Cachegrind,
    /// DHAT: a dynamic heap analysis tool
    /// <https://valgrind.org/docs/manual/dh-manual.html>
    DHAT,
    /// Memcheck: a memory error detector
    /// <https://valgrind.org/docs/manual/mc-manual.html>
    Memcheck,
    /// Helgrind: a thread error detector
    /// <https://valgrind.org/docs/manual/hg-manual.html>
    Helgrind,
    /// DRD: a thread error detector
    /// <https://valgrind.org/docs/manual/drd-manual.html>
    DRD,
    /// Massif: a heap profiler
    /// <https://valgrind.org/docs/manual/ms-manual.html>
    Massif,
    /// BBV: an experimental basic block vector generation tool
    /// <https://valgrind.org/docs/manual/bbv-manual.html>
    BBV,
}

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
    /// The directory of the package
    pub package_dir: PathBuf,
    /// This is the container with all the benchmark data (metrics, differences, comparisons, ...)
    ///
    /// If there were no errors during the benchmark run, there is at least one [`Profile`]
    /// present.
    pub profiles: Profiles,
    /// The project's root directory
    pub project_root: PathBuf,
    /// The destination and kind of the summary file
    pub summary_output: Option<SummaryOutput>,
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
    #[serde(with = "float_64")]
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub diff_pct: f64,
    /// The factor of the difference between two `Metrics` serialized as string to preserve
    /// infinity values and avoid `null` in json
    #[serde(with = "float_64")]
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub factor: f64,
}

/// File paths for one flamegraph associated with a specific [`EventKind`].
///
/// At least one of `regular_path`, `base_path`, or `diff_path` is present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct FlamegraphSummary {
    /// If present, the path to the file of the old regular (non-differential) flamegraph
    pub base_path: Option<PathBuf>,
    /// If present, the path to the file of the differential flamegraph
    pub diff_path: Option<PathBuf>,
    /// The `EventKind` of the flamegraph
    pub event_kind: EventKind,
    /// If present, the path to the file of the regular (non-differential) flamegraph
    pub regular_path: Option<PathBuf>,
}

/// Comparison data for one metric in a parsed summary.
///
/// If both, old and new values, are present, [`Diffs`] stores the derived percentage and factor.
/// Otherwise the summary only stores whichever side is available. Per convention, the left side or
/// [`EitherOrBoth::Left`] stores the new [`Metric`] and the right side or [`EitherOrBoth::Right`]
/// stores the old metric.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct MetricsDiff {
    /// If both metrics ([`EitherOrBoth::Both`]) are present there is also a `Diffs` present
    pub diffs: Option<Diffs>,
    /// Either the `new` ([`EitherOrBoth::Left`]), `old` ([`EitherOrBoth::Right`]) or both metrics
    pub metrics: EitherOrBoth<Metric>,
}

/// An insertion-ordered mapping from metric identifier to [`MetricsDiff`].
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct MetricsSummary<K: Hash + Eq = EventKind>(pub IndexMap<K, MetricsDiff>);

/// `Profile` data for one [`ValgrindTool`] recorded in a benchmark summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Profile {
    /// Details and information about the created flamegraphs if any
    pub flamegraphs: Vec<FlamegraphSummary>,
    /// The paths to the `*.log` files. All tools produce at least one log file
    pub log_paths: Vec<PathBuf>,
    /// The paths to the `*.out` files. Not all tools produce an output in addition to the log
    /// files
    pub out_paths: Vec<PathBuf>,
    /// The data with the metrics and details about the tool run
    pub summaries: ProfileData,
    /// The Valgrind tool like `DHAT`, `Memcheck` etc.
    pub tool: ValgrindTool,
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
    /// The part number of this tool run if present (only Callgrind)
    pub part: Option<u64>,
    /// The path to the output file containing the data of the tool run
    pub path: PathBuf,
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

/// Contains all [`Profile`]s with the data for each [`ValgrindTool`] run
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Profiles(pub Vec<Profile>);

/// Describes where Gungraun wrote the summary file and in which format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SummaryOutput {
    /// The [`SummaryFormat`]
    pub format: SummaryFormat,
    /// The path to the destination file of this summary
    pub path: PathBuf,
}
