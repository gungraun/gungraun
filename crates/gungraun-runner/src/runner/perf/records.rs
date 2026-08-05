//! Parse and process `perf stat -j` JSON output into [`Metrics`].

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

use anyhow::Result;
use approx::abs_diff_eq;
use log::trace;
use simplematch::{DoWild, Options};

use crate::api::{PerfMetric, Unit};
use crate::metrics::logic::MetricValue as _;
use crate::metrics::model::{AnnotatedMetric, Metric, Metrics, PerfQualities};
use crate::runner::perf::model::PerfStatRecord;
use crate::stats::runner::{OnlineStatsMap, Stats};

const DOWILD_OPTIONS: Options<u8> = Options::new().enable_escape(true).enable_classes(true);

struct MetricsParser {
    first: Option<(PerfMetric, bool)>,
    invalid: bool,
    metrics: Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>,
    online_stats: OnlineStatsMap<PerfMetric>,
    temp: Vec<(PerfMetric, (AnnotatedMetric<PerfQualities>, f64))>,
}

/// A collection of parsed `perf stat -j` JSON records.
#[derive(Debug, Clone, PartialEq)]
pub struct PerfStatRecords(pub Vec<PerfStatRecord>);

impl MetricsParser {
    fn new() -> Self {
        Self {
            first: None,
            invalid: false,
            metrics: Metrics::empty(),
            online_stats: OnlineStatsMap(HashMap::new()),
            temp: vec![],
        }
    }

    fn parse(
        mut self,
        records: &[PerfStatRecord],
        min_pcnt_running: f64,
        adjustment: Option<&Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>>,
        non_zero_metrics: &[String],
    ) -> (Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>, bool) {
        for record in records {
            match (
                &record.event,
                &record.counter_value,
                &record.unit,
                &record.event_runtime,
                &record.pcnt_running,
                &record.variance,
                &record.gungraun_mean,
                &record.gungraun_n,
            ) {
                (_, _, _, _, pcnt_running, _, _, _)
                    if pcnt_running.is_some_and(|p| {
                        p < min_pcnt_running && !abs_diff_eq!(p, min_pcnt_running, epsilon = 1e-9)
                    }) => {}
                // This is the old/base run (which has our `n` and `mean` stored with the original
                // perf data)
                (
                    Some(event),
                    Some(value),
                    unit,
                    event_runtime,
                    pcnt_running,
                    Some(variance),
                    Some(mean),
                    Some(n),
                ) => {
                    trace!("Found base event: '{event}': '{record:?}'");

                    self.parse_metric_base(
                        event,
                        value,
                        unit.as_deref(),
                        *event_runtime,
                        *pcnt_running,
                        *variance,
                        *mean,
                        *n,
                    );
                }
                (
                    Some(event),
                    Some(value),
                    unit,
                    event_runtime,
                    pcnt_running,
                    variance,
                    None,
                    None,
                ) if unit.as_deref().is_none_or(str::is_empty) => {
                    trace!("Found event: '{event}': '{value}', '{record:?}'");

                    self.parse_metric_without_unit(
                        non_zero_metrics,
                        adjustment,
                        event,
                        value,
                        *event_runtime,
                        *pcnt_running,
                        *variance,
                    );
                }
                // We have to pay attention to the units in this branch. For example, the
                // `online_stats` store the mean in the base scale of the respective unit (if there
                // is a base scale). That is because records can have different units for the same
                // metric.
                (
                    Some(event),
                    Some(value),
                    Some(unit),
                    event_runtime,
                    pcnt_running,
                    variance,
                    None,
                    None,
                ) => {
                    trace!("Found event: '{event}': '{value}' with unit '{unit}', '{record:?}'");

                    self.parse_metric_with_unit(
                        non_zero_metrics,
                        adjustment,
                        event,
                        value,
                        unit,
                        *event_runtime,
                        *pcnt_running,
                        *variance,
                    );
                }
                _ => {}
            }
        }

        if !self.invalid {
            for (key, (metric, float)) in self.temp {
                self.online_stats.insert_or_add(&key, float);
                self.metrics.insert_or_add(key, metric);
            }
        }

        let mut has_duplicates = false;
        for (key, value) in &mut self.metrics {
            if let Some(online_stats) = self.online_stats.get(key)
                && online_stats.n > 1
            {
                has_duplicates = true;

                *value = value.clone().into_mean(online_stats.mean);

                if let Some(event_runtime) = value.qualities.event_runtime.as_mut() {
                    *event_runtime /= online_stats.n;
                }

                let stats = Stats::new(*online_stats);
                value.qualities.rse = Some(stats.rse);
                value.qualities.n = Some(stats.online_stats.n);
                value.qualities.mean = Some(value.rebase(stats.online_stats.mean));

                trace!("Metric (mean): {key}: {value:?}");
            }
        }

        (self.metrics, has_duplicates)
    }

