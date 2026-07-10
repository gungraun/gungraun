use bon::builder;

use crate::runner::perf::model::PerfStatRecord;
use crate::runner::perf::records::PerfStatRecords;

#[builder(finish_fn = "fixture")]
pub fn perf_stat_records_f(
    #[builder(default = vec![], with = FromIterator::from_iter)] records: Vec<PerfStatRecord>,
) -> PerfStatRecords {
    PerfStatRecords(records)
}

#[builder(finish_fn = "fixture")]
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
