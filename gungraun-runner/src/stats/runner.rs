//! Statistical helpers for comparing benchmark metrics and maintaining running summary statistics.

use std::cmp::Ord;
use std::collections::HashMap;
use std::hash::Hash;

use approx::relative_eq;
use statrs::distribution::{ContinuousCDF, StudentsT};

use crate::metrics::model::{AnnotatedMetric, PerfQualities};

/// Statistical significance summary for the difference between two metrics.
///
/// Computes the [`Self::p_value`], [`Self::significance_factor`] and
/// [`Self::significance_threshold`]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffStats {
    /// The two-tailed p-value of the comparison stored as fraction
    ///
    /// See: <https://en.wikipedia.org/wiki/P-value>
    ///
    /// Smaller values mean stronger evidence that the observed change is real rather than noise. A
    /// comparison is typically [significant] when `p_value < alpha`.
    ///
    /// [significant]: DiffStats::is_significant
    pub p_value: f64,
    /// Ratio of the observed absolute relative change to the significance threshold.
    ///
    /// The natural interpretation is:
    ///
    /// - `< 1.0`: not significant
    /// - `= 1.0`: exactly on the significance boundary
    /// - `> 1.0`: significant
    ///
    /// And concretely:
    /// - `2.0` means the observed absolute relative change is twice the minimum change required
    ///   for significance
    /// - `0.5` means the observed absolute relative change is only half of what would be required,
    ///   so it is not significant
    pub significance_factor: f64,
    /// Smallest absolute relative change considered significant at the chosen confidence level.
    ///
    /// The value is stored as a fraction, not a percentage, so `0.10` means `10%`.
    ///
    /// A larger threshold means the comparison is more uncertain, so only larger changes count as
    /// significant. A smaller threshold means the data is more precise, so smaller changes can
    /// already be significant.
    ///
    /// For example, a threshold of `0.10` means the absolute relative change must be at least
    /// `10%` to be considered significant. This relationship makes it natural to show next to
    /// the relative change.
    pub significance_threshold: f64,
}

/// Running statistics of metrics
///
/// This uses an online algorithm to accumulate the mean and `M2` (sum of squared deviations).
/// The first sample is represented by:
///
/// - `n = 1`
/// - `mean = x`
/// - `m2 = 0.0`
///
/// `m2` must start at `0.0`, because the first sample has zero deviation from its own mean.
///
/// <https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance>
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct OnlineStats {
    /// Sum of squared deviations from the running mean (`M2` in Welford's algorithm).
    ///
    /// This is expected to stay non-negative, aside from possible tiny floating-point roundoff
    /// effects.
    pub m2: f64,
    /// Running mean of all observed samples.
    pub mean: f64,
    /// Number of observed samples accumulated into these statistics.
    pub n: u64,
}

/// Running statistics keyed by metric identifier.
#[derive(Debug, PartialEq)]
pub struct OnlineStatsMap<T>(pub HashMap<T, OnlineStats>)
where
    T: Eq + Hash;

/// Summary statistics derived from [`OnlineStats`].
#[derive(Debug, Copy, Clone)]
pub struct Stats {
    /// Running statistics from which this summary was derived.
    pub online_stats: OnlineStats,
    /// Relative standard error as a fraction, not a percentage.
    ///
    /// This is expected to be non-negative.
    pub rse: f64,
}

impl DiffStats {
    /// Returns whether this comparison is significant for the given `alpha`.
    pub fn is_significant(&self, alpha: f64) -> bool {
        self.p_value < alpha
    }

