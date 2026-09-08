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

use std::hash::Hash;
use std::path::PathBuf;

use either_or_both::EitherOrBoth;
use indexmap::IndexMap;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::de::{DeserializeOwned, Error as _};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind, PerfMetric, Tool};
use crate::metrics::model::{
    AnnotatedMetric, Metric, MetricKind, Metrics, MetricsSummary, PerfQualities,
};
use crate::units::Unit;

/// The version string stored in version summary JSON files.
pub const SCHEMA_VERSION: &str = "7";

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
#[derive(Debug, Clone, Default, PartialEq)]
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
        #[cfg_attr(feature = "schema", schemars(with = "String"))]
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
        #[cfg_attr(feature = "schema", schemars(with = "String"))]
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
    /// The path to the binary which is executed by Gungraun and in turn Valgrind or Perf.
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
    /// The name of the `benchmark_group`
    pub group: String,
    /// The optional id of this benchmark.
    ///
    /// Th id is provided by the `#[bench::id]` attribute macro. The `#[benches::id]` adds a
    /// counter as suffix in the form `id_X` for example (`id_0`, `id_100`, ...)
    ///
    /// A gungraun benchmark can be uniquely identified by the `module_path`
    /// (`benchmark_file::group::function_name`) and this `id`. If the `id` is not present, then
    /// the `module_path` is sufficient to identify a benchmark.
    pub id: Option<String>,
    /// Whether this summary describes a library or binary benchmark
    pub kind: BenchmarkKind,
    /// The rust path in the form `benchmark_file::group::function_name`
    ///
    /// If the `id` is not present, this `module_path` is sufficient to uniquely identify a
    /// gungraun benchmark.
    pub module_path: String,
    /// The directory containing all generated benchmark artifacts.
    ///
    /// This path, together with the other retained path fields, is relative to `project_root` when
    /// it is located below the project root. Otherwise, it is absolute.
    pub output_dir: PathBuf,
    /// The relative path to the directory of the cargo package containing the benchmark
    pub package_dir: PathBuf,
    /// This is the container with all the benchmark data (metrics, differences, comparisons, ...)
    ///
    /// If there were no errors during the benchmark run, there is at least one [`Profile`]
    /// present.
    #[serde(deserialize_with = "deserialize_profiles")]
    pub profiles: Profiles,
    /// The project's absolute root directory
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
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub event_kind: EventKind,
}

/// `Profile` data for one [`Tool`] recorded in a benchmark summary.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Profile {
    /// Details and information about the created flamegraphs if any
    pub flamegraphs: Vec<FlamegraphSummary>,
    /// The data with the metrics and details about the tool run
    pub summaries: ProfileData,
    /// The Valgrind tool like `DHAT`, `Memcheck` etc.
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub tool: Tool,
}

/// All [`ProfilePart`]-level and [`ProfileTotal`] data of a single tool run.
///
/// The [`ProfileTotal`] is always present and summarizes all [`ProfilePart`]s. If the tool produced
/// only one part, the total matches that part's metrics.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProfileData {
    /// All [`ProfilePart`]s
    pub parts: Vec<ProfilePart>,
    /// The total over the [`ProfilePart`]s
    pub total: ProfileTotal,
}

#[derive(Deserialize)]
struct ProfileDataWire {
    parts: Vec<ProfilePartWire>,
    total: ProfileTotalWire,
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
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProfilePart {
    /// [`ProfileInfo`] like command, pid, ppid, thread number etc.
    pub details: EitherOrBoth<ProfileInfo>,
    /// The [`ToolMetricSummary`] containing the actual data
    #[cfg_attr(feature = "schema", schemars(schema_with = "metric_summary_schema"))]
    pub metrics_summary: ToolMetricSummary,
}

#[derive(Deserialize)]
struct ProfilePartWire {
    details: EitherOrBoth<ProfileInfo>,
    metrics_summary: IndexMap<String, Value>,
}

/// Aggregated metrics, differences and regressions over all parts of a tool run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProfileTotal {
    /// The detected regressions if any
    pub regressions: Vec<ToolRegression>,
    /// The [`ToolMetricSummary`] of the tool containing the collected metric data
    #[cfg_attr(feature = "schema", schemars(schema_with = "metric_summary_schema"))]
    pub summary: ToolMetricSummary,
}

#[derive(Deserialize)]
struct ProfileTotalWire {
    regressions: Vec<Value>,
    summary: IndexMap<String, Value>,
}

#[derive(Deserialize)]
struct ProfileWire {
    flamegraphs: Vec<Value>,
    summaries: ProfileDataWire,
    tool: Tool,
}

