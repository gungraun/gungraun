<!-- spell-checker: ignore iotop iostat -->

# Running Benchmarks in Parallel

Gungraun supports running multiple benchmarks in parallel to reduce total
benchmark execution time. This is controlled by the `--parallel` command-line
option or `GUNGRAUN_PARALLEL` environment variable.

## Basic Usage

The `--parallel` option accepts either a positive integer or `auto`:

```bash
# Run up to 4 benchmarks in parallel
cargo bench -- --parallel=4

# Let Gungraun choose based on available CPU cores
cargo bench -- --parallel=auto
```

When `--parallel=auto` is used, Gungraun sets the parallelism level to the
number of available logical CPU cores. The default value is `1`, which runs
benchmarks serially.

You can also set the parallelism level via the environment variable
`GUNGRAUN_PARALLEL`:

```bash
GUNGRAUN_PARALLEL=4 cargo bench
```

The command-line `--parallel` option takes precedence over the environment
variable.

## Benefits

Running benchmarks in parallel can significantly reduce total execution time,
especially when:

- You have many independent benchmarks
- Benchmarks are not I/O-bound (reading files, network operations)
- You have multiple CPU cores available
- Benchmarks have varying execution times (better resource utilization)

## How Parallel Execution Works

### Benchmark Groups as Synchronization Points

Benchmark groups serve as synchronization barriers. Only benchmarks **within the
same group** are executed in parallel. Gungraun waits for all benchmarks in a
group to complete before moving on to the next group.

For example, given the following structure:

```rust
# extern crate gungraun;
use gungraun::prelude::*;

#[library_benchmark]
fn bench_a() { /* ... */ }

#[library_benchmark]
fn bench_b() { /* ... */ }

library_benchmark_group!(
    name = group_1,
    benchmarks = [bench_a, bench_b]
);

#[library_benchmark]
fn bench_c() { /* ... */ }

library_benchmark_group!(
    name = group_2,
    benchmarks = [bench_c]
);
# fn main() {}
```

