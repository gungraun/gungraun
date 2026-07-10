//! TODO DOCS

use std::fmt::Display;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "runner")]
use crate::metrics::model::Metric;

/// TODO: DOCS
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum Unit {
    /// TODO: DOCS
    Nanoseconds,
    /// TODO: DOCS
    Microseconds,
    /// TODO: DOCS
    Milliseconds,
    /// TODO: DOCS
    Seconds,

    /// TODO: DOCS
    Hertz,
    /// TODO: DOCS
    Kilohertz,
    /// TODO: DOCS
    Megahertz,
    /// TODO: DOCS
    Gigahertz,

    /// TODO: DOCS
    Bytes,
    /// TODO: DOCS
    Kilobytes,
    /// TODO: DOCS
    Megabytes,
    /// TODO: DOCS
    Gigabytes,
    /// TODO: DOCS
    Kibibytes,
    /// TODO: DOCS
    Mebibytes,
    /// TODO: DOCS
    Gibibytes,

    /// TODO: DOCS
    Percent,
    /// TODO: DOCS
    Joules,
    /// TODO: DOCS
    Watts,
    /// TODO: DOCS
    Volts,
    /// TODO: DOCS
    Amperes,
    /// TODO: DOCS
    RevolutionsPerMinute,
    /// TODO: DOCS
    Celsius,
    /// TODO: DOCS
    Capacity,
    /// TODO: DOCS
    Cycles,

    /// TODO: DOCS
    Rate(Box<Self>, Box<Self>),
    /// TODO: DOCS
    Unknown(String),
}

/// TODO: DOCS
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum UnitDimension {
    /// TODO: DOCS
    Time,
    /// TODO: DOCS
    Data,
    /// TODO: DOCS
    Frequency,
}

#[cfg(feature = "runner")]
impl Unit {
    /// TODO: DOCS
    pub fn base_scale(&self) -> Option<f64> {
        match self {
            Self::Nanoseconds => Some(1e-9),
            Self::Microseconds => Some(1e-6),
            Self::Milliseconds => Some(1e-3),
            Self::Seconds | Self::Bytes | Self::Hertz => Some(1.0),

            Self::Kilobytes | Self::Kilohertz => Some(1e3),
            Self::Megabytes | Self::Megahertz => Some(1e6),
            Self::Gigabytes | Self::Gigahertz => Some(1e9),
            Self::Kibibytes => Some(1024.0),
            Self::Mebibytes => Some(1024.0 * 1024.0),
            Self::Gibibytes => Some(1024.0 * 1024.0 * 1024.0),

            _ => None,
        }
    }

    /// TODO: DOCS
    pub fn dimension(&self) -> Option<UnitDimension> {
        match self {
            Self::Nanoseconds | Self::Microseconds | Self::Milliseconds | Self::Seconds => {
                Some(UnitDimension::Time)
            }
            Self::Bytes
            | Self::Kilobytes
            | Self::Megabytes
            | Self::Gigabytes
            | Self::Kibibytes
            | Self::Mebibytes
            | Self::Gibibytes => Some(UnitDimension::Data),
            Self::Hertz | Self::Kilohertz | Self::Megahertz | Self::Gigahertz => {
                Some(UnitDimension::Frequency)
            }
            _ => None,
        }
    }

    /// TODO: DOCS
    pub fn is_same_dimension(&self, other: &Self) -> bool {
        self.dimension()
            .is_some_and(|d| other.dimension().is_some_and(|o| d == o))
    }

