//! Common structs for `bin_bench` and `lib_bench`

use derive_more::AsRef;
use iai_callgrind_macros::IntoInner;

use super::{Direction, EventKind, FlamegraphKind, ValgrindTool, __internal};

/// The `FlamegraphConfig` which allows the customization of the created flamegraphs
///
/// Callgrind flamegraphs are very similar to `callgrind_annotate` output. In contrast to
/// `callgrind_annotate` text based output, the produced flamegraphs are svg files (located in the
/// `target/iai` directory) which can be viewed in a browser.
///
/// # Experimental
///
/// Note the following considerations only affect flamegraphs of multi-threaded/multi-process
/// benchmarks and benchmarks which produce multiple parts with a total over all sub-metrics.
///
/// Currently, Iai-Callgrind creates the flamegraphs only for the total over all threads/parts and
/// subprocesses. This leads to complications since the call graph is not be fully recovered just by
/// examining each thread/subprocess separately. So, the total metrics in the flamegraphs might not
/// be the same as the total metrics shown in the terminal output. If in doubt, the terminal output
/// shows the the correct metrics.
///
/// # Examples
///
/// ```rust
/// # use iai_callgrind::{library_benchmark, library_benchmark_group};
/// use iai_callgrind::{LibraryBenchmarkConfig, FlamegraphConfig, main};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group; benchmarks = some_func);
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default()
///                 .flamegraph(FlamegraphConfig::default());
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, Default, IntoInner, AsRef)]
pub struct FlamegraphConfig(__internal::InternalFlamegraphConfig);

/// Configure the default output format of the terminal output of Iai-Callgrind
///
/// This configuration is only applied to the default output format (`--output-format=default`) and
/// not to any of the json output formats like (`--output-format=json`).
///
/// # Examples
///
/// For example configure the truncation length of the description to `200` for all library
/// benchmarks in the same file with [`OutputFormat::truncate_description`]:
///
/// ```rust
/// use iai_callgrind::{main, LibraryBenchmarkConfig, OutputFormat};
/// # use iai_callgrind::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(
/// #    name = some_group;
/// #    benchmarks = some_func
/// # );
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default()
///         .output_format(OutputFormat::default()
///             .truncate_description(Some(200))
///         );
///     library_benchmark_groups = some_group
/// );
/// # }
#[derive(Debug, Clone, Default, IntoInner, AsRef)]
pub struct OutputFormat(__internal::InternalOutputFormat);

