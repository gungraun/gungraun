<!-- markdownlint-disable MD041 MD033 -->

# DHAT: A Dynamic Heap Analysis Tool

## Intro to DHAT

To fully understand DHAT please read the [Valgrind docs][dhat] for DHAT. Here's
just a short summary and quote from the docs:

> DHAT is primarily a tool for examining how programs use their heap
> allocations. It tracks the allocated blocks, and inspects every memory access
> to find which block, if any, it is to. It presents, on a program point basis,
> information about these blocks such as sizes, lifetimes, numbers of reads and
> writes, and read and write patterns.

The rest of this chapter is dedicated to how DHAT is integrated into Gungraun.

## The DHAT modes

Gungraun supports all three modes `heap` (the default), `copy` and `ad-hoc`
which can be changed on the [command-line](./cli_and_env/basics.md) with
`--dhat-args=--mode=ad-hoc` or in the benchmark itself with `Dhat::args`. Note
that `ad-hoc` mode requires [client requests](./client_requests.md) which have
prerequisites. If running the benchmarks in `ad-hoc` mode, it is highly
recommended to turn off the `EntryPoint` with `EntryPoint::None` (See next
section). However, DHAT is normally run in `heap` mode and it is assumed that
this is the mode used in the next sections.

## The Default Entry Point

The DHAT default entry point `EntryPoint::Default` in library benchmarks behaves
like
[`Callgrind's EntryPoint`](./benchmarks/library_benchmarks/custom_entry_point.md).
This centers the collected metrics shown in the terminal output on the benchmark
function. The entry point is set to `EntryPoint::None` for binary benchmarks.
But, if necessary, the entry point can be turned off or customized in
`Dhat::entry_point`.

Similar to Callgrind's entry point, the default entry point for DHAT excludes
metrics related to
[`setup` and/or `teardown` code](./benchmarks/library_benchmarks/setup_and_teardown.md),
as well as any elements specified in the `args` parameter of the `#[bench]` or
`#[benches]` attributes. This behavior typically aligns with user expectations.
However, DHAT has a unique characteristic: if the benchmarked function uses an
array created in the setup function, the metrics will not capture the reads and
writes to that array. To accurately measure these reads and writes, it is
necessary to set the entry point to the setup function (in this case, the
`setup_worst_case_array` function).

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }

use std::hint::black_box;
use gungraun::prelude::*;
use gungraun::{Dhat, EntryPoint};

pub fn setup_worst_case_array(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).rev().collect()
    } else {
        (0..start).rev().collect()
    }
}