    /// TODO: DOCS, most of these units come from perf
    pub fn parse(unit: &str) -> Self {
        let unit = unit.trim();
        if let Some((numerator, denominator)) = unit.split_once('/') {
            Self::Rate(
                Box::new(Self::parse(numerator)),
                Box::new(Self::parse(denominator)),
            )
        } else if unit.eq_ignore_ascii_case("ns") || unit.eq_ignore_ascii_case("nsec") {
            Self::Nanoseconds
        } else if unit.eq_ignore_ascii_case("us") || unit.eq_ignore_ascii_case("usec") {
            Self::Microseconds
        } else if unit.eq_ignore_ascii_case("ms") || unit.eq_ignore_ascii_case("msec") {
            Self::Milliseconds
        } else if unit.eq_ignore_ascii_case("s")
            || unit.eq_ignore_ascii_case("sec")
            || unit.eq_ignore_ascii_case("secs")
            || unit.eq_ignore_ascii_case("seconds")
        {
            Self::Seconds
        } else if unit.eq_ignore_ascii_case("hz") {
            Self::Hertz
        } else if unit.eq_ignore_ascii_case("khz") {
            Self::Kilohertz
        } else if unit.eq_ignore_ascii_case("mhz") {
            Self::Megahertz
        } else if unit.eq_ignore_ascii_case("ghz") {
            Self::Gigahertz
        } else if unit == "B" || unit.eq_ignore_ascii_case("bytes") {
            Self::Bytes
        } else if unit == "KB" || unit == "kB" {
            Self::Kilobytes
        } else if unit == "MB" {
            Self::Megabytes
        } else if unit == "GB" {
            Self::Gigabytes
        } else if unit.eq_ignore_ascii_case("kib") {
            Self::Kibibytes
        } else if unit.eq_ignore_ascii_case("mib") {
            Self::Mebibytes
        } else if unit.eq_ignore_ascii_case("gib") {
            Self::Gibibytes
        } else if unit == "%" {
            Self::Percent
        } else if unit.eq_ignore_ascii_case("joules") || unit == "J" {
            Self::Joules
        } else if unit.eq_ignore_ascii_case("watts") || unit == "W" {
            Self::Watts
        } else if unit.eq_ignore_ascii_case("volt")
            || unit.eq_ignore_ascii_case("volts")
            || unit == "V"
        {
            Self::Volts
        } else if unit.eq_ignore_ascii_case("ampere")
            || unit.eq_ignore_ascii_case("amperes")
            || unit == "A"
        {
            Self::Amperes
        } else if unit.eq_ignore_ascii_case("rpm") {
            Self::RevolutionsPerMinute
        } else if unit.eq_ignore_ascii_case("celsius") || unit.eq_ignore_ascii_case("'c") {
            Self::Celsius
        } else if unit.eq_ignore_ascii_case("capacity") {
            Self::Capacity
        } else if unit.eq_ignore_ascii_case("cycles") {
            Self::Cycles
        } else {
            Self::Unknown(unit.to_owned())
        }
    }

    /// TODO: DOCS
    pub fn scale_factor(&self, target: &Self) -> Option<f64> {
        if self == target {
            return Some(1.0);
        }

        if let (Self::Rate(s_num, s_den), Self::Rate(t_num, t_den)) = (self, target) {
            Some(s_num.scale_factor(t_num)? / s_den.scale_factor(t_den)?)
        } else {
            if !self.is_same_dimension(target) {
                return None;
            }

            Some(self.base_scale()? / target.base_scale()?)
        }
    }

