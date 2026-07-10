//! The module containing the basic elements for regression check configurations
use std::fmt::Display;
use std::hash::Hash;

use either_or_both::EitherOrBoth;

use crate::api;
use crate::metrics::logic::{MetricValue, Summarize};
use crate::metrics::model::{Metric, MetricsSummary};
use crate::runner::cachegrind::regression::CachegrindRegressionConfig;
use crate::runner::callgrind::regression::CallgrindRegressionConfig;
use crate::runner::dhat::regression::DhatRegressionConfig;
use crate::runner::perf::regression::PerfRegressionConfig;
use crate::summary::model::{ToolMetricSummary, ToolRegression};
use crate::units::Unit;

/// A short-lived utility enum used to hold the raw regressions until they can be transformed into a
/// real [`ToolRegression`]
#[derive(Debug)]
pub enum RegressionMetrics<T> {
    /// The result of a checked soft limit
    ///
    /// metric identifier, optional display label, optional unit, new metric value, old metric
    /// value, percentage diff, and configured soft limit.
    Soft(T, Option<String>, Option<Unit>, Metric, Metric, f64, f64),
    /// The result of a checked hard limit
    ///
    /// metric identifier, optional display label, optional unit, new metric value, amount above
    /// the limit, and configured hard limit.
    Hard(T, Option<String>, Option<Unit>, Metric, Metric, Metric),
}

/// The tool specific regression check configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ToolRegressionConfig {
    /// The Callgrind configuration
    Callgrind(CallgrindRegressionConfig),
    /// The Cachegrind configuration
    Cachegrind(CachegrindRegressionConfig),
    /// The DHAT configuration
    Dhat(DhatRegressionConfig),
    /// The DHAT configuration
    Perf(PerfRegressionConfig),
    /// If there is no configuration
    None,
}

/// The trait which needs to be implemented in a tool specific regression check configuration
pub trait RegressionConfig<T, V = Metric>
where
    T: Hash + Eq + Summarize<V> + Display + Clone,
    V: MetricValue + Clone,
{
    /// Check the `MetricsSummary` for regressions.
    ///
    /// The limits for event kinds which are not present in the `MetricsSummary` are ignored.
    fn check(&self, metrics_summary: &MetricsSummary<T, V>) -> Vec<ToolRegression>;

    /// Check for regressions and return the [`RegressionMetrics`]
    fn check_regressions(
        &self,
        metrics_summary: &MetricsSummary<T, V>,
    ) -> Vec<RegressionMetrics<T>> {
        let mut regressions = vec![];
        for (metric, new_cost, old_cost, pct, limit) in
            self.get_soft_limits().iter().filter_map(|(kind, limit)| {
                metrics_summary.diff_by_kind(kind).and_then(|d| {
                    if let EitherOrBoth::Both(new, old) = d.metrics.as_ref() {
                        // This unwrap is safe since the diffs are calculated if both costs are
                        // present
                        Some((kind, new, old, d.diffs.unwrap().diff_pct, limit))
                    } else {
                        None
                    }
                })
            })
        {
            if limit.is_sign_positive() {
                if pct > *limit {
                    regressions.push(RegressionMetrics::Soft(
                        metric.clone(),
                        None,
                        None,
                        new_cost.metric(),
                        old_cost.metric(),
                        pct,
                        *limit,
                    ));
                }
            } else if pct < *limit {
                regressions.push(RegressionMetrics::Soft(
                    metric.clone(),
                    None,
                    None,
                    new_cost.metric(),
                    old_cost.metric(),
                    pct,
                    *limit,
                ));
            } else {
                // no regression
            }
        }

        for (metric, new_cost, limit) in
            self.get_hard_limits().iter().filter_map(|(kind, limit)| {
                metrics_summary.diff_by_kind(kind).and_then(|d| {
                    d.metrics
                        .as_ref()
                        .left()
                        .map(|metric| (kind, metric, limit))
                })
            })
        {
            if new_cost.metric() > *limit {
                regressions.push(RegressionMetrics::Hard(
                    metric.clone(),
                    None,
                    None,
                    new_cost.metric(),
                    new_cost.metric() - *limit,
                    *limit,
                ));
            }
        }
        regressions
    }

    /// Returns the hard limits.
    fn get_hard_limits(&self) -> &[(T, Metric)];

    /// Returns the soft limits.
    fn get_soft_limits(&self) -> &[(T, f64)];
}

impl ToolRegressionConfig {
    /// Returns `true` if the configuration has fail fast set to true.
    pub fn is_fail_fast(&self) -> bool {
        match self {
            Self::Callgrind(regression_config) => regression_config.fail_fast,
            Self::Cachegrind(regression_config) => regression_config.fail_fast,
            Self::Dhat(regression_config) => regression_config.fail_fast,
            Self::Perf(regression_config) => regression_config.fail_fast,
            Self::None => false,
        }
    }

    /// Checks the tool summary against this regression configuration.
    ///
    /// The provided `tool_total` must contain metrics of the same tool family as this config.
    ///
    /// # Panics
    ///
    /// Panics if the metric summary type does not match the active regression configuration.
    pub fn check(&self, metrics_summary: &ToolMetricSummary) -> Vec<ToolRegression> {
        match (&self, &metrics_summary) {
            (
                Self::Callgrind(callgrind_regression_config),
                ToolMetricSummary::Callgrind(metrics_summary),
            ) => callgrind_regression_config.check(metrics_summary),
            (
                Self::Cachegrind(cachegrind_regression_config),
                ToolMetricSummary::Cachegrind(metrics_summary),
            ) => cachegrind_regression_config.check(metrics_summary),
            (Self::Dhat(dhat_regression_config), ToolMetricSummary::Dhat(metrics_summary)) => {
                dhat_regression_config.check(metrics_summary)
            }
            (Self::Perf(perf_regression_config), ToolMetricSummary::Perf(metrics_summary)) => {
                perf_regression_config.check(metrics_summary)
            }
            (Self::None, _) => vec![],
            _ => {
                panic!("The summary type should match the regression config")
            }
        }
    }
}

impl TryFrom<api::ToolRegressionConfig> for ToolRegressionConfig {
    type Error = String;

    fn try_from(value: api::ToolRegressionConfig) -> std::result::Result<Self, Self::Error> {
        match value {
            api::ToolRegressionConfig::Callgrind(regression_config) => {
                regression_config.try_into().map(Self::Callgrind)
            }
            api::ToolRegressionConfig::Cachegrind(regression_config) => {
                regression_config.try_into().map(Self::Cachegrind)
            }
            api::ToolRegressionConfig::Dhat(regression_config) => {
                regression_config.try_into().map(Self::Dhat)
            }
            api::ToolRegressionConfig::Perf(regression_config) => {
                regression_config.try_into().map(Self::Perf)
            }
            api::ToolRegressionConfig::None => Ok(Self::None),
        }
    }
}
