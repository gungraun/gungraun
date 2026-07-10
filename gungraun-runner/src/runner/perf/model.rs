//! Deserialization model for `perf stat --json` output.

use serde::{Deserialize, Serialize};

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
    /// TODO: DOCS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gungraun_mean: Option<f64>,
    /// TODO: DOCS
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