#[library_benchmark]
#[bench::worst_case_3(setup_worst_case_array(3))]
fn bench_library(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(array)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .entry_point(
                EntryPoint::Custom("*::setup_worst_case_array".to_owned())
            )
        ),
    library_benchmark_groups = my_group
);
# }
```

## Sanitized DHAT Output Files

Gungraun rewrites DHAT output files by default (`SanitizeOutput::Yes`) to match
the configured `Dhat::entry_point` and additional `Dhat::frames` filters. This
keeps the metrics shown in `dh_view.html` aligned with the metrics Gungraun
reports in the terminal.

Use `Dhat::sanitize_output(SanitizeOutput::No)` to keep DHAT output files
unchanged, or `SanitizeOutput::KeepOrig` to write sanitized output while keeping
the original files with an `.orig` extension.

## Usage on the Command-Line

Running DHAT instead of or in addition to Callgrind is straightforward and no
different from any [other tool](./tools.md):

Either use
[command-line arguments or environment variables](./cli_and_env/basics.md):
`--default-tool=dhat` or `GUNGRAUN_DEFAULT_TOOL=dhat` (replaces Callgrind as
default tool) or `--tools=dhat` or `GUNGRAUN_TOOLS=dhat` (runs DHAT in addition
to the default tool).

## Usage in a Benchmark and a Small Example Analysis

Running DHAT in addition to Callgrind can also be carried out in the benchmark
itself with the `Dhat` struct in `LibraryBenchmarkConfig::tool`. We stick to the
example from above. The above benchmark will produce the following metrics:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_dhat::my_group::bench_library</span> <span style="color:#0AA">worst_case_3</span><span style="color:#0AA">:</span><b><span style="color:#00A">vec! [3, 2, 1]</span></b>
<span style="color:#555">  </span><span style="color:#555">=======</span> CALLGRIND <span style="color:#555">====================================================================</span>
<span style="color:#555">  </span>Instructions:                          <b>83</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>L1 Hits:                              <b>110</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>LL Hits:                                <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>RAM Hits:                               <b>3</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total read+write:                     <b>113</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Estimated Cycles:                     <b>215</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                           <b>12</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total blocks:                           <b>1</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax bytes:                        <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Reads bytes:                           <b>24</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Writes bytes:                          <b>36</b>|N/A                  (<span style="color:#555">*********</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.55554s</code></pre>

Analyzing the DHAT data, there are a total of `12 bytes` of allocations (The
vector: `3 * sizeof(i32)` bytes = `3 * 4` bytes) in `1` block during the setup
of the benchmark in `setup_worst_case_array`. That's also `12` bytes of writes
to fill the vector with the values. That makes `24` bytes of reads and `24`
bytes of writes in the `bubble_sort` function. Also, there are no
(de-)allocations of heap memory in `bubble_sort` itself.

## Soft Limits and Hard Limits

Based on that data, we could define for example hard limits (or soft limits or
both whatever you think is appropriate) to ensure `bubble_sort` is not getting
worse than that.

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }
use gungraun::prelude::*;
use gungraun::{Dhat, DhatMetric};
use std::hint::black_box;

#[library_benchmark]
#[bench::worst_case_3(
    args = (vec![3, 2, 1]),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .hard_limits([
                (DhatMetric::ReadsBytes, 24),
                (DhatMetric::WritesBytes, 32)
            ])
        )
)]
fn bench_bubble_sort(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(black_box(array)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_bubble_sort);

# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()),
    library_benchmark_groups = my_group
);
# }
```

Now, if `bubble_sort` would read more than `24` bytes or if there were more than
`32` bytes of writes during the benchmark, the benchmark would fail and exit
with error.

## Frames and Benchmarking Multi-Threaded Functions

It is possible to specify additional `Dhat::frames` for example when
benchmarking multi-threaded functions. Like in Callgrind, each thread/subprocess
in DHAT is treated as a separate unit and thus requires `frames` (the Gungraun
specific approximation of Callgrind toggles) in addition to the default entry
point to include the interesting ones in the measurements.

By example. Suppose there's a function in the `gungraun_tests` library
`find_primes_multi_thread(num_threads: usize)` which searches for primes in the
range `0` - `10000 * num_threads`. This multi-threaded function is splitting the
work for each `10000` numbers into a separate thread each calling the
single-threaded function `gungraun_tests::find_primes` which does the actual
work. The inner workings aren't important but this description should be enough
to understand the basic idea.

```rust
# extern crate gungraun;
# mod gungraun_tests { pub fn find_primes_multi_thread (_: u64) -> Vec<u64> { vec![] } }
use std::hint::black_box;
use gungraun::prelude::*;
use gungraun::Tool;

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
)]
fn bench_library() -> Vec<u64> {
    black_box(gungraun_tests::find_primes_multi_thread(black_box(1)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Running the benchmark produces the following output:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_find_primes::my_group::bench_library</span>
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                        <b>11464</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total blocks:                           <b>9</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax bytes:                    <b>10264</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>4</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Reads bytes:                          <b>776</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Writes bytes:                       <b>10337</b>|N/A                  (<span style="color:#555">*********</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.55922s</code></pre>

The problem here is that the spawned thread is not included in the metrics. The
DHAT output files shown in `dh_view.html` also do not show any threads because
DHAT output files are sanitized by default. The output in this example is
shortened to save space:

```text
Invocation {
  Mode:    heap
  Command: /fast/lenny/workspace/programming/gungraun/gungraun/target/release/deps/lib_bench_find_primes-f6831ae18d944771 --gungraun-run 00000 00000 00000
  PID:     1971323
}

