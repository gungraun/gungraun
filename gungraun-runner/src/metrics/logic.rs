//! Runner-side logic for the [`metrics::model`][super::model] types.
//!
//! This module provides the metric traits and implements the internal behavior of metrics.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::{Add, AddAssign, Div, Mul, Sub};
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use either_or_both::EitherOrBoth;
use indexmap::IndexMap;

use crate::api::{Limit, PerfMetric};
use crate::metrics::model::{
    AnnotatedMetric, Metric, MetricKind, Metrics, MetricsDiff, MetricsSummary, PerfQualities,
};
use crate::summary::model::Diffs;
use crate::units::Unit;
use crate::util::{Union, to_string_unsigned_short};

/// Numeric behavior required by metric containers.
pub trait MetricValue: Clone {
    /// Adds two metric values.
    #[must_use]
    fn add(&self, other: &Self) -> Self;
    /// Returns the numeric metric used for ordering and diffs.
    #[must_use]
    fn metric(&self) -> Metric;
    /// Returns this value normalized into its canonical representation.
    ///
    /// Implementations use this to rescale values into a stable form before they are stored in
    /// summaries or compared.
    #[must_use]
    fn normalize(&self) -> Self;
    /// Normalizes `self` and `other` into a common representation suitable for comparison.
    ///
    /// This returns `None` when both values cannot be expressed in a compatible shared [`Unit`].
    /// When it returns `Some((lhs, rhs))`, both normalized values are safe to use for arithmetic,
    /// ordering, and diff calculations.
    #[must_use]
    fn normalize_with(&self, other: &Self) -> Option<(Self, Self)>;
    /// Subtracts `other` from `self`, saturating (float) results at zero.
    #[must_use]
    fn saturating_sub(&self, other: &Self) -> Self;
    /// Returns only the numeric portion of this value as a string.
    ///
    /// This is used when the caller needs a stable textual representation of the measured value
    /// without appending any display [`Unit`].
    fn to_string_without_unit(&self) -> String;
    /// Returns the [`Unit`] associated with this metric value, if any.
    fn unit(&self) -> Option<&Unit>;
}

/// Trait for tools which summarize and calculate derived metrics
pub trait Summarize<V = Metric>: Hash + Eq + Clone
where
    V: Clone,
{
    /// Calculate the derived metrics if any
    fn summarize(_: &mut Cow<Metrics<Self, V>>) {}
}

/// Trait for checking the [`Metric`] type of a metric kind (like [`api::EventKind`])
///
/// [`api::EventKind`]: crate::api::EventKind
pub trait TypeChecker {
    /// Returns `true` if the metric kind is a [`Metric::Float`].
    fn is_float(&self) -> bool;
    /// Returns `true` if the metric kind is a [`Metric::Int`].
    fn is_int(&self) -> bool;
    /// Returns `true` if the `Metric` has the expected metric type.
    fn verify_metric(&self, metric: Metric) -> bool {
        (self.is_int() && metric.is_int()) || (self.is_float() && metric.is_float())
    }
}

impl<Q> AnnotatedMetric<Q> {
    /// Creates an `AnnotatedMetric` from a numeric value, metadata, and an optional [`Unit`].
    pub fn new<M, U>(metric: M, qualities: Q, unit: U) -> Self
    where
        M: Into<Metric>,
        U: Into<Option<Unit>>,
    {
        Self {
            metric: metric.into(),
            qualities,
            unit: unit.into(),
        }
    }
}

impl AnnotatedMetric<PerfQualities> {
    /// Creates a perf metric with default [`PerfQualities`] and an optional [`Unit`].
    pub fn with_default_qualities<M, U>(metric: M, unit: U) -> Self
    where
        M: Into<Metric>,
        U: Into<Option<Unit>>,
    {
        Self::new(metric, PerfQualities::default(), unit)
    }

    /// Returns this metric value converted into the canonical base scale of its [`Unit`].
    ///
    /// Unit-less metrics are returned unchanged.
    ///
    /// ```
    /// use gungraun_runner::metrics::model::{AnnotatedMetric, Metric, PerfQualities};
    /// use gungraun_runner::units::Unit;
    ///
    /// let duration = AnnotatedMetric::new(750.0, PerfQualities::default(), Unit::Milliseconds);
    ///
    /// assert_eq!(duration.base_value(), 0.75);
    ///
    /// let bytes = AnnotatedMetric::new(2.0, PerfQualities::default(), Unit::Kilobytes);
    ///
    /// assert_eq!(bytes.base_value(), 2000.0);
    /// ```
    #[expect(clippy::cast_precision_loss)]
    pub fn base_value(&self) -> f64 {
        match self.metric {
            Metric::Int(value) => self
                .unit
                .as_ref()
                .map_or(value as f64, |unit| unit.base_value(value as f64)),
            Metric::Float(value) => self
                .unit
                .as_ref()
                .map_or(value, |unit| unit.base_value(value)),
        }
    }

    /// Converts a canonical base-scale value into this metric's current [`Unit`].
    ///
    /// This is useful when values are accumulated in a canonical scale but need to be written back
    /// using the metric's current display unit.
    ///
    /// Unit-less metrics return the input unchanged.
    pub fn rebase(&self, value: f64) -> f64 {
        self.unit.as_ref().map_or(value, |unit| unit.rebase(value))
    }

    /// Creates the averaged metric value for a merged perf event from a canonical mean.
    ///
    /// `canonical_mean` must be in the canonical base scale returned by
    /// [`AnnotatedMetric::base_value`]. This function converts that mean back into the metric's
    /// current unit and then reapplies normal display scaling for float metrics.
    #[expect(clippy::cast_possible_truncation)]
    #[expect(clippy::cast_sign_loss)]
    pub fn into_mean(self, canonical_mean: f64) -> Self {
        match self.metric {
            Metric::Int(_) => {
                let new_value = self.rebase(canonical_mean);
                match self.unit {
                    Some(unit) => {
                        let (rescaled, unit) = unit.rescale(new_value);
                        Self {
                            metric: Metric::Int(rescaled.round() as u64),
                            unit: Some(unit),
                            qualities: self.qualities,
                        }
                    }
                    None => Self {
                        metric: Metric::Int(new_value.round() as u64),
                        unit: None,
                        qualities: self.qualities,
                    },
                }
            }
            Metric::Float(_) => {
                let new_value = self.rebase(canonical_mean);
                match self.unit {
                    Some(unit) => Self {
                        metric: Metric::Float(new_value),
                        unit: Some(unit),
                        qualities: self.qualities,
                    }
                    .normalize(),
                    None => Self {
                        metric: Metric::Float(new_value),
                        unit: None,
                        qualities: self.qualities,
                    },
                }
            }
        }
    }
}

impl<Q> Display for AnnotatedMetric<Q> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.unit {
            Some(unit) => write!(f, "{} {unit}", self.metric),
            None => self.metric.fmt(f),
        }
    }
}

impl From<Metric> for AnnotatedMetric<PerfQualities> {
    fn from(metric: Metric) -> Self {
        Self {
            metric,
            unit: None,
            qualities: PerfQualities::default(),
        }
    }
}

impl MetricValue for AnnotatedMetric<PerfQualities> {
    fn metric(&self) -> Metric {
        self.metric
    }

    /// Adds another perf metric to this one returning the normalized result.
    ///
    /// Both operands are first normalized into a shared [`Unit`] and scale with
    /// [`MetricValue::normalize_with`]. The summed result is normalized again before it is
    /// returned.
    ///
    /// When both metrics carry [`PerfQualities`], their quality data is merged with
    /// [`PerfQualities::add`]. This keeps metadata such as `event_runtime`, `pcnt_running`, and `n`
    /// in sync with the combined metric value, while fields without a sound merge rule, such as
    /// `rse`, are discarded.
    ///
    /// # Panics
    ///
    /// Panics if the two metrics cannot be normalized into a shared unit, for example when their
    /// units are incompatible or when one metric has a unit and the other is unit-less.
    fn add(&self, other: &Self) -> Self {
        let (this_normalized, other_normalized) = self
            .normalize_with(other)
            .expect("Only compatible units should be summed up");
        let metric = this_normalized.metric + other_normalized.metric;

        Self {
            metric,
            unit: this_normalized.unit,
            qualities: this_normalized.qualities.add(&other_normalized.qualities),
        }
        .normalize()
    }

    /// Subtracts another perf metric from this one, saturating results at zero.
    ///
    /// Like in [`Self::add`], both operands are first normalized into a shared unit and scale with
    /// [`MetricValue::normalize_with`]. The difference is then normalized again before it is
    /// returned.
    ///
    /// Unlike [`MetricValue::add`], subtraction does not attempt to preserve or merge
    /// [`PerfQualities`]. The returned metric always uses default perf qualities, because the
    /// existing metadata fields do not have well-defined subtraction semantics.
    ///
    /// # Panics
    ///
    /// Panics if the two metrics cannot be normalized into a shared unit, for example when their
    /// units are incompatible or when one metric has a unit and the other is unit-less.
    fn saturating_sub(&self, other: &Self) -> Self {
        let (this_normalized, other_normalized) = self
            .normalize_with(other)
            .expect("Only compatible units should be subtracted");

        let metric = this_normalized
            .metric
            .saturating_sub(&other_normalized.metric);

        Self {
            metric,
            unit: this_normalized.unit,
            qualities: PerfQualities::default(),
        }
        .normalize()
    }

    fn to_string_without_unit(&self) -> String {
        self.metric.to_string()
    }

    fn unit(&self) -> Option<&Unit> {
        self.unit.as_ref()
    }

    /// Returns this value normalized into a stable scale for its [`Unit`].
    ///
    /// For finite, non-zero floating-point metrics with a unit, normalization chooses the unit
    /// returned by [`Unit::rescale`] for the current numeric value. This prefers a representation
    /// whose magnitude is easier to read than the original one, for example turning `1500 ms` into
    /// `1.5 s` or `0.0005 ms` into `500 ns`.
    ///
    /// Integer metrics, unit-less values, and floating-point values that are zero or non-finite are
    /// returned unchanged.
    ///
    /// When the metric value is rescaled, attached perf quality fields that use the same unit are
    /// scaled alongside it.
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun_runner::metrics::logic::MetricValue;
    /// use gungraun_runner::metrics::model::{AnnotatedMetric, Metric, PerfQualities};
    /// use gungraun_runner::units::Unit;
    ///
    /// let metric = AnnotatedMetric::new(
    ///     1_500.0,
    ///     PerfQualities::new(123, 45.0, 0.06, 1, 123.0),
    ///     Unit::Milliseconds,
    /// );
    ///
    /// let normalized = metric.normalize();
    ///
    /// assert_eq!(normalized.metric, Metric::Float(1.5));
    /// assert_eq!(normalized.unit, Some(Unit::Seconds));
    /// assert_eq!(normalized.qualities.mean, Some(0.123));
    /// ```
    ///
    /// ```
    /// use gungraun_runner::metrics::logic::MetricValue;
    /// use gungraun_runner::metrics::model::{AnnotatedMetric, Metric, PerfQualities};
    ///
    /// let metric = AnnotatedMetric::new(42, PerfQualities::default(), None);
    ///
    /// assert_eq!(metric.normalize(), metric);
    /// ```
    fn normalize(&self) -> Self {
        match (self.metric, self.unit.as_ref()) {
            (Metric::Float(float), Some(unit)) if float.is_finite() && float != 0.0 => {
                let (new_value, new_unit) = unit.rescale(float);
                let factor = new_value / float;
                Self {
                    metric: Metric::Float(new_value),
                    unit: Some(new_unit),
                    qualities: self.qualities.scale_by_metric(Metric::Float(factor)),
                }
            }
            _ => self.clone(),
        }
    }

