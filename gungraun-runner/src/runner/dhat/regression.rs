//! Module containing the dhat specific regression check configuration
use indexmap::{IndexMap, IndexSet};

use crate::api::{self, DhatMetric};
use crate::metrics::model::{Metric, MetricKind, MetricsSummary};
use crate::runner::tool::regression::RegressionConfig;
use crate::summary::model::ToolRegression;

/// The dhat regression check configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DhatRegressionConfig {
    /// True if benchmarks should fail on first encountered failed regression check
    pub fail_fast: bool,
    /// The hard limits
    pub hard_limits: Vec<(DhatMetric, Metric)>,
    /// The soft limits
    pub soft_limits: Vec<(DhatMetric, f64)>,
}

impl Default for DhatRegressionConfig {
    fn default() -> Self {
        Self {
            soft_limits: vec![(DhatMetric::TotalBytes, 10f64)],
            hard_limits: Vec::default(),
            fail_fast: Default::default(),
        }
    }
}

impl RegressionConfig<DhatMetric> for DhatRegressionConfig {
    fn check(&self, metrics_summary: &MetricsSummary<DhatMetric>) -> Vec<ToolRegression> {
        self.check_regressions(metrics_summary)
            .into_iter()
            .map(|regressions| ToolRegression::with(MetricKind::Dhat, regressions))
            .collect()
    }

    fn get_soft_limits(&self) -> &[(DhatMetric, f64)] {
        &self.soft_limits
    }

    fn get_hard_limits(&self) -> &[(DhatMetric, Metric)] {
        &self.hard_limits
    }
}

impl TryFrom<api::DhatRegressionConfig> for DhatRegressionConfig {
    type Error = String;

    fn try_from(value: api::DhatRegressionConfig) -> std::result::Result<Self, Self::Error> {
        let api::DhatRegressionConfig {
            soft_limits,
            hard_limits,
            fail_fast,
        } = value;

        let hard_limits = hard_limits
            .into_iter()
            .flat_map(|(dhat_metrics, metric)| {
                IndexSet::from(dhat_metrics)
                    .into_iter()
                    .map(move |metric_kind| {
                        Metric::from(metric)
                            .try_convert(metric_kind)
                            .ok_or_else(|| {
                                format!(
                                    "Invalid hard limit for '{metric_kind:?}/{dhat_metrics:?}': \
                                     Expected a 'Int' but found '{metric:?}'"
                                )
                            })
                    })
            })
            .collect::<Result<IndexMap<DhatMetric, Metric>, String>>()?;

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
    use DhatMetric::*;
    use either_or_both::EitherOrBoth;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::api::{DhatMetrics, Limit};
    use crate::fixtures::api::dhat_regression_config_f as api_dhat_regression_config_f;
    use crate::fixtures::dhat_regression_config_f;
    use crate::metrics::model::Metrics;
    use crate::runner::tool::regression::RegressionMetrics;

    #[rstest]
    #[case::fail_fast(
        api_dhat_regression_config_f().fail_fast(true).fixture(),
        dhat_regression_config_f().fail_fast(true).fixture(),
    )]
    #[case::soft_limit(
        api_dhat_regression_config_f()
            .soft_limits(vec![(DhatMetrics::from(TotalBytes), 5f64)])
            .fixture(),
        dhat_regression_config_f().soft_limits(vec![(TotalBytes, 5f64)]).fixture(),
    )]
    #[case::hard_limit(
        api_dhat_regression_config_f()
            .hard_limits(vec![(DhatMetrics::from(TotalBytes), Limit::Int(10))])
            .fixture(),
        dhat_regression_config_f().hard_limits(vec![(TotalBytes, Metric::Int(10))]).fixture(),
    )]
    fn test_try_from_regression_config(
        #[case] input: api::DhatRegressionConfig,
        #[case] expected: DhatRegressionConfig,
    ) {
        let config = DhatRegressionConfig::try_from(input).unwrap();

        assert_eq!(config, expected);
    }

    fn costs_fixture(costs: [u64; 2]) -> Metrics<DhatMetric> {
        Metrics::with_metric_kinds([(TotalBytes, costs[0]), (TotalBlocks, costs[1])])
    }

    #[rstest]
    #[case::all_zero_no_regression(
        vec![(TotalBytes, 0)],
        [0, 0],
        vec![]
    )]
    #[case::total_bytes_regression_by_one(
        vec![(TotalBytes, 0)],
        [1, 0],
        vec![(TotalBytes, 1, 1, 0)]
    )]
    #[case::total_bytes_regression_by_two(
        vec![(TotalBytes, 0)],
        [2, 0],
        vec![(TotalBytes, 2, 2, 0)]
    )]
    #[case::total_bytes_regression_some_value(
        vec![(TotalBytes, 10)],
        [11, 0],
        vec![(TotalBytes, 11, 1, 10)]
    )]
    #[case::total_bytes_and_block_regression(
        vec![(TotalBytes, 9), (TotalBlocks, 1)],
        [10, 4],
        vec![(TotalBytes, 10, 1, 9), (TotalBlocks, 4, 3, 1)]
    )]
    fn test_regression_check_when_hard<U>(
        #[case] limits: Vec<(DhatMetric, U)>,
        #[case] new: [u64; 2],
        #[case] expected: Vec<(DhatMetric, u64, u64, u64)>,
    ) where
        U: Into<Metric>,
    {
        let regression = DhatRegressionConfig {
            hard_limits: limits.into_iter().map(|(x, y)| (x, y.into())).collect(),
            soft_limits: vec![],
            ..Default::default()
        };

        let new_costs = costs_fixture(new);

        let summary = MetricsSummary::new(EitherOrBoth::Left(new_costs));
        let expected = expected
            .iter()
            .map(|(e, n, d, l)| ToolRegression::Hard {
                metric: MetricKind::Dhat(*e),
                display: None,
                unit: None,
                new: (*n).into(),
                diff: (*d).into(),
                limit: (*l).into(),
            })
            .collect::<Vec<ToolRegression>>();

        assert_eq!(regression.check(&summary), expected);
    }

    #[test]
    fn test_regression_check_when_hard_and_soft() {
        let config = DhatRegressionConfig {
            hard_limits: vec![(DhatMetric::TotalBytes, 2.into())],
            soft_limits: vec![(DhatMetric::TotalBlocks, 20f64)],
            ..Default::default()
        };

        let new_costs = costs_fixture([3, 4]);
        let old_costs = costs_fixture([1, 2]);

        let summary = MetricsSummary::new(EitherOrBoth::Both(new_costs, old_costs));
        let expected = vec![
            ToolRegression::with(
                MetricKind::Dhat,
                RegressionMetrics::Soft(
                    DhatMetric::TotalBlocks,
                    None,
                    None,
                    4.into(),
                    2.into(),
                    100f64,
                    20f64,
                ),
            ),
            ToolRegression::with(
                MetricKind::Dhat,
                RegressionMetrics::Hard(
                    DhatMetric::TotalBytes,
                    None,
                    None,
                    3.into(),
                    1.into(),
                    2.into(),
                ),
            ),
        ];

        assert_eq!(config.check(&summary), expected);
    }
}
