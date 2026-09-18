//! Metric value and comparison types
//!
//! These types describe metric values, per-metric diffs, and grouped metric summaries.
//!
//! The model contains the non-derive implementations of [`PartialEq`], [`Eq`] for [`Metric`] and
//! not the [`metrics::logic`][super::logic].

use std::cmp::Ordering;
use std::hash::Hash;

use either_or_both::EitherOrBoth;
use indexmap::IndexMap;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind, PerfMetric};
use crate::summary::model::Diffs;
use crate::units::Unit;

/// The value type used for metrics measured by a benchmark tool
///
/// Raw metrics emitted by Valgrind tools are [`Metric::Int`] which is the default metric type.
/// Metrics that have [`Metric::Float`] type are documented as such. Derived values, such as miss
/// rates and hit rates, require floating-point representation. `Metric` preserves both forms in the
/// parsed summary model.
///
/// # Developer Notes
///
/// Float operations with a `Metric` that stores a `u64` introduce a precision loss and are to be
/// avoided. Especially comparison between a `u64` metric and `f64` metric are not exact because the
/// `u64` has to be converted to a `f64`. Also, if adding/multiplying two `u64` metrics would result
/// in an overflow the metric saturates at `u64::MAX`. This choice was made to preserve precision
/// and the original type (instead of for example adding the two `u64` by converting both of them to
/// `f64`).
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum Metric {
    /// An integer `Metric`
    Int(u64),
    /// A float `Metric`
    Float(f64),
}

/// Identifies a metric kind by tool
///
/// This enum appears in places where a summary needs to describe a metric without separately
/// carrying the tool family that owns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricKind {
    /// The `None` kind if there are no metrics for a tool (i.e. BBV and Massif)
    None,
    /// The Callgrind metric kind: [`EventKind`]
    Callgrind(EventKind),
    /// The Cachegrind metric kind: [`CachegrindMetric`]
    Cachegrind(CachegrindMetric),
    /// The DHAT metric kind: [`DhatMetric`]
    Dhat(DhatMetric),
    /// The Memcheck metric kind: [`ErrorMetric`]
    Memcheck(ErrorMetric),
    /// The Helgrind metric kind: [`ErrorMetric`]
    Helgrind(ErrorMetric),
    /// The DRD metric kind: [`ErrorMetric`]
    DRD(ErrorMetric),
    /// The Perf metric kind: [`PerfMetric`]
    Perf(PerfMetric),
}

/// A per-tool collection of raw metric values.
///
/// This enum is used where the summary needs to store metrics keyed by the tool that produced them,
/// without comparison metadata.
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

/// A metric value paired with additional metadata and an optional [`Unit`].
///
/// This type is used for metrics, such as perf results, that need to carry more than the raw
/// numeric value when they are stored, merged, or compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct AnnotatedMetric<Q> {
    /// Additional metadata associated with the metric value.
    #[serde(flatten)]
    pub qualities: Q,
    /// The [`Unit`] of the metric value, if one is given or known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
    /// The measured numeric value.
    pub value: Metric,
}

/// An insertion-ordered mapping from metric identifier to [`Metric`].
///
/// # Benchmark Summary
///
/// This struct is not part of the recent summary anymore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metrics<K: Hash + Eq, V = Metric>(pub IndexMap<K, V>);

/// Comparison data for one metric in a parsed summary.
///
/// If both, old and new values, are present, [`Diffs`] stores the derived percentage and factor.
/// Otherwise the summary only stores whichever side is available. Per convention, the left side or
/// [`EitherOrBoth::Left`] stores the new [`Metric`] and the right side or [`EitherOrBoth::Right`]
/// stores the old metric.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(bound(serialize = "V: Serialize", deserialize = "V: Deserialize<'de>"))]
pub struct MetricsDiff<V = Metric> {
    /// If both values are present there is also a `diffs` present
    pub diffs: Option<Diffs>,
    /// Either the `new`, `old` or both values
    #[serde(with = "crate::serde::either_or_both")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "crate::serde::either_or_both::NewOldOrBoth<V, V>")
    )]
    pub values: EitherOrBoth<V>,
}

/// An insertion-ordered mapping from metric identifier to [`MetricsDiff`].
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct MetricsSummary<K: Hash + Eq = EventKind, V = Metric>(pub IndexMap<K, MetricsDiff<V>>);