    /// Normalizes this value and another perf metric into a shared [`Unit`] and scale.
    ///
    /// If both metrics have compatible units with a base scale, this method first chooses the finer
    /// of the two units as a common starting point, so neither value loses precision during
    /// conversion. When both converted metrics are floating-point values, it then calls
    /// [`Unit::rescale`] on the smaller absolute magnitude of the pair to choose a shared display
    /// unit that is readable for both values.
    ///
    /// Integer-only pairs, and mixed integer / floating-point pairs, keep the chosen target unit
    /// directly instead of applying the readability rescaling step. If one metric is unit-less and
    /// the other has a unit, or if the units are incompatible, this returns `None`.
    ///
    /// # Examples
    ///
    /// Normalize two compatible floating point units [`Unit::Seconds`] and [`Unit::Milliseconds`]:
    ///
    /// ```
    /// use gungraun_runner::metrics::logic::MetricValue;
    /// use gungraun_runner::metrics::model::{AnnotatedMetric, Metric, PerfQualities};
    /// use gungraun_runner::units::Unit;
    ///
    /// let lhs = AnnotatedMetric::new(1.0, PerfQualities::default(), Unit::Seconds);
    /// let rhs = AnnotatedMetric::new(1_500.0, PerfQualities::default(), Unit::Milliseconds);
    ///
    /// let (lhs, rhs) = lhs.normalize_with(&rhs).unwrap();
    ///
    /// assert_eq!(lhs.metric, Metric::Float(1.0));
    /// assert_eq!(rhs.metric, Metric::Float(1.5));
    /// assert_eq!(lhs.unit, Some(Unit::Seconds));
    /// assert_eq!(rhs.unit, Some(Unit::Seconds));
    /// ```
    ///
    /// The same, but this time as integer units
    ///
    /// ```
    /// use gungraun_runner::metrics::logic::MetricValue;
    /// use gungraun_runner::metrics::model::{AnnotatedMetric, Metric, PerfQualities};
    /// use gungraun_runner::units::Unit;
    ///
    /// let lhs = AnnotatedMetric::new(1, PerfQualities::default(), Unit::Seconds);
    /// let rhs = AnnotatedMetric::new(1_500, PerfQualities::default(), Unit::Milliseconds);
    ///
    /// let (lhs, rhs) = lhs.normalize_with(&rhs).unwrap();
    ///
    /// assert_eq!(lhs.metric, Metric::Int(1_000));
    /// assert_eq!(rhs.metric, Metric::Int(1_500));
    /// assert_eq!(lhs.unit, Some(Unit::Milliseconds));
    /// assert_eq!(rhs.unit, Some(Unit::Milliseconds));
    /// ```
    fn normalize_with(&self, other: &Self) -> Option<(Self, Self)> {
        match (&self.unit, &other.unit) {
            (None, None) => Some((self.clone(), other.clone())),
            (Some(_), None) | (None, Some(_)) => None,
            (Some(this_unit), Some(other_unit)) => {
                let (target_unit, this_factor, other_factor) =
                    match (this_unit.base_scale(), other_unit.base_scale()) {
                        (Some(ts), Some(os)) if ts <= os => {
                            // self has finer (smaller base_scale) or equal unit
                            let other_factor = other_unit.scale_factor_metric(this_unit)?;
                            (this_unit.clone(), Metric::Int(1), other_factor)
                        }
                        (Some(_), Some(_)) => {
                            // other has finer unit
                            let this_factor = this_unit.scale_factor_metric(other_unit)?;
                            (other_unit.clone(), this_factor, Metric::Int(1))
                        }
                        _ => {
                            // Rate or units without base_scale: pick self as target
                            let other_factor = other_unit.scale_factor_metric(this_unit)?;
                            (this_unit.clone(), Metric::Int(1), other_factor)
                        }
                    };

                // This is similar to what the normalize method does for a single metric, however we
                // have to scale both metrics with the same factor, or else it could happen that one
                // unit `0.5 ms` -> `500 us` while the other `1000 ms` -> `1 s`
                let (this_metric_value, other_metric_value) =
                    match (self.metric * this_factor, other.metric * other_factor) {
                        (Metric::Float(this_value), Metric::Float(other_value)) => {
                            let rescale_value = this_value.abs().min(other_value.abs());
                            let rescale_value = if rescale_value == 0.0 {
                                this_value.abs().max(other_value.abs())
                            } else {
                                rescale_value
                            };

                            let (_, mutual_unit) = target_unit.rescale(rescale_value);

                            match target_unit.scale_factor_metric(&mutual_unit) {
                                Some(mutual_factor) => (
                                    Self {
                                        metric: Metric::Float(this_value) * mutual_factor,
                                        unit: Some(mutual_unit.clone()),
                                        qualities: self
                                            .qualities
                                            .scale_by_metric(mutual_factor * this_factor),
                                    },
                                    Self {
                                        metric: Metric::Float(other_value) * mutual_factor,
                                        unit: Some(mutual_unit),
                                        qualities: other
                                            .qualities
                                            .scale_by_metric(mutual_factor * other_factor),
                                    },
                                ),
                                None => (
                                    Self {
                                        metric: Metric::Float(this_value),
                                        unit: Some(target_unit.clone()),
                                        qualities: self.qualities.scale_by_metric(this_factor),
                                    },
                                    Self {
                                        metric: Metric::Float(other_value),
                                        unit: Some(target_unit),
                                        qualities: other.qualities.scale_by_metric(other_factor),
                                    },
                                ),
                            }
                        }
                        (this_metric, other_metric) => (
                            Self {
                                metric: this_metric,
                                unit: Some(target_unit.clone()),
                                qualities: self.qualities.scale_by_metric(this_factor),
                            },
                            Self {
                                metric: other_metric,
                                unit: Some(target_unit),
                                qualities: other.qualities.scale_by_metric(other_factor),
                            },
                        ),
                    };

                Some((this_metric_value, other_metric_value))
            }
        }
    }
}

impl Metric {
    /// Divide by `rhs` normally but if rhs is `0` the result is by convention `0.0`
    ///
    /// No difference is made between negative 0.0 and positive 0.0 os rhs value. The result is
    /// always positive 0.0.
    #[must_use]
    pub fn div0(self, rhs: Self) -> Self {
        match (self, rhs) {
            (_, Self::Int(0) | Self::Float(0.0f64)) => Self::Float(0.0f64),
            (a, b) => a / b,
        }
    }

    /// Returns `true` if this `Metric` is [`Metric::Int`].
    pub fn is_int(&self) -> bool {
        match self {
            Self::Int(_) => true,
            Self::Float(_) => false,
        }
    }

    /// Returns `true` if this `Metric` is [`Metric::Float`].
    pub fn is_float(&self) -> bool {
        match self {
            Self::Int(_) => false,
            Self::Float(_) => true,
        }
    }

    /// If needed and possible convert this metric to the other [`Metric`] returning the result
    ///
    /// A metric is converted if the expected type of the `metric_kind` is [`Metric::Float`] but the
    /// given metric was [`Metric::Int`]. The metrics of float type are usually percentages with a
    /// value range of `0.0` to `100.0`. Converting `u64` to `f64` within this range happens without
    /// precision loss.
    #[expect(clippy::cast_precision_loss)]
    pub fn try_convert<T: Display + TypeChecker>(&self, metric_kind: T) -> Option<(T, Self)> {
        if metric_kind.verify_metric(*self) {
            Some((metric_kind, *self))
        } else if let Self::Int(a) = self {
            Some((metric_kind, Self::Float(*a as f64)))
        } else {
            None
        }
    }

    fn is_sign_negative(&self) -> bool {
        match self {
            Self::Int(_) => false,
            Self::Float(float) => float.is_sign_negative(),
        }
    }

    /// Convert this `Metric` to a float value
    #[expect(clippy::cast_precision_loss)]
    pub fn to_float(&self) -> f64 {
        match self {
            Self::Int(int) => *int as f64,
            Self::Float(float) => *float,
        }
    }
}

impl MetricValue for Metric {
    fn metric(&self) -> Metric {
        *self
    }

    fn add(&self, other: &Self) -> Self {
        *self + *other
    }

    fn saturating_sub(&self, other: &Self) -> Self {
        let result = self.sub(*other);
        if result.is_float() && result.is_sign_negative() {
            Self::Float(0.0)
        } else {
            result
        }
    }

    fn to_string_without_unit(&self) -> String {
        self.to_string()
    }

    fn unit(&self) -> Option<&Unit> {
        None
    }

    fn normalize(&self) -> Self {
        *self
    }

    fn normalize_with(&self, other: &Self) -> Option<(Self, Self)> {
        Some((*self, *other))
    }
}

impl Ord for Metric {
    #[expect(clippy::cast_precision_loss)]
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a.cmp(b),
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b),
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)),
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b),
        }
    }
}

impl PartialOrd for Metric {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for Metric {
    type Output = Self;

    #[expect(clippy::cast_precision_loss)]
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a.saturating_add(b)),
            (Self::Int(a), Self::Float(b)) => Self::Float((a as f64) + b),
            (Self::Float(a), Self::Int(b)) => Self::Float((b as f64) + a),
            (Self::Float(a), Self::Float(b)) => Self::Float(a + b),
        }
    }
}

impl AddAssign for Metric {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Display for Metric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(a) => f.pad(&format!("{a}")),
            Self::Float(a) => f.pad(&to_string_unsigned_short(*a)),
        }
    }
}

impl Div<u64> for Metric {
    type Output = Self;

    #[expect(clippy::cast_precision_loss)]
    fn div(self, rhs: u64) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), b) => Self::Int(a / b),
            (Self::Float(a), b) => Self::Float(a / (b as f64)),
        }
    }
}

