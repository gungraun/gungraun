//! Perf-specific regression checking with statistical significance filtering and unit-aware hard
//! limits.
//!
//! This module implements the [`RegressionConfig`] trait for perf metrics, providing soft and hard
//! limit checks that differ significantly from the default Valgrind-style regression checks in
//! [`crate::runner::tool::regression`].
//!
//! # Differences from Valgrind tool regression checks
//!
//! 1. **Statistical significance filtering (soft limits only)** — Perf records are inherently noisy
//!    due to hardware event sampling. Before reporting a soft-limit regression, the checker
//!    computes [`DiffStats`] between the old and new metric values at the configured `alpha` level.
//!    If the significance factor is at most `1.0`, the diff is considered noise and skipped.
//!    Valgrind tools do not perform this statistical pre-filtering; they report any percentage
//!    change that exceeds the configured limit.
//!
//! 2. **Metric name wildcard patterns** — Perf soft and hard limits use glob-style wildcard
//!    patterns (via `simplematch`) to match against raw perf event names (e.g. `"instructions*"` or
//!    `"cache-*"`). Valgrind tools typically match against fixed enum variants (e.g.
//!    [`CachegrindMetric::Ir`].
//!
//! 3. **Unit-aware hard limits** — When a hard limit specifies a unit, the checker attempts to
//!    normalize the measured metric to that unit before comparison. If the metric's unit is
//!    incompatible with the limit unit, the check is skipped with a warning. Valgrind hard limits
//!    are typically raw counts without unit conversion.
//!
//! 4. **Dedicated `alpha` threshold** — [`PerfRegressionConfig`] carries its own `alpha` field
//!    (which is the same value as [`PerfConfig::alpha`]) for the statistical significance test.
//!    Valgrind regression configs usually do not have an `alpha` because their measurements are
//!    deterministic and do not require noise filtering.
//!
//! [`PerfConfig::alpha`]: crate::runner::tool::config::PerfConfig
//! [`CachegrindMetric::Ir`]: crate::api::CachegrindMetric

use either_or_both::EitherOrBoth;
use indexmap::IndexMap;
use log::{info, warn};
use simplematch::{DoWild, Options};

use crate::api::{self, PerfMetric};
use crate::metrics::model::{AnnotatedMetric, Metric, MetricKind, MetricsSummary, PerfQualities};
use crate::runner::tool::config::resolve_perf_alpha;
use crate::runner::tool::regression::{RegressionConfig, RegressionMetrics};
use crate::stats::runner::DiffStats;
use crate::summary::model::ToolRegression;
use crate::units::Unit;

const DOWILD_OPTIONS: Options<u8> = Options::new()
    .case_insensitive(true)
    .enable_escape(true)
    .enable_classes(true);

/// The perf regression check configuration
#[derive(Debug, Clone, PartialEq)]
pub struct PerfRegressionConfig {
    /// Statistical significance threshold used to ignore noise when evaluating percentage
    /// regressions between old and new perf results.
    ///
    /// Soft-limit checks skip diffs whose significance factor is at most `1.0` at this alpha.
    pub alpha: f64,
    /// True if benchmarks should fail on first encountered failed regression check
    pub fail_fast: bool,
    /// The hard limits
    pub hard_limits: Vec<(PerfMetric, Option<Unit>, Metric)>,
    /// The soft limits
    pub soft_limits: Vec<(PerfMetric, f64)>,
}