Times {
  t-gmax: 2,942,796 instrs (99.58% of program duration)
  t-end:  2,955,202 instrs
}

▼ PP 1/1 (8 children) {
    Total:     11,464 bytes (100%, 3,879.26/Minstr) in 9 blocks (100%, 3.05/Minstr), avg size 1,273.78 bytes, avg lifetime 1,657,484.56 instrs (56.09% of program duration)
    At t-gmax: 10,264 bytes (100%) in 4 blocks (100%), avg size 2,566 bytes
    At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
    Reads:     776 bytes (100%, 262.59/Minstr), 0.07/byte
    Writes:    10,337 bytes (100%, 3,497.9/Minstr), 0.9/byte
    Allocated at {
      #0: [root]
    }
  }
  ├─▼ PP 1.1/8 (2 children) {
  │     Total:     9,928 bytes (86.6%, 3,359.5/Minstr) in 2 blocks (22.22%, 0.68/Minstr), avg size 4,964 bytes, avg lifetime 1,244,629 instrs (42.12% of program duration)
  │     At t-gmax: 9,928 bytes (96.73%) in 2 blocks (50%), avg size 4,964 bytes
  │     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
  │     Reads:     24 bytes (3.09%, 8.12/Minstr), 0/byte
  │     Writes:    9,856 bytes (95.35%, 3,335.14/Minstr), 0.99/byte
  │     Allocated at {
  │       #1: 0x40522FC: alloc (alloc.rs:95)
  │       #2: 0x40522FC: alloc_impl_runtime (alloc.rs:190)
  │       #3: 0x40522FC: alloc_impl (alloc.rs:312)
  │       #4: 0x40522FC: allocate (alloc.rs:429)
  │       #5: 0x40522FC: alloc::raw_vec::RawVecInner<A>::finish_grow (mod.rs:558)
  │     }
  │   }
  │   ├── PP 1.1.1/2 {
  │   │     Total:     9,832 bytes (85.76%, 3,327.01/Minstr) in 1 blocks (11.11%, 0.34/Minstr), avg size 9,832 bytes, avg lifetime 6,263 instrs (0.21% of program duration)
  │   │     Max:       9,832 bytes in 1 blocks, avg size 9,832 bytes
  │   │     At t-gmax: 9,832 bytes (95.79%) in 1 blocks (25%), avg size 9,832 bytes
  │   │     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
  │   │     Reads:     0 bytes (0%, 0/Minstr), 0/byte
  │   │     Writes:    9,832 bytes (95.11%, 3,327.01/Minstr), 1/byte
  │   │     Allocated at {
  │   │       ^1: 0x40522FC: alloc (alloc.rs:95)
  │   │       ^2: 0x40522FC: alloc_impl_runtime (alloc.rs:190)
  │   │       ^3: 0x40522FC: alloc_impl (alloc.rs:312)
  │   │       ^4: 0x40522FC: allocate (alloc.rs:429)
  │   │       ^5: 0x40522FC: alloc::raw_vec::RawVecInner<A>::finish_grow (mod.rs:558)
  │   │       #6: 0x4052393: grow_amortized<alloc::alloc::Global> (mod.rs:527)
  │   │       #7: 0x4052393: alloc::raw_vec::RawVecInner<A>::reserve::do_reserve_and_handle (mod.rs:666)
  │   │       #8: 0x4050284: reserve<alloc::alloc::Global> (mod.rs:673)
  │   │       #9: 0x4050284: reserve<u64, alloc::alloc::Global> (mod.rs:340)
  │   │       #10: 0x4050284: reserve<u64, alloc::alloc::Global> (mod.rs:1446)
  │   │       #11: 0x4050284: append_elements<u64, alloc::alloc::Global> (mod.rs:2879)
  │   │       #12: 0x4050284: spec_extend<u64, alloc::alloc::Global, alloc::alloc::Global> (spec_extend.rs:34)
  │   │       #13: 0x4050284: extend<u64, alloc::alloc::Global, alloc::vec::Vec<u64, alloc::alloc::Global>> (mod.rs:3933)
  │   │       #14: 0x4050284: gungraun_tests::find_primes_multi_thread (lib.rs:49)
  │   │       #15: 0x404D140: lib_bench_find_primes::bench_library::__gungraun_wrapper_mod::bench_library (lib_bench_find_primes.rs:16)
  │   │       #16: 0x404D158: lib_bench_find_primes::bench_library::__gungraun_wrapper_id_mod::wrapper (lib_bench_find_primes.rs:15)
  │   │       #17: 0x404D0F1: lib_bench_find_primes::bench_library::__run_wrapper (lib_bench_find_primes.rs:6)
  │   │       #18: 0x404E331: lib_bench_find_primes::main (macros.rs:588)
  │   │       #19: 0x404D1F2: call_once<fn(), ()> (function.rs:250)
  │   │       #20: 0x404D1F2: std::sys::backtrace::__rust_begin_short_backtrace (backtrace.rs:166)
  │   │       #21: 0x404D1E8: std::rt::lang_start::{{closure}} (rt.rs:206)
  │   │       #22: 0x4085263: call_once<(), (dyn core::ops::function::Fn<(), Output=i32> + core::marker::Sync + core::panic::unwind_safe::RefUnwindSafe)> (function.rs:287)
  │   │       #23: 0x4085263: do_call<&(dyn core::ops::function::Fn<(), Output=i32> + core::marker::Sync + core::panic::unwind_safe::RefUnwindSafe), i32> (panicking.rs:581)
  │   │       #24: 0x4085263: catch_unwind<i32, &(dyn core::ops::function::Fn<(), Output=i32> + core::marker::Sync + core::panic::unwind_safe::RefUnwindSafe)> (panicking.rs:544)
  │   │       #25: 0x4085263: catch_unwind<&(dyn core::ops::function::Fn<(), Output=i32> + core::marker::Sync + core::panic::unwind_safe::RefUnwindSafe), i32> (panic.rs:359)
  │   │       #26: 0x4085263: {closure#0} (rt.rs:175)
  │   │       #27: 0x4085263: do_call<std::rt::lang_start_internal::{closure_env#0}, isize> (panicking.rs:581)
  │   │       #28: 0x4085263: catch_unwind<isize, std::rt::lang_start_internal::{closure_env#0}> (panicking.rs:544)
  │   │       #29: 0x4085263: catch_unwind<std::rt::lang_start_internal::{closure_env#0}, isize> (panic.rs:359)
  │   │       #30: 0x4085263: std::rt::lang_start_internal (rt.rs:171)
  │   │       #31: 0x404F7FB: main (in /fast/lenny/workspace/programming/gungraun/gungraun/target/release/deps/lib_bench_find_primes-f6831ae18d944771)
  │   │     }
  │   │   }
  ...