    fn scale_ladder(&self) -> Option<&'static [Self]> {
        match self {
            Self::Nanoseconds | Self::Microseconds | Self::Milliseconds | Self::Seconds => Some(&[
                Self::Nanoseconds,
                Self::Microseconds,
                Self::Milliseconds,
                Self::Seconds,
            ]),
            Self::Hertz | Self::Kilohertz | Self::Megahertz | Self::Gigahertz => Some(&[
                Self::Hertz,
                Self::Kilohertz,
                Self::Megahertz,
                Self::Gigahertz,
            ]),
            Self::Bytes | Self::Kilobytes | Self::Megabytes | Self::Gigabytes => Some(&[
                Self::Bytes,
                Self::Kilobytes,
                Self::Megabytes,
                Self::Gigabytes,
            ]),
            Self::Kibibytes | Self::Mebibytes | Self::Gibibytes => Some(&[
                Self::Bytes,
                Self::Kibibytes,
                Self::Mebibytes,
                Self::Gibibytes,
            ]),
            // Rate: no single ladder — rescale processes numerator recursively
            Self::Rate(..)
            | Self::Unknown(..)
            | Self::Percent
            | Self::Joules
            | Self::Watts
            | Self::Volts
            | Self::Amperes
            | Self::RevolutionsPerMinute
            | Self::Celsius
            | Self::Capacity
            | Self::Cycles => None,
        }
    }

    /// Rescales `value` (expressed in this unit) to the nicest unit on the same ladder, returning
    /// `(rescaled_value, target_unit)`.
    ///
    /// Picks the largest unit where `|value_in_target| >= 1.0`. For `Rate`, rescales only the
    /// numerator. Returns `(value, self.clone())` for units without a ladder (e.g. `Percent`,
    /// `Celsius`, `Unknown`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun_runner::units::Unit::*;
    ///
    /// assert_eq!(Milliseconds.rescale(0.0005), (500.0, Nanoseconds));
    /// assert_eq!(Milliseconds.rescale(1500.0), (1.5, Seconds));
    /// assert_eq!(Nanoseconds.rescale(5000000.0), (5.0, Milliseconds));
    /// assert_eq!(Nanoseconds.rescale(0.5), (0.5, Nanoseconds));
    ///
    /// assert_eq!(Kibibytes.rescale(0.5), (512.0, Bytes));
    /// assert_eq!(Percent.rescale(0.5), (0.5, Percent));
    /// assert_eq!(Hertz.rescale(2500000.0), (2.5, Megahertz));
    ///
    /// assert_eq!(
    ///     Rate(Box::new(Megabytes), Box::new(Seconds)).rescale(0.5),
    ///     (500.0, Rate(Box::new(Kilobytes), Box::new(Seconds)))
    /// );
    /// ```
    pub fn rescale(&self, value: f64) -> (f64, Self) {
        if !value.is_finite() || value == 0.0 {
            return (value, self.clone());
        }

        // For Rate, rescale the numerator only
        if let Self::Rate(num, den) = self {
            let (new_value, new_num) = num.rescale(value);
            return (new_value, Self::Rate(Box::new(new_num), den.clone()));
        }

        let Some(ladder) = self.scale_ladder() else {
            return (value, self.clone());
        };

        let Some(this_base) = self.base_scale() else {
            return (value, self.clone());
        };

        let rebased_value = value * this_base;

        // Find the largest unit where abs(value_in_that_unit) >= 1.0. If not found (value is less
        // than 1.0 even in the smallest unit), use the smallest unit in the ladder.
        let target_unit = ladder
            .iter()
            .rev()
            .find_map(|unit| {
                unit.base_scale().and_then(|unit_base| {
                    ((rebased_value / unit_base).abs() >= 1.0).then_some(unit)
                })
            })
            .unwrap_or_else(|| {
                ladder
                    .first()
                    .expect("The scale ladder should always contain at least one element")
            });

        self.scale_factor(target_unit).map_or_else(
            || (value, self.clone()),
            |factor| (value * factor, target_unit.clone()),
        )
    }

    /// Converts a value in this unit into the canonical base scale for its dimension.
    ///
    /// For example:
    /// - `Milliseconds.base_value(750.0) == 0.75`
    /// - `Kilobytes.base_value(2.0) == 2000.0`
    ///
    /// Returns the input unchanged for units without a base scale.
    pub fn base_value(&self, value: f64) -> f64 {
        self.base_scale().map_or(value, |scale| value * scale)
    }

    /// Converts a canonical base-scale value into this unit.
    ///
    /// This is the inverse of [`Unit::base_value`] for scalable units.
    ///
    /// For example:
    /// - `Milliseconds.rebase(0.75) == 750.0`
    /// - `Kilobytes.rebase(2000.0) == 2.0`
    ///
    /// Returns the input unchanged for units without a base scale.
    pub fn rebase(&self, value: f64) -> f64 {
        self.base_scale().map_or(value, |scale| value / scale)
    }

    /// Returns the scale factor to convert a value in this unit to `target` as a [`Metric`].
    ///
    /// TODO: IMPROVE docs, Tries to preserve integer metrics by scaling upwards instead of
    /// downwards.
    ///
    /// Returns [`Metric::Int`] when the factor is an exact integer - always the case for
    /// same-ladder conversions from a larger unit to a smaller one (e.g., `Seconds` ->
    /// `Milliseconds` produces [`Metric::Int(1000)`]). Returns [`Metric::Float`] for fractional
    /// factors (e.g., `Milliseconds` -> `Seconds`). Returns `None` if the units are not convertible
    /// (cross-dimension or [`Unit::Unknown`]).
    ///
    /// For [`Unit::Rate`], recurses into numerator and denominator.
    ///
    /// [`Metric::Int(1000)`]: Metric::Int
    #[must_use]
    pub fn scale_factor_metric(&self, target: &Self) -> Option<Metric> {
        if self == target {
            return Some(Metric::Int(1));
        }

        // Same ladder, self is larger (later index) -> target is smaller: integer factor
        if let Some(ladder) = self.scale_ladder() {
            if let (Some(self_idx), Some(target_idx)) = (
                ladder.iter().position(|u| u == self),
                ladder.iter().position(|u| u == target),
            ) {
                if self_idx > target_idx {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "The ladder is too small for indices larger than u32"
                    )]
                    let steps = (self_idx - target_idx) as u32;
                    let base = Self::ladder_base(ladder);
                    return Some(Metric::Int(base.saturating_pow(steps)));
                }
                // self_idx < target_idx: smaller -> larger, fractional
                // Fall through to float computation
            }
        }

        // Rate: recurse into numerator and denominator
        if let (Self::Rate(s_num, s_den), Self::Rate(t_num, t_den)) = (self, target) {
            let num_factor = s_num.scale_factor_metric(t_num)?;
            let den_factor = s_den.scale_factor_metric(t_den)?;
            return Some(num_factor / den_factor);
        }

        // Cross-dimension or Unknown: not convertible
        if !self.is_same_dimension(target) {
            return None;
        }

        // Same dimension but different ladder (e.g., KB → KiB) or smaller -> larger or fractional
        Some(Metric::Float(self.base_scale()? / target.base_scale()?))
    }

    /// Returns the multiplicative base of a unit ladder (1000 for SI, 1024 for binary).
    fn ladder_base(ladder: &[Self]) -> u64 {
        if ladder.contains(&Self::Kibibytes)
            || ladder.contains(&Self::Mebibytes)
            || ladder.contains(&Self::Gibibytes)
        {
            1024
        } else {
            1000
        }
    }
}

