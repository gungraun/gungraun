//! This module includes all the structs to model the Callgrind output

use std::borrow::Cow;
use std::hash::Hash;
use std::str::FromStr;

use anyhow::{Result, anyhow};
use indexmap::{IndexMap, indexmap};
use serde::{Deserialize, Serialize};

use super::CacheSummary;
use crate::api::EventKind;
use crate::metrics::logic::Summarize;
use crate::metrics::model::Metric;

/// The Callgrind-specific `Metrics`
pub type Metrics = crate::metrics::model::Metrics<EventKind>;

/// The [`Positions`] type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionType {
    /// The address of an instruction
    Instr,
    /// The line number
    Line,
}

/// The call relationship among functions
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calls {
    /// The call count
    amount: u64,
    /// The target [`Positions`]
    positions: Positions,
}

/// The positions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Positions(pub IndexMap<PositionType, u64>);

impl Calls {
    /// Creates new `Calls` struct.
    pub fn from<I, T>(mut iter: T, mut positions: Positions) -> Self
    where
        I: AsRef<str>,
        T: Iterator<Item = I>,
    {
        let amount = iter.next().unwrap().as_ref().parse().unwrap();
        positions.set_iter_str(iter);
        Self { amount, positions }
    }
}

impl Summarize for EventKind {
    fn summarize(costs: &mut Cow<Metrics>) {
        if !costs.is_summarized() {
            let _ = costs.to_mut().make_summary();
        }
    }
}

impl Metrics {
    /// Calculate and add derived summary events (i.e. estimated cycles) in-place
    ///
    /// Additional calls to this function will overwrite the metrics for derived summary events.
    ///
    /// # Errors
    ///
    /// If the necessary cache simulation events (when running Callgrind with --cache-sim) were not
    /// present.
    pub fn make_summary(&mut self) -> Result<()> {
        let CacheSummary {
            l1_hits,
            l3_hits,
            ram_hits,
            total_memory_rw,
            cycles,
            i1_miss_rate,
            d1_miss_rate,
            ll_miss_rate,
            lli_miss_rate,
            lld_miss_rate,
            l1_hit_rate,
            l3_hit_rate,
            ram_hit_rate,
        } = (&*self).try_into()?;

        self.insert(EventKind::L1hits, l1_hits);
        self.insert(EventKind::LLhits, l3_hits);
        self.insert(EventKind::RamHits, ram_hits);
        self.insert(EventKind::TotalRW, total_memory_rw);
        self.insert(EventKind::EstimatedCycles, cycles);
        self.insert(EventKind::I1MissRate, i1_miss_rate);
        self.insert(EventKind::D1MissRate, d1_miss_rate);
        self.insert(EventKind::LLiMissRate, lli_miss_rate);
        self.insert(EventKind::LLdMissRate, lld_miss_rate);
        self.insert(EventKind::LLMissRate, ll_miss_rate);
        self.insert(EventKind::L1HitRate, l1_hit_rate);
        self.insert(EventKind::LLHitRate, l3_hit_rate);
        self.insert(EventKind::RamHitRate, ram_hit_rate);

        Ok(())
    }

    /// Returns `true` if costs are already summarized.
    ///
    /// This method just probes for [`EventKind::EstimatedCycles`] to detect the summarized state.
    pub fn is_summarized(&self) -> bool {
        self.metric_by_kind(&EventKind::EstimatedCycles).is_some()
    }

    /// Returns `true` if costs can be summarized.
    ///
    /// This method probes for [`EventKind::I1mr`] which is present if Callgrind was run with the
    /// cache simulation (`--cache-sim=yes`) enabled.
    pub fn can_summarize(&self) -> bool {
        self.metric_by_kind(&EventKind::I1mr).is_some()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self(indexmap! {EventKind::Ir => Metric::Int(0)})
    }
}

impl FromStr for PositionType {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        // "addr" is taken from the callgrind_annotate script although not officially documented
        match value.to_lowercase().as_str() {
            "instr" | "addr" => Ok(Self::Instr),
            "line" => Ok(Self::Line),
            _ => Err(anyhow!("Unknown positions type: '{value}")),
        }
    }
}

impl Positions {
    /// Creates a new `Positions` from the content of an iterator.
    pub fn try_from_iter_str<'a, I>(iter: I) -> Result<Self>
    where
        I: Iterator<Item = &'a str>,
    {
        iter.map(|s| s.parse::<PositionType>().map(|p| (p, 0)))
            .collect::<Result<IndexMap<_, _>>>()
            .map(Positions)
    }

    /// Set the positions from the contents of an iterator
    pub fn set_iter_str<I, T>(&mut self, iter: T)
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        for ((_, old), pos) in self.0.iter_mut().zip(iter) {
            let pos = pos.as_ref();
            *old = if let Some(hex) = pos.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).unwrap()
            } else {
                pos.parse::<u64>().unwrap()
            }
        }
    }

    /// Returns the length of the positions.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if positions is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for Positions {
    fn default() -> Self {
        Self(indexmap! {PositionType::Line => 0})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Not testing here if the numbers make sense. Just if all metrics are present in the correct
    // order
    #[test]
    fn test_metrics_make_summary_when_cache_sim() {
        use EventKind::*;

        let mut expected = Metrics::with_metric_kinds([
            (Ir, 1),
            (Dr, 2),
            (Dw, 3),
            (I1mr, 4),
            (D1mr, 5),
            (D1mw, 6),
            (ILmr, 7),
            (DLmr, 8),
            (DLmw, 9),
            (L1hits, 0),
            (LLhits, 0),
            (RamHits, 24),
            (TotalRW, 6),
            (EstimatedCycles, 840),
        ]);

        expected.insert_all(&[
            (I1MissRate, Metric::Float(400.0f64)),
            (D1MissRate, Metric::Float(220.000_000_000_000_03_f64)),
            (LLiMissRate, Metric::Float(700.0f64)),
            (LLdMissRate, Metric::Float(340.0f64)),
            (LLMissRate, Metric::Float(400.0f64)),
            (L1HitRate, Metric::Float(0.0f64)),
            (LLHitRate, Metric::Float(0.0f64)),
            (RamHitRate, Metric::Float(400.0f64)),
        ]);

        let mut metrics = Metrics::with_metric_kinds([
            (Ir, 1),
            (Dr, 2),
            (Dw, 3),
            (I1mr, 4),
            (D1mr, 5),
            (D1mw, 6),
            (ILmr, 7),
            (DLmr, 8),
            (DLmw, 9),
        ]);

        metrics.make_summary().unwrap();

        assert_eq!(metrics, expected);
    }
}
