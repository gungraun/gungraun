//! The main module for callgrind related elements

pub mod args;
pub mod flamegraph;
pub mod flamegraph_parser;
pub mod hashmap_parser;
pub mod model;
pub mod parser;
pub mod regression;
pub mod summary_parser;

use self::model::Metrics;
use crate::api::EventKind;
use crate::metrics::model::Metric;

/// The derived metrics of the cache metrics
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheSummary {
    /// The estimated cycles
    pub cycles: Metric,
    /// The L1 data miss rate
    pub d1_miss_rate: Metric,
    /// The L1 instruction fetch miss rate
    pub i1_miss_rate: Metric,
    /// The L1 hit rate
    pub l1_hit_rate: Metric,
    /// The L1 Hits
    pub l1_hits: Metric,
    /// The last-level hit rate
    pub l3_hit_rate: Metric,
    /// The last-level Hits
    pub l3_hits: Metric,
    /// The last-level miss rate
    pub ll_miss_rate: Metric,
    /// The last-level data miss rate
    pub lld_miss_rate: Metric,
    /// The last-level instruction fetch miss rate
    pub lli_miss_rate: Metric,
    /// The RAM hit rate
    pub ram_hit_rate: Metric,
    /// The RAM Hits
    pub ram_hits: Metric,
    /// The total cache reads and writes
    pub total_memory_rw: Metric,
}

/// The calculator for the estimated cycles and other derived metrics
#[derive(Debug, Clone)]
pub struct CyclesEstimator {
    instructions: Metric,
    l1_data_cache_read_misses: Metric,
    l1_data_cache_write_misses: Metric,
    l1_instructions_cache_read_misses: Metric,
    l3_data_cache_read_misses: Metric,
    l3_data_cache_write_misses: Metric,
    l3_instructions_cache_read_misses: Metric,
    total_data_cache_reads: Metric,
    total_data_cache_writes: Metric,
}

impl TryFrom<&Metrics> for CacheSummary {
    type Error = anyhow::Error;

