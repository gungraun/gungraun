//! Fixtures for types from the api module

use bon::builder;

use crate::api::{
    CachegrindMetrics, CachegrindRegressionConfig, DhatMetrics, DhatRegressionConfig, DhatSpec,
    Limit, PerfRegressionConfig,
};
use crate::units::Unit;

#[builder(finish_fn = "fx")]
pub fn cachegrind_regression_config_f(
    soft_limits: Option<Vec<(CachegrindMetrics, f64)>>,
    hard_limits: Option<Vec<(CachegrindMetrics, Limit)>>,
    fail_fast: Option<bool>,
) -> CachegrindRegressionConfig {
    CachegrindRegressionConfig {
        soft_limits: soft_limits.unwrap_or_default(),
        hard_limits: hard_limits.unwrap_or_default(),
        fail_fast,
    }
}

#[builder(finish_fn = "fx")]
pub fn dhat_regression_config_f(
    soft_limits: Option<Vec<(DhatMetrics, f64)>>,
    hard_limits: Option<Vec<(DhatMetrics, Limit)>>,
    fail_fast: Option<bool>,
) -> DhatRegressionConfig {
    DhatRegressionConfig {
        soft_limits: soft_limits.unwrap_or_default(),
        hard_limits: hard_limits.unwrap_or_default(),
        fail_fast,
    }
}

#[builder(finish_fn = "fx")]
pub fn dhat_spec_f(
    #[builder(default = vec![], with = FromIterator::from_iter)] frames: Vec<&str>,
) -> DhatSpec {
    DhatSpec {
        frames: (!frames.is_empty()).then(|| frames.into_iter().map(ToOwned::to_owned).collect()),
    }
}

#[builder(finish_fn = "fx")]
pub fn perf_regression_config_f(
    soft_limits: Option<Vec<(String, f64)>>,
    hard_limits: Option<Vec<(String, Option<Unit>, Limit)>>,
    fail_fast: Option<bool>,
    alpha: Option<f64>,
) -> PerfRegressionConfig {
    PerfRegressionConfig {
        alpha,
        soft_limits: soft_limits.unwrap_or_default(),
        hard_limits: hard_limits.unwrap_or_default(),
        fail_fast,
    }
}