impl Div for Metric {
    type Output = Self;

    #[expect(clippy::cast_precision_loss)]
    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Float((a as f64) / (b as f64)),
            (Self::Int(a), Self::Float(b)) => Self::Float((a as f64) / b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a / (b as f64)),
            (Self::Float(a), Self::Float(b)) => Self::Float(a / b),
        }
    }
}

impl From<u64> for Metric {
    fn from(value: u64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for Metric {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<Limit> for Metric {
    fn from(value: Limit) -> Self {
        match value {
            Limit::Int(a) => Self::Int(a),
            Limit::Float(f) => Self::Float(f),
        }
    }
}

impl From<Metric> for f64 {
    #[expect(clippy::cast_precision_loss)]
    fn from(value: Metric) -> Self {
        match value {
            Metric::Int(a) => a as Self,
            Metric::Float(a) => a,
        }
    }
}

impl FromStr for Metric {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.parse::<u64>() {
            Ok(a) => Ok(Self::Int(a)),
            Err(_) => match s.parse::<f64>() {
                Ok(a) => Ok(Self::Float(a)),
                Err(error) => Err(anyhow!("Invalid metric: {error}")),
            },
        }
    }
}

impl Mul<u64> for Metric {
    type Output = Self;

    #[expect(clippy::cast_precision_loss)]
    fn mul(self, rhs: u64) -> Self::Output {
        match self {
            Self::Int(a) => Self::Int(a.saturating_mul(rhs)),
            Self::Float(a) => Self::Float(a * (rhs as f64)),
        }
    }
}

impl Mul for Metric {
    type Output = Self;

    #[expect(clippy::cast_precision_loss)]
    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a.saturating_mul(b)),
            (Self::Int(a), Self::Float(b)) => Self::Float(a as f64 * b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a * b as f64),
            (Self::Float(a), Self::Float(b)) => Self::Float(a * b),
        }
    }
}

impl Mul<Metric> for u64 {
    type Output = Metric;

    #[expect(clippy::cast_precision_loss)]
    fn mul(self, rhs: Metric) -> Self::Output {
        match rhs {
            Metric::Int(b) => Metric::Int(self.saturating_mul(b)),
            Metric::Float(b) => Metric::Float((self as f64) * b),
        }
    }
}

impl Sub for Metric {
    type Output = Self;

    #[expect(clippy::cast_precision_loss)]
    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a.saturating_sub(b)),
            (Self::Int(a), Self::Float(b)) => Self::Float((a as f64) - b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a - (b as f64)),
            (Self::Float(a), Self::Float(b)) => Self::Float(a - b),
        }
    }
}

impl Display for MetricKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Callgrind(metric) => f.write_fmt(format_args!("Callgrind: {metric}")),
            Self::Cachegrind(metric) => f.write_fmt(format_args!("Cachegrind: {metric}")),
            Self::Dhat(metric) => f.write_fmt(format_args!("DHAT: {metric}")),
            Self::Memcheck(metric) => f.write_fmt(format_args!("Memcheck: {metric}")),
            Self::Helgrind(metric) => f.write_fmt(format_args!("Helgrind: {metric}")),
            Self::DRD(metric) => f.write_fmt(format_args!("DRD: {metric}")),
            Self::Perf(metric) => f.write_fmt(format_args!("Perf: {metric}")),
        }
    }
}

impl<K, V> Metrics<K, V>
where
    K: Hash + Eq + Display + Clone,
    V: MetricValue,
{
    /// Return empty `Metrics`
    pub fn empty() -> Self {
        Self(IndexMap::new())
    }

    /// The order matters. The index is derived from the insertion order
    pub fn with_metric_kinds<I, T>(kinds: T) -> Self
    where
        I: Into<V>,
        T: IntoIterator<Item = (K, I)>,
    {
        Self(kinds.into_iter().map(|(k, n)| (k, n.into())).collect())
    }

    /// Sum this `Metric` with another `Metric`
    ///
    /// Do not use this method if both `Metrics` can differ in their keys order.
    pub fn add(&mut self, other: &Self) {
        for (this, other) in self.0.values_mut().zip(other.0.values()) {
            *this = this.add(other);
        }
    }

    /// Remove all metrics from this container.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Subtract the other `Metric` from this `Metric`
    ///
    /// Do not use this method if both `Metrics` can differ in their keys order. Use
    /// [`Self::saturating_sub_by_key`] instead.
    pub fn saturating_sub(&mut self, other: &Self) {
        for (this, other) in self.0.values_mut().zip(other.0.values()) {
            *this = this.saturating_sub(other);
        }
    }

    /// Subtract matching metrics by key, leaving metrics without a matching key unchanged.
    pub fn saturating_sub_by_key(&mut self, other: &Self) {
        for (key, this) in &mut self.0 {
            if let Some(other) = other.0.get(key) {
                *this = this.saturating_sub(other);
            }
        }
    }

    /// Returns the metric of the kind at index (of insertion order) if present.
    ///
    /// This operation is O(1)
    pub fn metric_by_index(&self, index: usize) -> Option<V> {
        self.0.get_index(index).map(|(_, c)| c.clone())
    }

    /// Returns the metric of the `kind` if present.
    ///
    /// This operation is O(1)
    pub fn metric_by_kind(&self, kind: &K) -> Option<V> {
        self.0.get_key_value(kind).map(|(_, c)| c.clone())
    }

    /// Returns the metric kind or an error.
    ///
    /// # Errors
    ///
    /// If the metric kind is not present
    pub fn try_metric_by_kind(&self, kind: &K) -> Result<V> {
        self.metric_by_kind(kind)
            .with_context(|| format!("Missing event type '{kind}"))
    }

    /// Returns the contained metric kinds.
    pub fn metric_kinds(&self) -> Vec<K> {
        self.0.iter().map(|(k, _)| k.clone()).collect()
    }

    /// Create the union map over this and another `Metrics`
    ///
    /// The order of the keys and their values is preserved. New keys from the `other` Metrics are
    /// appended in their original order.
    pub fn union(self, other: Self) -> Union<K, V> {
        Union::new(self.0, other.0)
    }

    /// Return an iterator over the metrics in insertion order
    pub fn iter(&self) -> indexmap::map::Iter<'_, K, V> {
        self.0.iter()
    }

    /// Return an iterator over the metrics in insertion order
    pub fn iter_mut(&mut self) -> indexmap::map::IterMut<'_, K, V> {
        self.0.iter_mut()
    }

    /// Returns `true` if there are no metrics present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of metrics stored in insertion order.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Insert a single metric
    ///
    /// If an equivalent key already exists in the map: the key remains and retains in its place in
    /// the order, its corresponding value is updated with `value`, and the older value is returned
    /// inside `Some(_)`.
    ///
    /// If no equivalent key existed in the map: the new key-value pair is inserted, last in order,
    /// and `None` is returned.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    /// Inserts a metric for `key`, or adds `value` to the existing metric if `key` is already
    /// present.
    ///
    /// Addition follows the [`MetricValue::add`] semantics of the stored value type, including any
    /// unit normalization performed by that implementation.
    pub fn insert_or_add(&mut self, key: K, value: V) {
        match self.metric_by_kind(&key) {
            Some(metric) => {
                self.insert(key, metric.add(&value));
            }
            None => {
                self.insert(key, value);
            }
        }
    }

    /// Insert all metrics
    ///
    /// See also [`Metrics::insert`]
    pub fn insert_all(&mut self, entries: &[(K, V)]) {
        for (key, value) in entries {
            self.insert(key.clone(), value.clone());
        }
    }
}

impl<K> Metrics<K>
where
    K: Hash + Eq + Display + Clone,
{
    /// Add metrics from an iterator over strings
    ///
    /// Adding metrics stops as soon as there are no more keys in this `Metrics` or no more values
    /// in the iterator. This property is especially important for the metrics from the Callgrind
    /// output files. From the documentation of the Callgrind format:
    ///
    /// > If a cost line specifies less event counts than given in the "events" line, the
    /// > rest is assumed to be zero.
    ///
    /// # Errors
    ///
    /// If one of the strings in the iterator is not parsable as u64 or f64
    pub fn add_iter_str<I, T>(&mut self, iter: T) -> Result<()>
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        for (this, other) in self.0.values_mut().zip(iter) {
            *this += other
                .as_ref()
                .parse::<Metric>()
                .context("A metric must be a valid number")?;
        }

        Ok(())
    }
}

impl Metrics<PerfMetric, AnnotatedMetric<PerfQualities>> {
    /// Normalizes perf metrics by the number of benchmark repetitions.
    ///
    /// This scales the measured metric value and `event_runtime`. Percent-like qualities such as
    /// `pcnt_running` and perf's `"variance"` already describe the normalized aggregate and
    /// therefore remain unchanged.
    #[expect(clippy::cast_precision_loss)]
    pub fn normalize_by_repetitions(&mut self, repetitions: usize) {
        if repetitions <= 1 {
            return;
        }

        for metric in self.0.values_mut() {
            let metric_mean = metric.base_value() / repetitions as f64;
            let quality_mean = metric.qualities.mean.map(|mean| {
                let base_value = metric
                    .unit
                    .as_ref()
                    .map_or(mean, |unit| unit.base_value(mean));
                base_value / repetitions as f64
            });

            *metric = metric.clone().into_mean(metric_mean);

            if let Some(event_runtime) = metric.qualities.event_runtime.as_mut() {
                *event_runtime /= repetitions as u64;
            }
            if let Some(quality_mean) = quality_mean {
                metric.qualities.mean = Some(metric.rebase(quality_mean));
            }
        }
    }
}