    /// Creates a new `DiffStats` for the relative change between two [`AnnotatedMetric`]s
    ///
    /// This method requires the stored means, sample counts, and relative standard errors of
    /// [`PerfQualities`] to estimate the uncertainty of the relative change.
    ///
    /// This method returns `None` if the required statistical metadata is incomplete, or if the
    /// derived statistics are not usable.
    #[expect(clippy::cast_precision_loss)]
    #[expect(clippy::similar_names)]
    pub fn from_metrics(
        new: &AnnotatedMetric<PerfQualities>,
        old: &AnnotatedMetric<PerfQualities>,
        alpha: f64,
    ) -> Option<Self> {
        assert!(
            alpha > 0.0 && alpha < 1.0,
            "alpha should be in the range 0.0 < alpha < 1.0"
        );

        let (
            Some(new_n),
            Some(new_mean),
            Some(new_rse),
            Some(old_n),
            Some(old_mean),
            Some(old_rse),
        ) = (
            new.qualities.n,
            new.qualities.mean,
            new.qualities.rse,
            old.qualities.n,
            old.qualities.mean,
            old.qualities.rse,
        )
        else {
            return None;
        };

        let relative_change = if relative_eq!(new_mean, old_mean) {
            0.0
        } else {
            (new_mean - old_mean) / old_mean
        };

        // Let natural arithmetic produce inf/NaN if it happens
        let se_relative_change = (old_rse.powi(2) + (new_rse * new_mean / old_mean).powi(2)).sqrt();

        let t_statistic = relative_change.abs() / se_relative_change;

        // Calculate degrees of freedom (Welch-Satterthwaite)
        let old_se = old_rse * old_mean;
        let new_se = new_rse * new_mean;

        let df = if old_n > 1 && new_n > 1 {
            let numerator = old_se.mul_add(old_se, new_se.powi(2)).powi(2);

            let denom1 = old_se.powi(4) / (old_n - 1) as f64;
            let denom2 = new_se.powi(4) / (new_n - 1) as f64;

            numerator / (denom1 + denom2)
        } else {
            // Fallback to simple df if insufficient samples
            old_n.max(new_n).saturating_sub(1) as f64
        };

        (df > 0.0 && df.is_finite() && t_statistic.is_finite()).then(|| {
            let t_dist =
                StudentsT::new(0.0, 1.0, df).expect("Scale and degree of freedom should be > 0");

            let cdf = t_dist.cdf(t_statistic);
            let p_value = 2.0 * (1.0 - cdf);

            let t_critical = t_dist.inverse_cdf(1.0 - alpha / 2.0);

            let significance_threshold = t_critical * se_relative_change;
            let significance_factor = relative_change.abs() / significance_threshold;

            Self {
                p_value,
                significance_factor,
                significance_threshold,
            }
        })
    }
}

impl OnlineStats {
    /// Creates running statistics from the first observed sample
    ///
    /// The initial state is:
    /// - `n = 1`
    /// - `mean = mean`
    /// - `m2 = 0.0`
    ///
    /// `m2` starts at zero because a single sample has zero deviation from its own mean.
    ///
    /// # Panics
    ///
    /// Panics if `mean` is not finite.
    #[must_use]
    pub fn new(mean: f64) -> Self {
        assert!(mean.is_finite());

        Self {
            n: 1,
            mean,
            m2: 0.0,
        }
    }

    /// Adds one observed sample to the running statistics.
    ///
    /// This updates the sample count, the running mean, and `m2` (the sum of squared deviations)
    /// using [Welford's online algorithm][welford-algorithm]. The update is done incrementally, so
    /// the statistics can be maintained without storing all previously seen samples.
    ///
    /// After this call:
    /// - `n` is incremented by one
    /// - `mean` becomes the mean of all samples seen so far
    /// - `m2` accumulates the squared deviations needed to compute variance
    ///
    /// # Panics
    ///
    /// Panics if `value` is not finite.
    ///
    /// [welford-algorithm]:
    ///     https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance#Welford.27s_online_algorithm
    #[expect(clippy::cast_precision_loss)]
    pub fn add(&mut self, value: f64) {
        assert!(value.is_finite());

        self.n += 1;
        let old_mean = self.mean;
        self.mean += (value - self.mean) / self.n as f64;
        self.m2 = (value - old_mean).mul_add(value - self.mean, self.m2);
    }

    /// Returns the sample variance (`m2 / (n - 1)`) if `n >= 2` otherwise `None`
    #[expect(clippy::cast_precision_loss)]
    pub fn sample_variance(&self) -> Option<f64> {
        (self.n >= 2).then(|| self.m2 / (self.n - 1) as f64)
    }
}