    /// Records a base metric that carries previously-computed statistics.
    ///
    /// Base records are produced by prior runs and already contain `mean`, `variance`, and `n`.
    /// They are inserted directly into `metrics` without going through the duplicate-merging
    /// pipeline. Since base records have already been processed by us, we don't need to validate
    /// records like in [`Self::parse_metric_with_unit`] and [`Self::parse_metric_without_unit`].
    fn parse_metric_base(
        &mut self,
        event: &str,
        value: &str,
        unit: Option<&str>,
        event_runtime: Option<u64>,
        pcnt_running: Option<f64>,
        variance: f64,
        mean: f64,
        n: u64,
    ) {
        let (metric, unit) = if unit.is_none_or(str::is_empty) {
            let Some((int, _)) = parse_perf_u64(value) else {
                return;
            };
            (Metric::Int(int), None)
        } else {
            let Some(float) = parse_perf_f64(value) else {
                return;
            };

            (Metric::Float(float), unit.as_ref().map(|u| Unit::parse(u)))
        };

        // Sort out corrupt data
        if !variance.is_finite()
            || !mean.is_finite()
            || pcnt_running.is_some_and(|p| !p.is_finite())
        {
            return;
        }

        let key = PerfMetric(event.to_owned());
        let annotated_metric = AnnotatedMetric::new(
            metric,
            PerfQualities::new(
                event_runtime,
                pcnt_running,
                Some(variance / 100.0),
                Some(n),
                Some(mean),
            ),
            unit,
        );
        self.metrics
            .insert_or_add(key, annotated_metric.normalize());
    }

    /// Shared validation and cold-start bookkeeping for new (non-base) perf records.
    ///
    /// The first group of records for an event is dropped to mitigate cold-start effects. Pending
    /// records are committed once the second group arrives. Corrupt data is filtered and batches
    /// are marked `invalid` when a zero-valued metric matches a `non_zero_metrics` pattern.
    ///
    /// Returns `Some(key)` when the record should be processed, `None` when it should be skipped.
    #[must_use]
    fn validate_record(
        &mut self,
        non_zero_metrics: &[String],
        event: &str,
        float: f64,
        variance: Option<f64>,
        pcnt_running: Option<f64>,
    ) -> Option<PerfMetric> {
        if let Some((first_key, seen)) = self.first.as_mut() {
            if first_key.name() == event {
                if *seen && !self.invalid {
                    for (key, (metric, float)) in &self.temp {
                        self.online_stats.insert_or_add(key, *float);
                        self.metrics.insert_or_add(key.clone(), metric.clone());
                    }
                    self.temp.clear();
                } else {
                    *seen = true;
                    self.invalid = false;

                    self.temp.clear();

                    trace!("Removing invalid or first record group");
                }
            } else if self.invalid {
                return None;
            } else {
                // do nothing
            }
        } else {
            self.first = Some((PerfMetric(event.to_owned()), false));
        }

        // Sort out corrupt data
        if variance.is_some_and(|v| !v.is_finite()) || pcnt_running.is_some_and(|p| !p.is_finite())
        {
            return None;
        }

        let key = PerfMetric(event.to_owned());

        if float == 0.0
            && non_zero_metrics
                .iter()
                .any(|n| n.as_str().dowild_with(key.name(), DOWILD_OPTIONS))
        {
            trace!(
                "Found invalid perf measurement '{}': '{float}' == 0.0",
                key.name()
            );
            self.invalid = true;
            return None;
        }

        Some(key)
    }

