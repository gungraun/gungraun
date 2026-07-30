//! Module containing the Callgrind-specific regression check configuration
use indexmap::{IndexMap, IndexSet};

use crate::api::{self, EventKind};
use crate::metrics::model::{Metric, MetricKind, MetricsSummary};
use crate::runner::tool::regression::RegressionConfig;
use crate::summary::model::ToolRegression;

/// The callgrind regression check configuration
#[derive(Debug, Clone, PartialEq)]
pub struct CallgrindRegressionConfig {
    /// True if benchmarks should fail on first encountered failed regression check
    pub fail_fast: bool,
    /// The hard limits
    pub hard_limits: Vec<(EventKind, Metric)>,
    /// The soft limits
    pub soft_limits: Vec<(EventKind, f64)>,
}

impl Default for CallgrindRegressionConfig {
    fn default() -> Self {
        Self {
            soft_limits: vec![(EventKind::Ir, 10f64)],
            hard_limits: Vec::default(),
            fail_fast: Default::default(),
        }
    }
}

impl RegressionConfig<EventKind> for CallgrindRegressionConfig {
    /// Check the `MetricsSummary` for regressions.
    ///
    /// The limits for event kinds which are not present in the `MetricsSummary` are ignored.
    fn check(&self, metrics_summary: &MetricsSummary) -> Vec<ToolRegression> {
        self.check_regressions(metrics_summary)
            .into_iter()
            .map(|regressions| ToolRegression::with(MetricKind::Callgrind, regressions))
            .collect()
    }

    fn get_soft_limits(&self) -> &[(EventKind, f64)] {
        &self.soft_limits
    }

    fn get_hard_limits(&self) -> &[(EventKind, Metric)] {
        &self.hard_limits
    }
}

impl TryFrom<api::CallgrindRegressionConfig> for CallgrindRegressionConfig {
    type Error = String;

