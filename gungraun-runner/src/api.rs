//! Public API types for benchmark configuration and execution.
//!
//! This module defines the data structures that form the interface between the macro layer in
//! `gungraun` and the benchmark runner. All types usable in `gungraun` are serializable and
//! deserializable, enabling the benchmark harness to communicate the benchmark configuration to
//! the runner through a binary encoding.
//!
//! # Architecture
//!
//! Gungraun follows a two-stage execution model:
//!
//! 1. **Compile time**: Procedural macros in `gungraun-macros` parse attribute annotations like
//!    `#[library_benchmark]` and `#[binary_benchmark]`. The `gungraun` crate assembles these into a
//!    benchmarking harness and serializes the configuration into a binary format using `bincode`.
//!
//! 2. **Runtime**: The runner (`gungraun-runner`) receives this serialized data via stdin,
//!    deserializes it into the types defined here, and executes the benchmarks according to the
//!    configuration.
//!
//! Library benchmarks create [`LibraryBenchmarkGroups`], and binary benchmarks create
//! [`BinaryBenchmarkGroups`].
//!
//! # Data Contract
//!
//! The types in this module constitute a **data contract** between two independently compiled
//! components: the macro crate and the runner crate. This contract has several implications:
//!
//! - **Version coupling**: The macro crate and runner must use compatible versions of these types.
//!   The runner performs a version check at startup to ensure alignment.
//! - **Serialization stability**: All types derive `Serialize` and `Deserialize` via serde. The
//!   binary encoding must remain stable across compatible versions.
//! - **Feature-gated visibility**: Some implementations are only needed by the runner and are gated
//!   behind the `runner` feature. Users writing benchmarks do not need this feature enabled.
//! - **Schema generation**: The `schema` feature enables deriving `JsonSchema` for the json summary
//!   file validation.
//!
//! Because this module serves as a stable interface, users interact with some of these types
//! indirectly through attribute macros rather than constructing them directly. The macro syntax in
//! `gungraun` is the stable user-facing API; the types themselves are an implementation detail that
//! may evolve.
//!
//! # Stability
//!
//! Changes to this API can be considered breaking if they affect how `gungraun` uses these types.
//! Since this API facilitates communication between internal components, it does not follow semver.
//! However, every notable change requires a version bump.

#[cfg(feature = "runner")]
macro_rules! impl_from_str_metric {
    ($type:ty, $error_msg:literal, { $($alias:pat => $variant:ident),* $(,)? }) => {
        impl FromStr for $type {
            type Err = anyhow::Error;

            fn from_str(string: &str) -> Result<Self, Self::Err> {
                let lower = string.to_lowercase();
                let value = match lower.as_str() {
                    $($alias => Self::$variant,)*
                    _ => return Err(anyhow!($error_msg, string)),
                };
                Ok(value)
            }
        }
    };
}

#[cfg(feature = "runner")]
macro_rules! impl_from_str_metric_groups {
    (
        $type:ty,
        $inner_type:ty,
        $inner_variant:ident,
        $error_msg:literal,
        { $($alias:pat => $variant:ident),* $(,)? }
    ) => {
        impl FromStr for $type {
            type Err = anyhow::Error;

            fn from_str(string: &str) -> Result<Self, Self::Err> {
                let lower = string.to_lowercase();
                match lower.as_str().strip_prefix('@') {
                    Some(suffix) => match suffix {
                        $($alias => Ok(Self::$variant),)*
                        _ => Err(anyhow!($error_msg, string)),
                    },
                    None => <$inner_type>::from_str(string).map(Self::$inner_variant),
                }
            }
        }
    };
}

#[cfg(feature = "runner")]
use std::borrow::Cow;
#[cfg(feature = "runner")]
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Display;
#[cfg(feature = "runner")]
use std::fs::File;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
#[cfg(feature = "runner")]
use std::process::{Child, Command as StdCommand, Stdio as StdStdio};
#[cfg(feature = "runner")]
use std::str::FromStr;
use std::time::Duration;

#[cfg(feature = "runner")]
use anyhow::anyhow;
#[cfg(feature = "runner")]
use indexmap::IndexSet;
#[cfg(feature = "runner")]
use indexmap::indexset;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "runner")]
use strum::{EnumIter, IntoEnumIterator};

#[cfg(feature = "runner")]
use crate::metrics::logic::Summarize;
#[cfg(feature = "runner")]
use crate::metrics::logic::TypeChecker;
#[cfg(feature = "runner")]
use crate::metrics::model::{AnnotatedMetric, Metric, PerfQualities};
pub use crate::stats::common::{calibrate_linear, logistic};
pub use crate::units::Unit;
#[cfg(feature = "runner")]
use crate::util;

/// The file descriptor perf reads perf control messages from.
///
/// The runner remaps the control pipe's read end to this descriptor before spawning `perf`, while
/// the benchmark harness writes `enable` and `disable` commands to the paired [`PERF_CTL_FD_WRITE`]
/// endpoint.
pub const PERF_CTL_FD_READ: i32 = 100;
/// The file descriptor the benchmark harness writes perf control messages to.
///
/// Messages sent here coordinate perf-managed benchmark runs such as repetition calibration and
/// fixed-repeat execution.
pub const PERF_CTL_FD_WRITE: i32 = 102;
/// The file descriptor perf writes control acknowledgements to.
///
/// The runner remaps the acknowledgement pipe's write end to this descriptor so `perf` can signal
/// the harness after processing a control message.
pub const PERF_ACK_FD_WRITE: i32 = 101;
/// The file descriptor the benchmark harness reads perf acknowledgements from.
///
/// The harness waits on this paired read end after sending each control message so `perf` can
/// acknowledge the transition.
pub const PERF_ACK_FD_READ: i32 = 103;
/// The file descriptor reserved for the benchmark's perf coordination log.
///
/// The runner duplicates the perf log file onto this descriptor before `exec` so the harness can
/// emit messages without reopening the file.
pub const PERF_LOG_FD: i32 = 3;
/// The perf coordination log marker the harness uses to report calibrated repetition counts.
///
/// Perf-managed runs parse this prefix from the perf log body and treat the trailing value as the
/// repetition count selected by the harness.
pub const PERF_REPETITIONS_MARKER: &str = "gungraun::__perf_repetitions:";

/// Controls how the generated benchmark harness executes one benchmark run.
///
/// [`BenchRunMode::Default`] performs an ordinary benchmark invocation which is used by all
/// Valgrind tools. Perf-specific variants are internal coordination modes the runner uses to
/// calibrate repetitions, measure perf overhead, or execute a fixed number of repetitions inside
/// the perf fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchRunMode {
    /// Run the benchmark once without perf-specific coordination. Used by all Valgrind tools.
    Default,
    /// Let the harness determine a suitable repetition count to run the perf benchmark
    PerfDynamic,
    /// Run the harness calibration path without collecting the main perf sample.
    PerfCalibrate,
    /// Execute the benchmark the given number of times to measure perf overhead of batched runs.
    PerfOverhead(usize),
    /// Execute the benchmark the given number of times inside the perf fence.
    PerfRepeat(usize),
    /// Execute exactly one benchmark invocation inside the perf fence.
    PerfOnce,
}

/// Identifiers for Cachegrind metrics that can appear in a parsed summary.
///
/// This enum covers both raw Cachegrind events and Gungraun-derived values such as hit rates and
/// estimated cycles.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "runner", derive(EnumIter))]
pub enum CachegrindMetric {
    /// The default event. I cache reads (which equals the number of instructions executed)
    Ir,
    /// D Cache reads (which equals the number of memory reads) (--cache-sim=yes)
    Dr,
    /// D Cache writes (which equals the number of memory writes) (--cache-sim=yes)
    Dw,
    /// I1 cache read misses (--cache-sim=yes)
    I1mr,
    /// D1 cache read misses (--cache-sim=yes)
    D1mr,
    /// D1 cache write misses (--cache-sim=yes)
    D1mw,
    /// LL cache instruction read misses (--cache-sim=yes)
    ILmr,
    /// LL cache data read misses (--cache-sim=yes)
    DLmr,
    /// LL cache data write misses (--cache-sim=yes)
    DLmw,
    /// I1 cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    I1MissRate,
    /// LL/L2 instructions cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    LLiMissRate,
    /// D1 cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    D1MissRate,
    /// LL/L2 data cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    LLdMissRate,
    /// LL/L2 cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    LLMissRate,
    /// Derived event showing the L1 hits (--cache-sim=yes)
    L1hits,
    /// Derived event showing the LL hits (--cache-sim=yes)
    LLhits,
    /// Derived event showing the RAM hits (--cache-sim=yes)
    RamHits,
    /// L1 cache hit rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    L1HitRate,
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    /// LL/L2 cache hit rate (--cache-sim=yes)
    LLHitRate,
    /// RAM hit rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    RamHitRate,
    /// Derived event showing the total amount of cache reads and writes (--cache-sim=yes)
    TotalRW,
    /// Derived event showing estimated CPU cycles (--cache-sim=yes)
    EstimatedCycles,
    /// Conditional branches executed (--branch-sim=yes)
    Bc,
    /// Conditional branches mispredicted (--branch-sim=yes)
    Bcm,
    /// Indirect branches executed (--branch-sim=yes)
    Bi,
    /// Indirect branches mispredicted (--branch-sim=yes)
    Bim,
}

/// A collection of groups of [`CachegrindMetric`]s
///
/// The members of each group are fully documented in the docs of each variant of this enum
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachegrindMetrics {
    /// The default group contains all metrics except the [`CachegrindMetrics::CacheMisses`],
    /// [`CachegrindMetrics::CacheMissRates`], [`CachegrindMetrics::CacheHitRates`] and
    /// [`EventKind::Dr`], [`EventKind::Dw`]. More specifically, the following event kinds and
    /// groups in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetrics, CachegrindMetric};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::Ir.into(),
    ///     CachegrindMetrics::CacheHits,
    ///     CachegrindMetric::TotalRW.into(),
    ///     CachegrindMetric::EstimatedCycles.into(),
    ///     CachegrindMetrics::BranchSim,
    /// ];
    /// ```
    #[default]
    Default,

    /// The `CacheMisses` produced by `--cache-sim=yes` contain the following [`CachegrindMetric`]s
    /// in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::I1mr.into(),
    ///     CachegrindMetric::D1mr.into(),
    ///     CachegrindMetric::D1mw.into(),
    ///     CachegrindMetric::ILmr.into(),
    ///     CachegrindMetric::DLmr.into(),
    ///     CachegrindMetric::DLmw.into(),
    /// ];
    /// ```
    CacheMisses,

    /// The cache miss rates calculated from the [`CallgrindMetrics::CacheMisses`] produced by
    /// `--cache-sim`:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::I1MissRate.into(),
    ///     CachegrindMetric::LLiMissRate.into(),
    ///     CachegrindMetric::D1MissRate.into(),
    ///     CachegrindMetric::LLdMissRate.into(),
    ///     CachegrindMetric::LLMissRate.into(),
    /// ];
    /// ```
    CacheMissRates,

    /// `CacheHits` are gungraun specific and calculated from the metrics produced by
    /// `--cache-sim=yes` in this order:
    ///
    /// ```
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::L1hits.into(),
    ///     CachegrindMetric::LLhits.into(),
    ///     CachegrindMetric::RamHits.into(),
    /// ];
    /// ```
    CacheHits,

    /// The cache hit rates calculated from the [`CachegrindMetrics::CacheHits`]:
    ///
    /// ```
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::L1HitRate.into(),
    ///     CachegrindMetric::LLHitRate.into(),
    ///     CachegrindMetric::RamHitRate.into(),
    /// ];
    /// ```
    CacheHitRates,

    /// All metrics produced by `--cache-sim=yes` including the gungraun specific metrics
    /// [`CachegrindMetric::L1hits`], [`CachegrindMetric::LLhits`], [`CachegrindMetric::RamHits`],
    /// [`CachegrindMetric::TotalRW`], [`CachegrindMetric::EstimatedCycles`],
    /// [`CachegrindMetrics::CacheMissRates`] and [`CachegrindMetrics::CacheHitRates`] in this
    /// order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::Dr.into(),
    ///     CachegrindMetric::Dw.into(),
    ///     CachegrindMetrics::CacheMisses,
    ///     CachegrindMetrics::CacheMissRates,
    ///     CachegrindMetrics::CacheHits,
    ///     CachegrindMetrics::CacheHitRates,
    ///     CachegrindMetric::TotalRW.into(),
    ///     CachegrindMetric::EstimatedCycles.into(),
    /// ];
    /// ```
    CacheSim,

    /// The metrics produced by `--branch-sim=yes` in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::Bc.into(),
    ///     CachegrindMetric::Bcm.into(),
    ///     CachegrindMetric::Bi.into(),
    ///     CachegrindMetric::Bim.into(),
    /// ];
    /// ```
    BranchSim,

    /// All possible [`CachegrindMetric`]s in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// let metrics: Vec<CachegrindMetrics> = vec![
    ///     CachegrindMetric::Ir.into(),
    ///     CachegrindMetrics::CacheSim,
    ///     CachegrindMetrics::BranchSim,
    /// ];
    /// ```
    All,

    /// Selection of no [`CachegrindMetric`] at all
    None,

    /// Specify a single [`CachegrindMetric`].
    ///
    /// Note that [`CachegrindMetric`] implements the necessary traits to convert to the
    /// `CachegrindMetrics::SingleEvent` variant.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CachegrindMetric, CachegrindMetrics};
    /// # }
    /// use gungraun::{CachegrindMetric, CachegrindMetrics};
    ///
    /// assert_eq!(
    ///     CachegrindMetrics::SingleEvent(CachegrindMetric::Ir),
    ///     CachegrindMetric::Ir.into()
    /// );
    /// ```
    SingleEvent(CachegrindMetric),
}

