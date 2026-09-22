<!-- markdownlint-disable MD041 MD033 -->

# Perf

## Intro to Perf

Perf is a profiler that ships with the Linux kernel. It drives the hardware
performance counters and adds software events on top, so it can measure events
like instructions, cycles, task-clock or page faults as they happen on the
machine. If you are new to perf, Brendan Gregg's [perf tutorial][brendan-gregg]
and the [perf wiki][perf-wiki] are good starting points.

Gungraun integrates perf as a first-class tool alongside the Valgrind tools.
Despite Gungraun's Valgrind-centric, one-shot heritage, a perf benchmark gets
the same treatment as any other tool: it runs under the same harness and its
metrics flow into the same summaries and regression checks. Callgrind remains
the default tool, so perf is something you opt into.

## Why Perf?

The Valgrind tools simulate a synthetic CPU and caches in software. That's what
makes their measurements deterministic and comparable across machines, but it's
still a model. Perf reads the counters of the actual CPU the code runs on and
reports what the hardware did.

The difference shows in the overhead. A Valgrind run replays every instruction
through its simulation, which typically slows a program down by a factor of 10
to 50. Perf collects its events with little overhead and the benchmark runs at
near-native speed.

Also, Perf comes with a lot of additional metrics. Optimizations for specific
CPU features can be invisible to Valgrind because its simulated CPU doesn't have
them. Real hardware has them, and perf sees their effect on the events it
counts.

Perf is not a replacement for the Valgrind tools. For machine-independent
instruction counts, Callgrind remains the tool of choice. For measurements that
need the real CPU, perf is the one that can take them.

## Caveats

Benchmarking with perf behaves much closer to wall-clock benchmarking than to
benchmarking with Valgrind. The counters are collected while code runs on a real
machine, so they pick up the same noise as wall-clock measurements. Telling a
real change from noise takes an idle machine, repetition and statistics, just
like it does for wall-clock benchmarks.

Perf results also depend on the host CPU, the counters it provides and the
kernel's security settings must permit the requested events.

Gungraun requires Linux perf 5.9 or newer. Its measured-region control uses
`perf stat --control=fd:...,ack`, which first became available in that release;
otherwise, Gungraun is independent of the installed perf version.

Virtualized environments generally do not expose hardware counters. For example,
the native GitHub Actions runners are virtualized, so interesting events like
instructions are not available there. Perf benchmarks belong on real hardware.

## Usage on the Command-Line

Running perf instead of or in addition to Callgrind is straightforward and no
different from any [other tool](./tools.md):

Use [command-line arguments or environment variables](./cli_and_env/basics.md):
`--default-tool=perf` or `GUNGRAUN_DEFAULT_TOOL=perf` (replaces Callgrind as
default tool) or `--tools=perf` or `GUNGRAUN_TOOLS=perf` (runs perf in addition
to the default tool). Like any other tool perf can be enabled in the benchmark
settings:

```rust
# extern crate gungraun;
# use gungraun::prelude::*;
# use gungraun::Perf;
# #[library_benchmark] fn bench_library() -> u64 { 2 }
# library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(
  config = LibraryBenchmarkConfig::default().tool(Perf::default()),
  library_benchmark_groups = my_group
);
# }
```

Perf itself can be configured with the following command-line options, each with
an environment variable and benchmark-level equivalent:

- `--perf-args` (`GUNGRAUN_PERF_ARGS`): The command-line arguments to pass
  through to `perf stat`, for example `--perf-args='--all-cpus'`. By default, no
  arguments are passed. Not all perf arguments are allowed here; see below. To
  pass arguments to the `perf record` invocation, use `--perf-record-args`.
- `--perf-bin` (`GUNGRAUN_PERF_BIN`): The path to the `perf` executable. By
  default, Gungraun searches for `perf` in the system PATH. When used with
  `--tool-runner`, this path is passed to the runner as the perf binary to
  invoke.
- `--perf-events` (`GUNGRAUN_PERF_EVENTS`): Add an event set for perf (for the
  `-e`, `--event` perf argument). Each occurrence creates a separate event set,
  so comma-separated events within one occurrence are measured together and the
  same syntax as the perf `-e` event selector can be used, for example
  `--perf-events=instructions,cycles` or
  `--perf-events='{instructions,cycles},task-clock'`. By default, Gungraun
  measures the event set
  `instructions:u,cycles:u,task-clock,cpu-clock,faults,context-switches,branch-misses,cache-misses`.