impl OutputFormat {
    /// Adjust, enable or disable the truncation of the description in the iai-callgrind output
    ///
    /// The default is to truncate the description to the size of 50 ascii characters. A `None`
    /// value disables the truncation entirely and a `Some` value will truncate the description to
    /// the given amount of characters excluding the ellipsis.
    ///
    /// To clearify which part of the output is meant by `DESCRIPTION`:
    ///
    /// ```text
    /// benchmark_file::group_name::function_name id:DESCRIPTION
    ///   Instructions:              352135|352135          (No change)
    ///   L1 Hits:                   470117|470117          (No change)
    ///   L2 Hits:                      748|748             (No change)
    ///   RAM Hits:                    4112|4112            (No change)
    ///   Total read+write:          474977|474977          (No change)
    ///   Estimated Cycles:          617777|617777          (No change)
    /// ```
    ///
    /// # Examples
    ///
    /// For example, specifying this option with a `None` value in the `main!` macro disables the
    /// truncation of the description for all benchmarks.
    ///
    /// ```rust
    /// use iai_callgrind::{main, LibraryBenchmarkConfig, OutputFormat};
    /// # use iai_callgrind::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(
    /// #    name = some_group;
    /// #    benchmarks = some_func
    /// # );
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .output_format(OutputFormat::default()
    ///             .truncate_description(None)
    ///         );
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn truncate_description(&mut self, value: Option<usize>) -> &mut Self {
        self.0.truncate_description = Some(value);
        self
    }

    /// Show intermediate metrics from parts, subprocesses, threads, ... (Default: false)
    ///
    /// In callgrind, threads are treated as separate units (similar to subprocesses) and the
    /// metrics for them are dumped into an own file. Other valgrind tools usually separate the
    /// output files only by subprocesses. To also show the metrics of any intermediate fragments
    /// and not just the total over all of them, set the value of this method to `true`.
    ///
    /// Temporarily setting `show_intermediate` to `true` can help to find misconfigurations in
    /// multi-thread/multi-process benchmarks.
    ///
    /// # Examples
    ///
    /// As opposed to valgrind/callgrind, `--trace-children=yes`, `--separate-threads=yes` and
    /// `--fair-sched=try` are the defaults in Iai-Callgrind, so in the following example it's not
    /// necessary to specify `--separate-threads` to track the metrics of the spawned thread.
    /// However, it is necessary to specify an additional toggle or else the metrics of the thread
    /// are all zero. We also set the [`super::EntryPoint`] to `None` to disable the default entry
    /// point (toggle) which is the benchmark function. So, with this setup we collect only the
    /// metrics of the method `my_lib::heavy_calculation` in the spawned thread and nothing else.
    ///
    /// ```rust
    /// use iai_callgrind::{
    ///     main, LibraryBenchmarkConfig, OutputFormat, EntryPoint, library_benchmark,
    ///     library_benchmark_group
    /// };
    /// # mod my_lib { pub fn heavy_calculation() -> u64 { 42 }}
    ///
    /// #[library_benchmark(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .entry_point(EntryPoint::None)
    ///         .callgrind_args(["--toggle-collect=my_lib::heavy_calculation"])
    ///         .output_format(OutputFormat::default().show_intermediate(true))
    /// )]
    /// fn bench_thread() -> u64 {
    ///     let handle = std::thread::spawn(|| my_lib::heavy_calculation());
    ///     handle.join().unwrap()
    /// }
    ///
    /// library_benchmark_group!(name = some_group; benchmarks = bench_thread);
    /// # fn main() {
    /// main!(library_benchmark_groups = some_group);
    /// # }
    /// ```
    ///
    /// Running the above benchmark the first time will print something like the below (The exact
    /// metric counts are made up for demonstration purposes):
    ///
    /// ```text
    /// my_benchmark::some_group::bench_thread
    ///   ## pid: 633247 part: 1 thread: 1   |N/A
    ///   Command:            target/release/deps/my_benchmark-08fe8356975cd1af
    ///   Instructions:                     0|N/A             (*********)
    ///   L1 Hits:                          0|N/A             (*********)
    ///   L2 Hits:                          0|N/A             (*********)
    ///   RAM Hits:                         0|N/A             (*********)
    ///   Total read+write:                 0|N/A             (*********)
    ///   Estimated Cycles:                 0|N/A             (*********)
    ///   ## pid: 633247 part: 1 thread: 2   |N/A
    ///   Command:            target/release/deps/my_benchmark-08fe8356975cd1af
    ///   Instructions:                  3905|N/A             (*********)
    ///   L1 Hits:                       4992|N/A             (*********)
    ///   L2 Hits:                          0|N/A             (*********)
    ///   RAM Hits:                       464|N/A             (*********)
    ///   Total read+write:              5456|N/A             (*********)
    ///   Estimated Cycles:             21232|N/A             (*********)
    ///   ## Total
    ///   Instructions:                  3905|N/A             (*********)
    ///   L1 Hits:                       4992|N/A             (*********)
    ///   L2 Hits:                          0|N/A             (*********)
    ///   RAM Hits:                       464|N/A             (*********)
    ///   Total read+write:              5456|N/A             (*********)
    ///   Estimated Cycles:             21232|N/A             (*********)
    /// ```
    ///
    /// With `show_intermediate` set to `false` (the default), only the total is shown:
    ///
    /// ```text
    /// my_benchmark::some_group::bench_thread
    ///   Instructions:                  3905|N/A             (*********)
    ///   L1 Hits:                       4992|N/A             (*********)
    ///   L2 Hits:                          0|N/A             (*********)
    ///   RAM Hits:                       464|N/A             (*********)
    ///   Total read+write:              5456|N/A             (*********)
    ///   Estimated Cycles:             21232|N/A             (*********)
    /// ```
    pub fn show_intermediate(&mut self, value: bool) -> &mut Self {
        self.0.show_intermediate = Some(value);
        self
    }

    /// Show an ascii grid in the benchmark terminal output
    ///
    /// This option adds guiding lines which can help reading the benchmark output when running
    /// multiple tools with multiple threads/subprocesses.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use iai_callgrind::OutputFormat;
    ///
    /// let output_format = OutputFormat::default().show_grid(true);
    /// ```
    ///
    /// Below is the output of a Iai-Callgrind run with DHAT as additional tool benchmarking a
    /// function that executes a subprocess which itself starts multiple threads. For the benchmark
    /// run below [`OutputFormat::show_intermediate`] was also active to show the threads and
    /// subprocesses.
    ///
    /// ```text
    /// test_lib_bench_threads::bench_group::bench_thread_in_subprocess three:3
    /// |======== CALLGRIND ===================================================================
    /// |-## pid: 3186352 part: 1 thread: 1       |pid: 2721318 part: 1 thread: 1
    /// | Command:            target/release/deps/test_lib_bench_threads-b0b85adec9a45de1
    /// | Instructions:                       4697|4697                 (No change)
    /// | L1 Hits:                            6420|6420                 (No change)
    /// | L2 Hits:                              17|17                   (No change)
    /// | RAM Hits:                            202|202                  (No change)
    /// | Total read+write:                   6639|6639                 (No change)
    /// | Estimated Cycles:                  13575|13575                (No change)
    /// |-## pid: 3186468 part: 1 thread: 1       |pid: 2721319 part: 1 thread: 1
    /// | Command:            target/release/thread 3
    /// | Instructions:                      35452|35452                (No change)
    /// | L1 Hits:                           77367|77367                (No change)
    /// | L2 Hits:                             610|610                  (No change)
    /// | RAM Hits:                            784|784                  (No change)
    /// | Total read+write:                  78761|78761                (No change)
    /// | Estimated Cycles:                 107857|107857               (No change)
    /// |-## pid: 3186468 part: 1 thread: 2       |pid: 2721319 part: 1 thread: 2
    /// | Command:            target/release/thread 3
    /// | Instructions:                    2460507|2460507              (No change)
    /// | L1 Hits:                         2534939|2534939              (No change)
    /// | L2 Hits:                              17|17                   (No change)
    /// | RAM Hits:                            186|186                  (No change)
    /// | Total read+write:                2535142|2535142              (No change)
    /// | Estimated Cycles:                2541534|2541534              (No change)
    /// |-## pid: 3186468 part: 1 thread: 3       |pid: 2721319 part: 1 thread: 3
    /// | Command:            target/release/thread 3
    /// | Instructions:                    3650414|3650414              (No change)
    /// | L1 Hits:                         3724275|3724275              (No change)
    /// | L2 Hits:                              21|21                   (No change)
    /// | RAM Hits:                            130|130                  (No change)
    /// | Total read+write:                3724426|3724426              (No change)
    /// | Estimated Cycles:                3728930|3728930              (No change)
    /// |-## pid: 3186468 part: 1 thread: 4       |pid: 2721319 part: 1 thread: 4
    /// | Command:            target/release/thread 3
    /// | Instructions:                    4349846|4349846              (No change)
    /// | L1 Hits:                         4423438|4423438              (No change)
    /// | L2 Hits:                              24|24                   (No change)
    /// | RAM Hits:                            125|125                  (No change)
    /// | Total read+write:                4423587|4423587              (No change)
    /// | Estimated Cycles:                4427933|4427933              (No change)
    /// |-## Total
    /// | Instructions:                   10500916|10500916             (No change)
    /// | L1 Hits:                        10766439|10766439             (No change)
    /// | L2 Hits:                             689|689                  (No change)
    /// | RAM Hits:                           1427|1427                 (No change)
    /// | Total read+write:               10768555|10768555             (No change)
    /// | Estimated Cycles:               10819829|10819829             (No change)
    /// |======== DHAT ========================================================================
    /// |-## pid: 3186472 ppid: 3185288           |pid: 2721323 ppid: 2720196
    /// | Command:            target/release/deps/test_lib_bench_threads-b0b85adec9a45de1
    /// | Total bytes:                        2774|2774                 (No change)
    /// | Total blocks:                         24|24                   (No change)
    /// | At t-gmax bytes:                    1736|1736                 (No change)
    /// | At t-gmax blocks:                      3|3                    (No change)
    /// | At t-end bytes:                        0|0                    (No change)
    /// | At t-end blocks:                       0|0                    (No change)
    /// | Reads bytes:                       21054|21054                (No change)
    /// | Writes bytes:                      13165|13165                (No change)
    /// |-## pid: 3186473 ppid: 3186472           |pid: 2721324 ppid: 2721323
    /// | Command:            target/release/thread 3
    /// | Total bytes:                      156158|156158               (No change)
    /// | Total blocks:                         73|73                   (No change)
    /// | At t-gmax bytes:                   52225|52225                (No change)
    /// | At t-gmax blocks:                     19|19                   (No change)
    /// | At t-end bytes:                        0|0                    (No change)
    /// | At t-end blocks:                       0|0                    (No change)
    /// | Reads bytes:                      118403|118403               (No change)
    /// | Writes bytes:                     135926|135926               (No change)
    /// |-## Total
    /// | Total bytes:                      158932|158932               (No change)
    /// | Total blocks:                         97|97                   (No change)
    /// | At t-gmax bytes:                   53961|53961                (No change)
    /// | At t-gmax blocks:                     22|22                   (No change)
    /// | At t-end bytes:                        0|0                    (No change)
    /// | At t-end blocks:                       0|0                    (No change)
    /// | Reads bytes:                      139457|139457               (No change)
    /// | Writes bytes:                     149091|149091               (No change)
    /// |-Comparison with bench_find_primes_multi_thread three:3
    /// | Instructions:                   10494117|10500916             (-0.06475%) [-1.00065x]
    /// | L1 Hits:                        10757259|10766439             (-0.08526%) [-1.00085x]
    /// | L2 Hits:                             601|689                  (-12.7721%) [-1.14642x]
    /// | RAM Hits:                           1189|1427                 (-16.6783%) [-1.20017x]
    /// | Total read+write:               10759049|10768555             (-0.08828%) [-1.00088x]
    /// | Estimated Cycles:               10801879|10819829             (-0.16590%) [-1.00166x]
    pub fn show_grid(&mut self, value: bool) -> &mut Self {
        self.0.show_grid = Some(value);
        self
    }
}

/// Configure performance regression checks and behavior
///
/// A performance regression check consists of an [`EventKind`] and a percentage over which a
/// regression is assumed. If the percentage is negative, then a regression is assumed to be below
/// this limit. The default [`EventKind`] is [`EventKind::Ir`] with a value of
/// `+10f64`
///
/// If `fail_fast` is set to true, then the whole benchmark run fails on the first encountered
/// regression. Else, the default behavior is, that the benchmark run fails with a regression error
/// after all benchmarks have been run.
///
/// # Examples
///
/// ```rust
/// # use iai_callgrind::{library_benchmark, library_benchmark_group};
/// use iai_callgrind::{main, LibraryBenchmarkConfig, RegressionConfig};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group; benchmarks = some_func);
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default()
///                 .regression(RegressionConfig::default());
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Default, Clone, IntoInner, AsRef)]
pub struct RegressionConfig(__internal::InternalRegressionConfig);