    fn try_from(value: api::CallgrindRegressionConfig) -> Result<Self, Self::Error> {
        let api::CallgrindRegressionConfig {
            soft_limits,
            hard_limits,
            fail_fast,
        } = value;

        let hard_limits = hard_limits
            .into_iter()
            .flat_map(|(callgrind_metrics, metric)| {
                IndexSet::from(callgrind_metrics)
                    .into_iter()
                    .map(move |metric_kind| {
                        Metric::from(metric)
                            .try_convert(metric_kind)
                            .ok_or_else(|| {
                                format!(
                                    "Invalid hard limit for \
                                     '{metric_kind:?}/{callgrind_metrics:?}': Expected a 'Int' \
                                     but found '{metric:?}'"
                                )
                            })
                    })
            })
            .collect::<Result<IndexMap<EventKind, Metric>, String>>()?;

        let soft_limits = soft_limits
            .into_iter()
            .flat_map(|(m, l)| IndexSet::from(m).into_iter().map(move |e| (e, l)))
            .collect::<IndexMap<_, _>>();

        Ok(Self {
            soft_limits: soft_limits.into_iter().collect(),
            hard_limits: hard_limits.into_iter().collect(),
            fail_fast: fail_fast.unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use EventKind::*;
    use either_or_both::EitherOrBoth;
    use rstest::rstest;

    use super::*;
    use crate::api::{CallgrindMetrics, Limit};
    use crate::metrics::logic::TypeChecker;
    use crate::metrics::model::Metric;
    use crate::runner::callgrind::model::Metrics;

    fn cachesim_costs(costs: [u64; 9]) -> Metrics {
        Metrics::with_metric_kinds([
            (Ir, Metric::Int(costs[0])),
            (Dr, Metric::Int(costs[1])),
            (Dw, Metric::Int(costs[2])),
            (I1mr, Metric::Int(costs[3])),
            (D1mr, Metric::Int(costs[4])),
            (D1mw, Metric::Int(costs[5])),
            (ILmr, Metric::Int(costs[6])),
            (DLmr, Metric::Int(costs[7])),
            (DLmw, Metric::Int(costs[8])),
        ])
    }

    #[rstest]
    fn test_regression_check_when_old_is_none() {
        let regression = CallgrindRegressionConfig::default();
        let new = cachesim_costs([0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let summary = MetricsSummary::new(EitherOrBoth::Left(new));

        assert!(regression.check(&summary).is_empty());
    }

    #[rstest]
    #[case::ir_all_zero(
        vec![(Ir, 0f64)],
        [0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![]
    )]
    #[case::ir_when_regression(
        vec![(Ir, 0f64)],
        [2, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![(Ir, 2, 1, 100f64, 0f64)]
    )]
    #[case::ir_when_improved(
        vec![(Ir, 0f64)],
        [1, 0, 0, 0, 0, 0, 0, 0, 0],
        [2, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![]
    )]
    #[case::ir_when_negative_limit(
        vec![(Ir, -49f64)],
        [1, 0, 0, 0, 0, 0, 0, 0, 0],
        [2, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![(Ir, 1, 2, -50f64, -49f64)]
    )]
    #[case::derived_all_zero(
        vec![(EstimatedCycles, 0f64)],
        [0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![]
    )]
    #[case::derived_when_regression(
        vec![(EstimatedCycles, 0f64)],
        [2, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![(EstimatedCycles, 2, 1, 100f64, 0f64)]
    )]
    #[case::derived_when_regression_multiple(
        vec![(EstimatedCycles, 5f64), (Ir, 10f64)],
        [2, 0, 0, 0, 0, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![(EstimatedCycles, 2, 1, 100f64, 5f64), (Ir, 2, 1, 100f64, 10f64)]
    )]
    #[case::derived_when_improved(
        vec![(EstimatedCycles, 0f64)],
        [1, 0, 0, 0, 0, 0, 0, 0, 0],
        [2, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![]
    )]
    #[case::derived_when_regression_mixed(
        vec![(EstimatedCycles, 0f64)],
        [96, 24, 18, 6, 0, 2, 6, 0, 2],
        [48, 12, 9, 3, 0, 1, 3, 0, 1],
        vec![(EstimatedCycles, 410, 205, 100f64, 0f64)]
    )]
    fn test_regression_check_when_soft_and_old_is_some(
        #[case] soft_limits: Vec<(EventKind, f64)>,
        #[case] new: [u64; 9],
        #[case] old: [u64; 9],
        #[case] expected: Vec<(EventKind, u64, u64, f64, f64)>,
    ) {
        let regression = CallgrindRegressionConfig {
            soft_limits,
            ..Default::default()
        };

        let new = cachesim_costs(new);
        let old = cachesim_costs(old);
        let summary = MetricsSummary::new(EitherOrBoth::Both(new, old));
        let expected = expected
            .iter()
            .map(|(e, n, o, d, l)| ToolRegression::Soft {
                metric: MetricKind::Callgrind(*e),
                display: None,
                unit: None,
                new: (*n).into(),
                old: (*o).into(),
                diff_pct: *d,
                limit: *l,
            })
            .collect::<Vec<ToolRegression>>();

        assert_eq!(regression.check(&summary), expected);
    }

    #[rstest]
    #[case::empty(Vec::<(EventKind, f64)>::new(), Vec::<(EventKind, f64)>::new())]
    #[case::single(vec![(Ir, 0f64)], vec![(Ir, 0f64)])]
    #[case::two(vec![(Ir, 0f64), (Dr, 10f64)], vec![(Ir, 0f64), (Dr, 10f64)])]
    #[case::duplicate(vec![(Ir, 0f64), (Ir, 10f64)], vec![(Ir, 10f64)])]
    #[case::group(
        vec![(CallgrindMetrics::WriteBackBehaviour, 10f64)],
        vec![(ILdmr, 10f64), (DLdmr, 10f64), (DLdmw, 10f64)],
    )]
    #[case::group_overwrite_keeps_order(
        vec![(CallgrindMetrics::WriteBackBehaviour, 10f64), (ILdmr.into(), 20f64)],
        vec![(ILdmr, 20f64), (DLdmr, 10f64), (DLdmw, 10f64)],
    )]
    fn test_try_from_regression_config_for_soft_limits<T>(
        #[case] soft_limits: Vec<(T, f64)>,
        #[case] expected_soft_limits: Vec<(EventKind, f64)>,
    ) where
        T: Into<CallgrindMetrics>,
    {
        let expected = CallgrindRegressionConfig {
            soft_limits: expected_soft_limits,
            hard_limits: Vec::default(),
            fail_fast: false,
        };
        let api_regression_config = api::CallgrindRegressionConfig {
            soft_limits: soft_limits
                .into_iter()
                .map(|(m, l)| (m.into(), l))
                .collect(),
            hard_limits: Vec::default(),
            fail_fast: Option::default(),
        };

        assert_eq!(
            CallgrindRegressionConfig::try_from(api_regression_config).unwrap(),
            expected
        );
    }

    #[rstest]
    #[case::empty(Vec::<(EventKind, f64)>::new(), Vec::<(EventKind, f64)>::new())]
    #[case::single(vec![(Ir, 0)], vec![(Ir, 0)])]
    #[case::single_convert(vec![(L1HitRate, 1)], vec![(L1HitRate, 1f64)])]
    #[case::two(vec![(Ir, 0), (Dr, 2)], vec![(Ir, 0), (Dr, 2)])]
    #[case::duplicate_overwrite( vec![(Ir, 0), (Ir, 20)], vec![(Ir, 20)])]
    #[case::integer_group(
        vec![(CallgrindMetrics::WriteBackBehaviour, 10)],
        vec![(ILdmr, 10), (DLdmr, 10), (DLdmw, 10)],
    )]
    #[case::float_group(
        vec![(CallgrindMetrics::CacheHitRates, 10f64)],
        vec![(L1HitRate, 10f64), (LLHitRate, 10f64), (RamHitRate, 10f64)],
    )]
    #[case::float_group_convert(
        vec![(CallgrindMetrics::CacheHitRates, 10)],
        vec![(L1HitRate, 10f64), (LLHitRate, 10f64), (RamHitRate, 10f64)],
    )]
    #[case::mixed_group(
        vec![(CallgrindMetrics::CacheSim, 10)],
        IndexSet::from(CallgrindMetrics::CacheSim)
            .into_iter()
            .map(|m| {
                if m.is_int() {
                    (m, Metric::Int(10))
                } else {
                    (m, Metric::Float(10.0))
                }
           }).collect()
    )]
    #[case::group_overwrite_keeps_order(
        vec![(CallgrindMetrics::WriteBackBehaviour, 10), (ILdmr.into(), 20)],
        vec![(ILdmr, 20), (DLdmr, 10), (DLdmw, 10)],
    )]
    fn test_try_from_regression_config_for_hard_limits<T, U, V>(
        #[case] hard_limits: Vec<(T, U)>,
        #[case] expected_hard_limits: Vec<(EventKind, V)>,
    ) where
        T: Into<CallgrindMetrics>,
        U: Into<Limit>,
        V: Into<Metric>,
    {
        let expected = CallgrindRegressionConfig {
            soft_limits: Vec::default(),
            hard_limits: expected_hard_limits
                .into_iter()
                .map(|(m, l)| (m, l.into()))
                .collect::<Vec<(EventKind, Metric)>>(),
            fail_fast: false,
        };
        let api_regression_config = api::CallgrindRegressionConfig {
            soft_limits: Vec::default(),
            hard_limits: hard_limits
                .into_iter()
                .map(|(m, l)| (m.into(), l.into()))
                .collect(),
            fail_fast: Option::default(),
        };

        assert_eq!(
            CallgrindRegressionConfig::try_from(api_regression_config).unwrap(),
            expected
        );
    }

    #[test]
    fn test_try_from_regression_config_for_hard_limits_then_error() {
        let api_regression_config = api::CallgrindRegressionConfig {
            soft_limits: Vec::default(),
            hard_limits: vec![(EventKind::Ir.into(), Limit::Float(10f64))],
            fail_fast: Option::default(),
        };

        CallgrindRegressionConfig::try_from(api_regression_config).unwrap_err();

        let api_regression_config = api::CallgrindRegressionConfig {
            soft_limits: Vec::default(),
            hard_limits: vec![(CallgrindMetrics::All, Limit::Float(10f64))],
            fail_fast: Option::default(),
        };

        CallgrindRegressionConfig::try_from(api_regression_config).unwrap_err();
    }
}