    /// Processes a unit-less perf counter record and queues it for duplicate merging.
    ///
    /// Unit-less counters are parsed as `u64` integers when possible. The first group for an event
    /// is dropped (cold-start mitigation) and zero-valued metrics matching `non_zero_metrics`
    /// patterns invalidate the entire batch. If an `adjustment` metric exists, it is subtracted
    /// from the parsed value.
    fn parse_metric_without_unit(
        &mut self,
        non_zero_metrics: &[String],
        adjustment: Option<&Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>>,
        event: &str,
        value: &str,
        event_runtime: Option<u64>,
        pcnt_running: Option<f64>,
        variance: Option<f64>,
    ) {
        let Some((mut int, mut float)) = parse_perf_u64(value) else {
            return;
        };

        let key = PerfMetric(event.to_owned());

        if let Some(adjustment) = adjustment
            && let Some(metric) = adjustment.metric_by_kind(&key)
        {
            #[expect(clippy::cast_precision_loss)]
            #[expect(clippy::cast_possible_truncation)]
            #[expect(clippy::cast_sign_loss)]
            let (new_int, new_float) = match metric.metric {
                Metric::Int(int_metric) => (
                    int.saturating_sub(int_metric),
                    (float - int_metric as f64).max(0.0),
                ),
                // For completeness, but the adjustment metric should be an int like this metric
                Metric::Float(float_metric) => (
                    int.saturating_sub(float_metric as u64),
                    (float - float_metric).max(0.0),
                ),
            };

            int = new_int;
            float = new_float;
        }

        if self
            .validate_record(non_zero_metrics, event, float, variance, pcnt_running)
            .is_none()
        {
            return;
        }

        let new_metric = AnnotatedMetric::new(
            Metric::Int(int),
            PerfQualities::new(
                event_runtime,
                pcnt_running,
                variance.map(|v| v / 100.0),
                None,
                None,
            ),
            None,
        );

        self.temp.push((key, (new_metric, float)));
    }

    /// Processes a unit-aware perf counter record and queues it for duplicate merging.
    ///
    /// Counters with units are parsed as `f64` values. The first group for an event is dropped
    /// (cold-start mitigation) and zero-valued metrics matching `non_zero_metrics` patterns
    /// invalidate the entire batch. If an `adjustment` metric exists, it is subtracted from the
    /// parsed value via `saturating_sub`.
    fn parse_metric_with_unit(
        &mut self,
        non_zero_metrics: &[String],
        adjustment: Option<&Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>>,
        event: &str,
        value: &str,
        unit: &str,
        event_runtime: Option<u64>,
        pcnt_running: Option<f64>,
        variance: Option<f64>,
    ) {
        let Some(float) = parse_perf_f64(value) else {
            return;
        };

        let mut new_metric = AnnotatedMetric::new(
            Metric::Float(float),
            PerfQualities::new(
                event_runtime,
                pcnt_running,
                variance.map(|v| v / 100.0),
                None,
                None,
            ),
            Unit::parse(unit),
        );

        let key = PerfMetric(event.to_owned());

        if let Some(adjustment) = adjustment
            && let Some(metric) = adjustment.metric_by_kind(&key)
        {
            new_metric = new_metric.saturating_sub(&metric);
        }

        if self
            .validate_record(
                non_zero_metrics,
                event,
                new_metric.metric.to_float(),
                variance,
                pcnt_running,
            )
            .is_none()
        {
            return;
        }

        trace!(
            "Pushing record '{}': {float} with metric {new_metric:?}",
            key.name()
        );

        let base_value = new_metric.base_value();
        self.temp.push((key, (new_metric, base_value)));
    }
}

impl PerfStatRecords {
    /// Creates a `PerfStatRecords` parsing newline-delimited JSON records emitted by `perf stat -j`
    pub fn parse(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        serde_json::Deserializer::from_reader(reader)
            .into_iter::<PerfStatRecord>()
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
            .map_err(Into::into)
    }

    /// Filter duplicates and update each record with mean, variance and other values from `metrics`
    ///
    /// This preserves the original unit string from each record. When the metric's unit differs
    /// from the record's original unit, values are converted back to the original unit scale.
    pub fn filter_and_update(
        &mut self,
        metrics: &Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>,
    ) {
        let mut seen = HashSet::new();

        self.0.retain_mut(|record| {
            if seen.contains(&record.event) {
                false
            } else {
                seen.insert(record.event.clone());
                record.update(metrics);
                true
            }
        });
    }

    /// Write the records back to a JSON file
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut file = File::options().write(true).truncate(true).open(path)?;

