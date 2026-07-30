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

/// A metric value paired with additional metadata and an optional [`Unit`].
///
/// This type is used for metrics, such as perf results, that need to carry more than the raw
/// numeric value when they are stored, merged, or compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct AnnotatedMetric<Q> {
    /// The measured numeric value.
    #[serde(flatten)]
    pub metric: Metric,
    /// Additional metadata associated with the metric value.
    #[serde(flatten)]
    pub qualities: Q,
    /// The [`Unit`] of the metric value, if one is given or known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
}

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
#[cfg_attr(feature = "schema", derive(JsonSchema))]
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
pub struct MetricsDiff<V = Metric> {
    /// If both metrics ([`EitherOrBoth::Both`]) are present there is also a `Diffs` present
    pub diffs: Option<Diffs>,
    /// Either the `new` ([`EitherOrBoth::Left`]), `old` ([`EitherOrBoth::Right`]) or both metrics
    pub metrics: EitherOrBoth<V>,
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
