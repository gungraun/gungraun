<!-- TODO: This is just a collection of ideas for the final docs -->

# Benchmarking with Perf

Perf is supported only when the benchmark is compiled for Linux. Target support
is separate from runtime availability: the `perf` executable must be available
in the execution environment, and kernel security settings must permit the
requested events. Missing executables, insufficient permissions, and invalid
events are reported as runtime errors.

- Despite being centered in the past on Valgrind and on one-shot benchmark, Perf
  is a first-class citizen
- Perf bridges the gap between synthetic cpu profiles to hardware counters
- Perf suffers under the same disadvantages as wall-clock benchmarking. Perf
  benchmarking is far closer to wall-clock benchmarking than to benchmarking
  with Valgrind.
- Perf offers a wide variety of hardware counters including real
  cycles/instructions/...
- Callgrind (Valgrind) is still the primary/default benchmarking tool.
- Benchmarking Best Practices links:
    - <https://github.com/JuliaCI/BenchmarkTools.jl/blob/main/docs/src/linuxtips.md>
    - <https://github.com/JuliaCI/julia-perf/blob/master/docs/perf-runner.md>
    - Link to nnethercote's Rust Performance Book
- Using sampling durations has advantages over fixed numbers. The time scales
  with the size of the problem.
    - fast benchmark functions need more sampling to smooth out outliers than a
      slow function. This happens naturally with a time budget.
    - It is usually nicer to work with a time budget than fixed numbers which
      let's estimate the overall time consumption of the benchmarks. I want to
      spend 3 seconds per function for outlier detection
- `perf` may additionally require suitable system permissions and setting for
  /proc/sys/kernel/perf_event_paranoid. But user-space benchmarking works fine
  without setting perf_event_paranoid
- Links to
    - <https://www.brendangregg.com/perf.html>
    - <https://perfwiki.github.io/main/>

`fp` and `dwarf` are two user-space stack unwinding methods for
`perf record --call-graph`.

**Frame Pointer (`fp`)** `fp` walks the call stack by following a frame-pointer
chain, usually `RBP` on x86-64. Each function keeps a pointer to the previous
stack frame, so unwinding is cheap and simple.

Pros:

- Low overhead.
- Small `perf.data`.
- Works well for profiling/benchmarks.
- Perf’s default for `-g` on most architectures.

Cons:

- Requires binaries to preserve frame pointers.
- Optimized builds often omit frame pointers by default.
- If frame pointers are missing, call graphs can be incomplete or wrong.

Example:

```sh
perf record -g ./bench
perf record --call-graph=fp ./bench
```

**DWARF (`dwarf`)** `dwarf` uses DWARF Call Frame Information emitted by the
compiler. Instead of relying on a frame-pointer chain, perf records user
registers and a chunk of the user stack for each sample, then unwinds later in
user space.

Pros:

- Can recover stacks even when frame pointers were omitted.
- Often better for optimized Rust/C/C++ binaries built without frame pointers.
- Does not require `-C force-frame-pointers=yes` or equivalent.

Cons:

- Higher overhead.
- Much larger `perf.data`.
- Default stack dump size is 8192 bytes in perf.
- Requires unwind/debug info quality.
- Can perturb benchmark results more than `fp`.

Example:

```sh
perf record --call-graph=dwarf ./bench
perf record --call-graph=dwarf,16384 ./bench
```

**Practical Recommendation** For Gungraun defaults, prefer `fp`/bare `-g`.

Use `dwarf` as an explicit opt-in when the user wants better stacks from
optimized binaries that lack frame pointers. For Rust specifically, a good
alternative is building with frame pointers and using `fp`, because that keeps
profiling overhead lower.

**Good Docs**

- Perf record man page: https://man7.org/linux/man-pages/man1/perf-record.1.html
  Look for `-g` and `--call-graph`.
- Perf report man page: https://man7.org/linux/man-pages/man1/perf-report.1.html
  Useful for understanding how recorded call graphs are displayed.
- Brendan Gregg perf examples: https://www.brendangregg.com/perf.html Practical
  perf usage and call graph examples.
- Fedora frame pointer proposal:
  https://fedoraproject.org/wiki/Changes/fno-omit-frame-pointer Good explanation
  of why frame pointers matter and why DWARF unwinding is more expensive.
- DWARF standard homepage: https://dwarfstd.org/ Reference for DWARF itself,
  though not perf-specific.

---

But if I would use fp I would need tell users to use -C
force-frame-pointers=yes. You said dwarf works better with Rust?. Since Gungraun
is a benchmarking tool, the binaries are usually compiled for performance and
optimized.

---

**󰧑 Reasoning 5s**

**Evaluating profiling tools and overhead tradeoffs**

<!-- -->

**󰧑 Reasoning**

**Balancing dwarf overhead with profiling accuracy**

<!-- -->

Yes, that changes the product tradeoff.

For optimized Rust binaries, `dwarf` usually gives better call graphs out of the
box than `fp`, because optimized Rust builds normally omit frame pointers unless
users opt in with something like:

```sh
RUSTFLAGS="-C force-frame-pointers=yes"
```

or Cargo profile config:

```toml
[profile.release]
debug = true

[profile.bench]
debug = true
```

plus frame pointers via `RUSTFLAGS` or config where supported.

But for a benchmarking tool, “better stacks” is not the only criterion. `dwarf`
can add enough sampling overhead and data volume that it is less suitable as a
silent default if the recorded run is also the measured run.

I’d split the recommendation like this:

**If `perf record` is primarily for profiling output** Use `--call-graph=dwarf`
as the default. It gives users useful Rust call stacks without requiring them to
know about frame pointers. This is the better UX for flamegraphs/profile
diagnosis.

**If `perf record` is part of benchmark measurement** Do not default to `dwarf`.
Prefer `fp` or no call graph by default, because measurement perturbation
matters more. Tell users to use either:

```sh
-C force-frame-pointers=yes
```

with `fp`, or explicitly opt into:

```sh
--call-graph=dwarf
```

when stack quality matters more than overhead.

**My adjusted Gungraun recommendation** If Gungraun’s `perf record` mode is an
auxiliary profiling mode, not the authoritative benchmark timing/counting
source, I would default to:

```sh
--call-graph=dwarf
```

because optimized Rust users will otherwise get disappointing call graphs.

If the same run is used for benchmark results, I would not silently default to
dwarf. I’d make the default conservative and expose a documented `dwarf` opt-in.

A good compromise is:

- Default profiling preset: `--call-graph=dwarf`
- Default measurement preset: no call graph or `-g`/`fp`
- Documentation note: `fp` needs frame pointers; `dwarf` works better for
  optimized Rust stacks but has higher overhead

So: your intuition is right. For optimized Rust, `dwarf` is more likely to work
well without extra user setup. The question is whether Gungraun values “useful
profiling by default” or “minimal perturbation by default” for this mode.