        for record in &self.0 {
            serde_json::to_writer(&mut file, &record)?;
            writeln!(file)?;
        }

        Ok(())
    }

    /// Converts parsed perf JSON records into Gungraun perf metrics.
    ///
    /// Duplicate records for the same perf event are merged into a single metric. The numeric
    /// metric is averaged, `event_runtime` is averaged alongside it, `pcnt_running` is preserved
    /// through the quality merge rules, and perf's `"variance"` field is preserved for single
    /// records but recomputed for merged duplicates as perf's relative standard error percentage.
    ///
    /// Unit-less metrics are parsed as integer metrics when possible.
    ///
    /// If a metric matches any pattern in `non_zero_metrics` and has a zero value, the entire
    /// measurement batch is discarded. Patterns use `simplematch` glob syntax.
    pub fn to_metrics(
        &self,
        min_pcnt_running: f64,
        adjustment: Option<&Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>>,
        non_zero_metrics: &[String],
    ) -> (Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>, bool) {
        MetricsParser::new().parse(&self.0, min_pcnt_running, adjustment, non_zero_metrics)
    }
}

/// Parse a perf counter value string into a `(u64, f64)` pair.
///
/// Perf outputs counter values as strings with trailing decimals (e.g. `"1000.000000"`). This
/// function normalizes such values into a `u64` integer suitable for unit-less counters, while
/// preserving the original `f64` for reference.
///
/// Values without a decimal point are parsed directly as `u64`. This is for completeness - perf
/// counters have always fractions. Values with a decimal point are parsed as `f64` first, then
/// rounded to `u64` using standard rounding rules (except every exact half `.5` which rounds down).
/// The function rejects non-finite, negative, and unsupported values (like `"<not supported>"` or
/// `"<not counted>"`).
///
/// This is used for unit-less perf metrics which are always counts (without fractions). Small noise
/// fractions (e.g. `0.00000001`) are handled by the rounding behavior.
fn parse_perf_u64(value: &str) -> Option<(u64, f64)> {
    let value = value.trim();

    value.split_once('.').map_or_else(
        || value.parse::<u64>().ok().zip(value.parse::<f64>().ok()),
        |_| {
            value.parse::<f64>().ok().and_then(|v| {
                #[expect(clippy::cast_possible_truncation)]
                #[expect(clippy::cast_sign_loss)]
                (v.is_finite() && v.is_sign_positive()).then(|| {
                    #[expect(clippy::float_cmp, reason = "0.5 is exactly representable")]
                    if v.fract() == 0.5 {
                        (v.floor() as u64, v)
                    } else {
                        (v.round() as u64, v)
                    }
                })
            })
        },
    )
}