/// Configure to run other valgrind tools like `DHAT` or `Massif` in addition to callgrind
///
/// For a list of possible tools see [`ValgrindTool`].
///
/// See also the [Valgrind User Manual](https://valgrind.org/docs/manual/manual.html) for details
/// about possible tools and their command line arguments.
///
/// # Examples
///
/// ```rust
/// # use iai_callgrind::{library_benchmark, library_benchmark_group};
/// use iai_callgrind::{main, LibraryBenchmarkConfig, Tool, ValgrindTool};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group; benchmarks = some_func);
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default()
///                 .tool(Tool::new(ValgrindTool::DHAT));
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, IntoInner, AsRef)]
pub struct Tool(__internal::InternalTool);

impl FlamegraphConfig {
    /// Option to change the [`FlamegraphKind`]
    ///
    /// The default is [`FlamegraphKind::All`].
    ///
    /// # Examples
    ///
    /// For example, to only create a differential flamegraph:
    ///
    /// ```
    /// use iai_callgrind::{FlamegraphConfig, FlamegraphKind};
    ///
    /// let config = FlamegraphConfig::default().kind(FlamegraphKind::Differential);
    /// ```
    pub fn kind(&mut self, kind: FlamegraphKind) -> &mut Self {
        self.0.kind = Some(kind);
        self
    }