    fn try_from(value: &Metrics) -> std::result::Result<Self, Self::Error> {
        use EventKind::*;
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

impl CyclesEstimator {
    /// Creates a new `CyclesEstimator`.
    pub fn new(
        instructions: Metric,
        total_data_cache_reads: Metric,
        total_data_cache_writes: Metric,
        l1_instructions_cache_read_misses: Metric,
        l1_data_cache_read_misses: Metric,
        l1_data_cache_write_misses: Metric,
        l3_instructions_cache_read_misses: Metric,
        l3_data_cache_read_misses: Metric,
        l3_data_cache_write_misses: Metric,
    ) -> Self {
        Self {
            instructions,
            l1_data_cache_read_misses,
            l1_data_cache_write_misses,
            l1_instructions_cache_read_misses,
            l3_data_cache_read_misses,
            l3_data_cache_write_misses,
            l3_instructions_cache_read_misses,
            total_data_cache_reads,
            total_data_cache_writes,
        }
    }

    /// Calculate the `CacheSummary` from the native metrics
    #[expect(clippy::similar_names)]
    pub fn calculate(&self) -> CacheSummary {
        let ram_hits = self.l3_instructions_cache_read_misses
            + self.l3_data_cache_read_misses
            + self.l3_data_cache_write_misses;
        let l1_data_accesses = self.l1_data_cache_read_misses + self.l1_data_cache_write_misses;
        let l1_miss = self.l1_instructions_cache_read_misses + l1_data_accesses;
        let l3_accesses = l1_miss;
        let l3_hits = l3_accesses - ram_hits;

        let d_refs = self.total_data_cache_reads + self.total_data_cache_writes;

        let total_memory_rw = self.instructions + d_refs;
        let l1_hits = total_memory_rw - ram_hits - l3_hits;

        // Uses Itamar Turner-Trauring's formula from
        // <https://pythonspeed.com/articles/consistent-benchmarking-in-ci/>
        let cycles = l1_hits + (l3_hits * 5) + (ram_hits * 35);

        let l1_hit_rate = l1_hits.div0(total_memory_rw) * 100;
        let l3_hit_rate = l3_hits.div0(total_memory_rw) * 100;
        let ram_hit_rate = ram_hits.div0(total_memory_rw) * 100;

        let i1_miss_rate = self
            .l1_instructions_cache_read_misses
            .div0(self.instructions)
            * 100;
        let lli_miss_rate = self
            .l3_instructions_cache_read_misses
            .div0(self.instructions)
            * 100;

        let d1_miss_rate = l1_data_accesses.div0(d_refs) * 100;

        let lld_miss_rate =
            (self.l3_data_cache_read_misses + self.l3_data_cache_write_misses).div0(d_refs) * 100;

        let ll_miss_rate = ram_hits.div0(total_memory_rw) * 100;

        CacheSummary {
            cycles,
            d1_miss_rate,
            i1_miss_rate,
            l1_hit_rate,
            l1_hits,
            l3_hit_rate,
            l3_hits,
            ll_miss_rate,
            lld_miss_rate,
            lli_miss_rate,
            ram_hit_rate,
            ram_hits,
            total_memory_rw,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::metrics::model::Metric;

    #[rstest]
    #[case::zero([0, 0, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0], [
        0f64,
        0f64,
        0f64,
        0f64,
        0f64,
        0f64,
        0f64,
        0f64]
    )]
    // Test that even when the cache numbers don't add up there is no overflow, div by zero, etc.
    #[case::artificial([1, 2, 3, 4, 5, 6, 7, 8, 9], [0, 0, 24, 6, 840], [
        400.0f64, // i1 miss rate
        220.000_000_000_000_03_f64, // d1 miss rate
        400.0f64, // ll miss rate
        700.0f64, // lli miss rate
        340.0f64, // lld miss rate
        0.0f64, // l1 hit rate
        0.0f64, // ll hit rate
        400.0f64] // ram hit rate
    )]
    // A real world scenario with cache numbers that add up and produce correct (miss, hit) rates
    #[case::real_world([1353, 255, 233, 51, 12, 0, 50, 3, 0], [1778, 10, 53, 1841, 3683], [
        3.769_401_330_376_940_3_f64, // i1 miss rate
        2.459_016_393_442_623_f64, // d1 miss rate
        2.878_870_179_250_407_4_f64, // ll miss rate
        3.695_491_500_369_549_4_f64, // lli miss rate
        0.614_754_098_360_655_8_f64, // lld miss rate
        96.577_946_768_060_84_f64, // l1 hit rate
        0.543_183_052_688_756_1_f64, // ll hit rate
        2.878_870_179_250_407_4_f64] // ram hit rate
    )]
    #[case::round_floats([10, 20, 30, 1, 2, 3, 4, 2, 0], [54, 0, 6, 60, 264], [
        10f64, // i1 miss rate
        10f64,// d1 miss rate
        10f64, // ll miss rate
        40f64, // lli miss rate
        4f64, // lld miss rate
        90f64,// l1 hit rate
        0f64, // ll hit rate
        10f64] // ram hit rate
    )]
    fn test_cycles_estimator(
        #[case] data: [u64; 9],
        #[case] expected_basic: [u64; 5],
        #[case] expected_rates: [f64; 8],
    ) {
        let estimator = CyclesEstimator::new(
            Metric::Int(data[0]),
            Metric::Int(data[1]),
            Metric::Int(data[2]),
            Metric::Int(data[3]),
            Metric::Int(data[4]),
            Metric::Int(data[5]),
            Metric::Int(data[6]),
            Metric::Int(data[7]),
            Metric::Int(data[8]),
        );

        let expected = CacheSummary {
            l1_hits: Metric::Int(expected_basic[0]),
            l3_hits: Metric::Int(expected_basic[1]),
            ram_hits: Metric::Int(expected_basic[2]),
            total_memory_rw: Metric::Int(expected_basic[3]),
            cycles: Metric::Int(expected_basic[4]),
            i1_miss_rate: Metric::Float(expected_rates[0]),
            d1_miss_rate: Metric::Float(expected_rates[1]),
            ll_miss_rate: Metric::Float(expected_rates[2]),
            lli_miss_rate: Metric::Float(expected_rates[3]),
            lld_miss_rate: Metric::Float(expected_rates[4]),
            l1_hit_rate: Metric::Float(expected_rates[5]),
            l3_hit_rate: Metric::Float(expected_rates[6]),
            ram_hit_rate: Metric::Float(expected_rates[7]),
        };

        let actual = estimator.calculate();
        assert_eq!(actual, expected);
    }
}