- `--perf-limits` (`GUNGRAUN_PERF_LIMITS`): Set perf regression limits as
  comma-separated `pattern=limit` pairs. By default, no limits are set. The full
  pattern and limit grammar, including wildcards, soft percentage limits, and
  hard limits with units, is described in
  [regressions.md](./regressions.md#the-format-short-names-and-groups-in-full-detail).
- `--perf-record` (`GUNGRAUN_PERF_RECORD`): Run `perf record` in addition to the
  `perf stat` benchmark run. Disabled by default. This option does not enable
  running perf itself: without perf enabled, for example via `--tools=perf` or a
  benchmark configuration, it does nothing.
- `--perf-record-args` (`GUNGRAUN_PERF_RECORD_ARGS`): The command-line arguments
  to pass to `perf record`, for example
  `--perf-record-args='--all-cpus --freq=400'`. These are independent of the
  `perf stat` arguments from `--perf-args`. By default, no arguments are passed.
- `--perf-run-mode` (`GUNGRAUN_PERF_RUN_MODE`): Select how perf runs the
  benchmark. `direct` runs the benchmark once without calibration. `calibrate`
  samples the default calibration duration and `calibrate=<duration>` uses an
  explicit duration such as `250ms` or `2s`. Supported duration units are
  nanoseconds (`ns`, `nsec`), microseconds (`us`, `usec`), milliseconds (`ms`,
  `msec`), and seconds (`s`, `sec`, `secs`, `second`, `seconds`); a missing unit
  defaults to seconds.
- `--perf-sampling` (`GUNGRAUN_PERF_SAMPLING`): Enable, disable, or configure
  the duration for perf sampling, a time limit for continuously repeated
  `perf stat` sampling that improves measurement stability. `yes` enables
  sampling with the default duration of 2 seconds, `no` disables it so that the
  benchmark is measured by a single `perf stat` run, and a duration such as
  `250ms` or `2s` enables sampling with that duration. The sampling duration
  uses the same units as `--perf-run-mode`, affects only the main `perf stat`
  measurement and not the optional `perf record` capture, and is independent
  from the calibration duration of `--perf-run-mode=calibrate=<duration>`.

Not all perf arguments are allowed in `--perf-args` and `--perf-record-args`
and, if encountered, produce a warning. This includes the event selection
arguments (`-e`, `--event`, `--pfm-events`), since Gungraun manages events in
event sets (`--perf-events`). Additionally, `perf stat` invocation does not
accept the `record` or `report` subcommands; parsing stops at such an argument
and everything after it is ignored.

## Usage in a Benchmark

Most of what the command-line section described can also be configured from
within the benchmark itself. The `Perf` builder passed to
`LibraryBenchmarkConfig::tool` (or `BinaryBenchmarkConfig::tool` in binary
benchmarks) is the programmatic counterpart of the command-line options.
Selecting perf for a library benchmark looks like this:

```rust
# extern crate gungraun;
# mod my_lib { pub fn fibonacci(_: u64) -> u64 { 0 } }
use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{Perf, Tool};

#[library_benchmark]
#[bench::fibonacci_25(25)]
fn bench_library(n: u64) -> u64 {
    my_lib::fibonacci(n)
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);

# fn main() {
main!(
    config = LibraryBenchmarkConfig::default().tool(Perf::default()),
    library_benchmark_groups = my_group
);
# }
```

Although you should also configure [sampling](#sampling-duration) and possibly
the [run mode](#run-modes), we run this benchmark which runs perf once and
produces output similar to this:

<pre><code class="hljs">lib_bench_perf::my_group::bench_library fibonacci_25:(25)
  ======= CALLGRIND ===================================================================================
  Instructions:                                    2438801|N/A                  (*********)
  L1 Hits:                                         3484967|N/A                  (*********)
  LL Hits:                                               2|N/A                  (*********)
  RAM Hits:                                              1|N/A                  (*********)
  Total read+write:                                3484970|N/A                  (*********)
  Estimated Cycles:                                3485012|N/A                  (*********)
  ======= PERF ========================================================================================
  >> alpha: 0.05000, min_pcnt_running: 100.000
  cpu_core/instructions/u:                         2439354|N/A                  (*********)
  cpu_atom/cycles/u:                                746462|N/A                  (*********)
  cpu_core/cycles/u:                                746462|N/A                  (*********)
  task-clock/u [us]:                               201.900|N/A                  (*********)
  cpu-clock/u [us]:                                198.938|N/A                  (*********)
  faults/u:                                              0|N/A                  (*********)
  context-switches/u:                                    0|N/A                  (*********)
  cpu_core/branch-misses/u:                           1821|N/A                  (*********)
  cpu_core/cache-misses/u:                             235|N/A                  (*********)

Gungraun result: Ok. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.30242s</code></pre>

You can see that Callgrind cache metrics and cycles can differ from perf
metrics. Instruction counts however, are quiet comparable. Since perf runs only
once, the metrics contain all kind of noise and cold start effects. As you can
see in the following sections these can be mitigated to get more meaningful
metrics.

### Run Modes

`Perf::run_mode` taking a `PerfRunMode` controls how the benchmark invocation is
calibrated inside the perf measurement, mirroring `--perf-run-mode`.
`PerfRunMode::Direct` is the default and measures a single, normal benchmark
invocation. `PerfRunMode::DefaultCalibrate` first runs `perf` on the harness
itself to measure the overhead introduced by perf and Gungraun, without running
the actual benchmark. The calibration stops after a default duration of one
second, the first sample is discarded to mitigate cold-start effects, and the
mean calibration metrics are subtracted from the final benchmark metrics.
`PerfRunMode::Calibrate` takes a `Duration` for a custom calibration sampling
duration, trading total benchmark execution time for a more stable overhead
estimate.

```rust
# extern crate gungraun;
# mod my_lib { pub fn fibonacci(_: u64) -> u64 { 0 } }
# use gungraun::prelude::*;
# use gungraun::{Perf, PerfRunMode, Tool};
# use std::time::Duration;
#
# #[library_benchmark]
# #[bench::fibonacci_25(25)]
# fn bench_library(n: u64) -> u64 {
#     my_lib::fibonacci(n)
# }
#
# library_benchmark_group!(name = my_group, benchmarks = bench_library);
#
# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().run_mode(PerfRunMode::DefaultCalibrate)),
    library_benchmark_groups = my_group
);
# }
```

Note Perf is now the default tool, to keep the examples a little bit more
concise. Running the benchmark again producing metrics with calibration on the
left side and without on the right side:

<pre><code class="hljs">lib_bench_perf::my_group::bench_library fibonacci_25:(25)
  ======= PERF ========================================================================================
  >> alpha: 0.05000, min_pcnt_running: 100.000
  cpu_core/instructions/u:                         2437680|2439354              (-0.06862%) [-1.00069x]
  cpu_atom/cycles/u:                                749435|746462               (+0.39828%) [+1.00398x]
  cpu_core/cycles/u:                                751094|746462               (+0.62053%) [+1.00621x]
  task-clock/u [us]:                               190.646|201.900              (-5.57383%) [-1.05903x]
  cpu-clock/u [us]:                                189.120|198.938              (-4.93533%) [-1.05192x]
  faults/u:                                              0|0                    (+0.00000%) [+1.00000x]
  context-switches/u:                                    0|0                    (+0.00000%) [+1.00000x]
  cpu_core/branch-misses/u:                           1255|1821                 (-31.0818%) [-1.45100x]
  cpu_core/cache-misses/u:                              82|235                  (-65.1064%) [-2.86585x]

Gungraun result: Ok. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 1.07354s</code></pre>

Most noticeable are the changes in the branch-misses and cache misses likely due
to the mitigation of cold start effects and caches. Whether calibration is
worthwhile depends on the benchmark: if the overhead is small relative to the
main run, the default `Direct` is usually sufficient. For binary benchmarks, the
calibration modes are effectively ignored and fall back to `Direct`, because the
benchmark binary is invoked directly with its command arguments.

### Sampling Duration

`sample_duration` sets the wall-clock limit for the continuously repeated
`perf stat` sampling described with `--perf-sampling` in the command-line
section:

```rust
# extern crate gungraun;
# mod my_lib { pub fn fibonacci(_: u64) -> u64 { 0 } }
# use gungraun::prelude::*;
# use gungraun::{Perf, PerfRunMode, Tool};
# use std::time::Duration;
# #[library_benchmark]
# fn bench_library() -> u64 { 2 }
# library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().sample_duration(Duration::from_secs(2))),
    library_benchmark_groups = my_group
);
# }
```

Running the benchmark in comparison with sampling (left) to the original run
without sampling (right) displays the metrics now with statistics (See below
[example analysis](#a-small-example-analysis) for a detailed explanation of
them):

<pre><code class="hljs">lib_bench_perf::my_group::bench_library fibonacci_25:(25)
  ======= PERF ========================================================================================
  >> alpha: 0.05000, min_pcnt_running: 100.000
  cpu_core/instructions/u:                         2328445|2439354              (-4.54665%) [-1.04763x]
    rse% (sig.thr) [sig.fact]                      0.08565|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  cpu_atom/cycles/u:                                724977|746462               (-2.87824%) [-1.02964x]
    rse% (sig.thr) [sig.fact]                      0.09748|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  cpu_core/cycles/u:                                721724|746462               (-3.31403%) [-1.03428x]
    rse% (sig.thr) [sig.fact]                      0.10441|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  task-clock/u [us]:                               195.381|201.900              (-3.22858%) [-1.03336x]
    rse% (sig.thr) [sig.fact]                      0.10998|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  cpu-clock/u [us]:                                194.187|198.938              (-2.38801%) [-1.02446x]
    rse% (sig.thr) [sig.fact]                      0.11839|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  faults/u:                                              1|0                    (+++inf+++) [+++inf+++]
    rse% (sig.thr) [sig.fact]                      2.29012|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  context-switches/u:                                    0|0                    (+0.00000%) [+1.00000x]
    rse% (sig.thr) [sig.fact]                      0.00000|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  cpu_core/branch-misses/u:                           1077|1821                 (-40.8567%) [-1.69081x]
    rse% (sig.thr) [sig.fact]                      0.12999|N/A                  (*********)
    samples                                           2083|N/A                  (*********)
  cpu_core/cache-misses/u:                               5|235                  (-97.8723%) [-47.0000x]
    rse% (sig.thr) [sig.fact]                      2.67131|N/A                  (*********)
    samples                                           2083|N/A                  (*********)

Gungraun result: Ok. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 2.07493s</code></pre>

The sampling duration is only applied once at least one sample has been
recorded: a run is never stopped before its first sample, so every run produces
at least one sample, but it may exceed the configured duration until the first
sample is written. If the duration is long enough for multiple benchmark runs,
the first run is discarded to mitigate cold-start effects, and at least one
record is always kept. A sampling duration above one second typically works
well. The duration affects only the main `perf stat` measurement, not the
optional `perf record` capture, and is independent from the calibration duration
of `PerfRunMode::Calibrate`. In binary benchmarks, setup and teardown run once
before and after the sampling period; in library benchmarks they run per sample.

### Choosing Event Sets

`event_set` adds a single event selector to the configuration and `event_sets`
adds multiple selectors at once. Both append to the list of event sets; there is
no replace variant, so build the complete list where the builder chain starts.
Each event set is measured by a separate `perf stat` invocation and passed to
perf with `--event` as-is, which means perf's own selector syntax applies:
comma-separated events are measured together, and the group syntax
`{instructions,cycles}` works as well.

Splitting event sets can become necessary because the CPU cannot record an
endless amount of PMU hardware events in parallel and then falls back to
[multiplexing](#statistical-filters-and-significance) for these events.

```rust
# extern crate gungraun;
# mod my_lib { pub fn fibonacci(_: u64) -> u64 { 0 } }
# use gungraun::prelude::*;
# use gungraun::Perf;
#
# #[library_benchmark]
# #[bench::fibonacci_25(20)]
# fn bench_library(n: u64) -> u64 {
#     my_lib::fibonacci(n)
# }
#
# library_benchmark_group!(name = my_group, benchmarks = bench_library);
#
# fn main() {
main!(
    config = LibraryBenchmarkConfig::default().tool(
        Perf::default()
            .event_set("cycles,instructions")
            .event_set("{branch-misses,cache-misses},task-clock")
    ),
    library_benchmark_groups = my_group
);
# }
```

This benchmark runs `perf stat` twice, once with `--event=cycles,instructions`
and once with `--event={branch-misses,cache-misses},task-clock`. The same event
sets also apply to the optional `perf record` capture described below. As long
as no event set is configured, the default event set from the command-line
section applies; configured sets are measured instead of it.

## A Small Example Analysis

We reuse the example with sampling from above and with Perf as default tool.

```rust
# extern crate gungraun;
# mod my_lib { pub fn fibonacci(_: u64) -> u64 { 0 } }
use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{Perf, Tool};

#[library_benchmark]
#[bench::fibonacci_30(30)]
fn bench_library(n: u64) -> u64 {
    my_lib::fibonacci(n)
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);

# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::Perf)
        .tool(Perf::default().sample_duration(Duration::from_secs(2))),
    library_benchmark_groups = my_group
);
# }
```

The first invocation produces the following metrics. I am also establishing a
[baseline](./cli_and_env/baselines.md) with `--save-baseline=perf_example` for
demonstration purposes so I can run the next example runs against this baseline:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_perf::my_group::bench_library</span> <span style="color:#0AA">fibonacci_30</span>:<b><span style="color:#00A">(30)</span></b>
<span style="color:#555">  </span><span style="color:#555">=======</span> PERF <span style="color:#555">========================================================================================</span>
<span style="color:#555">  </span><span style="color:#555">>> alpha: 0.05000, min_pcnt_running: 100.000</span>
<span style="color:#555">  </span>Baselines:                                  <b>perf_example</b>|perf_example (old)
<span style="color:#555">  </span>cpu_core/instructions/u:                        <b>26938998</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.01200</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>cpu_atom/cycles/u:                               <b>8333327</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.04080</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>cpu_core/cycles/u:                               <b>8329664</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.04111</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>task-clock/u [ms]:                               <b>2.10894</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.04798</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>cpu-clock/u [ms]:                                <b>2.10781</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.04823</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>faults/u:                                              <b>3</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">1.05398</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>context-switches/u:                                    <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.00000</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>cpu_core/branch-misses/u:                          <b>11635</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.02553</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>cpu_core/cache-misses/u:                              <b>23</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">7.35942</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">687</span></b>|<span style="color:#555">N/A                 </span> (<span style="color:#555">*********</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 2.06148s</code></pre>

The header repeats the statistical settings used for the comparison: `alpha` is
the significance level, and `min_pcnt_running` is the minimum percentage of time
that an event must have run to be included. Because this is the first run, there
is no baseline yet, so the right-hand values are `N/A` and comparisons are shown
as `*********`. On later runs, parentheses contain the percentage change and
brackets contain the corresponding change factor.

The indented rows describe the samples behind each metric. `rse%` is the
[relative standard error](https://simple.wikipedia.org/wiki/Standard_error#Relative_standard_error).
It describes the uncertainty of the estimated metric mean relative to that mean:
more sample-to-sample fluctuation increases the RSE, while more samples
generally reduce it. A lower RSE therefore means that the reported mean is more
precise; it is not a direct measure of the sample spread alone. Its
parenthesized value is the smallest relative change that would be statistically
significant, and `sig.fact` (Significance factor) compares the observed change
with that threshold. A significance factor below `1` is not significant; a
factor above `1` is. The `samples` row is the number of observations collected
for the metric.

Running this benchmark a second time:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_perf::my_group::bench_library</span> <span style="color:#0AA">fibonacci_30</span>:<b><span style="color:#00A">(30)</span></b>
<span style="color:#555">  </span><span style="color:#555">=======</span> PERF <span style="color:#555">========================================================================================</span>
<span style="color:#555">  </span><span style="color:#555">>> alpha: 0.05000, min_pcnt_running: 100.000</span>
<span style="color:#555">  </span>Baselines:                                              |perf_example
<span style="color:#555">  </span>cpu_core/instructions/u:                        <b>26918731</b>|26938998             (<b><span style="color:#42c142">-0.07523%</span></b>) [<b><span style="color:#42c142">-1.00075x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.01338</span></b>|<span style="color:#555">0.01200             </span> (<span style="color:#555">>0.03523%</span>) [<b><span style="color:#00A"> 2.13541x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>cpu_atom/cycles/u:                               <b>8322779</b>|8333327              (<b><span style="color:#42c142">-0.12658%</span></b>) [<b><span style="color:#42c142">-1.00127x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.02851</span></b>|<span style="color:#555">0.04080             </span> (<span style="color:#555">>0.09762%</span>) [<b><span style="color:#00A"> 1.29667x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>cpu_core/cycles/u:                               <b>8318495</b>|8329664              (<b><span style="color:#42c142">-0.13409%</span></b>) [<b><span style="color:#42c142">-1.00134x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.02904</span></b>|<span style="color:#555">0.04111             </span> (<span style="color:#555">>0.09871%</span>) [<b><span style="color:#00A"> 1.35845x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>task-clock/u [ms]:                               <b>2.10663</b>|2.10894              (<b><span style="color:#42c142">-0.10953%</span></b>) [<b><span style="color:#42c142">-1.00110x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.03733</span></b>|<span style="color:#555">0.04798             </span> (<span style="color:#555">>0.11921%</span>) [<span style="color:#555"> 0.91883x</span>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>cpu-clock/u [ms]:                                <b>2.10536</b>|2.10781              (<b><span style="color:#42c142">-0.11631%</span></b>) [<b><span style="color:#42c142">-1.00116x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.03797</span></b>|<span style="color:#555">0.04823             </span> (<span style="color:#555">>0.12036%</span>) [<span style="color:#555"> 0.96632x</span>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>faults/u:                                              <b>3</b>|3                    (<b><span style="color:#F55">+0.00000%</span></b>) [<b><span style="color:#F55">+1.00000x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">1.17094</span></b>|<span style="color:#555">1.05398             </span> (<span style="color:#555">>3.06207%</span>) [<span style="color:#555"> 0.54652x</span>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>context-switches/u:                                    <b>0</b>|0                    (<b><span style="color:#F55">+0.00000%</span></b>) [<b><span style="color:#F55">+1.00000x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.00000</span></b>|<span style="color:#555">0.00000             </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>cpu_core/branch-misses/u:                          <b>11628</b>|11635                (<b><span style="color:#42c142">-0.06016%</span></b>) [<b><span style="color:#42c142">-1.00060x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.02443</span></b>|<span style="color:#555">0.02553             </span> (<span style="color:#555">>0.06930%</span>) [<span style="color:#555"> 0.84528x</span>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]
<span style="color:#555">  </span>cpu_core/cache-misses/u:                              <b>20</b>|23                   (<b><span style="color:#42c142">-13.0435%</span></b>) [<b><span style="color:#42c142">-1.15000x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">6.23706</span></b>|<span style="color:#555">7.35942             </span> (<span style="color:#555">>18.1609%</span>) [<span style="color:#555"> 0.54867x</span>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                            <b><span style="color:#555">683</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">-0.58224%</span>) [<span style="color:#555">-1.00586x</span>]

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 2.06087s</code></pre>

This is the same code measured again, and the high-volume counters and clocks
move by less than `0.5%`. Those small differences are normal run-to-run noise.
The cache-miss count appears to increase by `13.0435%`, but the underlying
change is only from `20` to `23` misses. Its significance threshold is
`18.1609%`, while the significance factor is only `0.54867x`. Because that
factor is below `1`, the observed increase is not statistically significant.

Now, assume a change improves the fibonacci runtime:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_perf::my_group::bench_library</span> <span style="color:#0AA">fibonacci_30</span>:<b><span style="color:#00A">(30)</span></b>
<span style="color:#555">  </span><span style="color:#555">=======</span> PERF <span style="color:#555">========================================================================================</span>
<span style="color:#555">  </span><span style="color:#555">>> alpha: 0.05000, min_pcnt_running: 100.000</span>
<span style="color:#555">  </span>Baselines:                                              |perf_example
<span style="color:#555">  </span>cpu_core/instructions/u:                        <b>10216912</b>|26938998             (<b><span style="color:#42c142">-62.0739%</span></b>) [<b><span style="color:#42c142">-2.63671x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.02611</span></b>|<span style="color:#555">0.01200             </span> (<span style="color:#555">>0.03051%</span>) [<b><span style="color:#00A"> 2034.45x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>cpu_atom/cycles/u:                               <b>3166639</b>|8333327              (<b><span style="color:#42c142">-62.0003%</span></b>) [<b><span style="color:#42c142">-2.63160x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.03937</span></b>|<span style="color:#555">0.04080             </span> (<span style="color:#555">>0.08530%</span>) [<b><span style="color:#00A"> 726.864x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>cpu_core/cycles/u:                               <b>3162611</b>|8329664              (<b><span style="color:#42c142">-62.0319%</span></b>) [<b><span style="color:#42c142">-2.63379x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.04069</span></b>|<span style="color:#555">0.04111             </span> (<span style="color:#555">>0.08620%</span>) [<b><span style="color:#00A"> 719.637x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>task-clock/u [us]:                               <b>810.932</b>|2108.94              (<b><span style="color:#42c142">-61.5478%</span></b>) [<b><span style="color:#42c142">-2.60063x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.05055</span></b>|<span style="color:#555">0.04798             </span> (<span style="color:#555">>0.10160%</span>) [<b><span style="color:#00A"> 605.812x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>cpu-clock/u [us]:                                <b>809.751</b>|2107.81              (<b><span style="color:#42c142">-61.5834%</span></b>) [<b><span style="color:#42c142">-2.60304x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.05188</span></b>|<span style="color:#555">0.04823             </span> (<span style="color:#555">>0.10242%</span>) [<b><span style="color:#00A"> 601.293x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>faults/u:                                              <b>3</b>|3                    (<b><span style="color:#F55">+0.00000%</span></b>) [<b><span style="color:#F55">+1.00000x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.96381</span></b>|<span style="color:#555">1.05398             </span> (<span style="color:#555">>2.75304%</span>) [<b><span style="color:#00A"> 1.39038x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>context-switches/u:                                    <b>0</b>|0                    (<b><span style="color:#F55">+0.00000%</span></b>) [<b><span style="color:#F55">+1.00000x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.00000</span></b>|<span style="color:#555">0.00000             </span> (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>cpu_core/branch-misses/u:                           <b>4475</b>|11635                (<b><span style="color:#42c142">-61.5385%</span></b>) [<b><span style="color:#42c142">-2.60000x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">0.04440</span></b>|<span style="color:#555">0.02553             </span> (<span style="color:#555">>0.06026%</span>) [<b><span style="color:#00A"> 1021.10x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]
<span style="color:#555">  </span>cpu_core/cache-misses/u:                               <b>8</b>|23                   (<b><span style="color:#42c142">-65.2174%</span></b>) [<b><span style="color:#42c142">-2.87500x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  rse% (sig.thr) [sig.fact]</span>                      <b><span style="color:#555">3.45528</span></b>|<span style="color:#555">7.35942             </span> (<span style="color:#555">>14.6450%</span>) [<b><span style="color:#00A"> 4.42071x</span></b>]
<span style="color:#555">  </span><span style="color:#555">  samples</span>                                           <b><span style="color:#555">1248</span></b>|<span style="color:#555">687                 </span> (<span style="color:#555">+81.6594%</span>) [<span style="color:#555">+1.81659x</span>]

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 2.06785s</code></pre>

This comparison is different: instructions fall by `62.0739%` but the other
metrics also improve considerably. The bracketed factors on the metric rows show
the scale of those improvements, while the significance factors on the `rse%`
rows are far above `1`; These are statistically very clear improvements rather
than sampling noise.

## Recording a perf.data Profile with perf record

`record` enables a companion `perf record` run in addition to the `perf stat`
measurement, mirroring `--perf-record`. The result is a sample-based profile
that can be analyzed with `perf report`:

```rust
# extern crate gungraun;
use gungraun::Perf;

let perf = Perf::default().record(true);
```

Arguments for the `perf record` invocation are added with `Perf::record_args`.

### Call Graphs

When `perf record` profiles a benchmark, call graphs attach a caller stack to
each recorded sample. They are what makes per-function attribution in
`perf report` — and flamegraphs as popularized by [brendan-gregg] — meaningful.
The unwinding method is a trade-off:

- With `--call-graph fp`, perf unwinds via frame pointers. This is the cheapest
  method, but it only yields complete stacks when every frame on the stack was
  compiled with frame pointers, and release builds usually omit them. Missing
  frame pointers truncate the call graph.
- With `--call-graph dwarf`, perf unwinds via DWARF debug information instead.
  No frame pointers are required, at the price of noticeably higher recording
  overhead and a considerably larger `perf.data` file.

The method is selected like any other `perf record` argument: on the
command-line via `--perf-record-args='--call-graph dwarf'`, and in the benchmark
config via `.record_args(["--call-graph", "dwarf"])`. Further methods, such as
`lbr` on supported CPUs, are described in the [perf wiki][perf-wiki].

## Advanced Usage

### Statistical Filters and Significance

Three builder methods tune which sampled records and metric changes are kept.
`Perf::alpha` sets the p-value threshold that decides whether a metric change is
statistically significant before soft limits are applied. The default of `0.05`
balances sensitivity and false-positive rate. Lower values such as `0.01` or
`0.001` make regression reports more conservative, which helps in a noisy CI; a
higher value catches smaller changes at the cost of more false positives. The
limits themselves are set with `soft_limits` and `hard_limits`, the builder
counterparts of `--perf-limits` described in
[regressions.md](./regressions.md#the-format-short-names-and-groups-in-full-detail).

`Perf::non_zero_metrics` sets patterns for metrics that must be nonzero. If a
matching metric is exactly zero, the whole measurement record containing it is
discarded, where each record holds all metrics selected by one event set. This
mitigates a source of low-end skew: short-running benchmarks can occasionally
produce zero values for metrics expected to be nonzero. The patterns default to
`task-clock*`, `cpu-clock*` and `*instructions*`, and a call replaces the
defaults. They use the same wildcard syntax as the limit patterns: `*`, `?`, `\`
escapes and character classes such as `[...]`, `[!...]` and `[a-zA-Z]`.

`Perf::min_pcnt_running` sets the minimum percentage of time a hardware counter
must have been running for a sampled record to be kept. When perf multiplexes
more events than the CPU has physical counter slots, `pcnt_running` reports the
fraction of the interval the counter was active, and records below the threshold
are discarded. The default of `100.0` tolerates no multiplexing, and valid
values range from `0.0` to `100.0`. Usually it is better to keep the default and
split the events into multiple event sets instead, at the cost of running perf
once per set.

```rust
# extern crate gungraun;
use gungraun::Perf;

let perf = Perf::default()
    .alpha(0.01)
    .non_zero_metrics(["task-clock*", "*instructions*"])
    .min_pcnt_running(80.0);
```

### Controlling the Measured Region

By default, perf measurement starts when the benchmark function is entered, like
the default entry point of the Valgrind tools. With `disable_entry_point(true)`
the automatic start is turned off and the measured region is bracketed manually
with the `perf_enable!` and `perf_disable!` macros:

```rust
# extern crate gungraun;
# mod my_lib { pub fn fibonacci(_: u64) -> u64 { 0 } }
use gungraun::prelude::*;
use gungraun::Perf;

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Perf::default().disable_entry_point(true))
)]
#[bench::fibonacci_25(25)]
fn bench_library(n: u64) -> u64 {
    // anything before perf_enable! is not measured
    let token = gungraun::perf_enable!();
    let result = my_lib::fibonacci(n);
    gungraun::perf_disable!(token);
    // anything after perf_disable! is not measured
    result
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);

# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

`perf_enable!` starts the measurement and returns an opaque token;
`perf_disable!` stops it again and consumes exactly that token. The macros drive
a single process-global control channel: they are not thread-safe, must not be
nested, and every token must be passed to exactly one matching `perf_disable!`.
Calling `disable_entry_point(false)` restores the default entry point.

The macros can also be called from production code that the benchmark process
executes. To use them outside benchmark code, the dependency on `gungraun` must
enable the `stubs` feature, or `perf_stubs` if only the perf macros are needed.
The macros compile to no-ops in production code if the feature is disabled
similar to [Valgrind client requests](./client_requests.md)

### Logging to the Perf Side Channel

`perf_log!` writes a message to the perf log side channel that the runner sets
up for the benchmark process. It accepts format arguments like `format!` and
writes one line per call, falling back to stderr if the log file is unavailable.
Like the control macros, it compiles to a no-op on non-Linux platforms or with
the feature disabled:

```rust
# extern crate gungraun;
# mod my_lib { pub fn fibonacci(_: u64) -> u64 { 0 } }
use gungraun::prelude::*;

fn setup(n: u64) -> u64 {
    gungraun::perf_log!("fibonacci({n}): preparing the measured region");
    n
}

#[library_benchmark]
#[bench::fibonacci_25(args = (25), setup = setup)]
fn bench_library(n: u64) -> u64 {
    my_lib::fibonacci(n)
}

# library_benchmark_group!(name = my_group, benchmarks = bench_library);
#
# fn main() {
# main!(library_benchmark_groups = my_group);
# }
```

[brendan-gregg]: https://www.brendangregg.com/perf.html
[perf-wiki]: https://perfwiki.github.io/main/