impl Stats {
    /// Creates a new `Stats` storing the given [`OnlineStats`] and computing the RSE from it
    pub fn new(online_stats: OnlineStats) -> Self {
        let rse = if online_stats.mean == 0.0 || online_stats.n < 2 {
            0.0
        } else {
            #[expect(clippy::cast_precision_loss)]
            let n = online_stats.n as f64;
            let var = online_stats
                .sample_variance()
                .expect("the variance should be valid since n>=2");
            let stddev = var.sqrt();
            let stderr = stddev / n.sqrt();
            stderr / online_stats.mean.abs()
        };

        Self { online_stats, rse }
    }
}

impl<T> OnlineStatsMap<T>
where
    T: Eq + Hash + Clone,
{
    /// Clears the map, removing all data
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Inserts a first sample for `metric` or adds `value` to the existing [`OnlineStats`]
    pub fn insert_or_add(&mut self, metric: &T, value: f64) {
        if let Some(stats_entry) = self.0.get_mut(metric) {
            stats_entry.add(value);
        } else {
            self.0.insert(metric.clone(), OnlineStats::new(value));
        }
    }

    /// Returns the [`OnlineStats`] for `key`, if present.
    pub fn get(&self, key: &T) -> Option<&OnlineStats> {
        self.0.get(key)
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::annotated_metric_perf_f;
    use crate::runner::tool::config::DEFAULT_PERF_ALPHA;

    #[rstest]
    #[case::all_zeroes(
        OnlineStats::new(0.0), 0.0, OnlineStats { n: 2, mean: 0.0, m2: 0.0 }
    )]
    #[case::add_zero(
        OnlineStats::new(1.0), 0.0, OnlineStats { n: 2, mean: 0.5, m2: 0.5 }
    )]
    #[case::init_zero(
        OnlineStats::new(0.0), 1.0, OnlineStats { n: 2, mean: 0.5, m2: 0.5 }
    )]
    #[case::same_value_then_m2_is_zero(
        OnlineStats::new(4.0), 4.0, OnlineStats { n: 2, mean: 4.0, m2: 0.0 }
    )]
    #[case::different_values(
        OnlineStats::new(1.0), 2.0, OnlineStats { n: 2, mean: 1.5, m2: 0.5 }
    )]
    #[case::negative_values(
        OnlineStats::new(-2.0), -4.0, OnlineStats { n: 2, mean: -3.0, m2: 2.0 }
    )]
    fn test_online_stats_add(
        #[case] mut stats: OnlineStats,
        #[case] value: f64,
        #[case] expected: OnlineStats,
    ) {
        stats.add(value);

        assert_eq!(stats.n, expected.n);
        assert_relative_eq!(stats.mean, expected.mean);
        assert_relative_eq!(stats.m2, expected.m2);
    }

    #[test]
    fn online_stats_add_when_multiple_samples() {
        let mut stats = OnlineStats::new(1.0);

        stats.add(3.0);
        stats.add(5.0);

        assert_eq!(stats.n, 3);
        assert_relative_eq!(stats.mean, 3.0);
        assert_relative_eq!(stats.m2, 8.0);
        assert_relative_eq!(stats.sample_variance().unwrap(), 4.0);
    }

    #[rstest]
    #[case::nan(OnlineStats::new(1.0), f64::NAN)]
    #[case::pos_infinity(OnlineStats::new(1.0), f64::INFINITY)]
    #[case::neg_infinity(OnlineStats::new(1.0), f64::NEG_INFINITY)]
    #[should_panic = "assertion failed"]
    fn test_online_stats_add_when_non_finite_values_then_panics(
        #[case] mut stats: OnlineStats,
        #[case] value: f64,
    ) {
        stats.add(value);
    }

    #[rstest]
    #[case::nan(f64::NAN)]
    #[case::pos_infinity(f64::INFINITY)]
    #[case::neg_infinity(f64::NEG_INFINITY)]
    #[should_panic = "assertion failed"]
    fn test_online_stats_new_when_non_finite_values_then_panics(#[case] value: f64) {
        let _ = OnlineStats::new(value);
    }

    #[rstest]
    #[case::insufficient_samples(
        OnlineStats { n: 1, mean: 5.0, m2: 0.0, }, 0.0
    )]
    #[case::zero_mean(
        OnlineStats { n: 2, mean: 0.0, m2: 2.0, }, 0.0
    )]
    #[case::zero_variance(
        OnlineStats { n: 2, mean: 4.0, m2: 0.0, }, 0.0
    )]
    #[case::valid_data(
        OnlineStats { n: 2, mean: 2.0, m2: 2.0, }, 0.5
    )]
    #[case::when_negative_mean_then_absolute_value(
        OnlineStats { n: 2, mean: -2.0, m2: 2.0, }, 0.5
    )]
    fn test_stats_new(#[case] online_stats: OnlineStats, #[case] expected_rse: f64) {
        let stats = Stats::new(online_stats);

        assert_eq!(stats.online_stats, online_stats);
        assert_relative_eq!(stats.rse, expected_rse);
    }

    #[rstest]
    #[case::factor_0(100.0, 2, 2, 1.0, 0.608_486_984_459_331_7, 0.0)]
    #[case::factor_0_5(
        117.908_971_961_638,
        5,
        5,
        0.280_978_325_663_371,
        0.358_179_439_232_759,
        0.5
    )]
    #[case::factor_1(
        140.529_209_207_751,
        5,
        5,
        0.049_999_999_999_998,
        0.405_292_092_077_506,
        1.0
    )]
    #[case::factor_2(
        220.905_819_739_997,
        5,
        5,
        0.003_067_313_737_731,
        0.604_529_098_699_988,
        2.0
    )]
    #[case::low_sample_count(
        200.0,
        1,
        5,
        0.011_056_493_393_450_051,
        0.620_831_999_101_882_6,
        1.610_741_716_674_777
    )]
    fn test_diff_stats_from_metrics(
        #[case] new_mean: f64,
        #[case] new_n: u64,
        #[case] old_n: u64,
        #[case] expected_p_value: f64,
        #[case] expected_significance_threshold: f64,
        #[case] expected_significance_factor: f64,
    ) {
        let metric = 0.0;
        let rse = 0.1;

        let new = annotated_metric_perf_f()
            .metric(metric)
            .mean(new_mean)
            .n(new_n)
            .rse(rse)
            .fx();
        let old = annotated_metric_perf_f()
            .metric(metric)
            .mean(100.0)
            .n(old_n)
            .rse(rse)
            .fx();

        let diff_stats = DiffStats::from_metrics(&new, &old, DEFAULT_PERF_ALPHA).unwrap();

        assert_relative_eq!(diff_stats.p_value, expected_p_value, epsilon = 1e-12);
        assert_relative_eq!(
            diff_stats.significance_threshold,
            expected_significance_threshold,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            diff_stats.significance_factor,
            expected_significance_factor,
            epsilon = 1e-12
        );
    }

    #[rstest]
    #[case::missing_new_n(
        annotated_metric_perf_f().metric(1.0).mean(1.0).rse(0.1).fx(),
        annotated_metric_perf_f().metric(2.0).mean(2.0).n(2).rse(0.1).fx(),
    )]
    #[case::all_missing(
        annotated_metric_perf_f().metric(0.0).fx(),
        annotated_metric_perf_f().metric(100.0).fx()
    )]
    #[case::old_mean_zero(
        annotated_metric_perf_f().metric(1.0).mean(1.0).n(2).rse(0.1).fx(),
        annotated_metric_perf_f().metric(2.0).mean(0.0).n(2).rse(0.1).fx(),
    )]
    fn test_diff_stats_from_metrics_when_invalid_input_then_none(
        #[case] new: AnnotatedMetric<PerfQualities>,
        #[case] old: AnnotatedMetric<PerfQualities>,
    ) {
        assert_eq!(
            DiffStats::from_metrics(&new, &old, DEFAULT_PERF_ALPHA),
            None
        );
    }

    #[rstest]
    #[case::zero(0.0)]
    #[case::one(1.0)]
    #[case::negative(-0.1)]
    #[case::too_large(1.1)]
    #[case::nan(f64::NAN)]
    #[should_panic = "alpha should be in the range"]
    fn test_diff_stats_from_metrics_when_alpha_invalid_then_panics(#[case] alpha: f64) {
        let metric = annotated_metric_perf_f().metric(0.0).fx();

        let _ = DiffStats::from_metrics(&metric, &metric, alpha);
    }
}
