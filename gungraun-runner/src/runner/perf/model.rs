//! Deserialization model for `perf stat --json` output.

use serde::{Deserialize, Serialize};

use crate::api::{PerfMetric, Unit};
use crate::metrics::model::{AnnotatedMetric, Metric, Metrics, PerfQualities};

/// A single record from `perf stat --json` output.
///
/// Fields are context-dependent: core fields (`counter_value`, `event`) are always present;
/// aggregation fields appear based on the `--per-*` flag used; metric and runtime fields appear
/// based on other flags.
///
/// Based on the Linux kernel's `tools/perf/util/stat-display.c`, these are **all** the JSON fields
/// that `perf stat` can emit. Not all of them are documented in `man perf-stat`:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PerfStatRecord {
    /// Cache aggregation identifier (e.g. `"S0-D0-L3-ID0"`).
    /// Introduced by `--per-cache`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<String>,
    /// Cgroup name.
    /// Introduced by `-G` / `--cgroup`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroup: Option<String>,
    /// Cluster aggregation identifier (e.g. `"S0-D0-CLS0"`).
    /// Introduced by `--per-cluster`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    /// Core aggregation identifier (e.g. `"S0-D0-C0"`).
    /// Introduced by `--per-core`, or with `--per-core` + no `--percore-show-thread`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub core: Option<String>,
    /// Counter value as a string. May be a float like `"1000000.000000"`,
    /// or `"<not supported>"` / `"<not counted>"` for unsupported or not counted events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counter_value: Option<String>,
    /// Number of hardware counters aggregated.
    /// Introduced by non-global aggregation modes (`--per-core`, `--per-socket`, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counters: Option<u64>,
    /// CPU identifier as a string (e.g. `"0"`).
    /// Introduced by `--per-core` (without `--percore-show-thread`) or `--per-thread` when the CPU
    /// ID is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    /// Die aggregation identifier (e.g. `"S0-D0"`).
    /// Introduced by `--per-die`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub die: Option<String>,
    /// Event name (e.g. `"cycles"`, `"instructions"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Time the event was enabled, in nanoseconds.
    /// Present when the counter was not running 100% of the time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_runtime: Option<u64>,
    /// Mean value of this [`Self::event`], added by Gungraun.
    ///
    /// This field is populated from [`PerfQualities::mean`] when the record is re-constructed and
    /// written back from processed and analyzed perf data.
    ///
    /// [`PerfQualities::mean`]: crate::metrics::model::PerfQualities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gungraun_mean: Option<f64>,
    /// Number of samples (n) of this [`Self::event`], added by Gungraun.
    ///
    /// This field is populated from [`PerfQualities::n`] when the record is re-constructed and
    /// written back from processed and analyzed perf data.
    ///
    /// [`PerfQualities::n`]: crate::metrics::model::PerfQualities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gungraun_n: Option<u64>,
    /// Timestamp as seconds since epoch (e.g. `1234.567890123`).
    /// Introduced by `-I` / `--interval-print`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<f64>,
    /// Metric threshold classification: `"unknown"`, `"bad"`, `"nearly bad"`, `"less good"`, or
    /// `"good"`.
    /// Introduced when metric threshold evaluation is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_threshold: Option<String>,
    /// Unit of a derived metric (e.g. `"insn per cycle"`).
    /// Present when a metric is associated with the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_unit: Option<String>,
    /// Value of a derived metric as a string, or `"none"`.
    /// Present when a metric is associated with the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_value: Option<String>,
    /// Metric group name.
    /// Introduced by `--metricgroup` / metric group grouping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metricgroup: Option<String>,
    /// Node aggregation identifier (e.g. `"N0"`).
    /// Introduced by `--per-node`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Percentage of time the counter was running (e.g. `100.00`).
    /// Present when `event_runtime` is present and the counter was not running 100% of the enabled
    /// time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcnt_running: Option<f64>,
    /// Socket aggregation identifier (e.g. `"S0"`). Introduced by `--per-socket`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    /// Thread identifier (e.g. `"comm-pid"`). Introduced by `--per-thread`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    /// Event unit (e.g. `"nJ"`, `"MiB"`).
    /// Present when the event has an associated unit but can be empty for an absent unit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Relative standard deviation as a percentage (coefficient of variation).
    /// Despite the JSON key name `"variance"`, this is not statistical variance; it is `100 *
    /// stddev / mean` — the same value shown as `( +-X.XX% )` in text mode. Introduced by `-r` /
    /// `--repeat`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variance: Option<f64>,
}

