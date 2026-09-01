use std::time::Duration;

use bon::builder;

use crate::api::{PerfMetric, PerfRunMode, PerfSpec, RawToolArgs, Unit};
use crate::metrics::model::{AnnotatedMetric, Metric, Metrics, PerfQualities};
use crate::runner::perf::json_parser::JsonParser;
use crate::runner::perf::model::PerfStatRecord;
use crate::runner::perf::records::PerfStatRecords;
use crate::runner::perf::regression::PerfRegressionConfig;
use crate::runner::tool::config::{DEFAULT_PERF_ALPHA, DEFAULT_PERF_MIN_PCNT_RUNNING, PerfConfig};
use crate::runner::tool::path::ToolOutputPath;
use crate::summary::model::ToolMetrics;

#[builder(finish_fn = "fx")]
pub fn json_parser_f(
    output_path: ToolOutputPath,
    min_pcnt_running: Option<f64>,
    #[builder(default = vec![], with = FromIterator::from_iter)] non_zero_metrics: Vec<&str>,
) -> JsonParser {
    JsonParser {
        min_pcnt_running: min_pcnt_running.unwrap_or(DEFAULT_PERF_MIN_PCNT_RUNNING),
        non_zero_metrics: non_zero_metrics
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        output_path,
    }
}

#[builder(finish_fn = "fx")]
pub fn metric_perf_f(
    #[builder(into)] event: Option<String>,
    #[builder(into)] value: Option<Metric>,
    qualities: Option<PerfQualities>,
    unit: Option<Unit>,
) -> (PerfMetric, AnnotatedMetric<PerfQualities>) {
    (
        PerfMetric(event.unwrap_or_else(|| "foo".to_owned())),
        AnnotatedMetric::new(
            value.unwrap_or(Metric::Int(1)),
            qualities.unwrap_or_default(),
            unit,
        ),
    )
}

#[builder(finish_fn = "fx")]
pub fn perf_config_f(
    alpha: f64,
    #[builder(into)] events: String,
    min_pcnt_running: f64,
    #[builder(with = FromIterator::from_iter)] non_zero_metrics: Vec<&str>,
    run_mode: PerfRunMode,
    use_sampling: bool,
) -> PerfConfig {
    PerfConfig {
        alpha,
        events,
        min_pcnt_running,
        non_zero_metrics: non_zero_metrics
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        run_mode,
        use_sampling,
    }
}

#[builder(finish_fn = "fx")]
pub fn perf_regression_config_f(
    soft_limits: Option<Vec<(PerfMetric, f64)>>,
    hard_limits: Option<Vec<(PerfMetric, Option<Unit>, Metric)>>,
    fail_fast: Option<bool>,
    alpha: Option<f64>,
) -> PerfRegressionConfig {
    PerfRegressionConfig {
        alpha: alpha.unwrap_or(DEFAULT_PERF_ALPHA),
        soft_limits: soft_limits.unwrap_or_default(),
        hard_limits: hard_limits.unwrap_or_default(),
        fail_fast: fail_fast.unwrap_or(false),
    }
}

#[builder(finish_fn = "fx")]
pub fn perf_spec_f(
    alpha: Option<f64>,
    #[builder(default = vec![], with = FromIterator::from_iter)] events: Vec<&str>,
    min_pcnt_running: Option<f64>,
    #[builder(default = vec![], with = FromIterator::from_iter)] non_zero_metrics: Vec<&str>,
    record: Option<bool>,
    record_args: Option<RawToolArgs>,
    run_mode: Option<PerfRunMode>,
    sample_duration: Option<Duration>,
) -> PerfSpec {
    PerfSpec {
        alpha,
        events: (!events.is_empty()).then(|| events.into_iter().map(ToOwned::to_owned).collect()),
        min_pcnt_running,
        non_zero_metrics: (!non_zero_metrics.is_empty()).then(|| {
            non_zero_metrics
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        }),
        record,
        record_args: record_args.unwrap_or_default(),
        run_mode,
        sample_duration,
    }
}

#[builder(finish_fn = "fx")]
pub fn perf_stat_record_f(
    instructions: Option<u64>,
    task_clock: Option<f64>,
    qualities: Option<(u64, f64, f64)>,
    some_qualities: Option<bool>,
    mut unit: Option<&str>,
    mut event: Option<&str>,
    runtime: Option<u64>,
    mut mean: Option<f64>,
    mut n: Option<u64>,
    pcnt_running: Option<f64>,
    mut variance: Option<f64>,
    value: Option<&str>,
) -> PerfStatRecord {
    let mut value = value.map(ToOwned::to_owned);

    if let Some(instructions) = instructions {
        if event.is_none() {
            event = Some("instructions:u");
        }
        if value.is_none() {
            value = Some(format!("{instructions}.000000"));
        }
        if unit.is_none() {
            unit = Some("");
        }
    } else if let Some(task_clock) = task_clock {
        if event.is_none() {
            event = Some("task-clock");
        }
        if value.is_none() {
            value = Some(format!("{task_clock:.6}"));
        }
        if unit.is_none() {
            unit = Some("msec");
        }
    } else {
        // do nothing
    }

    if some_qualities == Some(true) {
        if n.is_none() {
            n = Some(1);
        }
        if mean.is_none() {
            mean = Some(1.0);
        }
        if variance.is_none() {
            variance = Some(1.0);
        }
    } else if let Some((new_n, new_mean, new_variance)) = qualities {
        if n.is_none() {
            n = Some(new_n);
        }
        if mean.is_none() {
            mean = Some(new_mean);
        }
        if variance.is_none() {
            variance = Some(new_variance);
        }
    } else {
        // do nothing
    }

    PerfStatRecord {
        cache: None,
        cgroup: None,
        cluster: None,
        core: None,
        counter_value: value,
        counters: None,
        cpu: None,
        die: None,
        event: event.map(ToOwned::to_owned),
        event_runtime: runtime,
        gungraun_mean: mean,
        gungraun_n: n,
        interval: None,
        metric_threshold: None,
        metric_unit: None,
        metric_value: None,
        metricgroup: None,
        node: None,
        pcnt_running,
        socket: None,
        thread: None,
        unit: unit.map(ToOwned::to_owned),
        variance,
    }
}

#[builder(finish_fn = "fx")]
pub fn perf_stat_records_f(
    #[builder(default = vec![], with = FromIterator::from_iter)] records: Vec<PerfStatRecord>,
) -> PerfStatRecords {
    PerfStatRecords(records)
}

#[builder(finish_fn = "fx")]
pub fn tool_metrics_perf_f(
    #[builder(default = vec![], with = FromIterator::from_iter)] metrics: Vec<(
        PerfMetric,
        AnnotatedMetric<PerfQualities>,
    )>,
) -> ToolMetrics {
    ToolMetrics::Perf(Metrics::with_metric_kinds(metrics))
}