impl PerfRegressionConfig {
    fn soft_limit_matches<'a>(
        &self,
        metrics_summary: &'a MetricsSummary<PerfMetric, AnnotatedMetric<PerfQualities>>,
    ) -> impl Iterator<
        Item = (
            &'a PerfMetric,
            String,
            &'a AnnotatedMetric<PerfQualities>,
            &'a AnnotatedMetric<PerfQualities>,
            f64,
            f64,
            Option<&'a Unit>,
        ),
    > {
        let alpha = self.alpha;

        self.soft_limits.iter().flat_map(move |(pattern, limit)| {
            metrics_summary
                .all_diffs()
                .filter_map(move |(metric, metrics_diff)| {
                    if !pattern.name().dowild_with(metric.name(), DOWILD_OPTIONS) {
                        return None;
                    }

                    info!(
                        "perf soft limit pattern '{}' matched '{}'",
                        pattern.name(),
                        metric.name()
                    );

                    let EitherOrBoth::Both(new, old) = metrics_diff.metrics.as_ref() else {
                        return None;
                    };

                    if DiffStats::from_metrics(new, old, alpha)
                        .is_some_and(|d| d.significance_factor <= 1.0)
                    {
                        return None;
                    }

                    // new and old have the same unit, so it doesn't matter which one we pick
                    let result_unit = new.unit.as_ref();
                    let pct = metrics_diff
                        .diffs
                        .expect("diffs should exist when both metrics are present")
                        .diff_pct;

                    Some((
                        metric,
                        format_metric_display(metric, pattern),
                        new,
                        old,
                        pct,
                        *limit,
                        result_unit,
                    ))
                })
        })
    }

    fn hard_limit_matches<'a>(
        &'a self,
        metrics_summary: &'a MetricsSummary<PerfMetric, AnnotatedMetric<PerfQualities>>,
    ) -> impl Iterator<
        Item = (
            &'a PerfMetric,
            String,
            AnnotatedMetric<PerfQualities>,
            &'a Metric,
            Option<Unit>,
        ),
    > {
        self.hard_limits
            .iter()
            .flat_map(move |(pattern, unit, limit)| {
                metrics_summary
                    .all_diffs()
                    .filter_map(move |(metric, metrics_diff)| {
                        if !pattern.name().dowild_with(metric.name(), DOWILD_OPTIONS) {
                            return None;
                        }

                        info!(
                            "perf hard limit pattern '{}' matched '{}'",
                            pattern.name(),
                            metric.name()
                        );

                        metrics_diff.metrics.as_ref().left().and_then(|m| {
                            let (metric_value, result_unit) =
                                if let Some(limit_unit) = unit.as_ref() {
                                    let metric_value = normalize_metric_to_limit(
                                        "hard", pattern, metric, m, limit_unit,
                                    )?;

                                    (metric_value, Some(limit_unit.clone()))
                                } else {
                                    (m.clone(), m.unit.clone())
                                };

                            Some((
                                metric,
                                format_metric_display(metric, pattern),
                                metric_value,
                                limit,
                                result_unit,
                            ))
                        })
                    })
            })
    }
}

impl RegressionConfig<PerfMetric, AnnotatedMetric<PerfQualities>> for PerfRegressionConfig {
    fn check_regressions(
        &self,
        metrics_summary: &MetricsSummary<PerfMetric, AnnotatedMetric<PerfQualities>>,
    ) -> Vec<RegressionMetrics<PerfMetric>> {
        let mut regressions = vec![];

        for (metric, display, new, old, pct, limit, unit) in
            self.soft_limit_matches(metrics_summary)
        {
            if limit.is_sign_positive() {
                if pct > limit {
                    regressions.push(RegressionMetrics::Soft(
                        metric.clone(),
                        Some(display),
                        unit.cloned(),
                        new.metric,
                        old.metric,
                        pct,
                        limit,
                    ));
                }
            } else if pct < limit {
                regressions.push(RegressionMetrics::Soft(
                    metric.clone(),
                    Some(display),
                    unit.cloned(),
                    new.metric,
                    old.metric,
                    pct,
                    limit,
                ));
            } else {
                // no regression
            }
        }

        for (metric, display, new_cost, limit, result_unit) in
            self.hard_limit_matches(metrics_summary)
        {
            if new_cost.metric > *limit {
                regressions.push(RegressionMetrics::Hard(
                    metric.clone(),
                    Some(display),
                    result_unit.clone(),
                    new_cost.metric,
                    new_cost.metric - *limit,
                    *limit,
                ));
            }
        }
        regressions
    }

    fn check(
        &self,
        metrics_summary: &MetricsSummary<PerfMetric, AnnotatedMetric<PerfQualities>>,
    ) -> Vec<ToolRegression> {
        self.check_regressions(metrics_summary)
            .into_iter()
            .map(|regressions| ToolRegression::with(MetricKind::Perf, regressions))
            .collect()
    }

    fn get_hard_limits(&self) -> &[(PerfMetric, Metric)] {
        panic!("Do not use for perf")
    }

    fn get_soft_limits(&self) -> &[(PerfMetric, f64)] {
        panic!("Do not use for perf")
    }
}

impl TryFrom<api::PerfRegressionConfig> for PerfRegressionConfig {
    type Error = String;