/// A collection of groups of [`EventKind`]s
///
/// `Callgrind` supports a large amount of metrics and their collection can be enabled with various
/// command-line flags. [`CallgrindMetrics`] groups these metrics to make it less cumbersome to
/// specify multiple [`EventKind`]s at once if necessary.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum CallgrindMetrics {
    /// The default group contains all event kinds except the [`CallgrindMetrics::CacheMisses`],
    /// [`CallgrindMetrics::CacheMissRates`], [`CallgrindMetrics::CacheHitRates`] and
    /// [`EventKind::Dr`], [`EventKind::Dw`]. More specifically, the following event kinds and
    /// groups in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::Ir.into(),
    ///     CallgrindMetrics::CacheHits,
    ///     EventKind::TotalRW.into(),
    ///     EventKind::EstimatedCycles.into(),
    ///     CallgrindMetrics::SystemCalls,
    ///     EventKind::Ge.into(),
    ///     CallgrindMetrics::BranchSim,
    ///     CallgrindMetrics::WriteBackBehaviour,
    ///     CallgrindMetrics::CacheUse,
    /// ];
    /// ```
    #[default]
    Default,

    /// The `CacheMisses` produced by `--cache-sim=yes` contain the following [`EventKind`]s in
    /// this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::I1mr.into(),
    ///     EventKind::D1mr.into(),
    ///     EventKind::D1mw.into(),
    ///     EventKind::ILmr.into(),
    ///     EventKind::DLmr.into(),
    ///     EventKind::DLmw.into(),
    /// ];
    /// ```
    CacheMisses,

    /// The cache miss rates calculated from the [`CallgrindMetrics::CacheMisses`] produced by
    /// `--cache-sim`:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::I1MissRate.into(),
    ///     EventKind::D1MissRate.into(),
    ///     EventKind::LLiMissRate.into(),
    ///     EventKind::LLdMissRate.into(),
    ///     EventKind::LLMissRate.into(),
    /// ];
    /// ```
    CacheMissRates,

    /// `CacheHits` are gungraun specific and calculated from the metrics produced by
    /// `--cache-sim=yes` in this order:
    ///
    /// ```
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::L1hits.into(),
    ///     EventKind::LLhits.into(),
    ///     EventKind::RamHits.into(),
    /// ];
    /// ```
    CacheHits,

    /// The cache hit rates calculated from the [`CallgrindMetrics::CacheHits`]:
    ///
    /// ```
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::L1HitRate.into(),
    ///     EventKind::LLHitRate.into(),
    ///     EventKind::RamHitRate.into(),
    /// ];
    /// ```
    CacheHitRates,

    /// All metrics produced by `--cache-sim=yes` including the gungraun specific metrics
    /// [`EventKind::L1hits`], [`EventKind::LLhits`], [`EventKind::RamHits`],
    /// [`EventKind::TotalRW`], [`EventKind::EstimatedCycles`] and miss/hit rates in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::Dr.into(),
    ///     EventKind::Dw.into(),
    ///     CallgrindMetrics::CacheMisses,
    ///     CallgrindMetrics::CacheMissRates,
    ///     CallgrindMetrics::CacheHits,
    ///     EventKind::TotalRW.into(),
    ///     CallgrindMetrics::CacheHitRates,
    ///     EventKind::EstimatedCycles.into(),
    /// ];
    /// ```
    CacheSim,

    /// The metrics produced by `--cacheuse=yes` in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::AcCost1.into(),
    ///     EventKind::AcCost2.into(),
    ///     EventKind::SpLoss1.into(),
    ///     EventKind::SpLoss2.into(),
    /// ];
    /// ```
    CacheUse,

    /// `SystemCalls` are events of the `--collect-systime=yes` option in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::SysCount.into(),
    ///     EventKind::SysTime.into(),
    ///     EventKind::SysCpuTime.into(),
    /// ];
    /// ```
    SystemCalls,

    /// The metrics produced by `--branch-sim=yes` in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::Bc.into(),
    ///     EventKind::Bcm.into(),
    ///     EventKind::Bi.into(),
    ///     EventKind::Bim.into(),
    /// ];
    /// ```
    BranchSim,

    /// All metrics of `--simulate-wb=yes` in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::ILdmr.into(),
    ///     EventKind::DLdmr.into(),
    ///     EventKind::DLdmw.into(),
    /// ];
    /// ```
    WriteBackBehaviour,

    /// All possible [`EventKind`]s in this order:
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// let metrics: Vec<CallgrindMetrics> = vec![
    ///     EventKind::Ir.into(),
    ///     CallgrindMetrics::CacheSim,
    ///     CallgrindMetrics::SystemCalls,
    ///     EventKind::Ge.into(),
    ///     CallgrindMetrics::BranchSim,
    ///     CallgrindMetrics::WriteBackBehaviour,
    ///     CallgrindMetrics::CacheUse,
    /// ];
    /// ```
    All,

    /// Selection of no [`EventKind`] at all
    None,

    /// Specify a single [`EventKind`].
    ///
    /// Note that [`EventKind`] implements the necessary traits to convert to the
    /// `CallgrindMetrics::SingleEvent` variant which is shorter to write.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{CallgrindMetrics, EventKind};
    /// # }
    /// use gungraun::{CallgrindMetrics, EventKind};
    ///
    /// assert_eq!(
    ///     CallgrindMetrics::SingleEvent(EventKind::Ir),
    ///     EventKind::Ir.into()
    /// );
    /// ```
    SingleEvent(EventKind),
}

/// For internal use only: Used to differentiate between the `iter` and other `#[benches]` arguments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandKind {
    /// The default mode when `iter` was not used
    Default(Box<Command>),
    /// The mode when `iter` was used
    Iter(Vec<Command>),
}

/// The kind of `Delay`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelayKind {
    /// Delay the `Command` for a fixed [`Duration`]
    DurationElapse(Duration),
    /// Delay the `Command` until a successful tcp connection can be established
    TcpConnect(SocketAddr),
    /// Delay the `Command` until a successful udp response was received
    UdpResponse(SocketAddr, Vec<u8>),
    /// Delay the `Command` until the specified path exists
    PathExists(PathBuf),
}

/// Identifiers for DHAT metrics that can appear in a parsed summary.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "runner", derive(EnumIter))]
pub enum DhatMetric {
    /// In ad-hoc mode, Total units measured over the entire execution
    TotalUnits,
    /// Total ad-hoc events over the entire execution
    TotalEvents,
    /// Total bytes allocated over the entire execution
    TotalBytes,
    /// Total heap blocks allocated over the entire execution
    TotalBlocks,
    /// The bytes alive at t-gmax, the time when the heap size reached its global maximum
    AtTGmaxBytes,
    /// The blocks alive at t-gmax
    AtTGmaxBlocks,
    /// The amount of bytes at the end of the execution.
    ///
    /// This is the amount of bytes which were not explicitly freed.
    AtTEndBytes,
    /// The amount of blocks at the end of the execution.
    ///
    /// This is the amount of heap blocks which were not explicitly freed.
    AtTEndBlocks,
    /// The amount of bytes read during the entire execution
    ReadsBytes,
    /// The amount of bytes written during the entire execution
    WritesBytes,
    /// The total lifetimes of all heap blocks allocated
    TotalLifetimes,
    /// The maximum amount of bytes
    MaximumBytes,
    /// The maximum amount of heap blocks
    MaximumBlocks,
}

/// A collection of groups of [`DhatMetric`]s
///
/// The members of each group are fully documented in the docs of each variant of this enum
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DhatMetrics {
    /// The default group in this order
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{DhatMetrics, DhatMetric};
    /// # }
    /// use gungraun::{DhatMetric, DhatMetrics};
    ///
    /// let metrics: Vec<DhatMetrics> = vec![
    ///     DhatMetric::TotalUnits.into(),
    ///     DhatMetric::TotalEvents.into(),
    ///     DhatMetric::TotalBytes.into(),
    ///     DhatMetric::TotalBlocks.into(),
    ///     DhatMetric::AtTGmaxBytes.into(),
    ///     DhatMetric::AtTGmaxBlocks.into(),
    ///     DhatMetric::AtTEndBytes.into(),
    ///     DhatMetric::AtTEndBlocks.into(),
    ///     DhatMetric::ReadsBytes.into(),
    ///     DhatMetric::WritesBytes.into(),
    /// ];
    /// ```
    #[default]
    Default,

    /// All [`DhatMetric`]s in this order
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{DhatMetrics, DhatMetric};
    /// # }
    /// use gungraun::{DhatMetric, DhatMetrics};
    ///
    /// let metrics: Vec<DhatMetrics> = vec![
    ///     DhatMetrics::Default,
    ///     DhatMetric::TotalLifetimes.into(),
    ///     DhatMetric::MaximumBytes.into(),
    ///     DhatMetric::MaximumBlocks.into(),
    /// ];
    /// ```
    All,

    /// A single [`DhatMetric`]
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::{DhatMetrics, DhatMetric};
    /// # }
    /// use gungraun::{DhatMetric, DhatMetrics};
    ///
    /// assert_eq!(
    ///     DhatMetrics::SingleMetric(DhatMetric::TotalBytes),
    ///     DhatMetric::TotalBytes.into()
    /// );
    /// ```
    SingleMetric(DhatMetric),
}

/// The `Direction` in which the flamegraph should grow.
///
/// The default is `TopToBottom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Direction {
    /// Grow from top to bottom with the highest event costs at the top
    TopToBottom,
    /// Grow from bottom to top with the highest event costs at the bottom
    #[default]
    BottomToTop,
}

/// The `EntryPoint` of a benchmark
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryPoint {
    /// Disable the entry point
    None,
    /// The default entry point is the benchmark function
    #[default]
    Default,
    /// A custom entry point. The argument allows the same glob patterns as the
    /// [`--toggle-collect`](https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options)
    /// argument of callgrind. These are the wildcards `*` (match any amount of arbitrary
    /// characters) and `?` (match a single arbitrary character)
    Custom(String),
}

/// Identifiers for the error counts reported by error-detecting Valgrind tools.
///
/// These values appear in parsed summaries for `Memcheck`, `Helgrind`, and `DRD`.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "runner", derive(EnumIter))]
pub enum ErrorMetric {
    /// The amount of detected unsuppressed errors
    Errors,
    /// The amount of detected unsuppressed error contexts
    Contexts,
    /// The amount of suppressed errors
    SuppressedErrors,
    /// The amount of suppressed error contexts
    SuppressedContexts,
}

/// Identifiers for Callgrind events that can appear in a parsed summary.
///
/// This enum includes both raw Callgrind events and Gungraun-derived values such as hit rates and
/// aggregate counts.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "runner", derive(EnumIter))]
pub enum EventKind {
    /// The default event. I cache reads (which equals the number of instructions executed)
    Ir,
    /// D Cache reads (which equals the number of memory reads) (--cache-sim=yes)
    Dr,
    /// D Cache writes (which equals the number of memory writes) (--cache-sim=yes)
    Dw,
    /// I1 cache read misses (--cache-sim=yes)
    I1mr,
    /// D1 cache read misses (--cache-sim=yes)
    D1mr,
    /// D1 cache write misses (--cache-sim=yes)
    D1mw,
    /// LL cache instruction read misses (--cache-sim=yes)
    ILmr,
    /// LL cache data read misses (--cache-sim=yes)
    DLmr,
    /// LL cache data write misses (--cache-sim=yes)
    DLmw,
    /// I1 cache miss rate (--cache-sim=yes).
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    I1MissRate,
    /// LL/L2 instructions cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    LLiMissRate,
    /// D1 cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    D1MissRate,
    /// LL/L2 data cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    LLdMissRate,
    /// LL/L2 cache miss rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    LLMissRate,
    /// Derived event showing the L1 hits (--cache-sim=yes)
    L1hits,
    /// Derived event showing the LL hits (--cache-sim=yes)
    LLhits,
    /// Derived event showing the RAM hits (--cache-sim=yes)
    RamHits,
    /// L1 cache hit rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    L1HitRate,
    /// LL/L2 cache hit rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    LLHitRate,
    /// RAM hit rate (--cache-sim=yes)
    #[cfg_attr(
        any(feature = "runner", feature = "summary"),
        doc = "A [`Metric::Float`][crate::metrics::model::Metric]"
    )]
    RamHitRate,
    /// Derived event showing the total amount of cache reads and writes (--cache-sim=yes)
    TotalRW,
    /// Derived event showing estimated CPU cycles (--cache-sim=yes)
    EstimatedCycles,
    /// The number of system calls done (--collect-systime=yes)
    SysCount,
    /// The elapsed time spent in system calls (--collect-systime=yes)
    SysTime,
    /// The cpu time spent during system calls (--collect-systime=nsec)
    SysCpuTime,
    /// The number of global bus events (--collect-bus=yes)
    Ge,
    /// Conditional branches executed (--branch-sim=yes)
    Bc,
    /// Conditional branches mispredicted (--branch-sim=yes)
    Bcm,
    /// Indirect branches executed (--branch-sim=yes)
    Bi,
    /// Indirect branches mispredicted (--branch-sim=yes)
    Bim,
    /// Dirty miss because of instruction read (--simulate-wb=yes)
    ILdmr,
    /// Dirty miss because of data read (--simulate-wb=yes)
    DLdmr,
    /// Dirty miss because of data write (--simulate-wb=yes)
    DLdmw,
    /// Counter showing bad temporal locality for L1 caches (--cachuse=yes)
    AcCost1,
    /// Counter showing bad temporal locality for LL caches (--cachuse=yes)
    AcCost2,
    /// Counter showing bad spatial locality for L1 caches (--cachuse=yes)
    SpLoss1,
    /// Counter showing bad spatial locality for LL caches (--cachuse=yes)
    SpLoss2,
}

/// Set the expected exit status of a binary benchmark
///
/// By default, the benchmarked binary is expected to succeed, but if a benchmark is expected to
/// fail, setting this option is required.
///
/// # Examples
///
/// ```rust,ignore
/// use gungraun::prelude::*;
/// use gungraun::ExitWith;
///
/// # fn main() {
/// main!(
///     config = BinaryBenchmarkConfig::default().exit_with(ExitWith::Code(1)),
///     binary_benchmark_groups = my_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitWith {
    /// Exit with success is similar to `ExitCode(0)`
    Success,
    /// Exit with failure is similar to setting the `ExitCode` to something different from `0`
    /// without having to rely on a specific exit code
    Failure,
    /// The exact exit code the benchmark run is expected to exit with
    Code(i32),
    /// The exact signal code the benchmark run is expected to exit with
    Signal(i32),
    /// One of these signal codes, the benchmark run is expected to exit with
    Signals(Vec<i32>),
}

/// The kind of `Flamegraph` which is going to be constructed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlamegraphKind {
    /// The regular flamegraph for the new callgrind run
    Regular,
    /// A differential flamegraph showing the differences between the new and old callgrind run
    Differential,
    /// All flamegraph kinds that can be constructed (`Regular` and `Differential`). This
    /// is the default.
    All,
    /// Do not produce any flamegraphs
    None,
}

/// A `Limit` which can be either an integer or a float
///
/// Depending on the metric the type of the hard limit is a float or an integer. For example
/// [`EventKind::Ir`] is an integer and [`EventKind::L1HitRate`] is a percentage and therefore a
/// float.
///
/// The type of the metric can be seen in the terminal output of Gungraun: Floats always
/// contain a `.` and integers do not.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Limit {
    /// An integer `Limit`. For example [`EventKind::Ir`]
    Int(u64),
    /// A float `Limit`. For example [`EventKind::L1HitRate`] or [`EventKind::I1MissRate`]
    Float(f64),
}