impl<K, V> IntoIterator for Metrics<K, V>
where
    K: Hash + Eq,
{
    type Item = (K, V);
    type IntoIter = indexmap::map::IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a Metrics<K, V>
where
    K: Hash + Eq,
{
    type Item = (&'a K, &'a V);
    type IntoIter = indexmap::map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut Metrics<K, V>
where
    K: Hash + Eq,
{
    type Item = (&'a K, &'a mut V);
    type IntoIter = indexmap::map::IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl<I, K> FromIterator<I> for Metrics<K>
where
    K: Hash + Eq + From<I>,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = I>,
    {
        Self(
            iter.into_iter()
                .map(|s| (K::from(s), Metric::Int(0)))
                .collect::<IndexMap<_, _>>(),
        )
    }
}

impl<V> MetricsDiff<V>
where
    V: MetricValue,
{
    /// Creates a new `MetricsDiff` from an [`EitherOrBoth`] of metric values.
    pub fn new(metrics: EitherOrBoth<V>) -> Self {
        if let EitherOrBoth::Both(new, old) = &metrics {
            if let Some((normalized_new, normalized_old)) = new.normalize_with(old) {
                let diffs = Diffs::new(normalized_new.metric(), normalized_old.metric());
                Self {
                    diffs: Some(diffs),
                    metrics: EitherOrBoth::Both(normalized_new, normalized_old),
                }
            } else {
                // Can't create diffs for metrics with different units or scales
                Self {
                    diffs: None,
                    metrics: metrics.map(|m| m.normalize()),
                }
            }
        } else {
            Self {
                metrics: metrics.map(|m| m.normalize()),
                diffs: None,
            }
        }
    }

    /// Sum this metrics diff with another [`MetricsDiff`]
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        match (&self.metrics, &other.metrics) {
            (EitherOrBoth::Left(new), EitherOrBoth::Left(other_new)) => {
                Self::new(EitherOrBoth::Left(new.add(other_new)))
            }
            (EitherOrBoth::Right(old), EitherOrBoth::Left(new))
            | (EitherOrBoth::Left(new), EitherOrBoth::Right(old)) => {
                Self::new(EitherOrBoth::Both(new.clone(), old.clone()))
            }
            (EitherOrBoth::Right(old), EitherOrBoth::Right(other_old)) => {
                Self::new(EitherOrBoth::Right(old.add(other_old)))
            }
            (EitherOrBoth::Both(new, old), EitherOrBoth::Left(other_new))
            | (EitherOrBoth::Left(new), EitherOrBoth::Both(other_new, old)) => {
                Self::new(EitherOrBoth::Both(new.add(other_new), old.clone()))
            }
            (EitherOrBoth::Both(new, old), EitherOrBoth::Right(other_old))
            | (EitherOrBoth::Right(old), EitherOrBoth::Both(new, other_old)) => {
                Self::new(EitherOrBoth::Both(new.clone(), old.add(other_old)))
            }
            (EitherOrBoth::Both(new, old), EitherOrBoth::Both(other_new, other_old)) => {
                Self::new(EitherOrBoth::Both(new.add(other_new), old.add(other_old)))
            }
        }
    }
}

impl<K, V> MetricsSummary<K, V>
where
    K: Hash + Eq + Summarize<V> + Display + Clone,
    V: MetricValue,
{
    /// Creates a new `MetricsSummary` calculating the differences between new and old (if any).
    /// [`Metrics`]
    pub fn new(metrics: EitherOrBoth<Metrics<K, V>>) -> Self {
        let summarized = metrics.map(|metrics| {
            let mut summarized = Cow::Owned(metrics);
            K::summarize(&mut summarized);
            summarized
        });

        let diffs = match summarized {
            EitherOrBoth::Left(new) => new
                .into_owned()
                .into_iter()
                .map(|(metric_kind, metric)| {
                    (metric_kind, MetricsDiff::new(EitherOrBoth::Left(metric)))
                })
                .collect(),
            EitherOrBoth::Right(old) => old
                .into_owned()
                .into_iter()
                .map(|(metric_kind, metric)| {
                    (metric_kind, MetricsDiff::new(EitherOrBoth::Right(metric)))
                })
                .collect(),
            EitherOrBoth::Both(new, old) => new
                .into_owned()
                .union(old.into_owned())
                .into_iter()
                .map(|(metric_kind, metric)| (metric_kind, MetricsDiff::new(metric)))
                .collect(),
        };

        Self(diffs)
    }

    /// Try to return a [`MetricsDiff`] for the specified `MetricKind`
    pub fn diff_by_kind(&self, metric_kind: &K) -> Option<&MetricsDiff<V>> {
        self.0.get(metric_kind)
    }

    /// Return an iterator over all [`MetricsDiff`]s
    pub fn all_diffs(&self) -> impl Iterator<Item = (&K, &MetricsDiff<V>)> {
        self.0.iter()
    }

    /// Returns `true` if there are no metric diffs present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Extract the [`Metrics`] from this summary
    ///
    /// This is the exact reverse operation to [`MetricsSummary::new`]
    pub fn extract_costs(&self) -> EitherOrBoth<Metrics<K, V>> {
        self.0
            .iter()
            .map(|(metric_kind, diff)| {
                diff.metrics
                    .clone()
                    .map(|metric| (metric_kind.clone(), metric))
            })
            .collect::<EitherOrBoth<IndexMap<_, _>>>()
            .map(Metrics)
    }

    /// Sum up another `MetricsSummary` with this one
    ///
    /// If a [`MetricsDiff`] is not present in this summary but in the other, it is added to this
    /// summary.
    pub fn add(&mut self, other: &Self) {
        for (other_key, other_value) in &other.0 {
            if let Some(value) = self.0.get_mut(other_key) {
                *value = value.add(other_value);
            } else {
                self.0.insert(other_key.clone(), other_value.clone());
            }
        }
    }
}

impl<K, V> Default for MetricsSummary<K, V>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        Self(IndexMap::default())
    }
}

impl PerfQualities {
    /// Creates perf quality metadata from the optional values parsed or derived for one metric.
    ///
    /// These fields mirror the extra statistical information Gungraun tracks for perf metrics,
    /// such as runtime coverage, sample count, relative standard error, and mean.
    pub fn new<E, P, R, N, M>(event_runtime: E, pcnt_running: P, rse: R, n: N, mean: M) -> Self
    where
        E: Into<Option<u64>>,
        P: Into<Option<f64>>,
        R: Into<Option<f64>>,
        N: Into<Option<u64>>,
        M: Into<Option<f64>>,
    {
        Self {
            event_runtime: event_runtime.into(),
            mean: mean.into(),
            n: n.into(),
            pcnt_running: pcnt_running.into(),
            rse: rse.into(),
        }
    }

    /// Merges perf quality metadata when two perf metric values are combined with
    /// [`MetricValue::add`].
    ///
    /// `event_runtime` and `pcnt_running` are treated as a coupled pair and merged together. A
    /// present non-zero pair takes precedence over missing data or a zero-runtime pair. When both
    /// sides contain a present non-zero pair, `event_runtime` values are added and `pcnt_running`
    /// is recomputed from the combined runtime instead of being added directly.
    ///
    /// If both sides contain a zero-runtime pair, the merged result is canonicalized to
    /// `event_runtime = 0` and `pcnt_running = 0.0`.
    ///
    /// `n` is merged independently by summing both sample counts when present. `rse` is discarded,
    /// because this method has no sound rule for combining the relative standard error tracked for
    /// the individual inputs. `mean` is also discarded, because rescaling or recomputing it
    /// correctly would require unit context that is not available here.
    #[expect(clippy::cast_precision_loss)]
    pub fn add(&self, other: &Self) -> Self {
        let (event_runtime, pcnt_running) = match (
            (self.event_runtime, self.pcnt_running),
            (other.event_runtime, other.pcnt_running),
        ) {
            ((Some(_), Some(0.0)), (Some(_), Some(0.0))) => (Some(0), Some(0.0)),
            ((None, None) | (Some(_), Some(0.0)), (Some(oe), Some(op))) => (Some(oe), Some(op)),
            ((Some(te), Some(tp)), (None, None) | (Some(_), Some(0.0))) => (Some(te), Some(tp)),
            ((Some(te), Some(tp)), (Some(oe), Some(op))) => {
                let total = te.saturating_add(oe);

                // A zero runtime contributes zero to the total measurement time, regardless of
                // pcnt_running (avoiding 0.0 / 0.0 = NaN).
                let d1 = if te == 0 { 0.0 } else { te as f64 / tp };
                let d2 = if oe == 0 { 0.0 } else { oe as f64 / op };
                let denom = d1 + d2;

                let pcnt = if total == 0 || denom == 0.0 {
                    0.0
                } else {
                    total as f64 / denom
                };
                (Some(total), Some(pcnt))
            }
            _ => (None, None),
        };

        let n = match (self.n, other.n) {
            (None, None) => None,
            (None, Some(n)) | (Some(n), None) => Some(n),
            (Some(a), Some(b)) => Some(a + b),
        };

        Self {
            event_runtime,
            pcnt_running,
            rse: None,
            n,
            mean: None,
        }
    }

