use std::collections::HashMap;
use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{
    Cachegrind, CachegrindMetric, CachegrindMetrics, Callgrind, CallgrindMetrics, Dhat, DhatMetric,
    Drd, ErrorMetric, EventKind, Helgrind, Memcheck, OutputFormat, Tool,
};
use gungraun_tests::{bubble_sort, setup_worst_case_array};

fn make_hashmap(num: usize) -> HashMap<String, usize> {
    (0..num).fold(HashMap::new(), |mut acc, e| {
        acc.insert(format!("element: {e}"), e);
        acc
    })
}

// The --collect-systime=nsec option is not supported on freebsd and apple, so we use
// --collect-systime=yes instead on these targets
//
// --simulate-wb=yes does not work together with --cacheuse=yes, so it is excluded here
#[cfg(any(target_vendor = "apple", target_os = "freebsd"))]
pub fn base_config() -> LibraryBenchmarkConfig {
    LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "collect-jumps=yes",
            "collect-systime=yes",
            "collect-bus=yes",
            "cache-sim=yes",
            "branch-sim=yes",
        ]))
        .clone()
}

#[cfg(not(any(target_vendor = "apple", target_os = "freebsd")))]
pub fn base_config() -> LibraryBenchmarkConfig {
    LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args([
            "collect-jumps=yes",
            "collect-systime=nsec",
            "collect-bus=yes",
            "cache-sim=yes",
            "branch-sim=yes",
        ]))
        .clone()
}

#[library_benchmark]
#[bench::implicit_default_with_wb(config = base_config()
    .tool(Callgrind::with_args(["simulate-wb=yes"]))
)]
#[bench::implicit_default_with_cacheuse(config = base_config()
    .tool(Callgrind::with_args(["cacheuse=yes"]))
)]
#[bench::explicit_default_with_wb(config =
    base_config()
        .tool(Callgrind::with_args(["simulate-wb=yes"])
            .format([CallgrindMetrics::Default])
        )
)]
#[bench::explicit_default_with_cacheuse(config =
    base_config()
        .tool(Callgrind::with_args(["cacheuse=yes"])
            .format([CallgrindMetrics::Default])
        )
)]
#[bench::all_with_wb(config =
    base_config()
        .tool(Callgrind::with_args(["simulate-wb=yes"])
            .format([CallgrindMetrics::All])
        )
)]
#[bench::all_with_cachuse(config =
    base_config()
        .tool(Callgrind::with_args(["cacheuse=yes"])
            .format([CallgrindMetrics::All])
        )
)]
#[bench::cache_misses(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::CacheMisses])
        )
)]
#[bench::cache_miss_rates(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::CacheMissRates])
        )
)]
#[bench::cache_hits(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::CacheHits])
        )
)]
#[bench::cache_hit_rates(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::CacheHitRates])
        )
)]
#[bench::cache_sim(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::CacheSim])
        )
)]
#[bench::cache_use(config =
    base_config()
        .tool(Callgrind::with_args(["cacheuse=yes"])
            .format([CallgrindMetrics::CacheUse])
        )
)]
#[bench::system_calls(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::SystemCalls])
        )
)]
#[bench::branch_sim(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::BranchSim])
        )
)]
#[bench::write_back(config =
    base_config()
        .tool(Callgrind::with_args(["simulate-wb=yes"])
            .format([
                    // Without `Ir` the counts would be all zero
                    CallgrindMetrics::SingleEvent(EventKind::Ir),
                    CallgrindMetrics::WriteBackBehaviour
                ]
            )
        )
)]
#[bench::single_event_ir(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::SingleEvent(EventKind::Ir)])
        )
)]
#[bench::single_event_ge(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::SingleEvent(EventKind::Ge)])
        )
)]
#[bench::single_event_total_rw(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::SingleEvent(EventKind::TotalRW)])
        )
)]
#[bench::single_event_estimated_cycles(config =
    base_config()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::SingleEvent(EventKind::EstimatedCycles)])
        )
)]
fn bench_with_custom_callgrind_format() -> Vec<i32> {
    println!("Hello world!");
    black_box(bubble_sort(black_box(setup_worst_case_array(black_box(
        10,
    )))))
}

#[library_benchmark]
#[bench::cachegrind_all_shuffled(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
        .tool(Cachegrind::with_args(["branch-sim=yes"])
            .format([
                CachegrindMetric::Ir,
                CachegrindMetric::I1MissRate,
                CachegrindMetric::D1MissRate,
                CachegrindMetric::LLiMissRate,
                CachegrindMetric::LLdMissRate,
                CachegrindMetric::LLMissRate,
                CachegrindMetric::Dr,
                CachegrindMetric::Dw,
                CachegrindMetric::D1mr,
                CachegrindMetric::D1mw,
                CachegrindMetric::ILmr,
                CachegrindMetric::DLmr,
                CachegrindMetric::DLmw,
                CachegrindMetric::TotalRW,
                CachegrindMetric::L1hits,
                CachegrindMetric::LLhits,
                CachegrindMetric::RamHits,
                CachegrindMetric::EstimatedCycles,
                CachegrindMetric::Bc,
                CachegrindMetric::Bcm,
                CachegrindMetric::Bi,
                CachegrindMetric::Bim,
                CachegrindMetric::L1HitRate,
                CachegrindMetric::LLHitRate,
                CachegrindMetric::RamHitRate,
            ])
        )
)]
#[bench::cachegrind_all_default_order(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
        .tool(Cachegrind::with_args(["branch-sim=yes"])
            .format([CachegrindMetrics::All])
        )
)]
#[bench::cachegrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Cachegrind)
        .tool(Cachegrind::default()
            .format([CachegrindMetric::I1mr, CachegrindMetric::EstimatedCycles])
        )
)]
#[bench::dhat(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .tool(Dhat::default().format([DhatMetric::TotalBlocks, DhatMetric::TotalBytes]))
)]
#[bench::memcheck(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Memcheck)
        .tool(Memcheck::default().format([ErrorMetric::SuppressedErrors, ErrorMetric::Errors]))
)]
#[bench::helgrind(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Helgrind)
        .tool(Helgrind::default()
            .format([ErrorMetric::Contexts, ErrorMetric::SuppressedContexts])
        )
)]
#[bench::drd(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DRD)
        .tool(Drd::default().format([ErrorMetric::Errors, ErrorMetric::Contexts]))
)]
fn bench_with_custom_format() -> Vec<i32> {
    println!("Hello world!");
    black_box(bubble_sort(black_box(setup_worst_case_array(black_box(
        10,
    )))))
}

// Provoke a (very likely) change in the metrics and set tolerance high enough to make the
// `Tolerance` visible
#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .tolerance(100.0)
        )
)]
#[bench::hashmap(make_hashmap(100_000))]
fn bench_with_tolerance(map: HashMap<String, usize>) -> Option<usize> {
    black_box(
        map.iter()
            .find_map(|(key, value)| (key == "element: 12345").then_some(*value)),
    )
}

library_benchmark_group!(
    name = custom_format,
    benchmarks = [
        bench_with_custom_callgrind_format,
        bench_with_custom_format,
        bench_with_tolerance
    ]
);

main!(library_benchmark_groups = custom_format);