/// Controls how a `perf` measurement is executed.
///
/// The default is [`Self::Direct`], which is the normal mode and measures a single invocation with
/// no extra setup. Batch modes ([`Self::DynamicBatch`] and [`Self::FixedBatch`]) are experimental
/// and wrap multiple invocations to amortize `perf` startup cost. They are an alternative to the
/// calibration modes. Calibration modes ([`Self::DefaultCalibrate`] and [`Self::Calibrate`]) run a
/// separate overhead-measurement pass first, then subtract the best calibration run from the final
/// result.
///
/// # Examples
///
/// ```rust
/// # pub mod gungraun {
/// # pub use gungraun_runner::api::PerfRunMode;
/// # }
/// use gungraun::PerfRunMode;
///
/// let mode = PerfRunMode::DynamicBatch;
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerfRunMode {
    /// Let Gungraun detect the repetition count dynamically. (experimental)
    ///
    /// The harness estimates the benchmark execution time and chooses a repetition count that
    /// maximizes data points within a time budget while minimizing timer error. The algorithm is
    /// based on the scientific paper <https://arxiv.org/pdf/1608.04295> (battle-tested and used by
    /// the Julia benchmarking ecosystem).
    ///
    /// After calibration, the benchmark runs in a batched pipeline: all `setup` calls execute
    /// first. The setup results are stored, then `work` runs once per setup result (also storing
    /// the results), and finally all `teardown` calls run consuming the stored work result. This
    /// mode is only suitable when grouping setup/work/teardown in batches preserves the benchmark
    /// semantics and memory consumption and possible memory pressure due to the stored setup and
    /// work results are negligible.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::DynamicBatch;
    /// ```
    DynamicBatch,

    /// Run a fixed number of batched benchmark invocations. (experimental)
    ///
    /// Like [`Self::DynamicBatch`], this mode batches `setup`, `work`, and `teardown` calls, but
    /// the repetition count is fixed to the provided value instead of being auto-detected. A fixed
    /// count can be useful when the benchmark has side effects that must run an exact number of
    /// times.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::FixedBatch(10);
    /// ```
    FixedBatch(usize),

    /// Calibrate Gungraun by sampling the benchmark harness overhead, then run the benchmark once.
    ///
    /// Before the main measurement, the runner executes `perf` to measure the overhead introduced
    /// by `perf` and the Gungraun harness. This doesn't run the benchmark itself. perf stops
    /// sampling after a default duration of one second. The first sample is discarded to mitigate
    /// cold-start effects, and the mean calibration metrics are subtracted from the final benchmark
    /// metrics.
    ///
    /// Whether calibration is worthwhile depends on the benchmark: if the overhead is small
    /// relative to the main benchmark run, [`Self::Direct`] is usually sufficient.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::DefaultCalibrate;
    /// ```
    DefaultCalibrate,

    /// Like [`Self::DefaultCalibrate`] but with a custom calibration sampling duration.
    ///
    /// The provided [`Duration`] controls how long the runner samples `perf` overhead before
    /// taking the main measurement. A longer duration collects more samples and may yield a more
    /// stable overhead estimate, at the cost of increased total benchmark time.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use std::time::Duration;
    ///
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::Calibrate(Duration::from_secs(2));
    /// ```
    Calibrate(Duration),

    /// Run `perf` once with a normal single benchmark invocation.
    ///
    /// This is the default mode. It is suitable when the benchmark execution time is long enough
    /// that `perf` benchmark self costs are negligible compared to the main benchmark metrics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::Direct;
    /// ```
    #[default]
    Direct,
}

/// Configure the `Stream` which should be used as pipe in [`Stdin::Setup`]
///
/// The default is [`Pipe::Stdout`]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pipe {
    /// The `Stdout` default `Stream`
    #[default]
    Stdout,
    /// The `Stderr` error `Stream`
    Stderr,
}

/// Rewrite the output to match the configured [entry point][EntryPoint] and frame filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SanitizeOutput {
    /// Leave the output files unchanged.
    No,
    /// Like [`Yes`][SanitizeOutput::Yes], but back up the original file with an `.orig` extension.
    KeepOrig,
    /// Rewrite the output to match the configured [entry point][EntryPoint] and frame filters.
    Yes,
}

/// This is a special `Stdio` for the stdin method of [`Command`]
///
/// Contains all the standard [`Stdio`] options and the [`Stdin::Setup`] option
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stdin {
    /// Using this in [`Command::stdin`] pipes the stream specified with [`Pipe`] of the `setup`
    /// function into the `Stdin` of the [`Command`]. In this case the `setup` and [`Command`] are
    /// executed in parallel instead of sequentially. See [`Command::stdin`] for more details.
    Setup(Pipe),
    #[default]
    /// See [`Stdio::Inherit`]
    Inherit,
    /// See [`Stdio::Null`]
    Null,
    /// See [`Stdio::File`]
    File(PathBuf),
    /// See [`Stdio::Pipe`]
    Pipe,
}

/// Configure the `Stdio` of `Stdin`, `Stdout` and `Stderr`
///
/// Describes what to do with a standard I/O stream for the [`Command`] when passed to the stdin,
/// stdout, and stderr methods of [`Command`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stdio {
    /// The [`Command`]'s `Stream` inherits from the benchmark runner.
    #[default]
    Inherit,
    /// This stream will be ignored. This is the equivalent of attaching the stream to `/dev/null`
    Null,
    /// Redirect the content of a file into this `Stream`. This is equivalent to a redirection in a
    /// shell for example for the `Stdout` of `my-command`: `my-command > some_file`
    File(PathBuf),
    /// A new pipe should be arranged to connect the benchmark runner and the [`Command`]
    Pipe,
}

/// We use this enum only internally in the benchmark runner
#[cfg(feature = "runner")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// The standard input stream of a spawned process.
    Stdin,
    /// The standard error stream of a spawned process.
    Stderr,
    /// The standard output stream of a spawned process.
    Stdout,
}

/// The tool specific flamegraph configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolFlamegraphConfig {
    /// The callgrind configuration
    Callgrind(FlamegraphConfig),
    /// The option for tools which can't create flamegraphs
    None,
}

/// The tool specific metrics to show in the terminal output
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolOutputFormat {
    /// The Callgrind configuration
    Callgrind(Vec<CallgrindMetrics>),
    /// The Cachegrind configuration
    Cachegrind(Vec<CachegrindMetrics>),
    /// The DHAT configuration
    DHAT(Vec<DhatMetric>),
    /// The Memcheck configuration
    Memcheck(Vec<ErrorMetric>),
    /// The Helgrind configuration
    Helgrind(Vec<ErrorMetric>),
    /// The DRD configuration
    DRD(Vec<ErrorMetric>),
    /// If there is no configuration
    None,
}

/// The tool specific regression check configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolRegressionConfig {
    /// The [`CachegrindRegressionConfig`] configuration
    Cachegrind(CachegrindRegressionConfig),
    /// The [`CallgrindRegressionConfig`] configuration
    Callgrind(CallgrindRegressionConfig),
    /// The [`DhatRegressionConfig`] configuration
    Dhat(DhatRegressionConfig),
    /// The [`PerfRegressionConfig`] configuration.
    Perf(PerfRegressionConfig),
    /// The option for tools which don't perform regression checks
    None,
}

/// The Valgrind tools which can be run in a benchmark
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum Tool {
    /// Callgrind: a call-graph generating cache and branch prediction profiler
    /// <https://valgrind.org/docs/manual/cl-manual.html>
    Callgrind,
    /// Cachegrind: a high-precision tracing profiler
    /// <https://valgrind.org/docs/manual/cg-manual.html>
    Cachegrind,
    /// DHAT: a dynamic heap analysis tool
    /// <https://valgrind.org/docs/manual/dh-manual.html>
    DHAT,
    /// Memcheck: a memory error detector
    /// <https://valgrind.org/docs/manual/mc-manual.html>
    Memcheck,
    /// Helgrind: a thread error detector
    /// <https://valgrind.org/docs/manual/hg-manual.html>
    Helgrind,
    /// DRD: a thread error detector
    /// <https://valgrind.org/docs/manual/drd-manual.html>
    DRD,
    /// Massif: a heap profiler
    /// <https://valgrind.org/docs/manual/ms-manual.html>
    Massif,
    /// BBV: an experimental basic block vector generation tool
    /// <https://valgrind.org/docs/manual/bbv-manual.html>
    BBV,
    /// Linux `perf`-based benchmarking and profiling.
    Perf,
}

/// Tool-specific option payloads attached to a [`ToolSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolSpecOptions {
    /// [`PerfSpec`] options for [`Tool::Perf`].
    Perf(PerfSpec),
    /// [`DhatSpec`] options for [`Tool::DHAT`]
    Dhat(DhatSpec),
    /// For tools which don't have special options
    None,
}

/// The model for the `#[binary_benchmark]` attribute or the equivalent from the low level api
///
/// For internal use only
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BinaryBenchmark {
    /// The extracted binary benchmarks
    pub benches: Vec<BinaryBenchmarkBench>,
    /// The configuration at `#[binary_benchmark]` level
    pub config: Option<BinaryBenchmarkConfig>,
}

/// The model for the `#[bench]` attribute or the low level equivalent
///
/// For internal use only
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryBenchmarkBench {
    /// The arguments to the function
    pub args: Option<String>,
    /// The returned [`Command`]
    pub command: CommandKind,
    /// The configuration at `#[bench]` or `#[benches]` level
    pub config: Option<BinaryBenchmarkConfig>,
    /// The consts arguments for the benchmark function as single string
    pub consts_display: Option<String>,
    /// The name of the annotated function
    pub function_name: String,
    /// True if there is a `setup` function
    pub has_setup: bool,
    /// True if there is a `teardown` function
    pub has_teardown: bool,
    /// The `id` of the benchmark as in `#[bench::id]`
    pub id: Option<String>,
}

/// The model for the configuration in binary benchmarks
///
/// This is the configuration which is built from the configuration of the UI and for internal use
/// only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BinaryBenchmarkConfig {
    /// If some, set the working directory of the selected binary benchmark to this path
    pub current_dir: Option<PathBuf>,
    /// The tool to run instead of the default callgrind
    pub default_tool: Option<Tool>,
    /// True if the environment variables should be cleared
    pub env_clear: Option<bool>,
    /// The environment variables to set or pass through to the binary
    pub envs: Vec<(OsString, Option<OsString>)>,
    /// The [`ExitWith`] to set the expected exit code/signal of the benchmarked binary
    pub exit_with: Option<ExitWith>,
    /// The configuration of the output format
    pub output_format: Option<OutputFormat>,
    /// Run the benchmarked binary in a [`Sandbox`] or not
    pub sandbox: Option<Sandbox>,
    /// Run the `setup` function parallel to the benchmarked binary
    pub setup_parallel: Option<bool>,
    /// The valgrind tools to run in addition to the default tool
    pub tool_specs: ToolSpecs,
    /// The tool override at this configuration level
    pub tool_specs_override: Option<ToolSpecs>,
    /// The arguments to pass to all tools
    pub valgrind_args: RawToolArgs,
}

/// The model for the `binary_benchmark_group` macro
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BinaryBenchmarkGroup {
    /// The actual data and the benchmarks of this group
    pub binary_benchmarks: Vec<BinaryBenchmark>,
    /// If true compare the benchmarks in this group
    pub compare_by_id: Option<bool>,
    /// The configuration at this level
    pub config: Option<BinaryBenchmarkConfig>,
    /// True if there is a `setup` function
    pub has_setup: bool,
    /// True if there is a `teardown` function
    pub has_teardown: bool,
    /// The name or id of the `binary_benchmark_group!`
    pub id: String,
    /// The maximum amount of parallelism for this group (0 = no limit, 1 = serial, N >= 2 = limit
    /// to N)
    pub max_parallel: Option<usize>,
}

/// The model for the main! macro
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryBenchmarkGroups {
    /// The command line arguments as we receive them from `cargo bench`
    pub command_line_args: Vec<String>,
    /// The configuration of this level
    pub config: BinaryBenchmarkConfig,
    /// The default tool changed by the `cachegrind` feature
    pub default_tool: Tool,
    /// All groups of this benchmark
    pub groups: Vec<BinaryBenchmarkGroup>,
    /// True if there is a `setup` function
    pub has_setup: bool,
    /// True if there is a `teardown` function
    pub has_teardown: bool,
}

/// The model for the regression check configuration of Cachegrind
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CachegrindRegressionConfig {
    /// True if the benchmarks should fail on the first occurrence of a regression
    pub fail_fast: Option<bool>,
    /// The hard limits
    pub hard_limits: Vec<(CachegrindMetrics, Limit)>,
    /// The soft limits
    pub soft_limits: Vec<(CachegrindMetrics, f64)>,
}

/// The model for the regression check configuration of Callgrind
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CallgrindRegressionConfig {
    /// True if the benchmarks should fail on the first occurrence of a regression
    pub fail_fast: Option<bool>,
    /// The hard limits
    pub hard_limits: Vec<(CallgrindMetrics, Limit)>,
    /// The soft limits
    pub soft_limits: Vec<(CallgrindMetrics, f64)>,
}

/// The model for the command returned by the binary benchmark function
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Command {
    /// The arguments for the executable
    pub args: Vec<OsString>,
    /// The configuration at this level
    pub config: BinaryBenchmarkConfig,
    /// If present the command is delayed as configured in [`Delay`]
    pub delay: Option<Delay>,
    /// The path to the executable
    pub path: PathBuf,
    /// The command's stderr
    pub stderr: Option<Stdio>,
    /// The command's stdin
    pub stdin: Option<Stdin>,
    /// The command's stdout
    pub stdout: Option<Stdio>,
}

/// The delay of the [`Command`]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delay {
    /// The kind of delay
    pub kind: DelayKind,
    /// The polling time to check the delay condition
    pub poll: Option<Duration>,
    /// The timeout for the delay
    pub timeout: Option<Duration>,
}

/// DHAT-specific options attached to a [`ToolSpecOptions::Dhat`] tool specification.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhatSpec {
    /// Wildcard patterns used to match functions in a DHAT program point's call stack.
    pub frames: Option<Vec<String>>,
}

/// The model for the regression check configuration of DHAT
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DhatRegressionConfig {
    /// True if the benchmarks should fail on the first occurrence of a regression
    pub fail_fast: Option<bool>,
    /// The hard limits
    pub hard_limits: Vec<(DhatMetrics, Limit)>,
    /// The soft limits
    pub soft_limits: Vec<(DhatMetrics, f64)>,
}

/// The fixtures to copy into the [`Sandbox`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixtures {
    /// If true, follow symlinks
    pub follow_symlinks: bool,
    /// The path to the fixtures
    pub path: PathBuf,
}