    /// Scales perf quality fields that are expressed in the same units as the metric value.
    ///
    /// Currently this only rescales `mean`. Other quality fields are preserved as-is because they
    /// are counts, percentages, or otherwise not directly proportional to the metric magnitude.
    #[expect(clippy::cast_precision_loss)]
    pub fn scale_by_metric(&self, factor: Metric) -> Self {
        Self {
            mean: self.mean.map(|m| match factor {
                Metric::Int(int) => int as f64 * m,
                Metric::Float(float) => float * m,
            }),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::iter;

    use either_or_both::EitherOrBoth;
    use indexmap::indexmap;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::api::EventKind::{self, *};
    use crate::api::PerfMetric;

    fn expected_metrics<I, T>(events: T) -> Metrics<EventKind>
    where
        I: Into<Metric>,
        T: IntoIterator<Item = (EventKind, I)>,
    {
        Metrics(
            events
                .into_iter()
                .map(|(k, n)| (k, n.into()))
                .collect::<IndexMap<_, _>>(),
        )
    }

    fn expected_metrics_diff<D>(metrics: EitherOrBoth<Metric>, diffs: D) -> MetricsDiff
    where
        D: Into<Option<(f64, f64)>>,
    {
        MetricsDiff {
            metrics,
            diffs: diffs
                .into()
                .map(|(diff_pct, factor)| Diffs { diff_pct, factor }),
        }
    }

    fn metrics_fixture(metrics: &[u64]) -> Metrics<EventKind> {
        // events: Ir Dr Dw I1mr D1mr D1mw ILmr DLmr DLmw
        let event_kinds = [
            Ir,
            Dr,
            Dw,
            I1mr,
            D1mr,
            D1mw,
            ILmr,
            DLmr,
            DLmw,
            L1hits,
            LLhits,
            RamHits,
            TotalRW,
            EstimatedCycles,
            I1MissRate,
            D1MissRate,
            LLiMissRate,
            LLdMissRate,
            LLMissRate,
            L1HitRate,
            LLHitRate,
            RamHitRate,
        ];

        Metrics::with_metric_kinds(
            event_kinds
                .iter()
                .zip(metrics.iter())
                .map(|(e, v)| (*e, *v)),
        )
    }

    fn metrics_summary_fixture<T, U>(kinds: U) -> MetricsSummary<EventKind>
    where
        T: Into<Option<(f64, f64)>> + Clone,
        U: IntoIterator<Item = (EitherOrBoth<Metric>, T)>,
    {
        // events: Ir Dr Dw I1mr D1mr D1mw ILmr DLmr DLmw
        let event_kinds = [
            Ir,
            Dr,
            Dw,
            I1mr,
            D1mr,
            D1mw,
            ILmr,
            DLmr,
            DLmw,
            L1hits,
            LLhits,
            RamHits,
            TotalRW,
            EstimatedCycles,
            I1MissRate,
            D1MissRate,
            LLiMissRate,
            LLdMissRate,
            LLMissRate,
            L1HitRate,
            LLHitRate,
            RamHitRate,
        ];

        let map: IndexMap<EventKind, MetricsDiff> = event_kinds
            .iter()
            .zip(kinds)
            .map(|(e, (m, d))| (*e, expected_metrics_diff(m, d)))
            .collect();

        MetricsSummary(map)
    }

    #[rstest]
    #[case::same_unit(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(2.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(3.0, Unit::Seconds)
    )]
    #[case::compatible_scaled_units(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(500.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(1.5, Unit::Seconds)
    )]
    #[case::unitless_metrics(
        AnnotatedMetric::with_default_qualities(1, None),
        AnnotatedMetric::with_default_qualities(2, None),
        AnnotatedMetric::with_default_qualities(3, None)
    )]
    #[case::result_is_normalized(
        AnnotatedMetric::with_default_qualities(1000.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(500.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(1.5, Unit::Seconds)
    )]
    fn test_annotated_metric_add(
        #[case] lhs: AnnotatedMetric<PerfQualities>,
        #[case] rhs: AnnotatedMetric<PerfQualities>,
        #[case] expected: AnnotatedMetric<PerfQualities>,
    ) {
        assert_eq!(lhs.add(&rhs), expected);
    }

    #[test]
    fn test_annotated_metric_add_merges_perf_qualities() {
        let expected = AnnotatedMetric::new(
            1_500,
            PerfQualities::new(400, 66.666_666_666_666_67, None, 3, None),
            Unit::Milliseconds,
        );
        let lhs = AnnotatedMetric::new(
            500,
            PerfQualities::new(100, 50.0, 7.0, 1, 100.0),
            Unit::Milliseconds,
        );
        let rhs = AnnotatedMetric::new(
            1,
            PerfQualities::new(300, 75.0, 11.0, 2, 300.0),
            Unit::Seconds,
        );

        let actual = lhs.add(&rhs);

        assert_eq!(actual, expected);
    }

    #[test]
    #[should_panic(expected = "Only compatible units should be summed up")]
    fn test_annotated_metric_add_panics_for_incompatible_units() {
        let lhs = AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds);
        let rhs = AnnotatedMetric::with_default_qualities(1.0, Unit::Bytes);

        let _ = lhs.add(&rhs);
    }

    #[rstest]
    #[case::same_unit(
        AnnotatedMetric::with_default_qualities(3.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(2.0, Unit::Seconds)
    )]
    #[case::compatible_scaled_units(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(500.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(500.0, Unit::Milliseconds)
    )]
    #[case::unitless_metrics(
        AnnotatedMetric::with_default_qualities(3, None),
        AnnotatedMetric::with_default_qualities(2, None),
        AnnotatedMetric::with_default_qualities(1, None)
    )]
    #[case::saturates_at_zero(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(1500.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(0.0, Unit::Seconds)
    )]
    fn test_annotated_metric_saturating_sub(
        #[case] lhs: AnnotatedMetric<PerfQualities>,
        #[case] rhs: AnnotatedMetric<PerfQualities>,
        #[case] expected: AnnotatedMetric<PerfQualities>,
    ) {
        assert_eq!(lhs.saturating_sub(&rhs), expected);
    }

    #[test]
    #[should_panic(expected = "Only compatible units should be subtracted")]
    fn test_annotated_metric_saturating_sub_panics_for_incompatible_units() {
        let lhs = AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds);
        let rhs = AnnotatedMetric::with_default_qualities(1.0, Unit::Bytes);

        let _ = lhs.saturating_sub(&rhs);
    }

    #[test]
    fn test_annotated_metric_saturating_sub_resets_perf_qualities() {
        let lhs = AnnotatedMetric::new(
            500,
            PerfQualities::new(100, 50.0, 7.0, 1, 100.0),
            Unit::Milliseconds,
        );
        let rhs = AnnotatedMetric::new(
            1,
            PerfQualities::new(300, 75.0, 11.0, 2, 300.0),
            Unit::Seconds,
        );

        let expected = AnnotatedMetric::with_default_qualities(0, Unit::Milliseconds);

        let actual = lhs.saturating_sub(&rhs);

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::float_large_value_to_larger_unit(
        AnnotatedMetric::with_default_qualities(1500.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(1.5, Unit::Seconds)
    )]
    #[case::float_small_value_to_smaller_unit(
        AnnotatedMetric::with_default_qualities(0.0005, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(500.0, Unit::Nanoseconds)
    )]
    #[case::float_passthrough(
        AnnotatedMetric::with_default_qualities(1.5, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(1.5, Unit::Milliseconds)
    )]
    #[case::int_passthrough(
        AnnotatedMetric::with_default_qualities(1000, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(1000, Unit::Milliseconds)
    )]
    #[case::no_unit_passthrough(
        AnnotatedMetric::with_default_qualities(1.5, None),
        AnnotatedMetric::with_default_qualities(1.5, None)
    )]
    #[case::zero(
        AnnotatedMetric::with_default_qualities(0.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(0.0, Unit::Milliseconds)
    )]
    #[case::infinity(
        AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Milliseconds)
    )]
    #[case::neg_infinity(
        AnnotatedMetric::with_default_qualities(
            Metric::Float(f64::NEG_INFINITY),
            Unit::Milliseconds
        ),
        AnnotatedMetric::with_default_qualities(
            Metric::Float(f64::NEG_INFINITY),
            Unit::Milliseconds
        )
    )]
    fn test_annotated_metric_normalize(
        #[case] input: AnnotatedMetric<PerfQualities>,
        #[case] expected: AnnotatedMetric<PerfQualities>,
    ) {
        assert_eq!(input.normalize(), expected);
    }

    #[test]
    fn test_annotated_metric_normalize_when_mean_is_none_then_preserves_perf_qualities() {
        let qualities = PerfQualities::new(123, 45.0, 6.0, 1, None);

        let metric = AnnotatedMetric::new(
            Metric::Float(1_500.0),
            qualities.clone(),
            Unit::Milliseconds,
        );

        let expected = AnnotatedMetric::new(1.5, qualities, Unit::Seconds);

        let actual = metric.normalize();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_perf_qualities_deserialize_rse() {
        let qualities: PerfQualities = serde_json::from_str(r#"{"rse":0.5}"#).unwrap();

        assert_eq!(qualities.rse, Some(0.5));
    }

    #[test]
    fn test_perf_qualities_serialize_omits_absent_mean_and_n() {
        let qualities = PerfQualities::new(123, 45.0, 6.0, None, None);

        let value = serde_json::to_value(qualities).unwrap();

        assert_eq!(value.get("mean"), None);
        assert_eq!(value.get("n"), None);
    }

    #[test]
    fn test_perf_qualities_serialize_rse() {
        let qualities = PerfQualities::new(None, None, 0.5, None, None);

        let value = serde_json::to_value(qualities).unwrap();

        assert_eq!(value.get("rse"), Some(&serde_json::json!(0.5)));
        assert_eq!(value.get("variance"), None);
    }

    #[test]
    fn test_annotated_metric_normalize_scales_perf_qualities() {
        let metric = AnnotatedMetric::new(
            Metric::Float(1_500.0),
            PerfQualities::new(123, 45.0, 6.0, 1, 123.0),
            Unit::Milliseconds,
        );

        let expected = AnnotatedMetric::new(
            Metric::Float(1.5),
            PerfQualities::new(123, 45.0, 6.0, 1, 0.123),
            Unit::Seconds,
        );

        let actual = metric.normalize();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_annotated_metric_normalize_with_when_mean_is_none_then_preserves_perf_qualities() {
        let lhs_qualities = PerfQualities::new(100, 50.0, 7.0, 1, None);
        let rhs_qualities = PerfQualities::new(300, 75.0, 11.0, 2, None);

        let lhs = AnnotatedMetric::new(1.0, lhs_qualities.clone(), Unit::Seconds);
        let rhs = AnnotatedMetric::new(
            Metric::Float(1_500.0),
            rhs_qualities.clone(),
            Unit::Milliseconds,
        );

        let expected_lhs = AnnotatedMetric::new(1.0, lhs_qualities, Unit::Seconds);
        let expected_rhs = AnnotatedMetric::new(1.5, rhs_qualities, Unit::Seconds);

        let actual = lhs.normalize_with(&rhs);

        assert_eq!(actual, Some((expected_lhs, expected_rhs)));
    }

    #[test]
    fn test_annotated_metric_normalize_with_scales_perf_qualities() {
        let lhs_qualities = PerfQualities::new(100, 50.0, 7.0, 1, 100.0);
        let lhs = AnnotatedMetric::new(1.0, lhs_qualities.clone(), Unit::Seconds);
        let rhs = AnnotatedMetric::new(
            Metric::Float(1_500.0),
            PerfQualities::new(300, 75.0, 11.0, 2, 300.0),
            Unit::Milliseconds,
        );

        let expected_lhs = AnnotatedMetric::new(1.0, lhs_qualities, Unit::Seconds);
        let expected_rhs = AnnotatedMetric::new(
            Metric::Float(1.5),
            PerfQualities::new(300, 75.0, 11.0, 2, 0.3),
            Unit::Seconds,
        );

        let actual = lhs.normalize_with(&rhs);

        assert_eq!(actual, Some((expected_lhs, expected_rhs)));
    }

    #[rstest]
    #[case::same_unit_float(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(2.0, Unit::Seconds),
        (
            AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
            AnnotatedMetric::with_default_qualities(2.0, Unit::Seconds),
        ),
    )]
    #[case::different_unit_float(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(2.0, Unit::Bytes),
        None
    )]
    #[case::same_unit_int(
        AnnotatedMetric::with_default_qualities(100, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(200, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(100, Unit::Milliseconds),
            AnnotatedMetric::with_default_qualities(200, Unit::Milliseconds),
        ),
    )]
    #[case::different_unit_int(
        AnnotatedMetric::with_default_qualities(100, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(200, Unit::Bytes),
        None
    )]
    #[case::both_no_unit(
        AnnotatedMetric::with_default_qualities(1.0, None),
        AnnotatedMetric::with_default_qualities(2.0, None),
        (
            AnnotatedMetric::with_default_qualities(1.0, None),
            AnnotatedMetric::with_default_qualities(2.0, None),
        ),
    )]
    #[case::one_no_unit(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(2.0, None),
        None
    )]
    #[case::s_and_ms_float(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(1000.0, Unit::Milliseconds
        ),
        (
            AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
            AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        ),
    )]
    #[case::ms_and_s_float(
        AnnotatedMetric::with_default_qualities(1000.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        (
            AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
            AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        ),
    )]
    #[case::ms_and_s_int(
        AnnotatedMetric::with_default_qualities(500, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(1, Unit::Seconds),
        (
            AnnotatedMetric::with_default_qualities(500, Unit::Milliseconds),
            AnnotatedMetric::with_default_qualities(1000, Unit::Milliseconds),
        ),
    )]
    #[case::display_normalization(
        AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(1500.0, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
            AnnotatedMetric::with_default_qualities(1.5, Unit::Seconds),
        ),
    )]
    #[case::fract_ms_and_ms(
        AnnotatedMetric::with_default_qualities(0.05, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(0.005, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(50.0, Unit::Microseconds),
            AnnotatedMetric::with_default_qualities(5.0, Unit::Microseconds),
        ),
    )]
    #[case::one_is_zero(
        AnnotatedMetric::with_default_qualities(0.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(0.005, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(0.0, Unit::Microseconds),
            AnnotatedMetric::with_default_qualities(5.0, Unit::Microseconds),
        ),
    )]
    #[case::both_zero(
        AnnotatedMetric::with_default_qualities(0.0, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(0.0, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(0.0, Unit::Milliseconds),
            AnnotatedMetric::with_default_qualities(0.0, Unit::Milliseconds),
        ),
    )]
    #[case::one_is_infinity(
        AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(0.005, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Microseconds),
            AnnotatedMetric::with_default_qualities(5.0, Unit::Microseconds),
        ),
    )]
    #[case::both_infinity(
        AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Milliseconds),
            AnnotatedMetric::with_default_qualities(f64::INFINITY, Unit::Milliseconds),
        ),
    )]
    #[case::one_is_neg_infinity(
        AnnotatedMetric::with_default_qualities(f64::NEG_INFINITY, Unit::Milliseconds),
        AnnotatedMetric::with_default_qualities(0.005, Unit::Milliseconds),
        (
            AnnotatedMetric::with_default_qualities(f64::NEG_INFINITY, Unit::Microseconds),
            AnnotatedMetric::with_default_qualities(5.0, Unit::Microseconds),
        ),
    )]
    #[case::both_neg_infinity(
        AnnotatedMetric::with_default_qualities(f64::NEG_INFINITY, Unit::Seconds),
        AnnotatedMetric::with_default_qualities(f64::NEG_INFINITY, Unit::Seconds),
        (
            AnnotatedMetric::with_default_qualities(f64::NEG_INFINITY, Unit::Seconds),
            AnnotatedMetric::with_default_qualities(f64::NEG_INFINITY, Unit::Seconds),
        ),
    )]
    fn test_annotated_metric_normalize_with<E>(
        #[case] this: AnnotatedMetric<PerfQualities>,
        #[case] other: AnnotatedMetric<PerfQualities>,
        #[case] expected: E,
    ) where
        E: Into<
            Option<(
                AnnotatedMetric<PerfQualities>,
                AnnotatedMetric<PerfQualities>,
            )>,
        >,
    {
        assert_eq!(this.normalize_with(&other), expected.into());
    }

    #[rstest]
    #[case::single_zero(&[Ir], &["0"], expected_metrics([(Ir, 0)]))]
    #[case::single_one(&[Ir], &["1"], expected_metrics([(Ir, 1)]))]
    #[case::single_float(&[Ir], &["1.0"], expected_metrics([(Ir, 1.0f64)]))]
    #[case::single_u64_max(&[Ir], &[u64::MAX.to_string()], expected_metrics([(Ir, u64::MAX)]))]
    #[case::one_more_than_max_u64(&[Ir], &["18446744073709551616"],
        // This float has the correct value to represent the value above
        expected_metrics([(Ir, 18_446_744_073_709_552_000_f64)])
    )]
    #[case::more_values_than_kinds(&[Ir], &["1", "2"], expected_metrics([(Ir, 1)]))]
    #[case::more_kinds_than_values(&[Ir, I1mr], &["1"], expected_metrics([(Ir, 1), (I1mr, 0)]))]
    fn test_metrics_add_iter_str<I>(
        #[case] event_kinds: &[EventKind],
        #[case] to_add: &[I],
        #[case] expected_metrics: Metrics<EventKind>,
    ) where
        I: AsRef<str>,
    {
        let mut metrics =
            Metrics::with_metric_kinds(event_kinds.iter().copied().zip(iter::repeat(0)));
        metrics.add_iter_str(to_add).unwrap();

        assert_eq!(metrics, expected_metrics);
    }

    #[rstest]
    #[case::word(&[Ir], &["abc"])]
    #[case::empty(&[Ir], &[""])]
    fn test_metrics_add_iter_str_when_error<I>(
        #[case] event_kinds: &[EventKind],
        #[case] to_add: &[I],
    ) where
        I: AsRef<str>,
    {
        let mut metrics =
            Metrics::with_metric_kinds(event_kinds.iter().copied().zip(iter::repeat(0)));
        assert!(metrics.add_iter_str(to_add).is_err());
    }

    #[rstest]
    #[case::all_zero_int(0, 0, 0.0f64)]
    #[case::lhs_zero_int_one(0, 1, 0.0f64)]
    #[case::lhs_zero_int_two(0, 2, 0.0f64)]
    #[case::one_rhs_zero_int(1, 0, 0.0f64)]
    #[case::two_rhs_zero_int(2, 0, 0.0f64)]
    #[case::all_zero_float(0.0f64, 0.0f64, 0.0f64)]
    #[case::lhs_zero_float_one(0.0f64, 1.0f64, 0.0f64)]
    #[case::lhs_zero_float_two(0.0f64, 2.0f64, 0.0f64)]
    #[case::lhs_zero_float_neg_two(0.0f64, -2.0f64, -0.0f64)]
    #[case::one_rhs_zero_float(1.0f64, 0.0f64, 0.0f64)]
    #[case::two_rhs_zero_float(2.0f64, 0.0f64, 0.0f64)]
    #[case::one_neg_rhs_zero_float(1.0f64, -0.0f64, 0.0f64)]
    #[case::one_one_int(1, 1, 1.0f64)]
    #[case::two_one_int(2, 1, 2.0f64)]
    #[case::one_two_int(1, 2, 0.5f64)]
    #[case::one_float_one(1, 1.0f64, 1.0f64)]
    #[case::float_one_int_one(1.0f64, 1, 1.0f64)]
    #[case::float_one(1.0f64, 1.0f64, 1.0f64)]
    #[case::one_float_two(1, 2.0f64, 0.5f64)]
    #[case::float_one_int_two(1.0f64, 2, 0.5f64)]
    #[case::float_one_two(1.0f64, 2.0f64, 0.5f64)]
    fn test_metric_safe_div<L, R, E>(#[case] lhs: L, #[case] rhs: R, #[case] expected: E)
    where
        L: Into<Metric>,
        R: Into<Metric>,
        E: Into<Metric>,
    {
        let expected = expected.into();

        let lhs = lhs.into();
        let rhs = rhs.into();

        assert_eq!(lhs.div0(rhs), expected);
    }

    #[rstest]
    #[case::zero(0, 0, 0)]
    #[case::one_zero(1, 0, 1)]
    #[case::zero_one(0, 1, 1)]
    #[case::u64_max(0, u64::MAX, u64::MAX)]
    #[case::one_u64_max_saturates(1, u64::MAX, u64::MAX)]
    #[case::one(1, 1, 2)]
    #[case::two_one(2, 1, 3)]
    #[case::one_two(1, 2, 3)]
    #[case::float_one_int_zero(1.0f64, 0, 1.0f64)]
    #[case::int_zero_float_one(0, 1.0f64, 1.0f64)]
    #[case::float_zero(0.0f64, 0.0f64, 0.0f64)]
    #[case::float_one(1.0f64, 1.0f64, 2.0f64)]
    #[case::float_one_two(1.0f64, 2.0f64, 3.0f64)]
    #[case::float_two_one(2.0f64, 1.0f64, 3.0f64)]
    fn test_metric_add_and_add_assign<L, R, E>(#[case] lhs: L, #[case] rhs: R, #[case] expected: E)
    where
        L: Into<Metric>,
        R: Into<Metric>,
        E: Into<Metric>,
    {
        let expected = expected.into();

        let mut lhs = lhs.into();
        let rhs = rhs.into();

        assert_eq!(lhs + rhs, expected);

        lhs += rhs;
        assert_eq!(lhs, expected);
    }

    #[rstest]
    #[case::zero("0", 0)]
    #[case::one("1", 1)]
    #[case::u64_max(&format!("{}", u64::MAX), u64::MAX)]
    #[case::one_below_u64_max(&format!("{}", u64::MAX - 1), u64::MAX - 1)]
    #[case::zero_float("0.0", 0.0f64)]
    #[case::one_float("1.0", 1.0f64)]
    #[case::one_point("1.", 1.0f64)]
    #[case::point_one(".1", 0.1f64)]
    #[case::two_float("2.0", 2.0f64)]
    #[case::neg_one_float("-1.0", -1.0f64)]
    #[case::neg_two_float("-2.0", -2.0f64)]
    #[case::inf("inf", f64::INFINITY)]
    fn test_metric_from_str<E>(#[case] input: &str, #[case] expected: E)
    where
        E: Into<Metric>,
    {
        let expected = expected.into();
        assert_eq!(input.parse::<Metric>().unwrap(), expected);
    }

    #[test]
    fn test_metric_from_str_when_invalid_then_error() {
        let err = "abc".parse::<Metric>().unwrap_err();
        assert_eq!(
            "Invalid metric: invalid float literal".to_owned(),
            err.to_string()
        );
    }

    #[rstest]
    #[case::zero(0, 0, 0)]
    #[case::zero_one(0, 1, 0)]
    #[case::one(1, 1, 1)]
    #[case::one_two(1, 2, 2)]
    #[case::u64_max_one(u64::MAX, 1, u64::MAX)]
    #[case::u64_max_two_saturates(u64::MAX, 2, u64::MAX)]
    #[case::zero_float(0, 0.0f64, 0.0f64)]
    #[case::zero_one_float(0, 1.0f64, 0.0f64)]
    #[case::one_float(1, 1.0f64, 1.0f64)]
    #[case::one_two_float(1, 2.0f64, 2.0f64)]
    #[expect(clippy::cast_precision_loss)]
    #[case::u64_max_two_float(u64::MAX, 2.0f64, 2.0f64 * (u64::MAX as f64))]
    fn test_metric_mul_u64<B, E>(#[case] a: u64, #[case] b: B, #[case] expected: E)
    where
        B: Into<Metric>,
        E: Into<Metric>,
    {
        let expected = expected.into();
        let b = b.into();

        assert_eq!(a * b, expected);
        assert_eq!(b * a, expected);
    }

    #[rstest]
    #[case::zero(0, 0, 0)]
    #[case::one_zero(1, 0, 1)]
    #[case::zero_one_saturates(0, 1, 0)]
    #[case::u64_max_saturates(0, u64::MAX, 0)]
    #[case::one_u64_max_saturates(1, u64::MAX, 0)]
    #[case::u64_max_one(u64::MAX, 1, u64::MAX - 1)]
    #[case::one(1, 1, 0)]
    #[case::two_one(2, 1, 1)]
    #[case::one_two(1, 2, 0)]
    #[case::float_one_int_zero(1.0f64, 0, 1.0f64)]
    #[case::int_zero_float_one(0, 1.0f64, -1.0f64)]
    #[case::float_zero(0.0f64, 0.0f64, 0.0f64)]
    #[case::float_one(1.0f64, 1.0f64, 0.0f64)]
    #[case::float_one_two(1.0f64, 2.0f64, -1.0f64)]
    #[case::float_two_one(2.0f64, 1.0f64, 1.0f64)]
    fn test_metric_sub<L, R, E>(#[case] lhs: L, #[case] rhs: R, #[case] expected: E)
    where
        L: Into<Metric>,
        R: Into<Metric>,
        E: Into<Metric>,
    {
        let expected = expected.into();

        let lhs = lhs.into();
        let rhs = rhs.into();

        assert_eq!(lhs - rhs, expected);
    }

    #[rstest]
    #[case::zero(0, 0, Ordering::Equal)]
    #[case::one_zero(1, 0, Ordering::Greater)]
    #[case::zero_float(0.0f64, 0.0f64, Ordering::Equal)]
    #[case::one_zero_float(1.0f64, 0.0f64, Ordering::Greater)]
    #[case::one_int_zero_float(1, 0.0f64, Ordering::Greater)]
    #[case::one_float_zero_int(1.0f64, 0, Ordering::Greater)]
    #[case::some_number(220, 220.0f64, Ordering::Equal)]
    fn test_metric_ordering<L, R>(#[case] lhs: L, #[case] rhs: R, #[case] expected: Ordering)
    where
        L: Into<Metric>,
        R: Into<Metric>,
    {
        let lhs: Metric = lhs.into();
        let rhs = rhs.into();

        assert_eq!(lhs.cmp(&rhs), expected);
        assert_eq!(rhs.cmp(&lhs), expected.reverse());
    }

    #[rstest]
    #[case::new_zero(EitherOrBoth::Left(0), None)]
    #[case::new_one(EitherOrBoth::Left(1), None)]
    #[case::new_u64_max(EitherOrBoth::Left(u64::MAX), None)]
    #[case::old_zero(EitherOrBoth::Right(0), None)]
    #[case::old_one(EitherOrBoth::Right(1), None)]
    #[case::old_u64_max(EitherOrBoth::Right(u64::MAX), None)]
    #[case::both_zero(
        EitherOrBoth::Both(0, 0),
        (0f64, 1f64)
    )]
    #[case::both_one(
        EitherOrBoth::Both(1, 1),
        (0f64, 1f64)
    )]
    #[case::both_u64_max(
        EitherOrBoth::Both(u64::MAX, u64::MAX),
        (0f64, 1f64)
    )]
    #[case::new_one_old_zero(
        EitherOrBoth::Both(1, 0),
        (f64::INFINITY, f64::INFINITY)
    )]
    #[case::new_one_old_two(
        EitherOrBoth::Both(1, 2),
        (-50f64, -2f64)
    )]
    #[case::new_zero_old_one(
        EitherOrBoth::Both(0, 1),
        (-100f64, f64::NEG_INFINITY)
    )]
    #[case::new_two_old_one(
        EitherOrBoth::Both(2, 1),
        (100f64, 2f64)
    )]
    fn test_metrics_diff_new<T>(#[case] metrics: EitherOrBoth<u64>, #[case] expected_diffs: T)
    where
        T: Into<Option<(f64, f64)>>,
    {
        let expected = expected_metrics_diff(metrics.map(Metric::Int), expected_diffs);
        let actual = MetricsDiff::new(metrics.map(Metric::Int));

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::new_new(EitherOrBoth::Left(1), EitherOrBoth::Left(2), EitherOrBoth::Left(3))]
    #[case::new_old(
        EitherOrBoth::Left(1),
        EitherOrBoth::Right(2),
        EitherOrBoth::Both(1, 2)
    )]
    #[case::new_both(
        EitherOrBoth::Left(1),
        EitherOrBoth::Both(2, 5),
        EitherOrBoth::Both(3, 5)
    )]
    #[case::old_old(EitherOrBoth::Right(1), EitherOrBoth::Right(2), EitherOrBoth::Right(3))]
    #[case::old_new(
        EitherOrBoth::Right(1),
        EitherOrBoth::Left(2),
        EitherOrBoth::Both(2, 1)
    )]
    #[case::old_both(
        EitherOrBoth::Right(1),
        EitherOrBoth::Both(2, 5),
        EitherOrBoth::Both(2, 6)
    )]
    #[case::both_new(
        EitherOrBoth::Both(2, 5),
        EitherOrBoth::Left(1),
        EitherOrBoth::Both(3, 5)
    )]
    #[case::both_old(
        EitherOrBoth::Both(2, 5),
        EitherOrBoth::Right(1),
        EitherOrBoth::Both(2, 6)
    )]
    #[case::both_both(
        EitherOrBoth::Both(2, 5),
        EitherOrBoth::Both(1, 3),
        EitherOrBoth::Both(3, 8)
    )]
    #[case::saturating_new(
        EitherOrBoth::Left(u64::MAX),
        EitherOrBoth::Left(1),
        EitherOrBoth::Left(u64::MAX)
    )]
    #[case::saturating_new_other(
        EitherOrBoth::Left(1),
        EitherOrBoth::Left(u64::MAX),
        EitherOrBoth::Left(u64::MAX)
    )]
    #[case::saturating_old(
        EitherOrBoth::Right(u64::MAX),
        EitherOrBoth::Right(1),
        EitherOrBoth::Right(u64::MAX)
    )]
    #[case::saturating_old_other(
        EitherOrBoth::Right(1),
        EitherOrBoth::Right(u64::MAX),
        EitherOrBoth::Right(u64::MAX)
    )]
    #[case::saturating_both(
        EitherOrBoth::Both(u64::MAX, u64::MAX),
        EitherOrBoth::Both(1, 1),
        EitherOrBoth::Both(u64::MAX, u64::MAX)
    )]
    #[case::saturating_both_other(
        EitherOrBoth::Both(1, 1),
        EitherOrBoth::Both(u64::MAX, u64::MAX),
        EitherOrBoth::Both(u64::MAX, u64::MAX)
    )]
    fn test_metrics_diff_add(
        #[case] metric: EitherOrBoth<u64>,
        #[case] other_metric: EitherOrBoth<u64>,
        #[case] expected: EitherOrBoth<u64>,
    ) {
        let new_diff = MetricsDiff::new(metric.map(Metric::Int));
        let old_diff = MetricsDiff::new(other_metric.map(Metric::Int));
        let expected = MetricsDiff::new(expected.map(Metric::Int));

        assert_eq!(new_diff.add(&old_diff), expected);
        assert_eq!(old_diff.add(&new_diff), expected);
    }

    #[rstest]
    #[case::new_ir(&[0], &[], &[(EitherOrBoth::Left(Metric::Int(0)), None)])]
    #[case::new_is_summarized(&[10, 20, 30, 1, 2, 3, 4, 2, 0], &[],
        &[
            (EitherOrBoth::Left(Metric::Int(10)), None),
            (EitherOrBoth::Left(Metric::Int(20)), None),
            (EitherOrBoth::Left(Metric::Int(30)), None),
            (EitherOrBoth::Left(Metric::Int(1)), None),
            (EitherOrBoth::Left(Metric::Int(2)), None),
            (EitherOrBoth::Left(Metric::Int(3)), None),
            (EitherOrBoth::Left(Metric::Int(4)), None),
            (EitherOrBoth::Left(Metric::Int(2)), None),
            (EitherOrBoth::Left(Metric::Int(0)), None),
            (EitherOrBoth::Left(Metric::Int(54)), None),
            (EitherOrBoth::Left(Metric::Int(0)), None),
            (EitherOrBoth::Left(Metric::Int(6)), None),
            (EitherOrBoth::Left(Metric::Int(60)), None),
            (EitherOrBoth::Left(Metric::Int(264)), None),
            (EitherOrBoth::Left(Metric::Float(10f64)), None),
            (EitherOrBoth::Left(Metric::Float(10f64)), None),
            (EitherOrBoth::Left(Metric::Float(40f64)), None),
            (EitherOrBoth::Left(Metric::Float(4f64)), None),
            (EitherOrBoth::Left(Metric::Float(10f64)), None),
            (EitherOrBoth::Left(Metric::Float(90f64)), None),
            (EitherOrBoth::Left(Metric::Float(0f64)), None),
            (EitherOrBoth::Left(Metric::Float(10f64)), None),
        ]
    )]
    #[case::old_ir(&[], &[0], &[(EitherOrBoth::Right(Metric::Int(0)), None)])]
    #[case::old_is_summarized(&[], &[5, 10, 15, 1, 2, 3, 4, 1, 0],
        &[
            (EitherOrBoth::Right(Metric::Int(5)), None),
            (EitherOrBoth::Right(Metric::Int(10)), None),
            (EitherOrBoth::Right(Metric::Int(15)), None),
            (EitherOrBoth::Right(Metric::Int(1)), None),
            (EitherOrBoth::Right(Metric::Int(2)), None),
            (EitherOrBoth::Right(Metric::Int(3)), None),
            (EitherOrBoth::Right(Metric::Int(4)), None),
            (EitherOrBoth::Right(Metric::Int(1)), None),
            (EitherOrBoth::Right(Metric::Int(0)), None),
            (EitherOrBoth::Right(Metric::Int(24)), None),
            (EitherOrBoth::Right(Metric::Int(1)), None),
            (EitherOrBoth::Right(Metric::Int(5)), None),
            (EitherOrBoth::Right(Metric::Int(30)), None),
            (EitherOrBoth::Right(Metric::Int(204)), None),
            (EitherOrBoth::Right(Metric::Float(20f64)), None),
            (EitherOrBoth::Right(Metric::Float(20f64)), None),
            (EitherOrBoth::Right(Metric::Float(80f64)), None),
            (EitherOrBoth::Right(Metric::Float(4f64)), None),
            (EitherOrBoth::Right(Metric::Float(16.666_666_666_666_664_f64)), None),
            (EitherOrBoth::Right(Metric::Float(80f64)), None),
            (EitherOrBoth::Right(Metric::Float(3.333_333_333_333_333_5_f64)), None),
            (EitherOrBoth::Right(Metric::Float(16.666_666_666_666_664_f64)), None),
        ]
    )]
    #[case::new_and_old_ir_zero(&[0], &[0], &[
        (EitherOrBoth::Both(Metric::Int(0), Metric::Int(0)), (0f64, 1f64))
    ])]
    #[case::new_and_old_summarized_when_equal(
        &[10, 20, 30, 1, 2, 3, 4, 2, 0],
        &[10, 20, 30, 1, 2, 3, 4, 2, 0],
        &[
            (EitherOrBoth::Both(Metric::Int(10), Metric::Int(10)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(20), Metric::Int(20)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(30), Metric::Int(30)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(1), Metric::Int(1)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(2), Metric::Int(2)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(3), Metric::Int(3)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(4), Metric::Int(4)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(2), Metric::Int(2)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(0), Metric::Int(0)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(54), Metric::Int(54)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(0), Metric::Int(0)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(6), Metric::Int(6)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(60), Metric::Int(60)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(264), Metric::Int(264)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(10f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(10f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(40f64), Metric::Float(40f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(4f64), Metric::Float(4f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(10f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(90f64), Metric::Float(90f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(0f64), Metric::Float(0f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(10f64)), (0f64, 1f64)),
        ]
    )]
    #[case::new_and_old_summarized_when_not_equal(
        &[10, 20, 30, 1, 2, 3, 4, 2, 0],
        &[5, 10, 15, 1, 2, 3, 4, 1, 0],
        &[
            (EitherOrBoth::Both(Metric::Int(10), Metric::Int(5)), (100f64, 2f64)),
            (EitherOrBoth::Both(Metric::Int(20), Metric::Int(10)), (100f64, 2f64)),
            (EitherOrBoth::Both(Metric::Int(30), Metric::Int(15)), (100f64, 2f64)),
            (EitherOrBoth::Both(Metric::Int(1), Metric::Int(1)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(2), Metric::Int(2)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(3), Metric::Int(3)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(4), Metric::Int(4)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(2), Metric::Int(1)), (100f64, 2f64)),
            (EitherOrBoth::Both(Metric::Int(0), Metric::Int(0)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Int(54), Metric::Int(24)), (125f64, 2.25f64)),
            (EitherOrBoth::Both(Metric::Int(0), Metric::Int(1)), (-100f64, f64::NEG_INFINITY)),
            (EitherOrBoth::Both(Metric::Int(6), Metric::Int(5)), (20f64, 1.2f64)),
            (EitherOrBoth::Both(Metric::Int(60), Metric::Int(30)), (100f64, 2f64)),
            (EitherOrBoth::Both(Metric::Int(264), Metric::Int(204)),
                (29.411_764_705_882_355_f64, 1.294_117_647_058_823_6_f64)
            ),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(20f64)), (-50f64, -2f64)),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(20f64)), (-50f64, -2f64)),
            (EitherOrBoth::Both(Metric::Float(40f64), Metric::Float(80f64)), (-50f64, -2f64)),
            (EitherOrBoth::Both(Metric::Float(4f64), Metric::Float(4f64)), (0f64, 1f64)),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(16.666_666_666_666_664_f64)),
                (-39.999_999_999_999_99_f64, -1.666_666_666_666_666_5_f64)
            ),
            (EitherOrBoth::Both(Metric::Float(90f64), Metric::Float(80f64)), (12.5f64, 1.125f64)),
            (EitherOrBoth::Both(Metric::Float(0f64), Metric::Float(3.333_333_333_333_333_5_f64)),
                (-100f64, f64::NEG_INFINITY)
            ),
            (EitherOrBoth::Both(Metric::Float(10f64), Metric::Float(16.666_666_666_666_664_f64)),
                (-39.999_999_999_999_99_f64, -1.666_666_666_666_666_5_f64)
            ),
        ]
    )]
    fn test_metrics_summary_new<V>(
        #[case] new_metrics: &[u64],
        #[case] old_metrics: &[u64],
        #[case] expected: &[(EitherOrBoth<Metric>, V)],
    ) where
        V: Into<Option<(f64, f64)>> + Clone,
    {
        use either_or_both::EitherOrBoth;

        let expected_metrics_summary =
            metrics_summary_fixture(expected.iter().map(|(e, v)| (*e, v.clone())));
        let actual = match (
            (!new_metrics.is_empty()).then_some(new_metrics),
            (!old_metrics.is_empty()).then_some(old_metrics),
        ) {
            (None, None) => unreachable!(),
            (Some(new), None) => MetricsSummary::new(EitherOrBoth::Left(metrics_fixture(new))),
            (None, Some(old)) => MetricsSummary::new(EitherOrBoth::Right(metrics_fixture(old))),
            (Some(new), Some(old)) => MetricsSummary::new(EitherOrBoth::Both(
                metrics_fixture(new),
                metrics_fixture(old),
            )),
        };

        assert_eq!(actual, expected_metrics_summary);
    }

    #[test]
    fn test_metrics_summary_normalizes_compatible_units() {
        let metric_kind = PerfMetric("task-clock:u".to_owned());
        let summary = MetricsSummary::new(EitherOrBoth::Both(
            Metrics(indexmap! {
             metric_kind.clone() => AnnotatedMetric::with_default_qualities(
                 Metric::Float(1.0),
                 Unit::Seconds
            )}),
            Metrics(indexmap! {
            metric_kind.clone() => AnnotatedMetric::with_default_qualities(
                Metric::Float(1000.0),
                Unit::Milliseconds)
            }),
        ));

        let expected = MetricsDiff {
            diffs: Some(Diffs {
                diff_pct: 0.0,
                factor: 1.0,
            }),
            metrics: EitherOrBoth::Both(
                AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
                AnnotatedMetric::with_default_qualities(1.0, Unit::Seconds),
            ),
        };

        let diff = summary.diff_by_kind(&metric_kind).unwrap();

        assert_eq!(*diff, expected);
    }

    #[test]
    fn test_perf_metric_summary_serializes_with_string_keys() {
        let summary = MetricsSummary::new(EitherOrBoth::Left(Metrics(indexmap! {
            PerfMetric("task-clock:u".to_owned()) => AnnotatedMetric::with_default_qualities(
                Metric::Float(1.0),
                Unit::Milliseconds
           )
        })));

        let value = serde_json::to_value(summary).unwrap();
        let expected_value = serde_json::to_value("Milliseconds").unwrap();

        assert_eq!(
            expected_value,
            value.get("task-clock:u").unwrap()["metrics"]["Left"]["unit"],
        );
    }

    #[rstest]
    #[case::prefers_present_pair_over_absent_metadata(
        PerfQualities::default(),
        PerfQualities::new(100, 50.0, 7.0, 1, 100.0),
        PerfQualities::new(100, 50.0, None, 1, None)
    )]
    #[case::recomputes_running_percentage_and_clears_variance(
        PerfQualities::new(100, 50.0, 7.0, 1, 100.0),
        PerfQualities::new(300, 75.0, 11.0, 2, 300.0),
        PerfQualities::new(400, 66.666_666_666_666_67, None, 3, None)
    )]
    #[case::canonicalizes_double_zero_runtime_pair(
        PerfQualities::new(10, 0.0, 1.0, 1, 0.0),
        PerfQualities::new(20, 0.0, 2.0, 2, 0.0),
        PerfQualities::new(0, 0.0, None, 3, None)
    )]
    #[case::canonicalizes_both_zero_runtime_with_nonzero_pcnt(
        PerfQualities::new(0, 50.0, 1.0, 1, 0.0),
        PerfQualities::new(0, 75.0, 2.0, 2, 0.0),
        PerfQualities::new(0, 0.0, None, 3, None)
    )]
    #[case::one_zero_runtime_uses_other_pcnt(
        PerfQualities::new(0, 50.0, 1.0, 1, 0.0),
        PerfQualities::new(300, 75.0, 2.0, 2, 300.0),
        PerfQualities::new(300, 75.0, None, 3, None)
    )]
    fn test_perf_qualities_add(
        #[case] lhs: PerfQualities,
        #[case] rhs: PerfQualities,
        #[case] expected: PerfQualities,
    ) {
        let actual = lhs.add(&rhs);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_normalize_metrics_when_repetitions_then_divides_metric() {
        let mut metrics = Metrics::with_metric_kinds([
            (
                PerfMetric("instructions:u".to_owned()),
                AnnotatedMetric::with_default_qualities(100, None),
            ),
            (
                PerfMetric("task-clock".to_owned()),
                AnnotatedMetric::with_default_qualities(25.0, Unit::Milliseconds),
            ),
        ]);

        let expected = Metrics::with_metric_kinds([
            (
                PerfMetric("instructions:u".to_owned()),
                AnnotatedMetric::with_default_qualities(25, None),
            ),
            (
                PerfMetric("task-clock".to_owned()),
                AnnotatedMetric::with_default_qualities(6.25, Unit::Milliseconds),
            ),
        ]);

        metrics.normalize_by_repetitions(4);

        assert_eq!(metrics, expected);

        assert!(metrics.0.first().unwrap().1.metric.is_int());
        assert!(metrics.0.get_index(1).unwrap().1.metric.is_float());
    }

    #[test]
    fn test_normalize_metrics_calibration_subtraction_happens_before_repetition_normalization() {
        let mut metrics = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(120, None),
        )]);

        let calibration = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(20, None),
        )]);

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(25, None),
        )]);

        metrics.saturating_sub_by_key(&calibration);
        metrics.normalize_by_repetitions(4);

        assert_eq!(metrics, expected);
    }

    #[test]
    fn test_normalize_metrics_divides_event_runtime_but_preserves_percent_like_qualities() {
        let mut metrics = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(100, PerfQualities::new(200, 75.0, 5.0, 1, 200.0), None),
        )]);

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(25, PerfQualities::new(50, 75.0, 5.0, 1, 50.0), None),
        )]);

        metrics.normalize_by_repetitions(4);

        assert_eq!(metrics, expected);
    }

    #[test]
    fn test_normalize_metrics_rescales_time_metric_and_mean_together() {
        let mut metrics = Metrics::with_metric_kinds([(
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::new(
                1.5,
                PerfQualities::new(300, 75.0, 0.05, 2, 1.5),
                Unit::Seconds,
            ),
        )]);

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::new(
                375.0,
                PerfQualities::new(75, 75.0, 0.05, 2, 375.0),
                Unit::Milliseconds,
            ),
        )]);

        metrics.normalize_by_repetitions(4);

        assert_eq!(metrics, expected);
    }
}