With `--parallel=4`, `bench_a` and `bench_b` would run in parallel (since
they're in `group_1`), but `bench_c` would only start after both `bench_a` and
`bench_b` complete.

### Setup and Teardown

Group-level `setup` and `teardown` functions are executed **before** and
**after** all benchmarks in that group, respectively. They are not run in
parallel with the benchmarks themselves. However, if a benchmark has its own
`setup` and `teardown` functions, they are run in parallel to other benchmarks
(see [Setup and Teardown Interactions](#setup-and-teardown-interactions))

### Process Isolation

Each benchmark runs in its **own separate process**, which provides strong
isolation guarantees:

- **Independent environment variables**: Each process has its own set of
  environment variables. Modifying `std::env` in one benchmark doesn't affect
  others.
- **Isolated global state**: Global variables, static variables, and lazy static
  values are not shared between benchmarks. Each process has its own copy.
- **Separate memory space**: Each process has its own virtual memory space.
  Memory allocations in one benchmark don't affect others.
- **Independent file descriptors**: Each process has its own file descriptor
  table. Opening/closing files in one benchmark doesn't interfere with others.

**This simplifies parallel execution significantly:**

```rust
# extern crate gungraun;
use gungraun::prelude::*;
// This is SAFE with --parallel - each benchmark gets its own process
static mut COUNTER: usize = 0;

#[library_benchmark]
fn bench_a() {
    unsafe { COUNTER += 1; } // Only affects this process's COUNTER
    // COUNTER == 1
}

#[library_benchmark]
fn bench_b() {
    unsafe { COUNTER += 1; } // Different process, different COUNTER
    // COUNTER == 1
}
# fn main() {}
```

**What you DON'T need to worry about:**

- Race conditions on global variables between benchmarks
- Environment variable conflicts between benchmarks
- Memory corruption from one benchmark affecting others
- File descriptor exhaustion (each process has its own limit)

**What you STILL need to worry about:**

- Shared external resources (files on disk, network ports, databases)
- Setup/teardown interactions (if they modify shared filesystem state)
- Disk I/O contention (all processes write to the same disk)

This process isolation is a key advantage over thread-based parallelism, where
shared state requires careful synchronization.

## Limitations and Dangers

### Disk I/O Bottleneck

**This is the most common limitation.**

Valgrind and Gungraun perform disk I/O even if your benchmarks don't. They
write:

- Log files for each tool
- Profile data files
- Flamegraph data
- Summary JSON files

Running with high parallelism may not provide proportional speedup because disk
I/O becomes the bottleneck. For example, running with `--parallel=10` may
provide similar speedup as `--parallel=5` on systems with limited disk bandwidth
(especially HDDs or network-mounted filesystems).

**Recommendation**: Start with `--parallel=2` or `--parallel=4` and increase
gradually while monitoring actual speedup.

### Memory Usage

Each parallel benchmark runs its own Valgrind instance, which adds considerable
memory overhead beyond your benchmark's normal usage. Valgrind's memory overhead
varies depending on:

- **The tool used**: Memcheck typically has higher overhead than Callgrind or
  Cachegrind
- **Your program's memory usage**: Valgrind's DHAT, Massif, ... tracks
  allocations, so larger programs have more overhead

### Resource Contention

Parallel benchmarks compete for:

- **Memory bandwidth**: Especially for memory-intensive benchmarks
- **File descriptors**: Each Valgrind instance opens multiple files
- **Disk I/O bandwidth**: As mentioned above

This can lead to:

- Inconsistent measurements across runs
- Higher variance in results
- Slower individual benchmark times (even if total time is reduced)

**Recommendation**: If you notice high variance or inconsistent results, reduce
parallelism.

### Setup and Teardown Interactions

Benchmarks within a group which share resources through `setup`/`teardown` or in
themselves cause conflicts:

- **File system state**: Benchmarks may interfere with each other's files
- **Network ports**: Multiple benchmarks trying to bind the same port

**Example of a problematic scenario**:

```rust
# extern crate gungraun;
use std::path::PathBuf;
use gungraun::prelude::*;

const DIR: &str = "/tmp/benchmark_temp";

fn setup() -> PathBuf {
    std::fs::create_dir_all(DIR).unwrap();
    PathBuf::from(DIR)
}

fn teardown(_: ()) {
    std::fs::remove_dir_all(DIR).unwrap();
}

#[library_benchmark(setup = setup, teardown = teardown)]
fn bench_a(path: PathBuf) {
# let _ = path;
    /* ... */
}

#[library_benchmark(setup = setup, teardown = teardown)]
fn bench_b(path: PathBuf) {
# let _ = path;
    /* ... */
}

// If these run in parallel, they'll conflict on /tmp/benchmark_temp
library_benchmark_group!(
    name = my_group,
    benchmarks = [bench_a, bench_b]
);
# fn main() { }
```

**Recommendation**: Use unique identifiers for shared resources, or avoid
parallel execution for such groups (See
[Limiting Parallelism Per Group](#limiting-parallelism-per-group))

### Benchmarks with Internal Threading

Benchmarks that spawn their own threads present unique challenges when run with
`--parallel` and it should be avoided.

#### How Valgrind Handles Threads

Valgrind fully supports multi-threaded programs but serializes thread execution
with locks. Even though your multi-threaded program uses the threading, Valgrind
ensures only **one kernel thread runs at a time**. This means:

- Multi-threaded programs run on a **single CPU core** under Valgrind
- Thread scheduling is fundamentally different from native execution
- Your program sees "different scheduling" compared to normal runs
- The OS kernel still controls scheduling, but Valgrind serializes access

**From the Valgrind documentation:**

> "Valgrind serializes execution so that only one (kernel) thread is running at
> a time. This approach avoids the horrible implementation problems of
> implementing a truly multithreaded version of Valgrind, but it does mean that
> threaded apps run only on one CPU, even if you have a multiprocessor or
> multi-core machine."

#### The Problem: Variadic Metrics

When you combine:

1. **Gungraun's parallel execution** (`--parallel=N`)
2. **Benchmark's internal threading** (spawns M threads)
3. **Valgrind's serialization** (only 1 thread runs at a time)

You can get **highly variable, non-reproducible metrics**.

#### Example Scenario

```rust
# extern crate gungraun;
# fn compute_intensively() {}
use gungraun::prelude::*;
use std::thread;
// Benchmark spawns 8 threads internally
#[library_benchmark]
fn threaded_work() {
    let handles: Vec<_> = (0..8)
        .map(|_| thread::spawn(|| {
            compute_intensively();
        }))
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
# fn main() {}
```

With `--parallel=4`:

- 4 parallel benchmarks (Gungraun level)
- Each spawning 8 threads (benchmark level) serialized through Valgrind's single
  execution context
- **Total: 32 concurrent threads**
- **Result: Variable metrics across runs**

**Recommendation**: Run threaded benchmarks serially

## Error Handling

When running benchmarks in parallel, error handling follows a cooperative
cancellation model:

### What Triggers Shutdown

The parallel execution is immediately terminated when:

- **A benchmark fails**: Any error returned by a benchmark job triggers shutdown
- **Regression with `--regression-fail-fast`**: Performance regressions cause
  the shutdown sequence when this option is enabled

### Shutdown Sequence

When an error occurs:

1. **SIGTERM sent**: All running benchmark processes receive SIGTERM (graceful
   termination signal)
1. **Grace period**: Processes have about 5 seconds to terminate gracefully
1. **SIGKILL sent**: If processes don't terminate after the grace period, they
   receive SIGKILL (forced termination)
1. **Job draining**: Queued jobs are skipped, but running jobs are allowed to
   finish their shutdown sequence
1. **Temporary output files** are copied to the benchmark directory. This
   ensures you can inspect partial results even when benchmarks fail and log
   files remain available for debugging

### Implications

- **No partial results**: When one benchmark fails, all other running benchmarks
  are terminated
- **Clean state**: The shutdown sequence ensures processes don't become orphaned
- **Debugging**: Check the log files in the benchmark output directory to
  identify which benchmark failed

### Example Scenario

With `--parallel=8` running 8 benchmarks simultaneously:

1. Benchmark #3 encounters an error
2. Benchmarks #1, #2, #4-#8 receive SIGTERM
3. All processes terminate gracefully (or are killed after grace period)
4. Remaining queued benchmarks are skipped
5. Error from benchmark #3 is reported
6. Temporary files are copied for inspection

## Limiting Parallelism Per Group

Sometimes you may want to run most benchmarks in parallel but need to disable or
limit parallelism for specific benchmarks. For example, when:

- A group contains benchmarks that conflict with each other (shared files,
  ports)
- A group has benchmarks with internal threading that produce variable results
- You want to isolate problematic benchmarks

The `max_parallel` parameter in `library_benchmark_group!` and
`binary_benchmark_group!` lets you control parallelism at the group level:

```rust
# extern crate gungraun;
# use gungraun::prelude::*;
# #[library_benchmark] fn bench_a() {}
# #[library_benchmark] fn bench_b() {}
# #[library_benchmark] fn bench_c() {}
# #[library_benchmark] fn bench_limited_a() {}
# #[library_benchmark] fn bench_limited_b() {}
# #[library_benchmark] fn bench_serial_a() {}
# #[library_benchmark] fn bench_serial_b() {}
// This group runs with full parallelism (up to --parallel value)
library_benchmark_group!(
    name = parallel_safe,
    benchmarks = [bench_a, bench_b, bench_c]
);

// This group is limited to at most 2 parallel benchmarks
library_benchmark_group!(
    name = limited_parallel,
    max_parallel = 2,
    benchmarks = [bench_limited_a, bench_limited_b]
);

// This group runs serially (no parallelism)
library_benchmark_group!(
    name = needs_isolation,
    max_parallel = 1,
    benchmarks = [bench_serial_a, bench_serial_b]
);
# fn main() {}
```

### How `max_parallel` Values Work

| Value            | Behavior                                    |
| ---------------- | ------------------------------------------- |
| Not specified    | No limit (uses `--parallel` value)          |
| `0`              | No limit (same as not specifying)           |
| `1`              | Serial execution, including post-processing |
| `N` where N >= 2 | Limit to at most N parallel benchmarks      |

### Important Notes

- **Only effective with parallel execution enabled**: The `max_parallel`
  parameter has no effect unless parallel execution is enabled via `--parallel`
  or `GUNGRAUN_PARALLEL`. Benchmarks run serially by default.
- **Per-group setting**: Each group can have its own `max_parallel` value,
  allowing fine-grained control.
- **Groups still run sequentially**: Even with `max_parallel`, groups themselves
  still execute one after another. Only benchmarks _within_ a group run in
  parallel (subject to the limit).

### Example: Mixing Parallel and Serial Groups

```rust
# extern crate gungraun;
# #[library_benchmark] fn fast_bench_1() {}
# #[library_benchmark] fn fast_bench_2() {}
# #[library_benchmark] fn threaded_bench() {}
# #[library_benchmark] fn io_bound_bench() {}
use gungraun::prelude::*;
// These can run in parallel with each other
library_benchmark_group!(
    name = fast_benches,
    benchmarks = [fast_bench_1, fast_bench_2]
);

// These benchmarks must run serially (spawns internal threads) and conflicts on
// file resources
library_benchmark_group!(
    name = threaded_and_io_bound,
    max_parallel = 1,
    benchmarks = [threaded_bench, io_bound_bench]
);

# fn main() {
main!(library_benchmark_groups = [fast_benches, threaded_and_io_bound]);
# }
```

Running with `--parallel=4`:

1. `fast_benches` runs with up to 4 parallel benchmarks
2. `threaded_and_io_bound` runs serially (waits for `fast_benches` to complete
   first)

## Conclusion

### Best Practices

- Create a baseline for the serial execution

    ```shell
    GUNGRAUN_SAVE_BASELINE=serial cargo bench
    ```

    then start experimenting and compare the benchmark runs with each other

    ```shell
    GUNGRAUN_BASELINE=serial GUNGRAUN_PARALLEL=2 cargo bench
    ```

- Start conservatively with `--parallel=2`, then `--parallel=4`, ...

### Isolate Problematic Benchmarks

If some benchmarks don't work well in parallel, isolate them in their own group
which [limits parallelism](#limiting-parallelism-per-group):

```rust
# extern crate gungraun;
# #[library_benchmark] fn bench_a() {}
# #[library_benchmark] fn bench_b() {}
# #[library_benchmark] fn bench_c() {}
# #[library_benchmark] fn problematic_bench() {}
use gungraun::prelude::*;
// These can run in parallel
library_benchmark_group!(
    name = parallel_safe,
    benchmarks = [bench_a, bench_b, bench_c]
);

// The benchmarks in this group run serially
library_benchmark_group!(
    name = needs_isolation,
    max_parallel = 1,
    benchmarks = [problematic_bench]
);
# fn main() {}
```

#### Consider Your Storage or CI Environment

- **SSD**: Can handle higher parallelism
- **HDD**: Lower parallelism (2-4) due to seek times
- **Network storage**: Very low parallelism or serial execution

### When to Avoid Parallel Execution

Avoid `--parallel` (or use `--parallel=1`) when:

- Benchmarks are heavily I/O-bound
- Benchmarks write to the same files or directories
- Benchmarks bind to the same network ports
- You need 100% deterministic, reproducible results for comparison
- You're debugging a benchmark
- System resources are limited