impl PerfStatRecord {
    /// Update the record with the values from `metrics`
    ///
    /// The original units of the record are preserved transforming the units from metrics.
    pub fn update(&mut self, metrics: &Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>) {
        let Some(event) = self.event.as_ref() else {
            return;
        };

        let Some(metric) = metrics.metric_by_kind(&PerfMetric(event.clone())) else {
            return;
        };

        self.event_runtime = metric.qualities.event_runtime;
        self.pcnt_running = metric.qualities.pcnt_running;
        self.variance = metric.qualities.rse.map(|rse| rse * 100.0);
        self.gungraun_n = metric.qualities.n;

        if let Some(counter_value) = self.counter_value.as_mut() {
            #[expect(clippy::cast_precision_loss)]
            let metric_value = match metric.metric {
                Metric::Int(int) => int as f64,
                Metric::Float(float) => float,
            };

            if let (Some(self_unit), Some(metric_unit)) = (
                self.unit
                    .as_ref()
                    .and_then(|u| (!u.is_empty()).then_some(u)),
                &metric.unit,
            ) {
                let orig_unit = Unit::parse(self_unit);

                if orig_unit != *metric_unit && !matches!(orig_unit, Unit::Unknown(_)) {
                    let base = metric_unit.base_value(metric_value);
                    let converted_value = orig_unit.rebase(base);

                    let converted_mean = metric.qualities.mean.map(|mean| {
                        let base_mean = metric_unit.base_value(mean);
                        orig_unit.rebase(base_mean)
                    });

                    *counter_value = format!("{converted_value:.6}");
                    self.gungraun_mean = converted_mean;
                    return;
                }
            }

            *counter_value = Self::format_metric(&metric.metric);
        }

        self.gungraun_mean = metric.qualities.mean;
    }

    fn format_metric(metric: &Metric) -> String {
        match metric {
            Metric::Int(int) => format!("{int}.000000"),
            Metric::Float(float) => format!("{float:.6}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::fixtures::perf::perf_stat_record_f;

    #[rstest]
    #[case::no_unit(
        perf_stat_record_f()
            .instructions(1)
            .runtime(1)
            .pcnt_running(1.0)
            .variance(1.0)
            .fx(),
        (
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(
                Metric::Int(200),
                PerfQualities::new(200, 66.666_666_666_666_67, 0.5, 1, 200.0),
                None,
            ),
        ),
        perf_stat_record_f()
            .instructions(200)
            .runtime(200)
            .mean(200.0)
            .n(1)
            .pcnt_running(66.666_666_666_666_67)
            .variance(50.0)
            .fx()

    )]
    #[case::unit_with_different_scale(
        perf_stat_record_f()
            .task_clock(1.0)
            .runtime(1)
            .pcnt_running(1.0)
            .variance(1.0)
            .fx(),
        (
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::new(
                Metric::Float(1.5),
                PerfQualities::new(300, 75.0, 0.05, 2, 200.0),
                Unit::Seconds,
            ),
        ),
        perf_stat_record_f()
            .task_clock(1500.0)
            .runtime(300)
            .mean(200_000.0)
            .n(2)
            .pcnt_running(75.0)
            .variance(5.0)
            .fx()
    )]
    #[case::unit_with_same_scale(
        perf_stat_record_f()
            .task_clock(1.0)
            .runtime(1)
            .pcnt_running(1.0)
            .variance(1.0)
            .fx(),
        (
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::new(
                Metric::Float(1.5),
                PerfQualities::new(300, 75.0, 0.05, 2, 200.0),
                Unit::Milliseconds,
            ),
        ),
        perf_stat_record_f()
            .task_clock(1.5)
            .runtime(300)
            .mean(200.0)
            .n(2)
            .pcnt_running(75.0)
            .variance(5.0)
            .fx()
    )]
    #[case::unknown_unit(
        perf_stat_record_f()
            .task_clock(1.0)
            .unit("nope")
            .runtime(1)
            .pcnt_running(1.0)
            .variance(1.0)
            .fx(),
        (
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::new(
                Metric::Float(1.5),
                PerfQualities::new(300, 75.0, 0.05, 2, 200.0),
                Unit::Unknown("nope".to_owned()),
            ),
        ),
        perf_stat_record_f()
            .task_clock(1.5)
            .unit("nope")
            .runtime(300)
            .mean(200.0)
            .n(2)
            .pcnt_running(75.0)
            .variance(5.0)
            .fx()
    )]
    fn test_update_record(
        #[case] mut record: PerfStatRecord,
        #[case] metric: (PerfMetric, AnnotatedMetric<PerfQualities>),
        #[case] expected: PerfStatRecord,
    ) {
        let metrics = Metrics::with_metric_kinds([metric]);

        record.update(&metrics);

        assert_eq!(record, expected);
    }
}