    /// Negate the differential flamegraph [`FlamegraphKind::Differential`]
    ///
    /// The default is `false`.
    ///
    /// Instead of showing the differential flamegraph from the viewing angle of what has happened
    /// the negated differential flamegraph shows what will happen. Especially, this allows to see
    /// vanished event lines (in blue) for example because the underlying code has improved and
    /// removed an unnecessary function call.
    ///
    /// See also [Differential Flame
    /// Graphs](https://www.brendangregg.com/blog/2014-11-09/differential-flame-graphs.html) from
    /// Brendan Gregg's Blog.
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{FlamegraphConfig, FlamegraphKind};
    ///
    /// let config = FlamegraphConfig::default().negate_differential(true);
    /// ```
    pub fn negate_differential(&mut self, negate_differential: bool) -> &mut Self {
        self.0.negate_differential = Some(negate_differential);
        self
    }

    /// Normalize the differential flamegraph
    ///
    /// This'll make the first profile event count to match the second. This'll help in situations
    /// when everything looks read (or blue) to get a balanced profile with the full red/blue
    /// spectrum
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{FlamegraphConfig, FlamegraphKind};
    ///
    /// let config = FlamegraphConfig::default().normalize_differential(true);
    /// ```
    pub fn normalize_differential(&mut self, normalize_differential: bool) -> &mut Self {
        self.0.normalize_differential = Some(normalize_differential);
        self
    }