/// Contains all [`Profile`]s with the data for each [`Tool`] run
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Profiles(pub Vec<Profile>);

impl<'de> Deserialize<'de> for Profile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(ProfileWire::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl TryFrom<ProfileWire> for Profile {
    type Error = serde_json::Error;

    fn try_from(wire: ProfileWire) -> Result<Self, Self::Error> {
        let flamegraphs = parse_typed_values(wire.flamegraphs);
        let parts = wire
            .summaries
            .parts
            .into_iter()
            .map(|part| {
                let metrics_summary = parse_metrics_summary(wire.tool, part.metrics_summary)?;
                Ok(ProfilePart {
                    details: part.details,
                    metrics_summary,
                })
            })
            .collect::<Result<Vec<_>, serde_json::Error>>()?;

        let total = parse_metrics_summary(wire.tool, wire.summaries.total.summary)?;
        let regressions = parse_typed_values(wire.summaries.total.regressions);

        Ok(Self {
            flamegraphs,
            summaries: ProfileData {
                parts,
                total: ProfileTotal {
                    regressions,
                    summary: total,
                },
            },
            tool: wire.tool,
        })
    }
}

impl Serialize for ToolMetricSummary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::None => serializer.serialize_map(Some(0))?.end(),
            Self::Memcheck(summary) | Self::Helgrind(summary) | Self::DRD(summary) => {
                summary.serialize(serializer)
            }
            Self::Dhat(summary) => summary.serialize(serializer),
            Self::Callgrind(summary) => summary.serialize(serializer),
            Self::Cachegrind(summary) => summary.serialize(serializer),
            Self::Perf(summary) => summary.serialize(serializer),
        }
    }
}

fn deserialize_profiles<'de, D>(deserializer: D) -> Result<Profiles, D::Error>
where
    D: Deserializer<'de>,
{
    let mut profiles = Vec::new();

    for value in Vec::<Value>::deserialize(deserializer)? {
        let tool = value
            .get("tool")
            .ok_or_else(|| D::Error::custom("missing field `tool`"))?
            .as_str()
            .ok_or_else(|| D::Error::custom("field `tool` must be a string"))?;
        if serde_json::from_value::<Tool>(Value::String(tool.to_owned())).is_err() {
            continue;
        }
        profiles.push(serde_json::from_value(value).map_err(D::Error::custom)?);
    }

    Ok(Profiles(profiles))
}

#[cfg(feature = "schema")]
fn metric_summary_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let metric_diff = generator.subschema_for::<crate::metrics::model::MetricsDiff>();
    let perf_metric_diff = generator
        .subschema_for::<crate::metrics::model::MetricsDiff<AnnotatedMetric<PerfQualities>>>();

    schemars::json_schema!({
        "type": "object",
        "additionalProperties": {
            "anyOf": [metric_diff, perf_metric_diff]
        }
    })
}

fn parse_metrics<K, V>(
    metrics: IndexMap<String, Value>,
    wrap: impl FnOnce(MetricsSummary<K, V>) -> ToolMetricSummary,
) -> Result<ToolMetricSummary, serde_json::Error>
where
    K: DeserializeOwned + Eq + Hash,
    V: DeserializeOwned,
{
    let mut typed = IndexMap::new();
    for (name, value) in metrics {
        if let Ok(key) = serde_json::from_value(Value::String(name)) {
            typed.insert(key, serde_json::from_value(value)?);
        }
    }

    Ok(wrap(MetricsSummary(typed)))
}

fn parse_metrics_summary(
    tool: Tool,
    metrics: IndexMap<String, Value>,
) -> Result<ToolMetricSummary, serde_json::Error> {
    match tool {
        Tool::Callgrind => parse_metrics(metrics, ToolMetricSummary::Callgrind),
        Tool::Cachegrind => parse_metrics(metrics, ToolMetricSummary::Cachegrind),
        Tool::DHAT => parse_metrics(metrics, ToolMetricSummary::Dhat),
        Tool::Memcheck => parse_metrics(metrics, ToolMetricSummary::Memcheck),
        Tool::Helgrind => parse_metrics(metrics, ToolMetricSummary::Helgrind),
        Tool::DRD => parse_metrics(metrics, ToolMetricSummary::DRD),
        Tool::Perf => parse_metrics(metrics, ToolMetricSummary::Perf),
        Tool::Massif | Tool::BBV => Ok(ToolMetricSummary::None),
    }
}