/// The model for the configuration of flamegraphs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FlamegraphConfig {
    /// The direction of the flamegraph. Top to bottom or vice versa
    pub direction: Option<Direction>,
    /// The event kinds for which a flamegraph should be generated
    pub event_kinds: Option<Vec<EventKind>>,
    /// The flamegraph kind
    pub kind: Option<FlamegraphKind>,
    /// The minimum width which should be displayed
    pub min_width: Option<f64>,
    /// If true, negate a differential flamegraph
    pub negate_differential: Option<bool>,
    /// If true, normalize a differential flamegraph
    pub normalize_differential: Option<bool>,
    /// The subtitle to use for the flamegraphs
    pub subtitle: Option<String>,
    /// The title to use for the flamegraphs
    pub title: Option<String>,
}

/// The model for the `#[library_benchmark]` attribute
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LibraryBenchmark {
    /// The extracted benchmarks of the annotated function
    pub benches: Vec<LibraryBenchmarkBench>,
    /// The configuration at this level
    pub config: Option<LibraryBenchmarkConfig>,
}

/// The model for the `#[bench]` attribute in a `#[library_benchmark]`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LibraryBenchmarkBench {
    /// The arguments for the function
    pub args: Option<String>,
    /// The configuration at this level
    pub config: Option<LibraryBenchmarkConfig>,
    /// The consts for the function as a display string
    pub consts_display: Option<String>,
    /// The name of the function
    pub function_name: String,
    /// The id of the attribute as in `#[bench::id]`
    pub id: Option<String>,
    /// The amount of elements in the iterator of the `#[benches::id(iter = ITERATOR)]` if present
    pub iter_count: Option<usize>,
}

/// The model for the configuration in library benchmarks
///
/// This is the configuration which is built from the configuration of the UI and for internal use
/// only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LibraryBenchmarkConfig {
    /// If some, set the working directory of the library benchmark to this path
    pub current_dir: Option<PathBuf>,
    /// The tool to run instead of the default callgrind
    pub default_tool: Option<Tool>,
    /// True if the environment variables should be cleared
    pub env_clear: Option<bool>,
    /// The environment variables to set or pass through to the binary
    pub envs: Vec<(OsString, Option<OsString>)>,
    /// The configuration of the output format
    pub output_format: Option<OutputFormat>,
    /// Run the selected library benchmark in a [`Sandbox`] or not.
    pub sandbox: Option<Sandbox>,
    /// The valgrind tools to run in addition to the default tool
    pub tool_specs: ToolSpecs,
    /// The tool override at this configuration level
    pub tool_specs_override: Option<ToolSpecs>,
    /// The arguments to pass to all tools
    pub valgrind_args: RawToolArgs,
}

/// The model for the `library_benchmark_group` macro
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LibraryBenchmarkGroup {
    /// If true compare the benchmarks in this group
    pub compare_by_id: Option<bool>,
    /// The configuration at this level
    pub config: Option<LibraryBenchmarkConfig>,
    /// True if there is a `setup` function
    pub has_setup: bool,
    /// True if there is a `teardown` function
    pub has_teardown: bool,
    /// The name or id of the `library_benchmark_group!`
    pub id: String,
    /// The actual data and the benchmarks of this group
    pub library_benchmarks: Vec<LibraryBenchmark>,
    /// The maximum amount of parallelism for this group (0 = no limit, 1 = serial, N >= 2 = limit
    /// to N)
    pub max_parallel: Option<usize>,
}

/// The model for the `main` macro
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryBenchmarkGroups {
    /// The command line args as we receive them from `cargo bench`
    pub command_line_args: Vec<String>,
    /// The configuration of this level
    pub config: LibraryBenchmarkConfig,
    /// The default tool changed by the `cachegrind` feature
    pub default_tool: Tool,
    /// All groups of this benchmark
    pub groups: Vec<LibraryBenchmarkGroup>,
    /// True if there is a `setup` function
    pub has_setup: bool,
    /// True if there is a `teardown` function
    pub has_teardown: bool,
}

/// The configuration values for the output format
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputFormat {
    /// Show a grid instead of spaces in the terminal output
    pub show_grid: Option<bool>,
    /// Show intermediate results, for example in benchmarks for multi-threaded applications
    pub show_intermediate: Option<bool>,
    /// Don't show differences within the tolerance margin
    pub tolerance: Option<f64>,
    /// If set, truncate the description
    pub truncate_description: Option<Option<usize>>,
}

/// Identifies a perf metric by the event name emitted in the parsed summary.
///
/// Perf metrics are far too numerous to use an enum as usual with each event being a variant.
#[expect(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct PerfMetric(pub String);

/// The model for the regression check configuration of perf
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PerfRegressionConfig {
    /// The statistical significance threshold used for perf soft-limit checks.
    pub alpha: Option<f64>,
    /// `true` if the benchmarks should fail on the first occurrence of a regression
    pub fail_fast: Option<bool>,
    /// The hard limits (pattern, optional [`Unit`], [`Limit`])
    pub hard_limits: Vec<(String, Option<Unit>, Limit)>,
    /// The soft limits (pattern, limit as percentage)
    pub soft_limits: Vec<(String, f64)>,
}

/// The perf-specific configuration stored in [`ToolSpecOptions::Perf`].
///
/// Each configured event selector becomes a separate perf run, optionally with an additional `perf
/// record` companion run with special `record_args`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PerfSpec {
    /// The statistical significance threshold (p-value) used for perf significance handling.
    pub alpha: Option<f64>,
    /// The perf events to collect, passed through to perf as event selectors.
    ///
    /// When multiple selectors are configured, the runner expands them into separate perf tool
    /// configurations.
    pub events: Option<Vec<String>>,
    /// The minimum percentage of time a PMU counter must be running.
    pub min_pcnt_running: Option<f64>,
    /// Patterns for perf metrics that must not be zero.
    ///
    /// Defaults to [`DEFAULT_PERF_NON_ZERO_METRICS`] when not set.
    ///
    /// [`DEFAULT_PERF_NON_ZERO_METRICS`]: crate::runner::tool::config::DEFAULT_PERF_NON_ZERO_METRICS
    pub non_zero_metrics: Option<Vec<String>>,
    /// Whether to run a companion `perf record` capture in addition to `perf stat`.
    pub record: Option<bool>,
    /// Additional arguments to pass only to the optional `perf record` run.
    pub record_args: RawToolArgs,
    /// How the runner batches benchmark invocations inside each perf measurement.
    pub run_mode: Option<PerfRunMode>,
    /// The timeout used for sampled perf runs.
    ///
    /// Setting this enables sampling mode for `perf stat`.
    pub sample_duration: Option<Duration>,
}

/// The raw arguments to pass to a valgrind tool
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawToolArgs(Vec<String>);

/// The sandbox to run the benchmarks in
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sandbox {
    /// If this sandbox is enabled or not
    pub enabled: Option<bool>,
    /// The fixtures to copy into the sandbox
    pub fixtures: Vec<PathBuf>,
    /// If true follow symlinks when copying the fixtures
    pub follow_symlinks: Option<bool>,
}

/// The tool specification from the gungraun library side
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// If true the tool is run. Ignored for the default tool which always runs
    pub enable: Option<bool>,
    /// The entry point for the tool
    pub entry_point: Option<EntryPoint>,
    /// The configuration for flamegraphs
    pub flamegraph_config: Option<ToolFlamegraphConfig>,
    /// The tool-specific options for the selected [`Tool`].
    pub options: ToolSpecOptions,
    /// The configuration of the output format
    pub output_format: Option<ToolOutputFormat>,
    /// The arguments to pass to the tool
    pub raw_tool_args: RawToolArgs,
    /// The configuration for regression checks of tools which perform regression checks
    pub regression_config: Option<ToolRegressionConfig>,
    /// Whether this tool's output files should be sanitized after parsing.
    pub sanitize_output: Option<SanitizeOutput>,
    /// If true show the logging output of Valgrind (not Gungraun)
    pub show_log: Option<bool>,
    /// The tool this configuration is for
    pub tool: Tool,
}

/// The specifications of all tools to run in addition to the default tool
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolSpecs(pub Vec<ToolSpec>);

impl BenchRunMode {
    /// Encode this run mode into the stable benchmark-run identifier format.
    ///
    /// Counted perf modes use a zero-padded six-digit suffix so identifiers keep a fixed size and
    /// shape.
    pub fn id(&self) -> String {
        match self {
            Self::Default => "d:d:000000".to_owned(),
            Self::PerfDynamic => "p:p:000000".to_owned(),
            Self::PerfCalibrate => "p:c:000000".to_owned(),
            Self::PerfOverhead(x) => format!("p:o:{x:06}"),
            Self::PerfRepeat(x) => format!("p:r:{x:06}"),
            Self::PerfOnce => "p:s:000000".to_owned(),
        }
    }

    /// Decode a [`BenchRunMode`] from a benchmark-run identifier.
    ///
    /// Returns [`None`] if the identifier does not use Gungraun's bench-run prefix format or if a
    /// counted perf mode has an invalid numeric suffix.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            x if x.starts_with("d:d:") => Some(Self::Default),
            x if x.starts_with("p:p:") => Some(Self::PerfDynamic),
            x if x.starts_with("p:c:") => Some(Self::PerfCalibrate),
            x if x.starts_with("p:o:") => {
                let count = x.rsplit_once(':')?.1.parse::<usize>().ok()?;
                Some(Self::PerfOverhead(count))
            }
            x if x.starts_with("p:s:") => Some(Self::PerfOnce),
            x if x.starts_with("p:r:") => {
                let count = x.rsplit_once(':')?.1.parse::<usize>().ok()?;
                Some(Self::PerfRepeat(count))
            }
            _ => None,
        }
    }

    /// Return `true` if a benchmark-run identifier represents the default mode.
    ///
    /// This checks the encoded prefix used by [`BenchRunMode::id`].
    pub fn is_default(id: &str) -> bool {
        id.starts_with("d:d:")
    }

    /// Return `true` if a benchmark-run identifier represents any perf mode.
    ///
    /// This checks the encoded prefix used by [`BenchRunMode::id`].
    pub fn is_perf(id: &str) -> bool {
        id.starts_with("p:")
    }
}

#[cfg(feature = "runner")]
impl BinaryBenchmarkConfig {
    /// Update this configuration with all other configurations in the given order
    #[must_use]
    pub fn update_from_all<'a, T>(mut self, others: T) -> Self
    where
        T: IntoIterator<Item = Option<&'a Self>>,
    {
        for other in others.into_iter().flatten() {
            self.default_tool = update_option(&self.default_tool, &other.default_tool);
            self.env_clear = update_option(&self.env_clear, &other.env_clear);
            self.current_dir = update_option(&self.current_dir, &other.current_dir);
            self.exit_with = update_option(&self.exit_with, &other.exit_with);

            self.valgrind_args
                .extend_ignore_flag(other.valgrind_args.0.iter());

            self.envs.extend_from_slice(&other.envs);

            if let Some(other_tool_specs) = &other.tool_specs_override {
                self.tool_specs = other_tool_specs.clone();
            } else if !other.tool_specs.is_empty() {
                self.tool_specs.update_from_other(&other.tool_specs);
            } else {
                // do nothing
            }

            self.sandbox = update_option(&self.sandbox, &other.sandbox);
            self.setup_parallel = update_option(&self.setup_parallel, &other.setup_parallel);
            self.output_format = update_option(&self.output_format, &other.output_format);
        }
        self
    }

    /// Resolves the environment variables and create key, value pairs out of them.
    ///
    /// This is done especially for pass-through environment variables which have a `None` value at
    /// first.
    pub fn resolve_envs(&self) -> HashMap<OsString, OsString> {
        util::resolve_envs(self.envs.clone())
    }

    /// Collects all environment variables which don't have a `None` value.
    ///
    /// Pass-through variables have a `None` value.
    pub fn collect_envs(&self) -> Vec<(OsString, OsString)> {
        self.envs
            .iter()
            .filter_map(|(key, option)| option.as_ref().map(|value| (key.clone(), value.clone())))
            .collect()
    }
}

impl CachegrindMetric {
    /// Returns `true` if this `EventKind` is a derived event.
    ///
    /// Derived events are calculated from Cachegrind's native event types the same ways as for
    /// callgrind's [`EventKind`]
    ///
    /// * [`CachegrindMetric::L1hits`]
    /// * [`CachegrindMetric::LLhits`]
    /// * [`CachegrindMetric::RamHits`]
    /// * [`CachegrindMetric::TotalRW`]
    /// * [`CachegrindMetric::EstimatedCycles`]
    /// * [`CachegrindMetric::I1MissRate`]
    /// * [`CachegrindMetric::D1MissRate`]
    /// * [`CachegrindMetric::LLiMissRate`]
    /// * [`CachegrindMetric::LLdMissRate`]
    /// * [`CachegrindMetric::LLMissRate`]
    /// * [`CachegrindMetric::L1HitRate`]
    /// * [`CachegrindMetric::LLHitRate`]
    /// * [`CachegrindMetric::RamHitRate`]
    pub fn is_derived(&self) -> bool {
        matches!(
            self,
            Self::L1hits
                | Self::LLhits
                | Self::RamHits
                | Self::TotalRW
                | Self::EstimatedCycles
                | Self::I1MissRate
                | Self::D1MissRate
                | Self::LLiMissRate
                | Self::LLdMissRate
                | Self::LLMissRate
                | Self::L1HitRate
                | Self::LLHitRate
                | Self::RamHitRate
        )
    }

    /// Returns the name of the metric which is the exact name of the enum variant.
    pub fn to_name(&self) -> String {
        format!("{:?}", *self)
    }
}

impl Display for CachegrindMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            key @ (Self::Ir
            | Self::L1hits
            | Self::LLhits
            | Self::RamHits
            | Self::TotalRW
            | Self::EstimatedCycles
            | Self::I1MissRate
            | Self::D1MissRate
            | Self::LLiMissRate
            | Self::LLdMissRate
            | Self::LLMissRate
            | Self::L1HitRate
            | Self::LLHitRate
            | Self::RamHitRate) => write!(f, "{}", EventKind::from(*key)),
            _ => write!(f, "{self:?}"),
        }
    }
}