```

To actually see all program points that DHAT records you need to either run
without sanitization or keep the original files and inspect those. We're going
for the latter, which conveniently lets us inspect the sanitized _and_ original
output files.

```rust
# extern crate gungraun;
# mod gungraun_tests { pub fn find_primes_multi_thread (_: u64) -> Vec<u64> { vec![] } }
# use std::hint::black_box;
# use gungraun::prelude::*;
# use gungraun::{Dhat, Tool, SanitizeOutput};
#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .tool(Dhat::default().sanitize_output(SanitizeOutput::KeepOrig))
)]
fn bench_library() -> Vec<u64> {
    black_box(gungraun_tests::find_primes_multi_thread(black_box(1)))
}
# fn main() {}
```

After running the benchmark again, the `dhat.*.out.orig` file (also shortened)
includes the metrics of the thread:

```text
Invocation {
  Mode:    heap
  Command: /fast/lenny/workspace/programming/gungraun/gungraun/target/release/deps/lib_bench_find_primes-f6831ae18d944771 --gungraun-run 00000 00000 00000
  PID:     2007925
}

Times {
  t-gmax: 2,940,202 instrs (99.58% of program duration)
  t-end:  2,952,524 instrs
}

▼ PP 1/1 (19 children) {
    Total:     47,327 bytes (100%, 16,029.34/Minstr) in 38 blocks (100%, 12.87/Minstr), avg size 1,245.45 bytes, avg lifetime 863,000.68 instrs (29.23% of program duration)
    At t-gmax: 27,326 bytes (100%) in 10 blocks (100%), avg size 2,732.6 bytes
    At t-end:  544 bytes (100%) in 1 blocks (100%), avg size 544 bytes
    Reads:     45,739 bytes (100%, 15,491.49/Minstr), 0.97/byte
    Writes:    48,163 bytes (100%, 16,312.48/Minstr), 1.02/byte
    Allocated at {
      #0: [root]
    }
  }
  ├── PP 1.1/19 {
  │     Total:     32,736 bytes (69.17%, 11,087.46/Minstr) in 10 blocks (26.32%, 3.39/Minstr), avg size 3,273.6 bytes, avg lifetime 248,000.9 instrs (8.4% of program duration)
  │     Max:       16,384 bytes in 1 blocks, avg size 16,384 bytes
  │     At t-gmax: 16,384 bytes (59.96%) in 1 blocks (10%), avg size 16,384 bytes
  │     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
  │     Reads:     26,184 bytes (57.25%, 8,868.34/Minstr), 0.8/byte
  │     Writes:    26,184 bytes (54.37%, 8,868.34/Minstr), 0.8/byte
  │     Allocated at {
  │       #1: 0x4052446: alloc (alloc.rs:95)
  │       #2: 0x4052446: alloc_impl_runtime (alloc.rs:190)
  │       #3: 0x4052446: alloc_impl (alloc.rs:312)
  │       #4: 0x4052446: allocate (alloc.rs:429)
  │       #5: 0x4052446: try_allocate_in<alloc::alloc::Global> (mod.rs:464)
  │       #6: 0x4052446: with_capacity_in<alloc::alloc::Global> (mod.rs:433)
  │       #7: 0x4052446: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:177)
  │       #8: 0x4052446: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:965)
  │       #9: 0x4052446: with_capacity<u64> (mod.rs:524)
  │       #10: 0x4052446: <alloc::vec::Vec<T> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<T,I>>::from_iter (spec_from_iter_nested.rs:30)
  │       #11: 0x40507F0: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, gungraun_tests::find_primes::{closure_env#0}>> (spec_from_iter.rs:33)
  │       #12: 0x40507F0: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, gungraun_tests::find_primes::{closure_env#0}>> (mod.rs:3865)
  │       #13: 0x40507F0: collect<core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, gungraun_tests::find_primes::{closure_env#0}>, alloc::vec::Vec<u64, alloc::alloc::Global>> (iterator.rs:2064)
  │       #14: 0x40507F0: gungraun_tests::find_primes (lib.rs:33)
  │       #15: 0x4052BD9: {closure#0} (lib.rs:98)
  │       #16: 0x4052BD9: std::sys::backtrace::__rust_begin_short_backtrace (backtrace.rs:166)
  │       #17: 0x4051568: {closure#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>> (lifecycle.rs:91)
  │       #18: 0x4051568: call_once<alloc::vec::Vec<u64, alloc::alloc::Global>, std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>> (unwind_safe.rs:274)
  │       #19: 0x4051568: do_call<core::panic::unwind_safe::AssertUnwindSafe<std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panicking.rs:581)
  │       #20: 0x4051568: catch_unwind<alloc::vec::Vec<u64, alloc::alloc::Global>, core::panic::unwind_safe::AssertUnwindSafe<std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>>> (panicking.rs:544)
  │       #21: 0x4051568: catch_unwind<core::panic::unwind_safe::AssertUnwindSafe<std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panic.rs:359)
  │       #22: 0x4051568: {closure#1}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>> (lifecycle.rs:89)
  │       #23: 0x4051568: core::ops::function::FnOnce::call_once{{vtable.shim}} (function.rs:250)
  │       #24: 0x408D0FE: call_once<(), (dyn core::ops::function::FnOnce<(), Output=()> + core::marker::Send), alloc::alloc::Global> (boxed.rs:2240)
  │       #25: 0x408D0FE: <std::sys::thread::unix::Thread>::new::thread_start (unix.rs:118)
  │       #26: 0x49F81B8: ??? (in /usr/lib/libc.so.6)
  │       #27: 0x4A7D043: clone (in /usr/lib/libc.so.6)
  │     }
  │   }

  ...