impl Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nanoseconds => f.write_str("ns"),
            Self::Microseconds => f.write_str("us"),
            Self::Milliseconds => f.write_str("ms"),
            Self::Seconds => f.write_str("s"),
            Self::Hertz => f.write_str("Hz"),
            Self::Kilohertz => f.write_str("kHz"),
            Self::Megahertz => f.write_str("MHz"),
            Self::Gigahertz => f.write_str("GHz"),
            Self::Bytes => f.write_str("B"),
            Self::Kilobytes => f.write_str("KB"),
            Self::Megabytes => f.write_str("MB"),
            Self::Gigabytes => f.write_str("GB"),
            Self::Kibibytes => f.write_str("KiB"),
            Self::Mebibytes => f.write_str("MiB"),
            Self::Gibibytes => f.write_str("GiB"),
            Self::Percent => f.write_str("%"),
            Self::Joules => f.write_str("J"),
            Self::Watts => f.write_str("W"),
            Self::Volts => f.write_str("V"),
            Self::Amperes => f.write_str("A"),
            Self::RevolutionsPerMinute => f.write_str("rpm"),
            Self::Celsius => f.write_str("'C"),
            Self::Capacity => f.write_str("cap"),
            Self::Cycles => f.write_str("cyc"),
            Self::Rate(numerator, denominator) => write!(f, "{numerator}/{denominator}"),
            Self::Unknown(unit) => f.write_str(unit),
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use rstest::rstest;

    use super::*;
    use crate::metrics::model::Metric;

    #[rstest]
    #[case::ns("ns", Unit::Nanoseconds)]
    #[case::nsec("nsec", Unit::Nanoseconds)]
    #[case::us("us", Unit::Microseconds)]
    #[case::usec("usec", Unit::Microseconds)]
    #[case::ms("ms", Unit::Milliseconds)]
    #[case::msec("msec", Unit::Milliseconds)]
    #[case::s("s", Unit::Seconds)]
    #[case::sec("sec", Unit::Seconds)]
    #[case::secs("secs", Unit::Seconds)]
    #[case::seconds("seconds", Unit::Seconds)]
    #[case::hz("Hz", Unit::Hertz)]
    #[case::khz("kHz", Unit::Kilohertz)]
    #[case::mhz("MHz", Unit::Megahertz)]
    #[case::ghz("GHz", Unit::Gigahertz)]
    #[case::b("B", Unit::Bytes)]
    #[case::bytes("bytes", Unit::Bytes)]
    #[case::capital_kb("KB", Unit::Kilobytes)]
    #[case::kb("kB", Unit::Kilobytes)]
    #[case::mb("MB", Unit::Megabytes)]
    #[case::gb("GB", Unit::Gigabytes)]
    #[case::kib("KiB", Unit::Kibibytes)]
    #[case::mib("MiB", Unit::Mebibytes)]
    #[case::gib("GiB", Unit::Gibibytes)]
    #[case::percent("%", Unit::Percent)]
    #[case::j("J", Unit::Joules)]
    #[case::joules("joules", Unit::Joules)]
    #[case::w("W", Unit::Watts)]
    #[case::watts("watts", Unit::Watts)]
    #[case::v("V", Unit::Volts)]
    #[case::volts("volts", Unit::Volts)]
    #[case::a("A", Unit::Amperes)]
    #[case::amperes("amperes", Unit::Amperes)]
    #[case::rpm("rpm", Unit::RevolutionsPerMinute)]
    #[case::celsius_short("'C", Unit::Celsius)]
    #[case::celsius("celsius", Unit::Celsius)]
    #[case::capacity("capacity", Unit::Capacity)]
    #[case::cycles("cycles", Unit::Cycles)]
    #[case::unknown("foo", Unit::Unknown("foo".into()))]
    #[case::trimmed(" ms ", Unit::Milliseconds)]
    #[case::capital_ms("MS", Unit::Milliseconds)]
    #[case::capital_khz("KHZ", Unit::Kilohertz)]
    #[case::capital_joules("JOULES", Unit::Joules)]
    #[case::rate("KB/s", Unit::Rate(Box::new(Unit::Kilobytes), Box::new(Unit::Seconds)))]
    fn test_unit_parse(#[case] input: &str, #[case] expected: Unit) {
        assert_eq!(Unit::parse(input), expected);
    }

    #[rstest]
    #[case::rescale_down(Unit::Milliseconds, 0.0005, 500.0, Unit::Nanoseconds)]
    #[case::rescale_up(Unit::Milliseconds, 1500.0, 1.5, Unit::Seconds)]
    #[case::stays_at_smallest(Unit::Nanoseconds, 0.5, 0.5, Unit::Nanoseconds)]
    #[case::when_already_good_then_no_change(Unit::Milliseconds, 1.5, 1.5, Unit::Milliseconds)]
    #[case::when_not_scalable_then_return_unchanged(Unit::Percent, 42.0, 42.0, Unit::Percent)]
    #[case::rate_recurses_nominator(
        Unit::Rate(Box::new(Unit::Megabytes), Box::new(Unit::Seconds)),
        0.5,
        500.0,
        Unit::Rate(Box::new(Unit::Kilobytes), Box::new(Unit::Seconds))
    )]
    #[case::when_zero_then_no_change(Unit::Milliseconds, 0.0, 0.0, Unit::Milliseconds)]
    #[case::when_infinite_then_no_change(
        Unit::Milliseconds,
        f64::INFINITY,
        f64::INFINITY,
        Unit::Milliseconds
    )]
    #[case::when_neg_infinite_then_no_change(
        Unit::Milliseconds,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        Unit::Milliseconds
    )]
    fn test_unit_rescale(
        #[case] unit: Unit,
        #[case] value: f64,
        #[case] expected_value: f64,
        #[case] expected_unit: Unit,
    ) {
        let (rescaled, result_unit) = unit.rescale(value);
        assert_relative_eq!(rescaled, expected_value, epsilon = 1e-10);
        assert_eq!(result_unit, expected_unit);
    }

    #[rstest]
    fn test_unit_rescale_when_nan_then_no_change() {
        let (value, unit) = Unit::Milliseconds.rescale(f64::NAN);
        assert!(value.is_nan());
        assert_eq!(unit, Unit::Milliseconds);
    }

    #[rstest]
    #[case::when_same_as_target_then_1(Unit::Seconds, Unit::Seconds, Some(1.0))]
    #[case::scale_up(Unit::Milliseconds, Unit::Seconds, Some(0.001))]
    #[case::scale_down(Unit::Seconds, Unit::Milliseconds, Some(1000.0))]
    #[case::scale_up_lowest_to_highest(Unit::Nanoseconds, Unit::Seconds, Some(1e-9))]
    #[case::cross_data_dimension_kb_to_kib(Unit::Kilobytes, Unit::Kibibytes, Some(1000.0 / 1024.0))]
    #[case::when_different_dimension_then_none(Unit::Seconds, Unit::Bytes, None)]
    #[case::rate(
        Unit::Rate(Box::new(Unit::Kilobytes), Box::new(Unit::Seconds)),
        Unit::Rate(Box::new(Unit::Bytes), Box::new(Unit::Seconds)),
        Some(1000.0)
    )]
    fn test_unit_scale_factor(#[case] from: Unit, #[case] to: Unit, #[case] expected: Option<f64>) {
        let actual = from.scale_factor(&to);
        match (actual, expected) {
            (Some(a), Some(e)) => assert_relative_eq!(a, e, epsilon = 1e-10),
            (None, None) => {}
            _ => panic!("scale_factor mismatch: got {actual:?}, expected {expected:?}"),
        }
    }

    #[rstest]
    #[case::when_same_then_1(Unit::Seconds, Unit::Seconds, Some(Metric::Int(1)))]
    #[case::scale_down(Unit::Seconds, Unit::Milliseconds, Some(Metric::Int(1000)))]
    #[case::scale_up(Unit::Milliseconds, Unit::Seconds, Some(Metric::Float(0.001)))]
    #[case::when_different_dimension_then_none(Unit::Seconds, Unit::Bytes, None)]
    #[case::when_unknown_then_none(Unit::Unknown("x".into()), Unit::Seconds, None)]
    #[case::when_data_dimension_kb_to_kib(
        Unit::Kilobytes,
        Unit::Kibibytes,
        Some(Metric::Float(1000.0 / 1024.0))
    )]
    #[case::when_data_dimension_kib_to_kb(
        Unit::Kibibytes,
        Unit::Kilobytes,
        Some(Metric::Float(1024.0 / 1000.0))
    )]
    #[case::rate(
        Unit::Rate(Box::new(Unit::Megabytes), Box::new(Unit::Seconds)),
        Unit::Rate(Box::new(Unit::Kilobytes), Box::new(Unit::Seconds)),
        Some(Metric::Float(1000.0))
    )]
    fn test_unit_scale_factor_metric(
        #[case] from: Unit,
        #[case] to: Unit,
        #[case] expected: Option<Metric>,
    ) {
        let actual = from.scale_factor_metric(&to);
        match (actual, expected) {
            (Some(Metric::Float(a)), Some(Metric::Float(e))) => {
                assert_relative_eq!(a, e, epsilon = 1e-10);
            }
            (Some(Metric::Int(a)), Some(Metric::Int(e))) => assert_eq!(a, e),
            (None, None) => {}
            _ => panic!("scale_factor_metric mismatch: got {actual:?}, expected {expected:?}"),
        }
    }
}