    /// One or multiple [`EventKind`] for which a flamegraph is going to be created.
    ///
    /// The default is [`EventKind::Ir`]
    ///
    /// Currently, flamegraph creation is limited to one flamegraph for each [`EventKind`] and
    /// there's no way to merge all event kinds into a single flamegraph.
    ///
    /// Note it is an error to specify a [`EventKind`] which isn't recorded by callgrind. See the
    /// docs of the variants of [`EventKind`] which callgrind option is needed to create a record
    /// for it. See also the [Callgrind
    /// Documentation](https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options). The
    /// [`EventKind`]s recorded by callgrind which are available as long as the cache simulation is
    /// turned on with `--cache-sim=yes` (which is the default):
    ///
    /// * [`EventKind::Ir`]
    /// * [`EventKind::Dr`]
    /// * [`EventKind::Dw`]
    /// * [`EventKind::I1mr`]
    /// * [`EventKind::ILmr`]
    /// * [`EventKind::D1mr`]
    /// * [`EventKind::DLmr`]
    /// * [`EventKind::D1mw`]
    /// * [`EventKind::DLmw`]
    ///
    /// If the cache simulation is turned on, the following derived `EventKinds` are also available:
    ///
    /// * [`EventKind::L1hits`]
    /// * [`EventKind::LLhits`]
    /// * [`EventKind::RamHits`]
    /// * [`EventKind::TotalRW`]
    /// * [`EventKind::EstimatedCycles`]
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{EventKind, FlamegraphConfig};
    ///
    /// let config =
    ///     FlamegraphConfig::default().event_kinds([EventKind::EstimatedCycles, EventKind::Ir]);
    /// ```
    pub fn event_kinds<T>(&mut self, event_kinds: T) -> &mut Self
    where
        T: IntoIterator<Item = EventKind>,
    {
        self.0.event_kinds = Some(event_kinds.into_iter().collect());
        self
    }

    /// Set the [`Direction`] in which the flamegraph should grow.
    ///
    /// The default is [`Direction::TopToBottom`].
    ///
    /// # Examples
    ///
    /// For example to change the default
    ///
    /// ```
    /// use iai_callgrind::{Direction, FlamegraphConfig};
    ///
    /// let config = FlamegraphConfig::default().direction(Direction::BottomToTop);
    /// ```
    pub fn direction(&mut self, direction: Direction) -> &mut Self {
        self.0.direction = Some(direction);
        self
    }

    /// Overwrite the default title of the final flamegraph
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{Direction, FlamegraphConfig};
    ///
    /// let config = FlamegraphConfig::default().title("My flamegraph title".to_owned());
    /// ```
    pub fn title(&mut self, title: String) -> &mut Self {
        self.0.title = Some(title);
        self
    }