```

The missing metrics of the thread are caused by the default entry point which
only includes the program points with the benchmark function in their call
stack. But, looking closely at the program point `PP 1.1.1/12` and the call
stack, there's no frame of the benchmark function `bench_library` or a `main`
function. As mentioned earlier, this is because the thread is completely
separated by DHAT.

There are multiple ways to go on depending on what we want to measure. To show
two different approaches, at first, I'll go with measuring the benchmark
function with the function spawning the threads (the default entry point which
doesn't have to be specified) and additionally all threads which execute the
`gungraun_tests::find_primes` function.

```rust
# extern crate gungraun;
# mod gungraun_tests { pub fn find_primes_multi_thread (_: u64) -> Vec<u64> { vec![] } }
use std::hint::black_box;
use gungraun::prelude::*;
use gungraun::{Dhat, Tool};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .tool(Dhat::default()
            .frames(["gungraun_tests::find_primes"])
        )
)]
fn bench_library() -> Vec<u64> {
    black_box(gungraun_tests::find_primes_multi_thread(black_box(1)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Now, the metrics include the spawned thread(s):

<pre><code class="hljs"><span style="color:#0A0">lib_bench_find_primes::my_group::bench_library</span>
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                        <b>44208</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Total blocks:                          <b>19</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-gmax bytes:                    <b>26648</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>5</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Reads bytes:                        <b>26992</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Writes bytes:                       <b>36529</b>|N/A                  (<span style="color:#555">No change</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.48695s</code></pre>

If we were only interested in the threads themselves, then using
`EntryPoint::Custom` would be one way to do it. Setting a custom entry point is
syntactic sugar for disabling the entry point with `EntryPoint::None` and
specifying a frame with `Dhat::frames`:

```rust
# extern crate gungraun;
# mod gungraun_tests { pub fn find_primes_multi_thread (_: u64) -> Vec<u64> { vec![] } }
use std::hint::black_box;
use gungraun::prelude::*;
use gungraun::{Dhat, EntryPoint, Tool};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(Tool::DHAT)
        .tool(Dhat::default()
            .entry_point(
                EntryPoint::Custom("gungraun_tests::find_primes".to_owned())
            )
        )
)]
fn bench_library() -> Vec<u64> {
    black_box(gungraun_tests::find_primes_multi_thread(black_box(1)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Running this benchmark results in:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_find_primes::my_group::bench_library</span>
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                        <b>32736</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total blocks:                          <b>10</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax bytes:                    <b>16384</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>1</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Reads bytes:                        <b>26184</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Writes bytes:                       <b>26184</b>|N/A                  (<span style="color:#555">*********</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.45178s</code></pre>

Compare these numbers with the thread program point from the original output
shown above. The sanitized output file then contains only that program point:

```text
  Invocation {
  Mode:    heap
  Command: /fast/lenny/workspace/programming/gungraun/gungraun/target/release/deps/lib_bench_find_primes-f6831ae18d944771 --gungraun-run 00000 00000 00000
  PID:     1941428
}

Times {
  t-gmax: 2,940,191 instrs (99.58% of program duration)
  t-end:  2,952,454 instrs
}

─ PP 1/1 {
    Total:     32,736 bytes (100%, 11,087.73/Minstr) in 10 blocks (100%, 3.39/Minstr), avg size 3,273.6 bytes, avg lifetime 248,000.9 instrs (8.4% of program duration)
    At t-gmax: 16,384 bytes (100%) in 1 blocks (100%), avg size 16,384 bytes
    At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
    Reads:     26,184 bytes (100%, 8,868.55/Minstr), 0.8/byte
    Writes:    26,184 bytes (100%, 8,868.55/Minstr), 0.8/byte
    Allocated at {
      #0: [root]
      #1: 0x4052536: alloc (alloc.rs:95)
      #2: 0x4052536: alloc_impl_runtime (alloc.rs:190)
      #3: 0x4052536: alloc_impl (alloc.rs:312)
      #4: 0x4052536: allocate (alloc.rs:429)
      #5: 0x4052536: try_allocate_in<alloc::alloc::Global> (mod.rs:464)
      #6: 0x4052536: with_capacity_in<alloc::alloc::Global> (mod.rs:433)
      #7: 0x4052536: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:177)
      #8: 0x4052536: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:965)
      #9: 0x4052536: with_capacity<u64> (mod.rs:524)
      #10: 0x4052536: <alloc::vec::Vec<T> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<T,I>>::from_iter (spec_from_iter_nested.rs:30)
      #11: 0x40508E0: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, gungraun_tests::find_primes::{closure_env#0}>> (spec_from_iter.rs:33)
      #12: 0x40508E0: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, gungraun_tests::find_primes::{closure_env#0}>> (mod.rs:3865)
      #13: 0x40508E0: collect<core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, gungraun_tests::find_primes::{closure_env#0}>, alloc::vec::Vec<u64, alloc::alloc::Global>> (iterator.rs:2064)
      #14: 0x40508E0: gungraun_tests::find_primes (lib.rs:33)
      #15: 0x4052CC9: {closure#0} (lib.rs:98)
      #16: 0x4052CC9: std::sys::backtrace::__rust_begin_short_backtrace (backtrace.rs:166)
      #17: 0x4051658: {closure#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>> (lifecycle.rs:91)
      #18: 0x4051658: call_once<alloc::vec::Vec<u64, alloc::alloc::Global>, std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>> (unwind_safe.rs:274)
      #19: 0x4051658: do_call<core::panic::unwind_safe::AssertUnwindSafe<std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panicking.rs:581)
      #20: 0x4051658: catch_unwind<alloc::vec::Vec<u64, alloc::alloc::Global>, core::panic::unwind_safe::AssertUnwindSafe<std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>>> (panicking.rs:544)
      #21: 0x4051658: catch_unwind<core::panic::unwind_safe::AssertUnwindSafe<std::thread::lifecycle::spawn_unchecked::{closure#1}::{closure_env#0}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panic.rs:359)
      #22: 0x4051658: {closure#1}<gungraun_tests::find_primes_multi_thread::{closure_env#1}, alloc::vec::Vec<u64, alloc::alloc::Global>> (lifecycle.rs:89)
      #23: 0x4051658: core::ops::function::FnOnce::call_once{{vtable.shim}} (function.rs:250)
      #24: 0x408D24E: call_once<(), (dyn core::ops::function::FnOnce<(), Output=()> + core::marker::Send), alloc::alloc::Global> (boxed.rs:2240)
      #25: 0x408D24E: <std::sys::thread::unix::Thread>::new::thread_start (unix.rs:118)
      #26: 0x49F81B8: ??? (in /usr/lib/libc.so.6)
      #27: 0x4A7D043: clone (in /usr/lib/libc.so.6)
    }
  }

PP significance threshold: total >= 0.1 blocks (1%)
```

[dhat]: https://valgrind.org/docs/manual/dh-manual.html