fn parse_typed_values<T>(values: Vec<Value>) -> Vec<T>
where
    T: DeserializeOwned,
{
    values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{BenchmarkSummary, Profile, ToolMetricSummary};

    fn benchmark_summary(profiles: &[Value]) -> Value {
        json!({
            "baselines": [null, null],
            "benchmark_exe": "benchmark",
            "benchmark_file": "benches/benchmark.rs",
            "details": null,
            "function_name": "benchmark",
            "group": "group",
            "id": null,
            "kind": "LibraryBenchmark",
            "module_path": "benchmark::group::benchmark",
            "output_dir": "target/gungraun",
            "package_dir": ".",
            "profiles": profiles,
            "project_root": "/project",
            "version": "7"
        })
    }

    fn profile(tool: &str, metric: &str) -> Value {
        json!({
            "flamegraphs": [],
            "summaries": {
                "parts": [{
                    "details": {
                        "Left": {
                            "command": "benchmark",
                            "details": null,
                            "parent_pid": null,
                            "part": 0,
                            "pid": 42,
                            "thread": null
                        }
                    },
                    "metrics_summary": {
                        metric: {
                            "diffs": null,
                            "metrics": { "Left": { "Int": 100 } }
                        }
                    }
                }],
                "total": {
                    "regressions": [],
                    "summary": {}
                }
            },
            "tool": tool
        })
    }

    #[test]
    fn test_malformed_profile_tool_is_rejected() {
        let mut missing = profile("Callgrind", "Ir");
        missing.as_object_mut().unwrap().remove("tool");
        let mut non_string = profile("Callgrind", "Ir");
        non_string["tool"] = json!(42);

        for malformed in [missing, non_string] {
            serde_json::from_value::<BenchmarkSummary>(benchmark_summary(&[malformed]))
                .unwrap_err();
        }
    }

    #[test]
    fn test_none_metric_summary_serializes_as_empty_object() {
        assert_eq!(
            serde_json::to_value(ToolMetricSummary::None).unwrap(),
            json!({})
        );
    }

    #[test]
    fn test_profile_round_trips_flat_known_metrics() {
        let profile: Profile = serde_json::from_value(profile("Callgrind", "Ir")).unwrap();
        let serialized = serde_json::to_value(profile).unwrap();

        assert_eq!(
            serialized["summaries"]["parts"][0]["metrics_summary"]["Ir"]["metrics"]["Left"]["Int"],
            100
        );
        assert!(serialized["summaries"]["parts"][0]["metrics_summary"]["Callgrind"].is_null());
    }

    #[test]
    fn test_regression_optional_fields_remain_absent() {
        let mut input = profile("Callgrind", "Ir");
        input["summaries"]["total"]["regressions"] = json!([{
            "Soft": {
                "metric": { "Callgrind": "Ir" },
                "new": { "Int": 100 },
                "old": { "Int": 90 },
                "diff_pct": "11.11111111111111",
                "limit": "10"
            }
        }]);

        let profile: Profile = serde_json::from_value(input).unwrap();
        let serialized = serde_json::to_value(profile).unwrap();
        let regression = &serialized["summaries"]["total"]["regressions"][0]["Soft"];

        assert!(regression.get("display").is_none());
        assert!(regression.get("unit").is_none());
        assert_eq!(regression["diff_pct"], "11.11111111111111");
    }

    #[test]
    fn test_unknown_profile_data_is_dropped() {
        let mut input = profile("Callgrind", "FutureMetric");
        input["flamegraphs"] = json!([{ "event_kind": "FutureEvent" }]);
        input["summaries"]["total"]["regressions"] = json!([{
            "Soft": {
                "metric": { "Callgrind": "FutureMetric" },
                "new": { "Int": 100 },
                "old": { "Int": 90 },
                "diff_pct": "11.11111111111111",
                "limit": "10"
            }
        }]);
        input["summaries"]["total"]["summary"] =
            input["summaries"]["parts"][0]["metrics_summary"].clone();

        let profile: Profile = serde_json::from_value(input).unwrap();
        let serialized = serde_json::to_value(profile).unwrap();

        assert_eq!(serialized["flamegraphs"], json!([]));
        assert_eq!(
            serialized["summaries"]["parts"][0]["metrics_summary"],
            json!({})
        );
        assert_eq!(serialized["summaries"]["total"]["regressions"], json!([]));
        assert_eq!(serialized["summaries"]["total"]["summary"], json!({}));
    }

    #[test]
    fn test_unknown_profiles_are_dropped() {
        let input = benchmark_summary(&[
            profile("FutureTool", "FutureMetric"),
            profile("Callgrind", "Ir"),
            profile("Cachegrind", "Ir"),
        ]);

        let summary: BenchmarkSummary = serde_json::from_value(input).unwrap();
        let serialized = serde_json::to_value(summary).unwrap();
        let tools = serialized["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|profile| profile["tool"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(tools, ["Callgrind", "Cachegrind"]);
    }
}
