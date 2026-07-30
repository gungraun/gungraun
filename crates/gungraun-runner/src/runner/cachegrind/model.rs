//! This module includes all the structs to model the Cachegrind output
use std::borrow::Cow;

use anyhow::Result;
use indexmap::indexmap;

use crate::api::CachegrindMetric;
use crate::metrics::logic::Summarize;
use crate::metrics::model::Metric;
use crate::runner::callgrind::{CacheSummary, CyclesEstimator};

/// The Cachegrind-specific `Metrics`
pub type Metrics = crate::metrics::model::Metrics<CachegrindMetric>;

impl TryFrom<&Metrics> for CacheSummary {
    type Error = anyhow::Error;

    fn try_from(value: &Metrics) -> std::result::Result<Self, Self::Error> {
        use CachegrindMetric::*;
        let estimator = CyclesEstimator::new(
            value.try_metric_by_kind(&Ir)?,
            value.try_metric_by_kind(&Dr)?,
            value.try_metric_by_kind(&Dw)?,
            value.try_metric_by_kind(&I1mr)?,
            value.try_metric_by_kind(&D1mr)?,
            value.try_metric_by_kind(&D1mw)?,
            value.try_metric_by_kind(&ILmr)?,
            value.try_metric_by_kind(&DLmr)?,
            value.try_metric_by_kind(&DLmw)?,
        );

        Ok(estimator.calculate())
    }
}

impl Summarize for CachegrindMetric {
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
    /// If the necessary cache simulation events (when running Cachegrind with --cache-sim) were not
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

        self.insert(CachegrindMetric::L1hits, l1_hits);
        self.insert(CachegrindMetric::LLhits, l3_hits);
        self.insert(CachegrindMetric::RamHits, ram_hits);
        self.insert(CachegrindMetric::TotalRW, total_memory_rw);
        self.insert(CachegrindMetric::EstimatedCycles, cycles);
        self.insert(CachegrindMetric::I1MissRate, i1_miss_rate);
        self.insert(CachegrindMetric::D1MissRate, d1_miss_rate);
        self.insert(CachegrindMetric::LLiMissRate, lli_miss_rate);
        self.insert(CachegrindMetric::LLdMissRate, lld_miss_rate);
        self.insert(CachegrindMetric::LLMissRate, ll_miss_rate);
        self.insert(CachegrindMetric::L1HitRate, l1_hit_rate);
        self.insert(CachegrindMetric::LLHitRate, l3_hit_rate);
        self.insert(CachegrindMetric::RamHitRate, ram_hit_rate);

        Ok(())
    }

    /// Returns `true` if costs are already summarized.
    ///
    /// This method just probes for [`crate::api::EventKind::EstimatedCycles`] to detect the
    /// summarized state.
    pub fn is_summarized(&self) -> bool {
        self.metric_by_kind(&CachegrindMetric::EstimatedCycles)
            .is_some()
    }

    /// Returns `true` if costs can be summarized.
    ///
    /// This method probes for [`crate::api::EventKind::I1mr`] which is present if Cachegrind was
    /// run with the cache simulation (`--cache-sim=yes`) enabled.
    pub fn can_summarize(&self) -> bool {
        self.metric_by_kind(&CachegrindMetric::I1mr).is_some()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self(indexmap! {CachegrindMetric::Ir => Metric::Int(0)})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Not testing here if the numbers make sense. Just if all metrics are present in the correct
    // order
    #[test]
    fn test_metrics_make_summary_when_cache_sim() {
        use CachegrindMetric::*;

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