fn parse_perf_f64(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::perf::{perf_stat_record_f, perf_stat_records_f};
    use crate::runner::tool::config::DEFAULT_PERF_MIN_PCNT_RUNNING;

    #[rstest]
    #[case::integer_without_decimal("1000", Some((1000, 1000.0)))]
    #[case::integer_with_zero_decimal("1000.000000", Some((1000, 1000.0)))]
    #[case::zero_without_decimal("0", Some((0, 0.0)))]
    #[case::zero_with_decimal("0.000000", Some((0, 0.0)))]
    #[case::one_with_decimal("1.000000", Some((1, 1.0)))]
    #[case::float_noise_rounds_correctly(
        "999999999.0000000001",
        Some((999_999_999, 999_999_999.0))
    )]
    #[case::small_float_noise("1.0000000001", Some((1, 1.000_000_000_1)))]
    #[case::half_rounds_toward_zero("0.5", Some((0, 0.5)))]
    #[case::one_point_five_rounds_down("1.5", Some((1, 1.5)))]
    #[case::ten_point_five_round_down("10.5", Some((10, 10.5)))]
    #[case::large_point_five_round_down(
        "10000000000000.5",
        Some((10_000_000_000_000, 10_000_000_000_000.5))
    )]
    #[case::almost_one_rounds_up("0.999999", Some((1, 0.999_999)))]
    #[case::large_counter("999999999999.000000", Some((999_999_999_999, 999_999_999_999.0)))]
    #[case::trim_whitespace("  100.000000  ", Some((100, 100.0)))]
    #[case::negative_with_decimal("-1.000000", None)]
    #[case::negative_integer("-1", None)]
    #[case::nan("NaN", None)]
    #[case::positive_infinity("inf", None)]
    #[case::negative_infinity("-inf", None)]
    #[case::not_supported("<not supported>", None)]
    #[case::not_counted("<not counted>", None)]
    fn test_parse_perf_u64(#[case] input: &str, #[case] expected: Option<(u64, f64)>) {
        assert_eq!(parse_perf_u64(input), expected);
    }

    #[rstest]
    #[case::integer_string("1000", Some(1000.0))]
    #[case::float_string("12.5", Some(12.5))]
    #[case::zero("0.0", Some(0.0))]
    #[case::negative("-1.5", Some(-1.5))]
    #[case::nan("NaN", None)]
    #[case::positive_infinity("inf", None)]
    #[case::negative_infinity("-inf", None)]
    #[case::not_supported("<not supported>", None)]
    fn test_parse_perf_f64(#[case] input: &str, #[case] expected: Option<f64>) {
        assert_eq!(parse_perf_f64(input), expected);
    }

    #[test]
    fn test_records_filter_and_update_filters_second_and_updates_first() {
        let first = perf_stat_record_f()
            .instructions(1)
            .runtime(1)
            .pcnt_running(1.0)
            .variance(1.0)
            .fx();
        let second = perf_stat_record_f().instructions(10).fx();

        let metrics = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(
                Metric::Int(200),
                PerfQualities::new(300, 66.666_666_666_666_67, 0.5, 1, 400.0),
                None,
            ),
        )]);

        let expected = perf_stat_records_f()
            .records([perf_stat_record_f()
                .instructions(200)
                .runtime(300)
                .mean(400.0)
                .n(1)
                .pcnt_running(66.666_666_666_666_67)
                .variance(50.0)
                .fx()])
            .fx();

        let mut actual = perf_stat_records_f().records([first, second]).fx();

        actual.filter_and_update(&metrics);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parse_records_reads_newline_delimited_perf_json() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{"counter-value":"1000.000000","event":"instructions:u","unit":""}
{"counter-value":"200.000000","event":"cycles:u","unit":""}
"#,
        )
        .unwrap();

        let actual = PerfStatRecords::parse(file.path()).unwrap();

        let expected = perf_stat_records_f()
            .records([
                perf_stat_record_f().instructions(1000).fx(),
                perf_stat_record_f()
                    .event("cycles:u")
                    .value("200.000000")
                    .unit("")
                    .fx(),
            ])
            .fx();

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_no_unit() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f().instructions(1).fx()])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(1, None),
        )]);

        let (actual, has_duplicates) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert!(!has_duplicates);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_two_different_records_int_and_float() {
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f().instructions(1000).fx(),
                perf_stat_record_f().task_clock(12.5).fx(),
            ])
            .fx();

        let expected = Metrics::with_metric_kinds([
            (
                PerfMetric("instructions:u".to_owned()),
                AnnotatedMetric::with_default_qualities(1000, None),
            ),
            (
                PerfMetric("task-clock".to_owned()),
                AnnotatedMetric::with_default_qualities(12.5, Unit::Milliseconds),
            ),
        ]);

        let (metrics, has_duplicates) =
            records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert!(!has_duplicates);
        assert_eq!(metrics, expected);
    }

    #[test]
    fn test_to_metrics_when_qualities_are_present() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f()
                .instructions(1000)
                .runtime(100)
                .pcnt_running(50.0)
                .variance(7.0)
                .fx()])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(1000, PerfQualities::new(100, 50.0, 0.07, None, None), None),
        )]);

        let (actual, has_duplicates) = records.to_metrics(50.0, None, &[]);

        assert!(!has_duplicates);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_single_int_zero_metric_matching_pattern_then_empty() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f().instructions(0).fx()])
            .fx();

        let (metrics, _) = records.to_metrics(
            DEFAULT_PERF_MIN_PCNT_RUNNING,
            None,
            &["*instructions*".to_owned()],
        );

        assert!(metrics.is_empty());
    }

    #[test]
    fn test_to_metrics_when_float_zero_metric_matching_pattern_then_empty() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f().task_clock(0.0).fx()])
            .fx();

        let (metrics, _) = records.to_metrics(
            DEFAULT_PERF_MIN_PCNT_RUNNING,
            None,
            &["task-clock*".to_owned()],
        );

        assert!(metrics.is_empty());
    }

    #[test]
    fn test_to_metrics_when_int_zero_metric_in_batch_discards_all_metrics() {
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f()
                    .event("cycles:u")
                    .value("1000.000000")
                    .unit("")
                    .fx(),
                perf_stat_record_f().instructions(0).fx(),
            ])
            .fx();

        let (metrics, _) = records.to_metrics(
            DEFAULT_PERF_MIN_PCNT_RUNNING,
            None,
            &["*instructions*".to_owned()],
        );

        assert!(metrics.is_empty());
    }

    #[test]
    fn test_to_metrics_when_first_invalid() {
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f().instructions(0).fx(),
                perf_stat_record_f().instructions(1000).fx(),
            ])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(1000, None),
        )]);

        let (metrics, has_duplicates) = records.to_metrics(
            DEFAULT_PERF_MIN_PCNT_RUNNING,
            None,
            &["*instructions*".to_owned()],
        );

        assert!(!has_duplicates);
        assert_eq!(metrics, expected);
    }

    #[test]
    fn test_to_metrics_when_second_invalid_then_no_metric() {
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f().instructions(1).fx(),
                perf_stat_record_f().instructions(0).fx(),
            ])
            .fx();

        let (metrics, _) = records.to_metrics(
            DEFAULT_PERF_MIN_PCNT_RUNNING,
            None,
            &["*instructions*".to_owned()],
        );

        assert!(metrics.is_empty());
    }

    #[test]
    fn test_to_metrics_when_second_invalid_and_third_then_last() {
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f().instructions(1).fx(),
                perf_stat_record_f().instructions(0).fx(),
                perf_stat_record_f().instructions(2).fx(),
            ])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(2, None),
        )]);

        let (actual, _) = records.to_metrics(
            DEFAULT_PERF_MIN_PCNT_RUNNING,
            None,
            &["*instructions*".to_owned()],
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_corrupt_variance_then_empty() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f()
                .instructions(1)
                .variance(f64::INFINITY)
                .fx()])
            .fx();

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert!(actual.is_empty());
    }

    #[test]
    fn test_to_metrics_when_corrupt_pcnt_running_then_empty() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f()
                .instructions(1)
                .pcnt_running(f64::INFINITY)
                .fx()])
            .fx();

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert!(actual.is_empty());
    }

    #[rstest]
    #[case::value_with_unit(
        perf_stat_record_f().task_clock(f64::INFINITY).some_qualities(true).fx()
    )]
    #[case::value_without_unit(
        perf_stat_record_f().instructions(1).value("inf").some_qualities(true).fx()
    )]
    #[case::variance(
        perf_stat_record_f().task_clock(1.0).variance(f64::INFINITY).some_qualities(true).fx()
    )]
    #[case::mean(
        perf_stat_record_f().task_clock(1.0).mean(f64::INFINITY).some_qualities(true).fx()
    )]
    #[case::pcnt_running(
        perf_stat_record_f()
            .task_clock(1.0)
            .pcnt_running(f64::INFINITY)
            .some_qualities(true)
            .fx()
    )]
    fn test_to_metrics_when_base_metric_corrupt_data_then_no_metric(
        #[case] record: PerfStatRecord,
    ) {
        let records = perf_stat_records_f().records([record]).fx();

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert!(actual.is_empty());
    }

    #[test]
    fn test_to_metrics_when_integer_base_metric() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f()
                .instructions(1)
                .mean(1.0)
                .n(1)
                .variance(0.0)
                .fx()])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(1, PerfQualities::new(None, None, 0.0, 1, 1.0), None),
        )]);

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_float_base_metric() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f()
                .task_clock(1.0)
                .mean(1.0)
                .n(1)
                .variance(0.0)
                .fx()])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::new(
                1.0,
                PerfQualities::new(None, None, 0.0, 1, 1.0),
                Unit::Milliseconds,
            ),
        )]);

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_base_and_adjustment_then_ignores_adjustment() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f()
                .instructions(3)
                .some_qualities(true)
                .fx()])
            .fx();

        let adjustment = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(2, None),
        )]);

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(3, PerfQualities::new(None, None, 0.01, 1, 1.0), None),
        )]);

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, Some(&adjustment), &[]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_two_perf_qualities_then_first_is_dropped() {
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f()
                    .instructions(100)
                    .runtime(100)
                    .pcnt_running(50.0)
                    .variance(7.0)
                    .fx(),
                perf_stat_record_f()
                    .instructions(300)
                    .runtime(300)
                    .pcnt_running(75.0)
                    .variance(11.0)
                    .fx(),
            ])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(300, PerfQualities::new(300, 75.0, 0.11, None, None), None),
        )]);

        let (actual, has_duplicates) = records.to_metrics(50.0, None, &[]);

        assert!(!has_duplicates);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_three_perf_qualities_then_merge_dropping_variance() {
        // The first one is sorted out
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f()
                    .instructions(1000)
                    .runtime(500)
                    .pcnt_running(100.0)
                    .variance(10.0)
                    .fx(),
                perf_stat_record_f()
                    .instructions(100)
                    .runtime(100)
                    .pcnt_running(50.0)
                    .variance(7.0)
                    .fx(),
                perf_stat_record_f()
                    .instructions(300)
                    .runtime(300)
                    .pcnt_running(75.0)
                    .variance(11.0)
                    .fx(),
            ])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(
                200,
                PerfQualities::new(
                    Some(200),
                    Some(66.666_666_666_666_67),
                    Some(0.5),
                    Some(2),
                    Some(200.0),
                ),
                None,
            ),
        )]);

        let (actual, has_duplicates) = records.to_metrics(50.0, None, &[]);

        assert!(has_duplicates);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_duplicate_time_metrics_then_mean_uses_metric_unit_scale() {
        // First one is dropped
        let records = perf_stat_records_f()
            .records([
                perf_stat_record_f()
                    .task_clock(100.0)
                    .runtime(100)
                    .pcnt_running(100.0)
                    .variance(7.0)
                    .fx(),
                perf_stat_record_f()
                    .task_clock(1000.0)
                    .runtime(1000)
                    .pcnt_running(100.0)
                    .variance(7.0)
                    .fx(),
                perf_stat_record_f()
                    .task_clock(500.0)
                    .runtime(500)
                    .pcnt_running(100.0)
                    .variance(11.0)
                    .fx(),
            ])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::new(
                750.0,
                PerfQualities::new(
                    Some(750),
                    Some(100.0),
                    Some(0.333_333_333_333_333_3),
                    Some(2),
                    Some(750.0),
                ),
                Unit::Milliseconds,
            ),
        )]);

        let (actual, has_duplicates) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert!(has_duplicates);
        assert_eq!(actual, expected);
    }

    // --- to_metrics: Adjustment Subtraction ---

    #[test]
    fn test_to_metrics_when_float_with_unit_and_adjustment_then_applies_adjustment() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f().task_clock(3.0).fx()])
            .fx();

        let adjustment = Metrics::with_metric_kinds([(
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::with_default_qualities(2.0, Unit::Milliseconds),
        )]);

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("task-clock".to_owned()),
            AnnotatedMetric::with_default_qualities(1.0, Unit::Milliseconds),
        )]);

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, Some(&adjustment), &[]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_int_without_unit_and_adjustment_then_applies_adjustment() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f().instructions(3).fx()])
            .fx();

        let adjustment = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(2, None),
        )]);

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::with_default_qualities(1, None),
        )]);

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, Some(&adjustment), &[]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_pcnt_running_is_lower_than_threshold_then_no_metrics() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f().instructions(3).pcnt_running(75.0).fx()])
            .fx();

        let (actual, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);

        assert!(actual.is_empty());
    }

    #[test]
    fn test_to_metrics_when_pcnt_running_is_custom_and_higher() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f().instructions(3).pcnt_running(75.0).fx()])
            .fx();

        let expected = Metrics::with_metric_kinds([(
            PerfMetric("instructions:u".to_owned()),
            AnnotatedMetric::new(3, PerfQualities::new(None, 75.0, None, None, None), None),
        )]);

        let (actual, _) = records.to_metrics(50.0, None, &[]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_metrics_when_no_counter_value_then_no_metric() {
        let records = perf_stat_records_f()
            .records([perf_stat_record_f()
                .event("instructions:u")
                .value("<not supported>")
                .unit("")
                .fx()])
            .fx();

        let (metrics, _) = records.to_metrics(DEFAULT_PERF_MIN_PCNT_RUNNING, None, &[]);
        assert!(metrics.is_empty());
    }
}