#[cfg(feature = "runner")]
impl_from_str_metric!(
    CachegrindMetric,
    "Unknown cachegrind metric: '{}'",
    {
        "instructions" | "ir" => Ir,
        "dr" => Dr,
        "dw" => Dw,
        "i1mr" => I1mr,
        "ilmr" => ILmr,
        "d1mr" => D1mr,
        "dlmr" => DLmr,
        "d1mw" => D1mw,
        "dlmw" => DLmw,
        "bc" => Bc,
        "bcm" => Bcm,
        "bi" => Bi,
        "bim" => Bim,
        "l1hits" => L1hits,
        "llhits" => LLhits,
        "ramhits" => RamHits,
        "totalrw" => TotalRW,
        "estimatedcycles" => EstimatedCycles,
        "i1missrate" => I1MissRate,
        "d1missrate" => D1MissRate,
        "llimissrate" => LLiMissRate,
        "lldmissrate" => LLdMissRate,
        "llmissrate" => LLMissRate,
        "l1hitrate" => L1HitRate,
        "llhitrate" => LLHitRate,
        "ramhitrate" => RamHitRate,
    }
);

#[cfg(feature = "runner")]
impl TypeChecker for CachegrindMetric {
    fn is_int(&self) -> bool {
        match self {
            Self::Ir
            | Self::Dr
            | Self::Dw
            | Self::I1mr
            | Self::D1mr
            | Self::D1mw
            | Self::ILmr
            | Self::DLmr
            | Self::DLmw
            | Self::L1hits
            | Self::LLhits
            | Self::RamHits
            | Self::TotalRW
            | Self::EstimatedCycles
            | Self::Bc
            | Self::Bcm
            | Self::Bi
            | Self::Bim => true,
            Self::I1MissRate
            | Self::LLiMissRate
            | Self::D1MissRate
            | Self::LLdMissRate
            | Self::LLMissRate
            | Self::L1HitRate
            | Self::LLHitRate
            | Self::RamHitRate => false,
        }
    }

    fn is_float(&self) -> bool {
        !self.is_int()
    }
}

impl From<CachegrindMetric> for CachegrindMetrics {
    fn from(value: CachegrindMetric) -> Self {
        Self::SingleEvent(value)
    }
}

#[cfg(feature = "runner")]
impl_from_str_metric_groups!(
    CachegrindMetrics,
    CachegrindMetric,
    SingleEvent,
    "Invalid cachegrind metric group: '{}'",
    {
        "default" | "def" => Default,
        "all" => All,
        "cachemisses" | "misses" | "ms" => CacheMisses,
        "cachemissrates" | "missrates" | "mr" => CacheMissRates,
        "cachehits" | "hits" | "hs" => CacheHits,
        "cachehitrates" | "hitrates" | "hr" => CacheHitRates,
        "cachesim" | "cs" => CacheSim,
        "branchsim" | "bs" => BranchSim,
    }
);

impl From<EventKind> for CallgrindMetrics {
    fn from(value: EventKind) -> Self {
        Self::SingleEvent(value)
    }
}

#[cfg(feature = "runner")]
impl_from_str_metric_groups!(
    CallgrindMetrics,
    EventKind,
    SingleEvent,
    "Invalid event group: '{}'",
    {
        "default" | "def" => Default,
        "all" => All,
        "cachemisses" | "misses" | "ms" => CacheMisses,
        "cachemissrates" | "missrates" | "mr" => CacheMissRates,
        "cachehits" | "hits" | "hs" => CacheHits,
        "cachehitrates" | "hitrates" | "hr" => CacheHitRates,
        "cachesim" | "cs" => CacheSim,
        "cacheuse" | "cu" => CacheUse,
        "systemcalls" | "syscalls" | "sc" => SystemCalls,
        "branchsim" | "bs" => BranchSim,
        "writebackbehaviour" | "writeback" | "wb" => WriteBackBehaviour,
    }
);

impl Default for DelayKind {
    fn default() -> Self {
        Self::DurationElapse(Duration::from_secs(60))
    }
}

impl Display for DhatMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TotalUnits => f.write_str("Total units"),
            Self::TotalEvents => f.write_str("Total events"),
            Self::TotalBytes => f.write_str("Total bytes"),
            Self::TotalBlocks => f.write_str("Total blocks"),
            Self::AtTGmaxBytes => f.write_str("At t-gmax bytes"),
            Self::AtTGmaxBlocks => f.write_str("At t-gmax blocks"),
            Self::AtTEndBytes => f.write_str("At t-end bytes"),
            Self::AtTEndBlocks => f.write_str("At t-end blocks"),
            Self::ReadsBytes => f.write_str("Reads bytes"),
            Self::WritesBytes => f.write_str("Writes bytes"),
            Self::TotalLifetimes => f.write_str("Total lifetimes"),
            Self::MaximumBytes => f.write_str("Maximum bytes"),
            Self::MaximumBlocks => f.write_str("Maximum blocks"),
        }
    }
}

#[cfg(feature = "runner")]
impl_from_str_metric!(
    DhatMetric,
    "Unknown dhat metric: '{}'",
    {
        "totalunits" | "tun" => TotalUnits,
        "totalevents" | "tev" => TotalEvents,
        "totalbytes" | "tb" => TotalBytes,
        "totalblocks" | "tbk" => TotalBlocks,
        "attgmaxbytes" | "gb" => AtTGmaxBytes,
        "attgmaxblocks" | "gbk" => AtTGmaxBlocks,
        "attendbytes" | "eb" => AtTEndBytes,
        "attendblocks" | "ebk" => AtTEndBlocks,
        "readsbytes" | "rb" => ReadsBytes,
        "writesbytes" | "wb" => WritesBytes,
        "totallifetimes" | "tl" => TotalLifetimes,
        "maximumbytes" | "mb" => MaximumBytes,
        "maximumblocks" | "mbk" => MaximumBlocks,
    }
);

#[cfg(feature = "runner")]
impl Summarize for DhatMetric {}

#[cfg(feature = "runner")]
impl TypeChecker for DhatMetric {
    fn is_int(&self) -> bool {
        true
    }

    fn is_float(&self) -> bool {
        false
    }
}

impl From<DhatMetric> for DhatMetrics {
    fn from(value: DhatMetric) -> Self {
        Self::SingleMetric(value)
    }
}

#[cfg(feature = "runner")]
impl_from_str_metric_groups!(
    DhatMetrics,
    DhatMetric,
    SingleMetric,
    "Invalid dhat metrics group: '{}'",
    {
        "default" | "def" => Default,
        "all" => All,
    }
);

impl<T> From<T> for EntryPoint
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self::Custom(value.into())
    }
}

impl Display for ErrorMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Errors => f.write_str("Errors"),
            Self::Contexts => f.write_str("Contexts"),
            Self::SuppressedErrors => f.write_str("Suppressed Errors"),
            Self::SuppressedContexts => f.write_str("Suppressed Contexts"),
        }
    }
}

#[cfg(feature = "runner")]
impl_from_str_metric!(
    ErrorMetric,
    "Unknown error metric: '{}'",
    {
        "errors" | "err" => Errors,
        "contexts" | "ctx" => Contexts,
        "suppressederrors" | "serr" => SuppressedErrors,
        "suppressedcontexts" | "sctx" => SuppressedContexts,
    }
);

#[cfg(feature = "runner")]
impl Summarize for ErrorMetric {}

impl EventKind {
    /// Returns `true` if this `EventKind` is a derived event.
    ///
    /// Derived events are calculated from Callgrind's native event types. See also
    /// [`crate::runner::callgrind::model::Metrics::make_summary`]. Currently all derived events
    /// are:
    ///
    /// * [`EventKind::L1hits`]
    /// * [`EventKind::LLhits`]
    /// * [`EventKind::RamHits`]
    /// * [`EventKind::TotalRW`]
    /// * [`EventKind::EstimatedCycles`]
    /// * [`EventKind::I1MissRate`]
    /// * [`EventKind::D1MissRate`]
    /// * [`EventKind::LLiMissRate`]
    /// * [`EventKind::LLdMissRate`]
    /// * [`EventKind::LLMissRate`]
    /// * [`EventKind::L1HitRate`]
    /// * [`EventKind::LLHitRate`]
    /// * [`EventKind::RamHitRate`]
    pub fn is_derived(&self) -> bool {
        matches!(
            self,
            Self::L1hits
                | Self::LLhits
                | Self::RamHits
                | Self::TotalRW
                | Self::EstimatedCycles
                | Self::I1MissRate
                | Self::D1MissRate
                | Self::LLiMissRate
                | Self::LLdMissRate
                | Self::LLMissRate
                | Self::L1HitRate
                | Self::LLHitRate
                | Self::RamHitRate
        )
    }

    /// Returns the name of the metric which is the exact name of the enum variant.
    pub fn to_name(&self) -> String {
        format!("{:?}", *self)
    }
}

impl Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ir => f.write_str("Instructions"),
            Self::L1hits => f.write_str("L1 Hits"),
            Self::LLhits => f.write_str("LL Hits"),
            Self::RamHits => f.write_str("RAM Hits"),
            Self::TotalRW => f.write_str("Total read+write"),
            Self::EstimatedCycles => f.write_str("Estimated Cycles"),
            Self::I1MissRate => f.write_str("I1 Miss Rate"),
            Self::D1MissRate => f.write_str("D1 Miss Rate"),
            Self::LLiMissRate => f.write_str("LLi Miss Rate"),
            Self::LLdMissRate => f.write_str("LLd Miss Rate"),
            Self::LLMissRate => f.write_str("LL Miss Rate"),
            Self::L1HitRate => f.write_str("L1 Hit Rate"),
            Self::LLHitRate => f.write_str("LL Hit Rate"),
            Self::RamHitRate => f.write_str("RAM Hit Rate"),
            _ => write!(f, "{self:?}"),
        }
    }
}

impl From<CachegrindMetric> for EventKind {
    fn from(value: CachegrindMetric) -> Self {
        match value {
            CachegrindMetric::Ir => Self::Ir,
            CachegrindMetric::Dr => Self::Dr,
            CachegrindMetric::Dw => Self::Dw,
            CachegrindMetric::I1mr => Self::I1mr,
            CachegrindMetric::D1mr => Self::D1mr,
            CachegrindMetric::D1mw => Self::D1mw,
            CachegrindMetric::ILmr => Self::ILmr,
            CachegrindMetric::DLmr => Self::DLmr,
            CachegrindMetric::DLmw => Self::DLmw,
            CachegrindMetric::L1hits => Self::L1hits,
            CachegrindMetric::LLhits => Self::LLhits,
            CachegrindMetric::RamHits => Self::RamHits,
            CachegrindMetric::TotalRW => Self::TotalRW,
            CachegrindMetric::EstimatedCycles => Self::EstimatedCycles,
            CachegrindMetric::Bc => Self::Bc,
            CachegrindMetric::Bcm => Self::Bcm,
            CachegrindMetric::Bi => Self::Bi,
            CachegrindMetric::Bim => Self::Bim,
            CachegrindMetric::I1MissRate => Self::I1MissRate,
            CachegrindMetric::D1MissRate => Self::D1MissRate,
            CachegrindMetric::LLiMissRate => Self::LLiMissRate,
            CachegrindMetric::LLdMissRate => Self::LLdMissRate,
            CachegrindMetric::LLMissRate => Self::LLMissRate,
            CachegrindMetric::L1HitRate => Self::L1HitRate,
            CachegrindMetric::LLHitRate => Self::LLHitRate,
            CachegrindMetric::RamHitRate => Self::RamHitRate,
        }
    }
}

#[cfg(feature = "runner")]
impl_from_str_metric!(
    EventKind,
    "Unknown event kind: '{}'",
    {
        "instructions" | "ir" => Ir,
        "dr" => Dr,
        "dw" => Dw,
        "i1mr" => I1mr,
        "d1mr" => D1mr,
        "d1mw" => D1mw,
        "ilmr" => ILmr,
        "dlmr" => DLmr,
        "dlmw" => DLmw,
        "syscount" => SysCount,
        "systime" => SysTime,
        "syscputime" => SysCpuTime,
        "ge" => Ge,
        "bc" => Bc,
        "bcm" => Bcm,
        "bi" => Bi,
        "bim" => Bim,
        "ildmr" => ILdmr,
        "dldmr" => DLdmr,
        "dldmw" => DLdmw,
        "accost1" => AcCost1,
        "accost2" => AcCost2,
        "sploss1" => SpLoss1,
        "sploss2" => SpLoss2,
        "l1hits" => L1hits,
        "llhits" => LLhits,
        "ramhits" => RamHits,
        "totalrw" => TotalRW,
        "estimatedcycles" => EstimatedCycles,
        "i1missrate" => I1MissRate,
        "d1missrate" => D1MissRate,
        "llimissrate" => LLiMissRate,
        "lldmissrate" => LLdMissRate,
        "llmissrate" => LLMissRate,
        "l1hitrate" => L1HitRate,
        "llhitrate" => LLHitRate,
        "ramhitrate" => RamHitRate,
    }
);

#[cfg(feature = "runner")]
impl TypeChecker for EventKind {
    fn is_int(&self) -> bool {
        match self {
            Self::Ir
            | Self::Dr
            | Self::Dw
            | Self::I1mr
            | Self::D1mr
            | Self::D1mw
            | Self::ILmr
            | Self::DLmr
            | Self::DLmw
            | Self::L1hits
            | Self::LLhits
            | Self::RamHits
            | Self::TotalRW
            | Self::EstimatedCycles
            | Self::SysCount
            | Self::SysTime
            | Self::SysCpuTime
            | Self::Ge
            | Self::Bc
            | Self::Bcm
            | Self::Bi
            | Self::Bim
            | Self::ILdmr
            | Self::DLdmr
            | Self::DLdmw
            | Self::AcCost1
            | Self::AcCost2
            | Self::SpLoss1
            | Self::SpLoss2 => true,
            Self::I1MissRate
            | Self::LLiMissRate
            | Self::D1MissRate
            | Self::LLdMissRate
            | Self::LLMissRate
            | Self::L1HitRate
            | Self::LLHitRate
            | Self::RamHitRate => false,
        }
    }

    fn is_float(&self) -> bool {
        !self.is_int()
    }
}

