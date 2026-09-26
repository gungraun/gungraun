<!-- markdownlint-disable MD041 MD033 -->

# Output Format

The Gungraun output can be customized with
[command-line arguments](../../../cli_and_env/output.md). But, the fine-grained
terminal output format is adjusted in the benchmark itself. For example
[truncating the description][`OutputFormat.truncate_description`], [showing a
grid][`OutputFormat.show_grid`], .... Please read the [docs][`OutputFormat`] for
further details.

In this section, I want to point out the possibility to show the cache misses,
and in the same manner cache miss rates and cache hit rates in the Gungraun
output.

## Showing Cache Misses

A default Gungraun benchmark run displays the following metrics:

<pre><code class="hljs"><span style="color:#0A0">test_lib_bench_readme_example_fibonacci::bench_fibonacci_group::bench_fibonacci</span> <span style="color:#0AA">short</span><span style="color:#0AA">:</span><b><span style="color:#00A">10</span></b>
<span style="color:#555">  </span>Instructions:                        <b>1734</b>|1734                 (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>L1 Hits:                             <b>2359</b>|2359                 (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>LL Hits:                                <b>0</b>|0                    (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>RAM Hits:                               <b>3</b>|3                    (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Total read+write:                    <b>2362</b>|2362                 (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Estimated Cycles:                    <b>2464</b>|2464                 (<span style="color:#555">No change</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.49333s</code></pre>

The cache and ram hits, `Total read+write` and `Estimated Cycles` are actually
not part of the original collected Callgrind metrics but calculated from them.
If you want to see the cache misses nonetheless, you can achieve this by
specifying the output format for example at top-level for all benchmarks in the
same file in the `main!` macro:

```rust
# extern crate gungraun;
use gungraun::prelude::*;
use gungraun::{CallgrindMetrics, Callgrind};

# #[library_benchmark] fn bench() {}
# library_benchmark_group!(name = my_group, benchmarks = bench);
# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::All])
        ),
    library_benchmark_groups = my_group
);
# }
```

or by using the command-line argument `--callgrind-metrics=@all` or the
environment variable `GUNGRAUN_CALLGRIND_METRICS=@all`.

The Gungraun output will then show all cache metrics:

<pre><code class="hljs"><span style="color:#0A0">test_lib_bench_readme_example_fibonacci::bench_fibonacci_group::bench_fibonacci</span> <span style="color:#0AA">short</span><span style="color:#0AA">:</span><b><span style="color:#00A">10</span></b>
<span style="color:#555">  </span>Instructions:                        <b>1734</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Dr:                                   <b>270</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Dw:                                   <b>358</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>I1mr:                                   <b>3</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>D1mr:                                   <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>D1mw:                                   <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>ILmr:                                   <b>3</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>DLmr:                                   <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>DLmw:                                   <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>I1 Miss Rate:                     <b>0.17301</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>LLi Miss Rate:                    <b>0.17301</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>D1 Miss Rate:                     <b>0.00000</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>LLd Miss Rate:                    <b>0.00000</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>LL Miss Rate:                     <b>0.12701</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>L1 Hits:                             <b>2359</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>LL Hits:                                <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>RAM Hits:                               <b>3</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>L1 Hit Rate:                      <b>99.8730</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>LL Hit Rate:                      <b>0.00000</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>RAM Hit Rate:                     <b>0.12701</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total read+write:                    <b>2362</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Estimated Cycles:                    <b>2464</b>|N/A                  (<span style="color:#555">*********</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.48898s</code></pre>

The Callgrind output format can be fully customized showing only the metrics
you're interested in and in any order. The docs of
[`Callgrind::format`][`Callgrind.format`] and [`CallgrindMetrics`] show all the
possibilities for [`Callgrind`]. The output format of the other Valgrind tools
can be customized in the same way. More details can be found in the docs for the
respective format (`Dhat::format`, `DhatMetric`, `Cachegrind::format`,
`CachegrindMetric`, ...) and for their respective command-line arguments with
`gungraun-runner --help`.

## Setting a Tolerance Margin for Metric Changes

Not every benchmark is deterministic, for example when hash maps or sets are
involved or even just by using `std::env::var` in the benchmarked code.
Benchmarks which show variances in the output of the metrics can be configured
to tolerate a specific margin in the benchmark output:

```rust
# extern crate gungraun;
use std::collections::HashMap;
use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{LibraryBenchmarkConfig, OutputFormat};

fn make_hashmap(num: usize) -> HashMap<String, usize> {
    (0..num).fold(HashMap::new(), |mut acc, e| {
        acc.insert(format!("element: {e}"), e);
        acc
    })
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .tolerance(0.9)
        )
)]
#[bench::tolerance(make_hashmap(100))]
fn bench_hash_map(map: HashMap<String, usize>) -> Option<usize> {
    black_box(
        black_box(map)
            .iter()
            .find_map(|(key, value)| (key == "element: 12345").then_some(*value)),
    )
}

library_benchmark_group!(name = my_group, benchmarks = bench_hash_map);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

or by using the command-line argument `--tolerance=0.9` (or
`GUNGRAUN_TOLERANCE=0.9`).

The second or any following Gungraun run might then show something like that:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_tolerance::my_group::bench_hash_map</span> <span style="color:#0AA">tolerance</span><span style="color:#0AA">:</span><b><span style="color:#00A">make_hashmap(100)</span></b>
<span style="color:#555">  </span>Instructions:                       <b>19787</b>|19623                (<span style="color:#555">Tolerance</span>)
<span style="color:#555">  </span>L1 Hits:                            <b>26395</b>|26123                (<b><span style="color:#F55">+1.04123%</span></b>) [<b><span style="color:#F55">+1.01041x</span></b>]
<span style="color:#555">  </span>LL Hits:                                <b>0</b>|0                    (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>RAM Hits:                              <b>22</b>|22                   (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Total read+write:                   <b>26417</b>|26145                (<b><span style="color:#F55">+1.04035%</span></b>) [<b><span style="color:#F55">+1.01040x</span></b>]
<span style="color:#555">  </span>Estimated Cycles:                   <b>27165</b>|26893                (<b><span style="color:#F55">+1.01142%</span></b>) [<b><span style="color:#F55">+1.01011x</span></b>]

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.15735s</code></pre>

and `Instructions` displays `Tolerance` instead of a difference.

[`Callgrind`]: https://docs.rs/gungraun/0.20.0/gungraun/struct.Callgrind.html
[`Callgrind.format`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.Callgrind.html#method.format
[`CallgrindMetrics`]:
    https://docs.rs/gungraun/0.20.0/gungraun/enum.CallgrindMetrics.html
[`OutputFormat`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.OutputFormat.html
[`OutputFormat.show_grid`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.OutputFormat.html#method.show_grid
[`OutputFormat.truncate_description`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.OutputFormat.html#method.truncate_description