    /// Overwrite the default subtitle of the final flamegraph
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::FlamegraphConfig;
    ///
    /// let config = FlamegraphConfig::default().subtitle("My flamegraph subtitle".to_owned());
    /// ```
    pub fn subtitle(&mut self, subtitle: String) -> &mut Self {
        self.0.subtitle = Some(subtitle);
        self
    }

    /// Set the minimum width (in pixels) for which event lines are going to be shown.
    ///
    /// The default is `0.1`
    ///
    /// To show all events, set the `min_width` to `0f64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::FlamegraphConfig;
    ///
    /// let config = FlamegraphConfig::default().min_width(0f64);
    /// ```
    pub fn min_width(&mut self, min_width: f64) -> &mut Self {
        self.0.min_width = Some(min_width);
        self
    }
}

/// Enable performance regression checks with a [`RegressionConfig`]
///
/// A performance regression check consists of an [`EventKind`] and a percentage over which a
/// regression is assumed. If the percentage is negative, then a regression is assumed to be below
/// this limit. The default [`EventKind`] is [`EventKind::Ir`] with a value of
/// `+10f64`
///
/// If `fail_fast` is set to true, then the whole benchmark run fails on the first encountered
/// regression. Else, the default behavior is, that the benchmark run fails with a regression error
/// after all benchmarks have been run.
///
/// # Examples
///
/// ```rust
/// # use iai_callgrind::{library_benchmark, library_benchmark_group, main};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group; benchmarks = some_func);
/// use iai_callgrind::{LibraryBenchmarkConfig, RegressionConfig};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default()
///                 .regression(RegressionConfig::default());
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
impl RegressionConfig {
    /// Configure the limits percentages over/below which a performance regression can be assumed
    ///
    /// A performance regression check consists of an [`EventKind`] and a percentage over which a
    /// regression is assumed. If the percentage is negative, then a regression is assumed to be
    /// below this limit.
    ///
    /// If no `limits` or empty `targets` are specified with this function, the default
    /// [`EventKind`] is [`EventKind::Ir`] with a value of `+10f64`
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{EventKind, RegressionConfig};
    ///
    /// let config = RegressionConfig::default().limits([(EventKind::Ir, 5f64)]);
    /// ```
    pub fn limits<T>(&mut self, targets: T) -> &mut Self
    where
        T: IntoIterator<Item = (EventKind, f64)>,
    {
        self.0.limits.extend(targets);
        self
    }

    /// If set to true, then the benchmarks fail on the first encountered regression
    ///
    /// The default is `false` and the whole benchmark run fails with a regression error after all
    /// benchmarks have been run.
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::RegressionConfig;
    ///
    /// let config = RegressionConfig::default().fail_fast(true);
    /// ```
    pub fn fail_fast(&mut self, value: bool) -> &mut Self {
        self.0.fail_fast = Some(value);
        self
    }
}

impl Tool {
    /// Create a new `Tool` configuration
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{Tool, ValgrindTool};
    ///
    /// let tool = Tool::new(ValgrindTool::DHAT);
    /// ```
    pub fn new(tool: ValgrindTool) -> Self {
        Self(__internal::InternalTool {
            kind: tool,
            enable: Option::default(),
            show_log: Option::default(),
            raw_args: __internal::InternalRawArgs::default(),
        })
    }

    /// If true, enable running this `Tool` (Default: true)
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{Tool, ValgrindTool};
    ///
    /// let tool = Tool::new(ValgrindTool::DHAT).enable(true);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }

    /// Pass one or more arguments directly to the valgrind `Tool`
    ///
    /// # Examples
    ///
    /// ```
    /// use iai_callgrind::{Tool, ValgrindTool};
    ///
    /// let tool = Tool::new(ValgrindTool::DHAT).args(["--num-callers=5", "--mode=heap"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_args.extend_ignore_flag(args);
        self
    }
}

/// __DEPRECATED__: A function that is opaque to the optimizer
///
/// It is used to prevent the compiler from optimizing away computations in a benchmark.
///
/// This method is deprecated and is in newer versions of `iai-callgrind` merely a wrapper around
/// [`std::hint::black_box`]. Please use `std::hint::black_box` directly.
#[inline]
pub fn black_box<T>(dummy: T) -> T {
    std::hint::black_box(dummy)
}