#[cfg(feature = "runner")]
impl From<CachegrindMetrics> for IndexSet<CachegrindMetric> {
    fn from(value: CachegrindMetrics) -> Self {
        let mut metrics = Self::new();
        match value {
            CachegrindMetrics::None => {}
            CachegrindMetrics::All => metrics.extend(CachegrindMetric::iter()),
            CachegrindMetrics::Default => {
                metrics.insert(CachegrindMetric::Ir);
                metrics.extend(Self::from(CachegrindMetrics::CacheHits));
                metrics.extend([CachegrindMetric::TotalRW, CachegrindMetric::EstimatedCycles]);
                metrics.extend(Self::from(CachegrindMetrics::BranchSim));
            }
            CachegrindMetrics::CacheMisses => metrics.extend([
                CachegrindMetric::I1mr,
                CachegrindMetric::D1mr,
                CachegrindMetric::D1mw,
                CachegrindMetric::ILmr,
                CachegrindMetric::DLmr,
                CachegrindMetric::DLmw,
            ]),
            CachegrindMetrics::CacheMissRates => metrics.extend([
                CachegrindMetric::I1MissRate,
                CachegrindMetric::LLiMissRate,
                CachegrindMetric::D1MissRate,
                CachegrindMetric::LLdMissRate,
                CachegrindMetric::LLMissRate,
            ]),
            CachegrindMetrics::CacheHits => {
                metrics.extend([
                    CachegrindMetric::L1hits,
                    CachegrindMetric::LLhits,
                    CachegrindMetric::RamHits,
                ]);
            }
            CachegrindMetrics::CacheHitRates => {
                metrics.extend([
                    CachegrindMetric::L1HitRate,
                    CachegrindMetric::LLHitRate,
                    CachegrindMetric::RamHitRate,
                ]);
            }
            CachegrindMetrics::CacheSim => {
                metrics.extend([CachegrindMetric::Dr, CachegrindMetric::Dw]);
                metrics.extend(Self::from(CachegrindMetrics::CacheMisses));
                metrics.extend(Self::from(CachegrindMetrics::CacheMissRates));
                metrics.extend(Self::from(CachegrindMetrics::CacheHits));
                metrics.extend(Self::from(CachegrindMetrics::CacheHitRates));
                metrics.insert(CachegrindMetric::TotalRW);
                metrics.insert(CachegrindMetric::EstimatedCycles);
            }
            CachegrindMetrics::BranchSim => {
                metrics.extend([
                    CachegrindMetric::Bc,
                    CachegrindMetric::Bcm,
                    CachegrindMetric::Bi,
                    CachegrindMetric::Bim,
                ]);
            }
            CachegrindMetrics::SingleEvent(metric) => {
                metrics.insert(metric);
            }
        }

        metrics
    }
}

#[cfg(feature = "runner")]
impl From<DhatMetrics> for IndexSet<DhatMetric> {
    fn from(value: DhatMetrics) -> Self {
        use DhatMetric::*;
        match value {
            DhatMetrics::All => DhatMetric::iter().collect(),
            DhatMetrics::Default => indexset! {
            TotalUnits,
            TotalEvents,
            TotalBytes,
            TotalBlocks,
            AtTGmaxBytes,
            AtTGmaxBlocks,
            AtTEndBytes,
            AtTEndBlocks,
            ReadsBytes,
            WritesBytes },
            DhatMetrics::SingleMetric(dhat_metric) => indexset! { dhat_metric },
        }
    }
}

#[cfg(feature = "runner")]
impl From<CallgrindMetrics> for IndexSet<EventKind> {
    fn from(value: CallgrindMetrics) -> Self {
        let mut event_kinds = Self::new();
        match value {
            CallgrindMetrics::None => {}
            CallgrindMetrics::All => event_kinds.extend(EventKind::iter()),
            CallgrindMetrics::Default => {
                event_kinds.insert(EventKind::Ir);
                event_kinds.extend(Self::from(CallgrindMetrics::CacheHits));
                event_kinds.extend([EventKind::TotalRW, EventKind::EstimatedCycles]);
                event_kinds.extend(Self::from(CallgrindMetrics::SystemCalls));
                event_kinds.insert(EventKind::Ge);
                event_kinds.extend(Self::from(CallgrindMetrics::BranchSim));
                event_kinds.extend(Self::from(CallgrindMetrics::WriteBackBehaviour));
                event_kinds.extend(Self::from(CallgrindMetrics::CacheUse));
            }
            CallgrindMetrics::CacheMisses => event_kinds.extend([
                EventKind::I1mr,
                EventKind::D1mr,
                EventKind::D1mw,
                EventKind::ILmr,
                EventKind::DLmr,
                EventKind::DLmw,
            ]),
            CallgrindMetrics::CacheMissRates => event_kinds.extend([
                EventKind::I1MissRate,
                EventKind::LLiMissRate,
                EventKind::D1MissRate,
                EventKind::LLdMissRate,
                EventKind::LLMissRate,
            ]),
            CallgrindMetrics::CacheHits => {
                event_kinds.extend([EventKind::L1hits, EventKind::LLhits, EventKind::RamHits]);
            }
            CallgrindMetrics::CacheHitRates => {
                event_kinds.extend([
                    EventKind::L1HitRate,
                    EventKind::LLHitRate,
                    EventKind::RamHitRate,
                ]);
            }
            CallgrindMetrics::CacheSim => {
                event_kinds.extend([EventKind::Dr, EventKind::Dw]);
                event_kinds.extend(Self::from(CallgrindMetrics::CacheMisses));
                event_kinds.extend(Self::from(CallgrindMetrics::CacheMissRates));
                event_kinds.extend(Self::from(CallgrindMetrics::CacheHits));
                event_kinds.extend(Self::from(CallgrindMetrics::CacheHitRates));
                event_kinds.insert(EventKind::TotalRW);
                event_kinds.insert(EventKind::EstimatedCycles);
            }
            CallgrindMetrics::CacheUse => event_kinds.extend([
                EventKind::AcCost1,
                EventKind::AcCost2,
                EventKind::SpLoss1,
                EventKind::SpLoss2,
            ]),
            CallgrindMetrics::SystemCalls => {
                event_kinds.extend([
                    EventKind::SysCount,
                    EventKind::SysTime,
                    EventKind::SysCpuTime,
                ]);
            }
            CallgrindMetrics::BranchSim => {
                event_kinds.extend([EventKind::Bc, EventKind::Bcm, EventKind::Bi, EventKind::Bim]);
            }
            CallgrindMetrics::WriteBackBehaviour => {
                event_kinds.extend([EventKind::ILdmr, EventKind::DLdmr, EventKind::DLdmw]);
            }
            CallgrindMetrics::SingleEvent(event_kind) => {
                event_kinds.insert(event_kind);
            }
        }

        event_kinds
    }
}

#[cfg(feature = "runner")]
impl LibraryBenchmarkConfig {
    /// Update this configuration with all other configurations in the given order
    #[must_use]
    pub fn update_from_all<'a, T>(mut self, others: T) -> Self
    where
        T: IntoIterator<Item = Option<&'a Self>>,
    {
        for other in others.into_iter().flatten() {
            self.default_tool = update_option(&self.default_tool, &other.default_tool);
            self.env_clear = update_option(&self.env_clear, &other.env_clear);

            self.valgrind_args
                .extend_ignore_flag(other.valgrind_args.0.iter());

            self.envs.extend_from_slice(&other.envs);
            if let Some(other_tool_specs) = &other.tool_specs_override {
                self.tool_specs = other_tool_specs.clone();
            } else if !other.tool_specs.is_empty() {
                self.tool_specs.update_from_other(&other.tool_specs);
            } else {
                // do nothing
            }

            self.output_format = update_option(&self.output_format, &other.output_format);
            self.current_dir = update_option(&self.current_dir, &other.current_dir);
            self.sandbox = update_option(&self.sandbox, &other.sandbox);
        }
        self
    }

    /// Resolves the environment variables and create key, value pairs out of them.
    ///
    /// Same as [`BinaryBenchmarkConfig::resolve_envs`]
    pub fn resolve_envs(&self) -> HashMap<OsString, OsString> {
        util::resolve_envs(self.envs.clone())
    }

    /// Collects all environment variables which don't have a `None` value.
    ///
    /// Same as [`BinaryBenchmarkConfig::collect_envs`]
    pub fn collect_envs(&self) -> Vec<(OsString, OsString)> {
        self.envs
            .iter()
            .filter_map(|(key, option)| option.as_ref().map(|value| (key.clone(), value.clone())))
            .collect()
    }
}

#[cfg(feature = "runner")]
impl From<Metric> for Limit {
    fn from(value: Metric) -> Self {
        match value {
            Metric::Int(a) => Self::Int(a),
            Metric::Float(b) => Self::Float(b),
        }
    }
}

impl From<f64> for Limit {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<u64> for Limit {
    fn from(value: u64) -> Self {
        Self::Int(value)
    }
}

#[cfg(feature = "runner")]
impl PerfMetric {
    /// Return the perf event name represented by this metric.
    pub fn name(&self) -> &str {
        &self.0
    }

    /// Return a display-oriented copy of this perf metric name.
    ///
    /// Perf metric names may use `:` as an internal separator and can end with trailing separators.
    /// This replaces `:` with `/` and trims trailing `/` characters for terminal output.
    pub fn display(&self) -> Self {
        let mut display = self.0.clone();

        // SAFETY: `:` and `/` are both single ASCII bytes. Replacing `:` with `/` preserves the
        // string's length and UTF-8 validity, since ASCII bytes are valid single-byte UTF-8 code
        // units.
        let bytes = unsafe { display.as_bytes_mut() };
        for b in bytes.iter_mut() {
            if *b == b':' {
                *b = b'/';
            }
        }

        if let Some(new_len) = display.bytes().rposition(|b| b != b'/').map(|i| i + 1) {
            display.truncate(new_len);
        }

        Self(display)
    }
}

impl Display for PerfMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for PerfMetric {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for PerfMetric {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[cfg(feature = "runner")]
impl Summarize for PerfMetric {}

#[cfg(feature = "runner")]
impl Summarize<AnnotatedMetric<PerfQualities>> for PerfMetric {}

impl RawToolArgs {
    /// Returns a slice of the underlying argument strings
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Creates new `RawToolArgs` for a valgrind tool not prefixing `args` with `--`
    pub fn new<I, T>(args: T) -> Self
    where
        I: Into<String>,
        T: IntoIterator<Item = I>,
    {
        args.into_iter().map(Into::into).collect()
    }

    /// Creates new arguments for a valgrind tool prefixing `args` with `--`
    pub fn new_ignore_flag<I, T>(args: T) -> Self
    where
        I: Into<String>,
        T: IntoIterator<Item = I>,
    {
        Self::from_iter_ignore_flag(args.into_iter().map(Into::into))
    }

    /// Extends the arguments with the contents of an iterator prefixing `args` with `--`
    ///
    /// Arguments starting with `-` are kept unchanged, and all other arguments are prefixed with
    /// `--`.
    pub fn extend_ignore_flag<I, T>(&mut self, args: T)
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.extend(
            args.into_iter()
                .filter(|s| !s.as_ref().is_empty())
                .map(|s| {
                    let string = s.as_ref();
                    if string.starts_with('-') {
                        string.to_owned()
                    } else {
                        format!("--{string}")
                    }
                }),
        );
    }

    /// Creates a new `RawToolArgs` while prefixing arguments with a flag `--`
    ///
    /// Empty arguments are ignored. Arguments starting with `-` are kept unchanged, and all other
    /// arguments are prefixed with `--`.
    pub fn from_iter_ignore_flag<I, T>(iter: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        let mut this = Self::default();
        this.extend_ignore_flag(iter);
        this
    }

    /// Returns `true` if there are no tool arguments.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Appends the arguments of another `RawToolArgs` prefixing arguments with `--`
    pub fn update_ignore_flag(&mut self, other: &Self) {
        self.extend_ignore_flag(other.0.iter());
    }

    /// Appends the arguments of another `RawToolArgs`.
    pub fn update(&mut self, other: &Self) {
        self.extend(other.0.iter());
    }

    /// Prepends the arguments of another `RawToolArgs` prefixing the arguments with `--`
    pub fn prepend_ignore_flag(&mut self, other: &Self) {
        if !other.is_empty() {
            let mut other = other.clone();
            other.update_ignore_flag(self);
            *self = other;
        }
    }

    /// Extends the arguments with the given strings exactly as provided and not prefixing with `--`
    pub fn extend<I, T>(&mut self, args: T)
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.extend(args.into_iter().map(|s| s.as_ref().into()));
    }
}

impl<I> FromIterator<I> for RawToolArgs
where
    I: AsRef<str>,
{
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        let mut this = Self::default();
        this.extend(iter);
        this
    }
}

impl Stdin {
    /// Applies this [`Stdin`] configuration to a [`Command`] for the selected [`Stream`].
    ///
    /// This method configures the given [`Command`] according to this [`Stdin`], using the
    /// [`Stream`] to select which process stream is being configured. When this is
    /// [`Stdin::Setup`], it optionally pipes data from the provided [`Child`] and falls back to
    /// regular stdio handling for unsupported combinations. If `current_dir` is provided,
    /// file-based paths are resolved relative to that directory.
    ///
    /// The behavior varies by variant:
    /// - [`Stdin::Setup(Pipe::Stdout)`][`Stdin::Setup`] or
    ///   [`Stdin::Setup(Pipe::Stderr)`][`Stdin::Setup`]: Pipes the setup process's stdout or stderr
    ///   to this process's stdin
    /// - [`Stdin::Pipe`]: Creates a piped stdin stream
    /// - [`Stdin::Inherit`]: Inherits stdin from the parent process
    /// - [`Stdin::Null`]: Connects stdin to `/dev/null` or equivalent
    /// - [`Stdin::File(path)`][`Stdin::File`]: Reads stdin from the specified file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Setup stream piping is requested but the expected setup stream handle is not available
    /// - Applying the underlying stdio configuration fails (e.g., file cannot be opened)
    ///
    /// # Examples
    ///
    /// Piping setup stdout to benchmark stdin:
    ///
    /// ```no_run
    /// # let mut setup_child = std::process::Command::new("something").spawn().unwrap();
    /// use std::process::Command;
    ///
    /// use gungraun_runner::api::{Pipe, Stdin, Stream};
    ///
    /// let mut command = Command::new("benchmark");
    /// let stdin = Stdin::Setup(Pipe::Stdout);
    /// stdin.apply(&mut command, Stream::Stdin, Some(&mut setup_child), None)?;
    ///
    /// # Ok::<(), String>(())
    /// ```
    ///
    /// Reading stdin from a file:
    ///
    /// ```no_run
    /// use std::path::Path;
    /// use std::process::Command;
    ///
    /// use gungraun_runner::api::{Stdin, Stream};
    ///
    /// let mut command = Command::new("benchmark");
    /// let stdin = Stdin::File("input.txt".into());
    /// stdin.apply(
    ///     &mut command,
    ///     Stream::Stdin,
    ///     None,
    ///     Some(Path::new("/workspace")),
    /// )?;
    /// # Ok::<(), String>(())
    /// ```
    ///
    /// [`Command`]: std::process::Command
    /// [`Child`]: std::process::Child
    /// [`Stdin::Setup`]: crate::api::Stdin::Setup
    /// [`Stdin::Pipe`]: crate::api::Stdin::Pipe
    /// [`Stdin::Inherit`]: crate::api::Stdin::Inherit
    /// [`Stdin::Null`]: crate::api::Stdin::Null
    /// [`Stdin::File`]: crate::api::Stdin::File
    #[cfg(feature = "runner")]
    pub fn apply(
        &self,
        command: &mut StdCommand,
        stream: Stream,
        child: Option<&mut Child>,
        current_dir: Option<&Path>,
    ) -> Result<(), String> {
        match (self, child) {
            (Self::Setup(Pipe::Stdout), Some(child)) => {
                command.stdin(
                    child
                        .stdout
                        .take()
                        .ok_or_else(|| "Error piping setup stdout".to_owned())?,
                );
                Ok(())
            }
            (Self::Setup(Pipe::Stderr), Some(child)) => {
                command.stdin(
                    child
                        .stderr
                        .take()
                        .ok_or_else(|| "Error piping setup stderr".to_owned())?,
                );
                Ok(())
            }
            (Self::Setup(_) | Self::Pipe, _) => Stdio::Pipe.apply(command, stream, current_dir),
            (Self::Inherit, _) => Stdio::Inherit.apply(command, stream, current_dir),
            (Self::Null, _) => Stdio::Null.apply(command, stream, current_dir),
            (Self::File(path), _) => Stdio::File(path.clone()).apply(command, stream, current_dir),
        }
    }
}