/// Perf-specific metadata attached to a metric value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct PerfQualities {
    /// Runtime reported by perf for the event
    ///
    /// This field is extracted directly from perf's JSON field with the same name.
    ///
    /// Together with `pcnt_running`, this forms a coupled value during merges.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_runtime: Option<u64>,
    /// The mean value computed for this metric, if available.
    ///
    /// This value is computed by Gungraun, if there were multiple perf records for a metric (for
    /// example using sampling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean: Option<f64>,
    /// The number of repetitions or samples perf used for this metric, if reported.
    ///
    /// This value is computed by Gungraun, if there were multiple perf records for a metric (for
    /// example using sampling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u64>,
    /// Percentage of enabled time reported by perf during which the event was running.
    ///
    /// This field is extracted directly from perf's JSON field with the same name.
    ///
    /// This value is only meaningful together with `event_runtime`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcnt_running: Option<f64>,
    /// Gungraun's correct name for perf's relative standard error ("variance"), as a fraction.
    ///
    /// Perf's own JSON calls this field `"variance"`, but the value is not a statistical variance.
    /// It corresponds to the percentage shown in perf text output as `( +-X.XX% )`, but Gungraun
    /// stores it as a fraction instead, so `5%` is stored as `0.05`.
    ///
    /// Gungraun stores this value in summaries under the name `rse`. For a single parsed perf
    /// record the value is preserved from perf JSON. When duplicate records for the same event are
    /// merged, it is recomputed by Gungraun from the aggregated samples.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rse: Option<f64>,
}

impl Eq for Metric {}

impl PartialEq for Metric {
    #[expect(clippy::cast_precision_loss)]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b) == Ordering::Equal,
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)) == Ordering::Equal,
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b) == Ordering::Equal,
        }
    }
}

#[cfg(feature = "schema")]
impl JsonSchema for MetricKind {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "MetricKind".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "anyOf": [
                { "type": "string" },
                { "type": "object", "additionalProperties": true }
            ]
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_annotated_metric_deserializes_value_field() {
        let deserialized: AnnotatedMetric<PerfQualities> =
            serde_json::from_value(json!({ "value": 5 })).unwrap();

        assert_eq!(deserialized.value, Metric::Int(5));
        assert_eq!(deserialized.qualities, PerfQualities::default());
        assert_eq!(deserialized.unit, None);
    }

    #[test]
    fn test_annotated_metric_serializes_named_value_field() {
        let annotated = AnnotatedMetric {
            qualities: PerfQualities {
                mean: Some(2.0),
                ..PerfQualities::default()
            },
            unit: None,
            value: Metric::Float(1.5),
        };

        assert_eq!(
            serde_json::to_value(&annotated).unwrap(),
            json!({ "mean": 2.0, "value": 1.5 })
        );

        let roundtrip: AnnotatedMetric<PerfQualities> =
            serde_json::from_value(serde_json::to_value(&annotated).unwrap()).unwrap();
        assert_eq!(roundtrip, annotated);
    }

    #[test]
    #[cfg(feature = "schema")]
    fn test_metric_kind_schema_is_open_for_string_and_object_kinds() {
        let schema = serde_json::to_value(schemars::schema_for!(MetricKind)).unwrap();

        assert_eq!(
            schema["anyOf"],
            json!([
                { "type": "string" },
                { "type": "object", "additionalProperties": true }
            ])
        );
    }

    #[test]
    fn test_metrics_diff_serializes_values_as_new_and_old() {
        let metrics_diff = MetricsDiff {
            diffs: None,
            values: EitherOrBoth::Both(Metric::Int(2), Metric::Int(1)),
        };

        assert_eq!(
            serde_json::to_value(metrics_diff).unwrap(),
            json!({
                "diffs": null,
                "values": { "new": 2, "old": 1 }
            })
        );
    }

    // Untagged JSON numbers cannot express non-finite floats. This is the accepted trade-off of
    // serializing `Metric::Float` as plain JSON numbers instead of strings: `serde_json` maps
    // `NaN` and infinities to `null`, which cannot round-trip back into `Metric`.
    #[rstest]
    #[case::nan(f64::NAN)]
    #[case::infinity(f64::INFINITY)]
    #[case::neg_infinity(f64::NEG_INFINITY)]
    fn test_metric_serde_non_finite_floats_map_to_null(#[case] float: f64) {
        assert_eq!(
            serde_json::to_string(&Metric::Float(float)).unwrap(),
            "null"
        );
    }

    #[rstest]
    #[case::int_zero(Metric::Int(0))]
    #[case::int(Metric::Int(42))]
    #[case::int_max(Metric::Int(u64::MAX))]
    #[case::float_zero(Metric::Float(0.0))]
    #[case::float_negative(Metric::Float(-2.5))]
    #[case::float(Metric::Float(42.0))]
    #[case::float_max(Metric::Float(f64::MAX))]
    fn test_metric_serde_roundtrip(#[case] metric: Metric) {
        let deserialized: Metric =
            serde_json::from_value(serde_json::to_value(metric).unwrap()).unwrap();
        assert_eq!(deserialized, metric);
    }
}