    fn try_from(value: api::PerfRegressionConfig) -> Result<Self, Self::Error> {
        let api::PerfRegressionConfig {
            alpha,
            soft_limits,
            hard_limits,
            fail_fast,
        } = value;

        let hard_limits = hard_limits
            .into_iter()
            .map(|(metric, unit, limit)| (PerfMetric(metric), (unit, Metric::from(limit))))
            .collect::<IndexMap<_, _>>();

        let soft_limits = soft_limits
            .into_iter()
            .map(|(metric, limit)| (PerfMetric(metric), limit))
            .collect::<IndexMap<_, _>>();

        Ok(Self {
            alpha: resolve_perf_alpha(alpha)?,
            soft_limits: soft_limits.into_iter().collect(),
            hard_limits: hard_limits
                .into_iter()
                .map(|(k, (u, l))| (k, u, l))
                .collect(),
            fail_fast: fail_fast.unwrap_or(false),
        })
    }
}

fn format_metric_display(metric: &PerfMetric, pattern: &PerfMetric) -> String {
    if metric.name() == pattern.name() {
        metric.name().to_owned()
    } else {
        format!("{} [{}]", metric.name(), pattern.name())
    }
}

fn normalize_metric_to_limit(
    kind: &str,
    pattern: &PerfMetric,
    metric: &PerfMetric,
    value: &AnnotatedMetric<PerfQualities>,
    limit_unit: &Unit,
) -> Option<AnnotatedMetric<PerfQualities>> {
    let Some(metric_unit) = value.unit.as_ref() else {
        warn!(
            "Skipping regression check for perf {kind} limit {}: This metric has no unit while \
             configured limit has '{}'",
            format_metric_display(metric, pattern),
            limit_unit,
        );
        return None;
    };

    if metric_unit == limit_unit {
        return Some(value.clone());
    }

    let Some(factor) = metric_unit.scale_factor_metric(limit_unit) else {
        warn!(
            "Skipping regression check for perf {kind} limit {}: This metric unit '{}' cannot be \
             compared to the limit unit '{}'",
            format_metric_display(metric, pattern),
            metric_unit,
            limit_unit,
        );
        return None;
    };

    Some(AnnotatedMetric::new(
        value.metric * factor,
        value.qualities.scale_by_metric(factor),
        limit_unit.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use either_or_both::EitherOrBoth;
    use indexmap::indexmap;
    use rstest::rstest;

    use super::PerfRegressionConfig;
    use crate::api::{self, Limit, PerfMetric};
    use crate::fixtures::api::perf_regression_config_f as api_perf_regression_config_f;
    use crate::fixtures::perf::perf_regression_config_f;
    use crate::metrics::model::{
        AnnotatedMetric, Metric, MetricKind, Metrics, MetricsSummary, PerfQualities,
    };
    use crate::runner::tool::config::DEFAULT_PERF_ALPHA;
    use crate::runner::tool::regression::{RegressionConfig, RegressionMetrics};
    use crate::summary::model::ToolRegression;
    use crate::units::Unit;

    fn perf_summary(
        name: &str,
        new: AnnotatedMetric<PerfQualities>,
        old: AnnotatedMetric<PerfQualities>,
    ) -> MetricsSummary<PerfMetric, AnnotatedMetric<PerfQualities>> {
        MetricsSummary::new(EitherOrBoth::Both(
            Metrics(indexmap! { PerfMetric(name.to_owned()) => new }),
            Metrics(indexmap! { PerfMetric(name.to_owned()) => old }),
        ))
    }

    fn perf_summary_new_only(
        name: &str,
        new: AnnotatedMetric<PerfQualities>,
    ) -> MetricsSummary<PerfMetric, AnnotatedMetric<PerfQualities>> {
        MetricsSummary::new(EitherOrBoth::Left(Metrics(indexmap! {
            PerfMetric(name.to_owned()) => new
        })))
    }

    #[rstest]
    #[case::fail_fast(
        api_perf_regression_config_f().fail_fast(true).fixture(),
        perf_regression_config_f().fail_fast(true).fixture(),
    )]
    #[case::alpha(
        api_perf_regression_config_f().alpha(0.10).fixture(),
        perf_regression_config_f().alpha(0.10).fixture(),
    )]
    #[case::soft_limit(
        api_perf_regression_config_f()
            .soft_limits(vec![("instructions".to_owned(), 5f64)])
            .fixture(),
        perf_regression_config_f().soft_limits(vec![(PerfMetric("instructions".to_owned()), 5f64)]).fixture(),
    )]
    #[case::hard_limit(
        api_perf_regression_config_f()
            .hard_limits(vec![("instructions".to_owned(), Some(Unit::Seconds), Limit::Int(10))])
            .fixture(),
        perf_regression_config_f()
            .hard_limits(vec![(
                PerfMetric("instructions".to_owned()),
                Some(Unit::Seconds),
                Metric::Int(10),
            )])
            .fixture(),
    )]
    fn test_try_from_regression_config(
        #[case] input: api::PerfRegressionConfig,
        #[case] expected: PerfRegressionConfig,
    ) {
        let config = PerfRegressionConfig::try_from(input).unwrap();

        assert_eq!(config, expected);
    }

    #[test]
    fn test_soft_limit_preserves_metric_unit() {
        let config = PerfRegressionConfig {
            alpha: DEFAULT_PERF_ALPHA,
            fail_fast: false,
            soft_limits: vec![(PerfMetric("duration".to_owned()), 50.0)],
            hard_limits: vec![],
        };

        let summary = perf_summary(
            "duration",
            AnnotatedMetric::with_default_qualities(2000, Unit::Milliseconds),
            AnnotatedMetric::with_default_qualities(1000, Unit::Milliseconds),
        );

        let regressions = config.check(&summary);

        assert_eq!(
            regressions,
            vec![ToolRegression::Soft {
                metric: MetricKind::Perf(PerfMetric("duration".to_owned())),
                display: Some("duration".to_owned()),
                unit: Some(Unit::Milliseconds),
                new: Metric::Int(2000),
                old: Metric::Int(1000),
                diff_pct: 100.0,
                limit: 50.0,
            }]
        );
    }

    #[test]
    fn test_hard_limit_normalizes_to_limit_unit() {
        let config = PerfRegressionConfig {
            alpha: DEFAULT_PERF_ALPHA,
            fail_fast: false,
            soft_limits: vec![],
            hard_limits: vec![(
                PerfMetric("memory".to_owned()),
                Some(Unit::Kilobytes),
                1.5.into(),
            )],
        };

        let summary = perf_summary_new_only(
            "memory",
            AnnotatedMetric::with_default_qualities(2000, Unit::Bytes),
        );

        let regressions = config.check(&summary);

        assert_eq!(
            regressions,
            vec![ToolRegression::Hard {
                metric: MetricKind::Perf(PerfMetric("memory".to_owned())),
                display: Some("memory".to_owned()),
                unit: Some(Unit::Kilobytes),
                new: Metric::Float(2.0),
                diff: Metric::Float(0.5),
                limit: Metric::Float(1.5),
            }]
        );
    }

    #[test]
    fn test_hard_limit_skips_when_units_are_incompatible() {
        let config = PerfRegressionConfig {
            alpha: DEFAULT_PERF_ALPHA,
            fail_fast: false,
            soft_limits: vec![],
            hard_limits: vec![(
                PerfMetric("memory".to_owned()),
                Some(Unit::Seconds),
                1.into(),
            )],
        };

        let summary = perf_summary_new_only(
            "memory",
            AnnotatedMetric::with_default_qualities(2000, Unit::Bytes),
        );

        assert!(config.check(&summary).is_empty());
    }

    #[test]
    fn test_soft_limit_without_limit_unit_preserves_metric_unit() {
        let config = PerfRegressionConfig {
            alpha: DEFAULT_PERF_ALPHA,
            fail_fast: false,
            soft_limits: vec![(PerfMetric("duration".to_owned()), 50.0)],
            hard_limits: vec![],
        };

        let summary = perf_summary(
            "duration",
            AnnotatedMetric::with_default_qualities(2000, Unit::Milliseconds),
            AnnotatedMetric::with_default_qualities(1000, Unit::Milliseconds),
        );

        let regressions = config.check_regressions(&summary);
        let unit = match &regressions[0] {
            RegressionMetrics::Soft(_, _, unit, ..) => unit.as_ref(),
            RegressionMetrics::Hard(..) => panic!("expected soft regression"),
        };

        assert_eq!(unit, Some(&Unit::Milliseconds));
    }
}