impl From<Stdio> for Stdin {
    fn from(value: Stdio) -> Self {
        match value {
            Stdio::Inherit => Self::Inherit,
            Stdio::Null => Self::Null,
            Stdio::File(file) => Self::File(file),
            Stdio::Pipe => Self::Pipe,
        }
    }
}

impl From<PathBuf> for Stdin {
    fn from(value: PathBuf) -> Self {
        Self::File(value)
    }
}

impl From<&PathBuf> for Stdin {
    fn from(value: &PathBuf) -> Self {
        Self::File(value.to_owned())
    }
}

impl From<&Path> for Stdin {
    fn from(value: &Path) -> Self {
        Self::File(value.to_path_buf())
    }
}

impl Stdio {
    /// Applies this stdio configuration to the selected command stream.
    ///
    /// This method configures the given [`Command`] according to this [`Stdio`], using the
    /// [`Stream`] to select which process stream is being configured. For [`Stdio::File`], the
    /// file path is interpreted relative to `current_dir` when provided, otherwise it is used
    /// as-is.
    ///
    /// The behavior varies by variant:
    /// - [`Stdio::Pipe`]: Creates a piped stream for the selected process stream
    /// - [`Stdio::Inherit`]: Inherits the stream from the parent process
    /// - [`Stdio::Null`]: Connects the stream to `/dev/null` or equivalent
    /// - [`Stdio::File(path)`][`Stdio::File`]: Opens or creates the specified file for the stream
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A file cannot be opened for reading (when configuring stdin)
    /// - A file cannot be created for writing (when configuring stdout or stderr)
    ///
    /// # Examples
    ///
    /// Piping stdout to a file:
    ///
    /// ```no_run
    /// use std::path::Path;
    /// use std::process::Command;
    ///
    /// use gungraun_runner::api::{Stdio, Stream};
    ///
    /// let mut command = Command::new("benchmark");
    /// let stdout = Stdio::File("output.txt".into());
    /// stdout.apply(&mut command, Stream::Stdout, Some(Path::new("/workspace")))?;
    /// # Ok::<(), String>(())
    /// ```
    ///
    /// Inheriting stderr from parent:
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// use gungraun_runner::api::{Stdio, Stream};
    ///
    /// let mut command = Command::new("benchmark");
    /// Stdio::Inherit.apply(&mut command, Stream::Stderr, None)?;
    /// # Ok::<(), String>(())
    /// ```
    ///
    /// [`Command`]: std::process::Command
    /// [`Stdio::Pipe`]: crate::api::Stdio::Pipe
    /// [`Stdio::Inherit`]: crate::api::Stdio::Inherit
    /// [`Stdio::Null`]: crate::api::Stdio::Null
    /// [`Stdio::File`]: crate::api::Stdio::File
    #[cfg(feature = "runner")]
    pub fn apply(
        &self,
        command: &mut StdCommand,
        stream: Stream,
        current_dir: Option<&Path>,
    ) -> Result<(), String> {
        let stdio = match self {
            Self::Pipe => StdStdio::piped(),
            Self::Inherit => StdStdio::inherit(),
            Self::Null => StdStdio::null(),
            Self::File(path) => {
                let path = if let Some(current_dir) = current_dir {
                    Cow::Owned(current_dir.join(path))
                } else {
                    Cow::Borrowed(path)
                };
                match stream {
                    Stream::Stdin => {
                        StdStdio::from(File::open(path.as_path()).map_err(|error| {
                            format!(
                                "Failed to open file '{}' in read mode for {stream}: {error}",
                                path.display()
                            )
                        })?)
                    }
                    Stream::Stdout | Stream::Stderr => {
                        StdStdio::from(File::create(path.as_path()).map_err(|error| {
                            format!(
                                "Failed to create file '{}' for {stream}: {error}",
                                path.display()
                            )
                        })?)
                    }
                }
            }
        };

        match stream {
            Stream::Stdin => command.stdin(stdio),
            Stream::Stdout => command.stdout(stdio),
            Stream::Stderr => command.stderr(stdio),
        };

        Ok(())
    }
}

impl From<PathBuf> for Stdio {
    fn from(value: PathBuf) -> Self {
        Self::File(value)
    }
}

impl From<&PathBuf> for Stdio {
    fn from(value: &PathBuf) -> Self {
        Self::File(value.to_owned())
    }
}

impl From<&Path> for Stdio {
    fn from(value: &Path) -> Self {
        Self::File(value.to_path_buf())
    }
}

#[cfg(feature = "runner")]
impl Display for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{self:?}").to_lowercase())
    }
}

impl ToolSpec {
    /// Creates a new `ToolSpec` configuration.
    pub fn new<T>(tool: T) -> Self
    where
        T: Into<Tool>,
    {
        let tool = tool.into();

        Self {
            tool,
            enable: None,
            raw_tool_args: RawToolArgs::default(),
            show_log: None,
            regression_config: None,
            flamegraph_config: None,
            output_format: None,
            options: match tool {
                Tool::DHAT => ToolSpecOptions::Dhat(DhatSpec::default()),
                Tool::Perf => ToolSpecOptions::Perf(PerfSpec::default()),
                _ => ToolSpecOptions::None,
            },
            entry_point: None,
            sanitize_output: None,
        }
    }

    /// Creates a new `ToolSpec` configuration with the given command-line `args`.
    pub fn with_args<I, T, K>(kind: K, args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
        K: Into<Tool>,
    {
        let mut this = Self::new(kind);
        this.raw_tool_args = RawToolArgs::from_iter(args);
        this
    }

    /// Update this tool configuration with another configuration
    pub fn update(&mut self, other: &Self) {
        if self.tool == other.tool {
            self.enable = update_option(&self.enable, &other.enable);
            self.show_log = update_option(&self.show_log, &other.show_log);
            self.regression_config =
                update_option(&self.regression_config, &other.regression_config);
            self.flamegraph_config =
                update_option(&self.flamegraph_config, &other.flamegraph_config);
            self.output_format = update_option(&self.output_format, &other.output_format);
            self.entry_point = update_option(&self.entry_point, &other.entry_point);

            if self.tool == Tool::Perf {
                self.raw_tool_args.update(&other.raw_tool_args);
            } else {
                self.raw_tool_args.update_ignore_flag(&other.raw_tool_args);
            }

            self.sanitize_output = update_option(&self.sanitize_output, &other.sanitize_output);

            match (&mut self.options, &other.options) {
                (ToolSpecOptions::Perf(this), ToolSpecOptions::Perf(other)) => {
                    this.events = update_option(&this.events, &other.events);
                    this.record = update_option(&this.record, &other.record);
                    this.record_args.update(&other.record_args);
                    this.run_mode = update_option(&this.run_mode, &other.run_mode);
                    this.sample_duration =
                        update_option(&this.sample_duration, &other.sample_duration);
                    this.min_pcnt_running =
                        update_option(&this.min_pcnt_running, &other.min_pcnt_running);
                }
                (ToolSpecOptions::Dhat(this), ToolSpecOptions::Dhat(other)) => {
                    this.frames = update_option(&this.frames, &other.frames);
                }
                _ => {}
            }
        }
    }
}

impl ToolSpecs {
    /// Returns `true` if `ToolSpecs` is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Update `ToolSpecs`
    pub fn update(&mut self, other: ToolSpec) {
        if let Some(tool_spec) = self.0.iter_mut().find(|t| t.tool == other.tool) {
            tool_spec.update(&other);
        } else {
            self.0.push(other);
        }
    }

    /// Updates `ToolSpecs` with all [`ToolSpec`]s from an iterator.
    pub fn update_all<T>(&mut self, tool_specs: T)
    where
        T: IntoIterator<Item = ToolSpec>,
    {
        for tool_spec in tool_specs {
            self.update(tool_spec);
        }
    }

    /// Updates `ToolSpecs` with another `ToolSpecs`.
    pub fn update_from_other(&mut self, other: &Self) {
        self.update_all(other.0.iter().cloned());
    }

    /// Searches for the [`ToolSpec`] with `kind` and if present remove it from this `ToolSpecs` and
    /// return it.
    pub fn consume(&mut self, tool: Tool) -> Option<ToolSpec> {
        self.0
            .iter()
            .position(|p| p.tool == tool)
            .map(|position| self.0.remove(position))
    }
}

impl Tool {
    /// Returns the id used by the tool invocation.
    pub fn id(&self) -> String {
        match self {
            Self::DHAT => "dhat".to_owned(),
            Self::Callgrind => "callgrind".to_owned(),
            Self::Memcheck => "memcheck".to_owned(),
            Self::Helgrind => "helgrind".to_owned(),
            Self::DRD => "drd".to_owned(),
            Self::Massif => "massif".to_owned(),
            Self::BBV => "exp-bbv".to_owned(),
            Self::Cachegrind => "cachegrind".to_owned(),
            Self::Perf => "perf".to_owned(),
        }
    }

    /// Returns `true` if this tool has output files in addition to log files.
    pub fn has_output_file(&self) -> bool {
        matches!(
            self,
            Self::Callgrind | Self::DHAT | Self::BBV | Self::Massif | Self::Cachegrind | Self::Perf
        )
    }

    /// Returns `true` if this tool supports xtree memory files.
    pub fn has_xtree_file(&self) -> bool {
        matches!(self, Self::Memcheck | Self::Massif | Self::Helgrind)
    }

    /// Returns `true` if this tool supports xleak files.
    pub fn has_xleak_file(&self) -> bool {
        *self == Self::Memcheck
    }
}

impl Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id())
    }
}

#[cfg(feature = "runner")]
impl FromStr for Tool {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.to_lowercase().as_str())
    }
}

#[cfg(feature = "runner")]
impl TryFrom<&str> for Tool {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "callgrind" => Ok(Self::Callgrind),
            "cachegrind" => Ok(Self::Cachegrind),
            "dhat" => Ok(Self::DHAT),
            "memcheck" => Ok(Self::Memcheck),
            "helgrind" => Ok(Self::Helgrind),
            "drd" => Ok(Self::DRD),
            "massif" => Ok(Self::Massif),
            "exp-bbv" => Ok(Self::BBV),
            "perf" => Ok(Self::Perf),
            v => Err(anyhow!("Unknown tool '{v}'")),
        }
    }
}

/// Updates the value of an [`Option`].
pub fn update_option<T: Clone>(first: &Option<T>, other: &Option<T>) -> Option<T> {
    other.clone().or_else(|| first.clone())
}

#[cfg(test)]
mod tests {
    use indexmap::indexset;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::EventKind::*;
    use super::{CachegrindMetric as Cm, *};

    #[rstest]
    #[case::default("d:d:000000", Some(BenchRunMode::Default))]
    #[case::perf_dynamic("p:p:000000", Some(BenchRunMode::PerfDynamic))]
    #[case::no_number("p:p:", Some(BenchRunMode::PerfDynamic))]
    #[case::perf_calibrate("p:c:000000", Some(BenchRunMode::PerfCalibrate))]
    #[case::perf_overhead_zero("p:o:000000", Some(BenchRunMode::PerfOverhead(0)))]
    #[case::perf_overhead_one("p:o:000001", Some(BenchRunMode::PerfOverhead(1)))]
    #[case::perf_overhead_max("p:o:999999", Some(BenchRunMode::PerfOverhead(999_999)))]
    #[case::perf_repeat_zero("p:r:000000", Some(BenchRunMode::PerfRepeat(0)))]
    #[case::perf_repeat_one("p:r:000001", Some(BenchRunMode::PerfRepeat(1)))]
    #[case::perf_repeat_max("p:r:999999", Some(BenchRunMode::PerfRepeat(999_999)))]
    #[case::perf_once("p:s:000000", Some(BenchRunMode::PerfOnce))]
    #[case::invalid_mode("p:x:000000", None)]
    #[case::not_using_colons("pp000000", None)]
    #[case::missing_first_colon("pp:", None)]
    #[case::missing_last_colon("p:p", None)]
    fn test_bench_run_mode_from_id(#[case] id: &str, #[case] expected: Option<BenchRunMode>) {
        assert_eq!(BenchRunMode::from_id(id), expected);
    }

    #[rstest]
    #[case::default(BenchRunMode::Default)]
    #[case::perf_dynamic(BenchRunMode::PerfDynamic)]
    #[case::perf_calibrate(BenchRunMode::PerfCalibrate)]
    #[case::perf_overhead_zero(BenchRunMode::PerfOverhead(0))]
    #[case::perf_overhead_one(BenchRunMode::PerfOverhead(1))]
    #[case::perf_overhead_max(BenchRunMode::PerfOverhead(999_999))]
    #[case::perf_repeat_zero(BenchRunMode::PerfRepeat(0))]
    #[case::perf_repeat_one(BenchRunMode::PerfRepeat(1))]
    #[case::perf_repeat_max(BenchRunMode::PerfRepeat(999_999))]
    #[case::perf_once(BenchRunMode::PerfOnce)]
    fn test_bench_run_mode_round_trip(#[case] mode: BenchRunMode) {
        assert_eq!(BenchRunMode::from_id(&mode.id()), Some(mode));
    }

    #[test]
    fn test_cachegrind_metric_from_str_ignore_case() {
        for metric in CachegrindMetric::iter() {
            let string = format!("{metric:?}");
            let actual = CachegrindMetric::from_str(&string);
            assert_eq!(actual.unwrap(), metric);
        }
    }

    #[test]
    fn test_event_kind_from_str_ignore_case() {
        for event_kind in EventKind::iter() {
            let string = format!("{event_kind:?}");
            let actual = EventKind::from_str(&string);
            assert_eq!(actual.unwrap(), event_kind);
        }
    }

    #[test]
    fn test_library_benchmark_config_update_from_all_when_default() {
        assert_eq!(
            LibraryBenchmarkConfig::default()
                .update_from_all([Some(&LibraryBenchmarkConfig::default())]),
            LibraryBenchmarkConfig::default()
        );
    }

    #[test]
    fn test_library_benchmark_config_update_from_all_when_no_tools_override() {
        let base = LibraryBenchmarkConfig::default();
        let other = LibraryBenchmarkConfig {
            current_dir: Some(PathBuf::from("/tmp")),
            env_clear: Some(true),
            valgrind_args: RawToolArgs(vec!["--valgrind-arg=yes".to_owned()]),
            envs: vec![(OsString::from("MY_ENV"), Some(OsString::from("value")))],
            tool_specs: ToolSpecs(vec![ToolSpec {
                tool: Tool::DHAT,
                enable: None,
                raw_tool_args: RawToolArgs(vec![]),
                show_log: None,
                regression_config: Some(ToolRegressionConfig::Callgrind(
                    CallgrindRegressionConfig::default(),
                )),
                flamegraph_config: Some(ToolFlamegraphConfig::Callgrind(
                    FlamegraphConfig::default(),
                )),
                entry_point: Some(EntryPoint::default()),
                output_format: Some(ToolOutputFormat::None),
                sanitize_output: Some(SanitizeOutput::KeepOrig),
                options: ToolSpecOptions::Dhat(DhatSpec::default()),
            }]),
            tool_specs_override: None,
            output_format: None,
            default_tool: Some(Tool::BBV),
            sandbox: Some(Sandbox::default()),
        };

        assert_eq!(base.update_from_all([Some(&other.clone())]), other);
    }

    #[test]
    fn test_library_benchmark_config_update_from_all_when_tools_override() {
        let base = LibraryBenchmarkConfig::default();
        let other = LibraryBenchmarkConfig {
            current_dir: Some(PathBuf::from("/tmp")),
            env_clear: Some(true),
            valgrind_args: RawToolArgs(vec!["--valgrind-arg=yes".to_owned()]),
            envs: vec![(OsString::from("MY_ENV"), Some(OsString::from("value")))],
            tool_specs: ToolSpecs(vec![ToolSpec {
                tool: Tool::DHAT,
                enable: None,
                raw_tool_args: RawToolArgs(vec![]),
                show_log: None,
                regression_config: Some(ToolRegressionConfig::Callgrind(
                    CallgrindRegressionConfig::default(),
                )),
                flamegraph_config: Some(ToolFlamegraphConfig::Callgrind(
                    FlamegraphConfig::default(),
                )),
                entry_point: Some(EntryPoint::default()),
                output_format: Some(ToolOutputFormat::None),
                sanitize_output: Some(SanitizeOutput::KeepOrig),
                options: ToolSpecOptions::Dhat(DhatSpec::default()),
            }]),
            tool_specs_override: Some(ToolSpecs(vec![])),
            output_format: Some(OutputFormat::default()),
            default_tool: Some(Tool::BBV),
            sandbox: Some(Sandbox::default()),
        };
        let expected = LibraryBenchmarkConfig {
            tool_specs: other.tool_specs_override.as_ref().unwrap().clone(),
            tool_specs_override: None,
            ..other.clone()
        };

        assert_eq!(base.update_from_all([Some(&other)]), expected);
    }

    #[rstest]
    #[case::env_clear(
        LibraryBenchmarkConfig {
            env_clear: Some(true),
            ..Default::default()
        }
    )]
    fn test_library_benchmark_config_update_from_all_truncate_description(
        #[case] config: LibraryBenchmarkConfig,
    ) {
        let actual = LibraryBenchmarkConfig::default().update_from_all([Some(&config)]);
        assert_eq!(actual, config);
    }

    #[rstest]
    #[case::all_none(None, None, None)]
    #[case::some_and_none(Some(true), None, Some(true))]
    #[case::none_and_some(None, Some(true), Some(true))]
    #[case::some_and_some(Some(false), Some(true), Some(true))]
    #[case::some_and_some_value_does_not_matter(Some(true), Some(false), Some(false))]
    fn test_update_option(
        #[case] first: Option<bool>,
        #[case] other: Option<bool>,
        #[case] expected: Option<bool>,
    ) {
        assert_eq!(update_option(&first, &other), expected);
    }

    #[rstest]
    #[case::empty(vec![], &[], vec![])]
    #[case::empty_base(vec![], &["--a=yes"], vec!["--a=yes"])]
    #[case::no_flags(vec![], &["a=yes"], vec!["--a=yes"])]
    #[case::already_exists_single(vec!["--a=yes"], &["--a=yes"], vec!["--a=yes","--a=yes"])]
    #[case::already_exists_when_multiple(
    vec!["--a=yes", "--b=yes"],
    &["--a=yes"],
    vec!["--a=yes", "--b=yes", "--a=yes"]
)]
    fn test_raw_tool_args_extend_ignore_flags(
        #[case] base: Vec<&str>,
        #[case] data: &[&str],
        #[case] expected: Vec<&str>,
    ) {
        let mut base = RawToolArgs(base.iter().map(std::string::ToString::to_string).collect());
        base.extend_ignore_flag(data.iter().map(std::string::ToString::to_string));

        assert_eq!(base.0.into_iter().collect::<Vec<String>>(), expected);
    }

    #[rstest]
    #[case::none(CallgrindMetrics::None, indexset![])]
    #[case::all(CallgrindMetrics::All, indexset![Ir, Dr, Dw, I1mr, D1mr, D1mw, ILmr, DLmr,
        DLmw, I1MissRate, LLiMissRate, D1MissRate, LLdMissRate, LLMissRate, L1hits, LLhits, RamHits,
        TotalRW, L1HitRate, LLHitRate, RamHitRate, EstimatedCycles, SysCount, SysTime, SysCpuTime,
        Ge, Bc, Bcm, Bi, Bim, ILdmr, DLdmr, DLdmw, AcCost1, AcCost2, SpLoss1, SpLoss2]
    )]
    #[case::default(CallgrindMetrics::Default, indexset![Ir, L1hits, LLhits, RamHits, TotalRW,
        EstimatedCycles, SysCount, SysTime, SysCpuTime, Ge, Bc,
        Bcm, Bi, Bim, ILdmr, DLdmr, DLdmw, AcCost1, AcCost2, SpLoss1, SpLoss2]
    )]
    #[case::cache_misses(CallgrindMetrics::CacheMisses, indexset![I1mr, D1mr, D1mw, ILmr,
        DLmr, DLmw]
    )]
    #[case::cache_miss_rates(CallgrindMetrics::CacheMissRates, indexset![I1MissRate,
        D1MissRate, LLMissRate, LLiMissRate, LLdMissRate]
    )]
    #[case::cache_hits(CallgrindMetrics::CacheHits, indexset![L1hits, LLhits, RamHits])]
    #[case::cache_hit_rates(CallgrindMetrics::CacheHitRates, indexset![
        L1HitRate, LLHitRate, RamHitRate
    ])]
    #[case::cache_sim(CallgrindMetrics::CacheSim, indexset![Dr, Dw, I1mr, D1mr, D1mw, ILmr, DLmr,
        DLmw, I1MissRate, LLiMissRate, D1MissRate, LLdMissRate, LLMissRate, L1hits, LLhits, RamHits,
        TotalRW, L1HitRate, LLHitRate, RamHitRate, EstimatedCycles]
    )]
    #[case::cache_use(CallgrindMetrics::CacheUse, indexset![AcCost1, AcCost2, SpLoss1, SpLoss2])]
    #[case::system_calls(CallgrindMetrics::SystemCalls, indexset![SysCount, SysTime, SysCpuTime])]
    #[case::branch_sim(CallgrindMetrics::BranchSim, indexset![Bc, Bcm, Bi, Bim])]
    #[case::write_back(CallgrindMetrics::WriteBackBehaviour, indexset![ILdmr, DLdmr, DLdmw])]
    #[case::single_event(CallgrindMetrics::SingleEvent(Ir), indexset![Ir])]
    fn test_callgrind_metrics_into_index_set(
        #[case] callgrind_metrics: CallgrindMetrics,
        #[case] expected_metrics: IndexSet<EventKind>,
    ) {
        assert_eq!(IndexSet::from(callgrind_metrics), expected_metrics);
    }

    #[rstest]
    #[case::none(CachegrindMetrics::None, indexset![])]
    #[case::all(CachegrindMetrics::All, indexset![Cm::Ir, Cm::Dr, Cm::Dw, Cm::I1mr, Cm::D1mr,
        Cm::D1mw, Cm::ILmr, Cm::DLmr, Cm::DLmw, Cm::I1MissRate, Cm::LLiMissRate, Cm::D1MissRate,
        Cm::LLdMissRate, Cm::LLMissRate, Cm::L1hits, Cm::LLhits, Cm::RamHits, Cm::TotalRW,
        Cm::L1HitRate, Cm::LLHitRate, Cm::RamHitRate, Cm::EstimatedCycles, Cm::Bc, Cm::Bcm, Cm::Bi,
        Cm::Bim,
    ])]
    #[case::default(CachegrindMetrics::Default, indexset![Cm::Ir, Cm::L1hits, Cm::LLhits,
        Cm::RamHits, Cm::TotalRW, Cm::EstimatedCycles, Cm::Bc, Cm::Bcm, Cm::Bi, Cm::Bim
    ])]
    #[case::cache_misses(CachegrindMetrics::CacheMisses, indexset![Cm::I1mr, Cm::D1mr, Cm::D1mw,
        Cm::ILmr, Cm::DLmr, Cm::DLmw
    ])]
    #[case::cache_miss_rates(CachegrindMetrics::CacheMissRates, indexset![Cm::I1MissRate,
        Cm::D1MissRate, Cm::LLMissRate, Cm::LLiMissRate, Cm::LLdMissRate
    ])]
    #[case::cache_hits(CachegrindMetrics::CacheHits, indexset![
        Cm::L1hits, Cm::LLhits, Cm::RamHits
    ])]
    #[case::cache_hit_rates(CachegrindMetrics::CacheHitRates, indexset![
        Cm::L1HitRate, Cm::LLHitRate, Cm::RamHitRate
    ])]
    #[case::cache_sim(CachegrindMetrics::CacheSim, indexset![Cm::Dr, Cm::Dw, Cm::I1mr, Cm::D1mr,
        Cm::D1mw, Cm::ILmr, Cm::DLmr, Cm::DLmw, Cm::I1MissRate, Cm::LLiMissRate, Cm::D1MissRate,
        Cm::LLdMissRate, Cm::LLMissRate, Cm::L1hits, Cm::LLhits, Cm::RamHits, Cm::TotalRW,
        Cm::L1HitRate, Cm::LLHitRate, Cm::RamHitRate, Cm::EstimatedCycles
    ])]
    #[case::branch_sim(CachegrindMetrics::BranchSim, indexset![
        Cm::Bc, Cm::Bcm, Cm::Bi, Cm::Bim
    ])]
    #[case::single_event(CachegrindMetrics::SingleEvent(Cm::Ir), indexset![Cm::Ir])]
    fn test_cachegrind_metrics_into_index_set(
        #[case] cachegrind_metrics: CachegrindMetrics,
        #[case] expected_metrics: IndexSet<CachegrindMetric>,
    ) {
        assert_eq!(IndexSet::from(cachegrind_metrics), expected_metrics);
    }

    #[rstest]
    #[case::empty(&[], &[], &[])]
    #[case::prepend_empty(&["--some"], &[], &["--some"])]
    #[case::initial_empty(&[], &["--some"], &["--some"])]
    #[case::both_same_arg(&["--some"], &["--some"], &["--some", "--some"])]
    #[case::both_different_arg(&["--some"], &["--other"], &["--other", "--some"])]
    fn test_raw_tool_args_prepend(
        #[case] raw_args: &[&str],
        #[case] other: &[&str],
        #[case] expected: &[&str],
    ) {
        let mut raw_args = RawToolArgs::new_ignore_flag(raw_args.iter().map(ToOwned::to_owned));
        let other = RawToolArgs::new_ignore_flag(other.iter().map(ToOwned::to_owned));
        let expected = RawToolArgs::new_ignore_flag(expected.iter().map(ToOwned::to_owned));

        raw_args.prepend_ignore_flag(&other);
        assert_eq!(raw_args, expected);
    }

    #[test]
    fn test_tool_update_when_tools_match() {
        let mut base = ToolSpec::new(Tool::Callgrind);
        let other = ToolSpec {
            tool: Tool::Callgrind,
            enable: Some(true),
            raw_tool_args: RawToolArgs::new_ignore_flag(["--some"]),
            show_log: Some(false),
            regression_config: Some(ToolRegressionConfig::None),
            flamegraph_config: Some(ToolFlamegraphConfig::None),
            output_format: Some(ToolOutputFormat::None),
            entry_point: Some(EntryPoint::Default),
            sanitize_output: Some(SanitizeOutput::KeepOrig),
            options: ToolSpecOptions::None,
        };
        let expected = other.clone();
        base.update(&other);
        assert_eq!(base, expected);
    }

    #[test]
    fn test_tool_update_when_tool_is_perf_then_extends_without_flag() {
        let mut base = ToolSpec::new(Tool::Perf);
        let other = ToolSpec {
            tool: Tool::Perf,
            enable: Some(true),
            // This should stay `some` instead of becoming `--some` as it is done for valgrind tools
            raw_tool_args: RawToolArgs::new_ignore_flag(["some"]),
            show_log: Some(false),
            regression_config: Some(ToolRegressionConfig::None),
            flamegraph_config: Some(ToolFlamegraphConfig::None),
            output_format: Some(ToolOutputFormat::None),
            entry_point: Some(EntryPoint::Default),
            sanitize_output: Some(SanitizeOutput::KeepOrig),
            options: ToolSpecOptions::Perf(PerfSpec::default()),
        };
        let expected = other.clone();
        base.update(&other);
        assert_eq!(base, expected);
    }

    #[test]
    fn test_tool_update_when_tools_not_match() {
        let mut base = ToolSpec::new(Tool::Callgrind);
        let other = ToolSpec {
            tool: Tool::DRD,
            enable: Some(true),
            raw_tool_args: RawToolArgs::new_ignore_flag(["--some"]),
            show_log: Some(false),
            regression_config: Some(ToolRegressionConfig::None),
            flamegraph_config: Some(ToolFlamegraphConfig::None),
            output_format: Some(ToolOutputFormat::None),
            entry_point: Some(EntryPoint::Default),
            sanitize_output: Some(SanitizeOutput::KeepOrig),
            options: ToolSpecOptions::None,
        };

        let expected = base.clone();
        base.update(&other);

        assert_eq!(base, expected);
    }
}
