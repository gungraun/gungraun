//! Common structs for `bin_bench` and `lib_bench`

use std::path::PathBuf;
use std::time::Duration;
use std::vec::Vec;

use derive_more::AsRef;
use gungraun_macros::IntoInner;

use super::{
    __internal, CachegrindMetric, CachegrindMetrics, CallgrindMetrics, DhatMetric, DhatMetrics,
    Direction, ErrorMetric, EventKind, FlamegraphKind, Limit, SanitizeOutput, Tool, Unit,
};
use crate::EntryPoint;

/// Controls how a `perf` measurement is executed.
///
/// The default is [`Self::Direct`], which is the normal mode and measures a single invocation with
/// no extra setup. Calibration modes ([`Self::DefaultCalibrate`] and [`Self::Calibrate`]) run a
/// separate overhead-measurement pass first, then subtract the best calibration run from the final
/// result.
///
/// # Examples
///
/// ```rust
/// # pub mod gungraun {
/// # pub use gungraun_runner::api::PerfRunMode;
/// # }
/// use gungraun::PerfRunMode;
///
/// let mode = PerfRunMode::DynamicBatch;
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PerfRunMode {
    /// Calibrate Gungraun by sampling the benchmark harness overhead, then run the benchmark once.
    ///
    /// Before the main measurement, the runner executes `perf` to measure the overhead introduced
    /// by `perf` and the Gungraun harness. This doesn't run the benchmark itself. perf stops
    /// sampling after a default duration of one second. The first sample is discarded to mitigate
    /// cold-start effects, and the mean calibration metrics are subtracted from the final benchmark
    /// metrics.
    ///
    /// Whether calibration is worthwhile depends on the benchmark: if the overhead is small
    /// relative to the main benchmark run, [`Self::Direct`] is usually sufficient.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::DefaultCalibrate;
    /// ```
    DefaultCalibrate,

    /// Like [`Self::DefaultCalibrate`] but with a custom calibration sampling duration.
    ///
    /// The provided [`Duration`] controls how long the runner samples `perf` overhead before
    /// taking the main measurement. A longer duration collects more samples and may yield a more
    /// stable overhead estimate, at the cost of increased total benchmark time.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use std::time::Duration;
    ///
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::Calibrate(Duration::from_secs(2));
    /// ```
    Calibrate(Duration),

    /// Run `perf` once with a normal single benchmark invocation.
    ///
    /// This is the default mode. It is suitable when the benchmark execution time is long enough
    /// that `perf` benchmark self costs are negligible compared to the main benchmark metrics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # pub mod gungraun {
    /// # pub use gungraun_runner::api::PerfRunMode;
    /// # }
    /// use gungraun::PerfRunMode;
    ///
    /// let mode = PerfRunMode::Direct;
    /// ```
    #[default]
    Direct,
}

/// The configuration for the experimental bbv
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{Bbv, LibraryBenchmarkConfig, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Bbv::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Bbv(__internal::InternalToolSpec);

/// The configuration for Cachegrind
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{Cachegrind, LibraryBenchmarkConfig, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Cachegrind::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Cachegrind(__internal::InternalToolSpec);

/// The configuration for Callgrind
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{Callgrind, LibraryBenchmarkConfig, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Callgrind::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Callgrind(__internal::InternalToolSpec);

/// The configuration for Dhat
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{Dhat, LibraryBenchmarkConfig, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Dhat::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Dhat(__internal::InternalToolSpec);

/// The configuration for DRD
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{Drd, LibraryBenchmarkConfig, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Drd::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Drd(__internal::InternalToolSpec);

/// The `FlamegraphConfig` which allows the customization of the created flamegraphs
///
/// Callgrind flamegraphs are very similar to `callgrind_annotate` output. In contrast to
/// `callgrind_annotate` text based output, the produced flamegraphs are svg files (located in the
/// `target/gungraun` directory) which can be viewed in a browser.
///
/// # Experimental
///
/// Note the following considerations only affect flamegraphs of multi-threaded/multi-process
/// benchmarks and benchmarks which produce multiple parts with a total over all sub-metrics.
///
/// Currently, Gungraun creates the flamegraphs only for the total over all threads/parts and
/// subprocesses. This leads to complications since the call graph is not be fully recovered just by
/// examining each thread/subprocess separately. So, the total metrics in the flamegraphs might not
/// be the same as the total metrics shown in the terminal output. If in doubt, the terminal output
/// shows the correct metrics.
///
/// # Examples
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// use gungraun::{Callgrind, FlamegraphConfig, LibraryBenchmarkConfig, main};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default()
///         .tool(Callgrind::default().flamegraph(FlamegraphConfig::default())),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, Default, IntoInner, AsRef)]
pub struct FlamegraphConfig(__internal::InternalFlamegraphConfig);

/// The configuration for Helgrind
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{Helgrind, LibraryBenchmarkConfig, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Helgrind::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Helgrind(__internal::InternalToolSpec);

/// The configuration for Massif
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{LibraryBenchmarkConfig, Massif, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Massif::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Massif(__internal::InternalToolSpec);

/// The configuration for Memcheck
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{LibraryBenchmarkConfig, Memcheck, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Memcheck::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Memcheck(__internal::InternalToolSpec);

/// Configures the default output format of the terminal output of Gungraun.
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
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(
/// #    name = some_group,
/// #    benchmarks = some_func
/// # );
/// # fn main() {
/// use gungraun::{LibraryBenchmarkConfig, OutputFormat, main};
/// main!(
///     config = LibraryBenchmarkConfig::default()
///         .output_format(OutputFormat::default().truncate_description(Some(200))),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, Default, IntoInner, AsRef)]
pub struct OutputFormat(__internal::InternalOutputFormat);

/// The configuration for perf
///
/// Can be specified in [`crate::LibraryBenchmarkConfig::tool`] or
/// [`crate::BinaryBenchmarkConfig::tool`].
///
/// # Example
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// use gungraun::{LibraryBenchmarkConfig, Perf, main};
///
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default().tool(Perf::default()),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Perf(__internal::InternalToolSpec);

/// The `Sandbox` in which benchmarks are run.
///
/// The `Sandbox` is a temporary directory which is created before a benchmark is executed and
/// deleted afterwards. Binary benchmarks execute the benchmark-level [`setup`], [`Command`] and
/// [`teardown`] inside this temporary directory. Similar to binary benchmarks, library benchmarks
/// execute the benchmark-level `setup`, the benchmark function and `teardown` inside this temporary
/// directory.
///
/// Main-level and group-level setup and teardown functions are not executed inside this
/// per-benchmark sandbox.
///
/// # Background and reasons for using a `Sandbox`
///
/// A [`Sandbox`] can help mitigating differences in benchmark results on different machines. As
/// long as `$TMPDIR` is unset or set to `/tmp`, the temporary directory has a constant length on
/// unix machines (except android which uses `/data/local/tmp`). The directory itself
/// is created with a constant length but random name like `/tmp/.a23sr8fk`. It is not implausible
/// that an executable has different event counts just because the directory it is executed in has a
/// different length. For example, if a member of your project has set up the project in
/// `/home/bob/workspace/our-project` running the benchmarks in this directory, and the ci runs the
/// benchmarks in `/runner/our-project`, the event counts might differ. If possible, the benchmarks
/// should be run in an as constant as possible environment. Clearing the environment variables is
/// also such a counter-measure.
///
/// Other reasons for using a `Sandbox` are convenience, such as if you're creating files during a
/// benchmark run and don't want to delete all the files manually. Or, more importantly, if a
/// benchmark is destructive and deletes files, it is usually safer to execute it in a temporary
/// directory where it cannot do any harm to your or others file systems during the benchmark runs.
///
/// # Sandbox cleanup
///
/// The changes a benchmark makes in this directory persist until the sandbox is deleted after the
/// benchmark run. In binary benchmarks, changes made by the benchmark-level `setup` function are
/// visible to the [`Command`] and benchmark-level `teardown` function. If run in a `Sandbox`, the
/// benchmark usually doesn't have to delete any files, because the whole directory is deleted after
/// its usage. There is an exception to the rule. If any of the files inside the directory is not
/// removable, for example because the permissions of a file don't allow the file to be deleted,
/// then the whole directory persists. You can use a benchmark-level `teardown` to reset all
/// permission bits to be readable and writable, so the cleanup can succeed.
///
/// To copy fixtures or whole directories into the `Sandbox` use [`Sandbox::fixtures`].
///
/// [`Command`]: crate::Command
/// [`setup`]: crate::binary_benchmark
/// [`teardown`]: crate::binary_benchmark
#[derive(Debug, Clone, IntoInner, AsRef)]
pub struct Sandbox(__internal::InternalSandbox);

impl Bbv {
    /// Creates a new `BBV` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`] and [`Bbv::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Bbv;
    ///
    /// let config = Bbv::with_args(["interval-size=10000"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(Tool::BBV, args))
    }

    /// Adds command-line arguments to the `BBV` configuration.
    ///
    /// Valid arguments
    /// are <https://valgrind.org/docs/manual/bbv-manual.html#bbv-manual.usage> and the core
    /// Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Bbv;
    ///
    /// let config = Bbv::default().args(["interval-size=10000"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// See also [`Callgrind::enable`]
    ///
    /// ```rust
    /// use gungraun::Bbv;
    ///
    /// let config = Bbv::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }
}

impl Default for Bbv {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::BBV))
    }
}

impl Cachegrind {
    /// Creates a new `Cachegrind` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`] and [`Cachegrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Cachegrind;
    ///
    /// let config = Cachegrind::with_args(["intr-at-start=no"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(
            Tool::Cachegrind,
            args,
        ))
    }

    /// Adds command-line arguments to the `Cachegrind` configuration.
    ///
    /// Valid arguments
    /// are <https://valgrind.org/docs/manual/cg-manual.html#cg-manual.cgopts> and the core
    /// Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Cachegrind;
    ///
    /// let config = Cachegrind::default().args(["intr-at-start=no"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// See also [`Callgrind::enable`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Cachegrind;
    ///
    /// let config = Cachegrind::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }

    /// Customize the format of the Cachegrind output
    ///
    /// See also [`Callgrind::format`] for more details and [`crate::CachegrindMetrics`] for valid
    /// metrics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{Cachegrind, CachegrindMetric, CachegrindMetrics};
    ///
    /// let config =
    ///     Cachegrind::default().format([CachegrindMetric::Ir.into(), CachegrindMetrics::CacheSim]);
    /// ```
    pub fn format<I, T>(&mut self, cachegrind_metrics: T) -> &mut Self
    where
        I: Into<CachegrindMetrics>,
        T: IntoIterator<Item = I>,
    {
        let format = self
            .0
            .output_format
            .get_or_insert_with(|| __internal::InternalToolOutputFormat::Cachegrind(Vec::new()));

        if let __internal::InternalToolOutputFormat::Cachegrind(items) = format {
            items.extend(cachegrind_metrics.into_iter().map(Into::into));
        }

        self
    }

    /// Configures the limits percentages over/below which a performance regression can be assumed.
    ///
    /// DEPRECATED: Please use [`Cachegrind::soft_limits`] instead.
    #[deprecated = "Please use Cachegrind::soft_limits instead"]
    pub fn limits<T>(&mut self, limits: T) -> &mut Self
    where
        T: IntoIterator<Item = (CachegrindMetric, f64)>,
    {
        self.soft_limits(limits)
    }

    /// Configures the soft limits over/below which a performance regression can be assumed.
    ///
    /// Same as [`Callgrind::soft_limits`] but for [`CachegrindMetric`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Cachegrind, CachegrindMetric};
    ///
    /// let config = Cachegrind::default().soft_limits([(CachegrindMetric::Ir, 5f64)]);
    /// ```
    ///
    /// or for a group of metrics but with a special value for `Ir`:
    ///
    /// ```
    /// use gungraun::{Cachegrind, CachegrindMetric, CachegrindMetrics};
    ///
    /// let config = Cachegrind::default().soft_limits([
    ///     (CachegrindMetrics::All, 10f64),
    ///     (CachegrindMetric::Ir.into(), 5f64),
    /// ]);
    /// ```
    pub fn soft_limits<K, T>(&mut self, soft_limits: T) -> &mut Self
    where
        K: Into<CachegrindMetrics>,
        T: IntoIterator<Item = (K, f64)>,
    {
        let iter = soft_limits.into_iter().map(|(k, l)| (k.into(), l));

        if let Some(__internal::InternalToolRegressionConfig::Cachegrind(config)) =
            &mut self.0.regression_config
        {
            config.soft_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Cachegrind(
                __internal::InternalCachegrindRegressionConfig {
                    soft_limits: iter.collect(),
                    hard_limits: Vec::default(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// Sets hard limits above which a performance regression can be assumed.
    ///
    /// Same as [`Callgrind::hard_limits`] but for [`CachegrindMetrics`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Cachegrind, CachegrindMetric};
    ///
    /// let config = Cachegrind::default().hard_limits([(CachegrindMetric::Ir, 10_000)]);
    /// ```
    ///
    /// or for a group of metrics but with a special value for `Ir`:
    ///
    /// ```
    /// use gungraun::{Cachegrind, CachegrindMetric, CachegrindMetrics};
    ///
    /// let config = Cachegrind::default().hard_limits([
    ///     (CachegrindMetrics::Default, 10_000),
    ///     (CachegrindMetric::Ir.into(), 5_000),
    /// ]);
    /// ```
    pub fn hard_limits<K, L, T>(&mut self, hard_limits: T) -> &mut Self
    where
        K: Into<CachegrindMetrics>,
        L: Into<Limit>,
        T: IntoIterator<Item = (K, L)>,
    {
        let iter = hard_limits.into_iter().map(|(k, l)| (k.into(), l.into()));

        if let Some(__internal::InternalToolRegressionConfig::Cachegrind(config)) =
            &mut self.0.regression_config
        {
            config.hard_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Cachegrind(
                __internal::InternalCachegrindRegressionConfig {
                    soft_limits: Vec::default(),
                    hard_limits: iter.collect(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// If set to true, then the benchmarks fail on the first encountered regression
    ///
    /// The default is `false` and the whole benchmark run fails with a regression error after all
    /// benchmarks have been run. This option does not enable regression checks by itself. Configure
    /// regression checks explicitly with [`Cachegrind::soft_limits`] or
    /// [`Cachegrind::hard_limits`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Cachegrind, CachegrindMetric};
    ///
    /// let config = Cachegrind::default()
    ///     .soft_limits([(CachegrindMetric::Ir, 5f64)])
    ///     .fail_fast(true);
    /// ```
    pub fn fail_fast(&mut self, value: bool) -> &mut Self {
        if let Some(__internal::InternalToolRegressionConfig::Cachegrind(config)) =
            &mut self.0.regression_config
        {
            config.fail_fast = Some(value);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Cachegrind(
                __internal::InternalCachegrindRegressionConfig {
                    soft_limits: Vec::default(),
                    hard_limits: Vec::default(),
                    fail_fast: Some(value),
                },
            ));
        }
        self
    }
}

impl Default for Cachegrind {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::Cachegrind))
    }
}

impl Callgrind {
    /// Creates a new `Callgrind` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Callgrind;
    ///
    /// let config = Callgrind::with_args(["collect-bus=yes"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(
            Tool::Callgrind,
            args,
        ))
    }

    /// Adds command-line arguments to the `Callgrind` configuration.
    ///
    /// The command-line arguments are passed directly to the Callgrind invocation. Valid arguments
    /// are <https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options> and the core
    /// Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>. Note that not all
    /// command-line arguments are supported especially the ones which change output paths.
    /// Unsupported arguments will be ignored, printing a warning.
    ///
    /// The flags can be omitted ("collect-bus" instead of "--collect-bus").
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Callgrind;
    ///
    /// let config = Callgrind::default().args(["collect-bus=yes"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// This is mostly useful to disable a tool which has been enabled in a
    /// [`crate::LibraryBenchmarkConfig`] (or [`crate::BinaryBenchmarkConfig`]) at a higher level.
    /// However, the default tool (usually Callgrind) cannot be disabled.
    ///
    /// ```rust
    /// use gungraun::Callgrind;
    ///
    /// let config = Callgrind::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }

    /// Sets or unset the entry point for a benchmark.
    ///
    /// Gungraun sets the [`--toggle-collect`] argument of callgrind to the benchmark function
    /// which we call [`EntryPoint::Default`]. Specifying a `--toggle-collect` argument, sets
    /// automatically `--collect-at-start=no`. This ensures that only the metrics from the benchmark
    /// itself are collected and not the `setup` or `teardown` or anything before/after the
    /// benchmark function.
    ///
    /// However, there are cases when the default toggle is not enough [`EntryPoint::Custom`] or in
    /// the way [`EntryPoint::None`].
    ///
    /// Setting [`EntryPoint::Custom`] is convenience for disabling the entry point with
    /// [`EntryPoint::None`] and setting `--toggle-collect=CUSTOM_ENTRY_POINT` in
    /// [`Callgrind::args`]. [`EntryPoint::Custom`] can be useful if you
    /// want to benchmark a private function and only need the function in the benchmark function as
    /// access point. [`EntryPoint::Custom`] accepts glob patterns the same way as
    /// [`--toggle-collect`] does.
    ///
    /// # Examples
    ///
    /// If you're using callgrind client requests either in the benchmark function itself or in your
    /// library, then using [`EntryPoint::None`] is presumably be required. Consider the following
    /// example (`DEFAULT_ENTRY_POINT` marks the default entry point):
    #[cfg_attr(not(feature = "stubs"), doc = "```rust,ignore")]
    #[cfg_attr(feature = "stubs", doc = "```rust")]
    /// use gungraun::{
    ///     main, LibraryBenchmarkConfig,library_benchmark, library_benchmark_group
    /// };
    /// use std::hint::black_box;
    ///
    /// fn to_be_benchmarked() -> u64 {
    ///     println!("Some info output");
    ///     gungraun::client_requests::callgrind::start_instrumentation();
    ///     let result = {
    ///         // some heavy calculations
    /// #       10
    ///     };
    ///     gungraun::client_requests::callgrind::stop_instrumentation();
    ///
    ///     result
    /// }
    ///
    /// #[library_benchmark]
    /// fn some_bench() -> u64 { // <-- DEFAULT ENTRY POINT
    ///     black_box(to_be_benchmarked())
    /// }
    ///
    /// library_benchmark_group!(name = some_group, benchmarks = some_bench);
    /// # fn main() {
    /// main!(library_benchmark_groups = some_group);
    /// # }
    /// ```
    /// In the example above [`EntryPoint::Default`] is active, so the counting of events starts
    /// when the `some_bench` function is entered. In `to_be_benchmarked`, the client request
    /// `start_instrumentation` does effectively nothing and `stop_instrumentation` will stop the
    /// event counting as requested. This is most likely not what you intended. The event counting
    /// should start with `start_instrumentation`. To achieve this, you can set [`EntryPoint::None`]
    /// which removes the default toggle, but also `--collect-at-start=no`. So, you need to specify
    /// `--collect-at-start=no` in [`Callgrind::args`]. The example would then look like this:
    /// ```rust
    /// use std::hint::black_box;
    ///
    /// use gungraun::{library_benchmark, EntryPoint, LibraryBenchmarkConfig, Callgrind};
    /// # use gungraun::{library_benchmark_group, main};
    /// # fn to_be_benchmarked() -> u64 { 10 }
    ///
    /// // ...
    ///
    /// #[library_benchmark(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Callgrind::with_args(["--collect-at-start=no"])
    ///             .entry_point(EntryPoint::None)
    ///         )
    /// )]
    /// fn some_bench() -> u64 {
    ///     black_box(to_be_benchmarked())
    /// }
    ///
    /// // ...
    ///
    /// # library_benchmark_group!(name = some_group, benchmarks = some_bench);
    /// # fn main() {
    /// # main!(library_benchmark_groups = some_group);
    /// # }
    /// ```
    /// [`--toggle-collect`]: https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options
    pub fn entry_point(&mut self, entry_point: EntryPoint) -> &mut Self {
        self.0.entry_point = Some(entry_point);
        self
    }

    /// Configures the limits percentages over/below which a performance regression can be assumed.
    ///
    /// DEPRECATED: Use [`Callgrind::soft_limits`] instead.
    #[deprecated = "Please use Callgrind::soft_limits instead"]
    pub fn limits<T>(&mut self, limits: T) -> &mut Self
    where
        T: IntoIterator<Item = (EventKind, f64)>,
    {
        self.soft_limits(limits)
    }

    /// Configures the soft limits over/below which a performance regression can be assumed.
    ///
    /// A soft limit consists of an [`EventKind`] and a percentage over which a regression is
    /// assumed. If the limit is negative, then a regression is assumed to be below this limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Callgrind, EventKind};
    ///
    /// let config = Callgrind::default().soft_limits([(EventKind::Ir, 5f64)]);
    /// ```
    ///
    /// or for a whole group of metrics but a special value for `Ir`:
    ///
    /// ```
    /// use gungraun::{Callgrind, CallgrindMetrics, EventKind};
    ///
    /// let config = Callgrind::default()
    ///     .soft_limits([(CallgrindMetrics::All, 10f64), (EventKind::Ir.into(), 5f64)]);
    /// ```
    pub fn soft_limits<K, T>(&mut self, soft_limits: T) -> &mut Self
    where
        K: Into<CallgrindMetrics>,
        T: IntoIterator<Item = (K, f64)>,
    {
        let iter = soft_limits.into_iter().map(|(k, l)| (k.into(), l));

        if let Some(__internal::InternalToolRegressionConfig::Callgrind(config)) =
            &mut self.0.regression_config
        {
            config.soft_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Callgrind(
                __internal::InternalCallgrindRegressionConfig {
                    soft_limits: iter.collect(),
                    hard_limits: Vec::default(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// Sets hard limits above which a performance regression can be assumed.
    ///
    /// In contrast to [`Callgrind::soft_limits`], hard limits restrict an [`EventKind`] in absolute
    /// numbers instead of a percentage. A hard limit only affects the `new` benchmark run.
    ///
    /// # Errors
    ///
    /// Specifying limits with [`Limit::Float`] for metric groups which contain mixed metrics of
    /// [`Limit::Float`] and [`Limit::Int`] type is an error because [`Limit::Float`] can't be
    /// converted to [`Limit::Int`]. Use [`Limit::Int`] instead and overwrite the float metrics of
    /// this group with [`Limit::Float`] if required.
    ///
    /// ```
    /// use gungraun::{Callgrind, CallgrindMetrics, Limit};
    ///
    /// // This is an error
    /// let config = Callgrind::default().hard_limits([(CallgrindMetrics::All, 10_000.0)]);
    ///
    /// // This is ok
    /// let config = Callgrind::default().hard_limits([(CallgrindMetrics::All, 10_000)]);
    ///
    /// // Overwriting metrics is fine too
    /// let config = Callgrind::default().hard_limits([
    ///     (CallgrindMetrics::All, Limit::Int(10_000)),
    ///     (CallgrindMetrics::CacheMissRates, Limit::Float(5f64)),
    ///     (CallgrindMetrics::CacheHitRates, Limit::Float(100f64)),
    /// ]);
    /// ```
    ///
    /// # Examples
    ///
    /// If in a benchmark configured like below, there are more than `10_000` instruction fetches, a
    /// performance regression is registered failing the benchmark run.
    ///
    /// ```
    /// use gungraun::{Callgrind, EventKind};
    ///
    /// let config = Callgrind::default().hard_limits([(EventKind::Ir, 10_000)]);
    /// ```
    ///
    /// or for a group of metrics but with a special value for `Ir`:
    ///
    /// ```
    /// use gungraun::{Callgrind, CallgrindMetrics, EventKind};
    ///
    /// let config = Callgrind::default().hard_limits([
    ///     (CallgrindMetrics::Default, 10_000),
    ///     (EventKind::Ir.into(), 5_000),
    /// ]);
    /// ```
    pub fn hard_limits<K, L, T>(&mut self, hard_limits: T) -> &mut Self
    where
        K: Into<CallgrindMetrics>,
        L: Into<Limit>,
        T: IntoIterator<Item = (K, L)>,
    {
        let iter = hard_limits.into_iter().map(|(k, l)| (k.into(), l.into()));

        if let Some(__internal::InternalToolRegressionConfig::Callgrind(config)) =
            &mut self.0.regression_config
        {
            config.hard_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Callgrind(
                __internal::InternalCallgrindRegressionConfig {
                    soft_limits: Vec::default(),
                    hard_limits: iter.collect(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// If set to true, then the benchmarks fail on the first encountered regression
    ///
    /// The default is `false` and the whole benchmark run fails with a regression error after all
    /// benchmarks have been run. This option does not enable regression checks by itself. Configure
    /// regression checks explicitly with [`Callgrind::soft_limits`] or [`Callgrind::hard_limits`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Callgrind, EventKind};
    ///
    /// let config = Callgrind::default()
    ///     .soft_limits([(EventKind::Ir, 5f64)])
    ///     .fail_fast(true);
    /// ```
    pub fn fail_fast(&mut self, value: bool) -> &mut Self {
        if let Some(__internal::InternalToolRegressionConfig::Callgrind(config)) =
            &mut self.0.regression_config
        {
            config.fail_fast = Some(value);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Callgrind(
                __internal::InternalCallgrindRegressionConfig {
                    soft_limits: Vec::default(),
                    hard_limits: Vec::default(),
                    fail_fast: Some(value),
                },
            ));
        }
        self
    }

    /// Option to produce flamegraphs from Callgrind output with a [`crate::FlamegraphConfig`]
    ///
    /// The flamegraphs are usable but still in an experimental stage. Callgrind lacks the tool like
    /// `cg_diff` for Cachegrind to compare two different profiles. Flamegraphs on the other hand
    /// can bridge the gap and be [`FlamegraphKind::Differential`] to compare two benchmark runs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// use gungraun::{Callgrind, FlamegraphConfig, FlamegraphKind, LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().tool(
    ///         Callgrind::default()
    ///             .flamegraph(FlamegraphConfig::default().kind(FlamegraphKind::Differential))
    ///     ),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn flamegraph<T>(&mut self, flamegraph: T) -> &mut Self
    where
        T: Into<__internal::InternalFlamegraphConfig>,
    {
        self.0.flamegraph_config = Some(__internal::InternalToolFlamegraphConfig::Callgrind(
            flamegraph.into(),
        ));
        self
    }

    /// Customize the format of the Callgrind output
    ///
    /// This option allows customizing the output format of Callgrind metrics. It does not set any
    /// flags for the Callgrind execution (i.e. `--branch-sim=yes`) which actually enable the
    /// collection of these metrics. Consult the docs of [`EventKind`] and [`CallgrindMetrics`] to
    /// see which flag is necessary to enable the collection of a specific metric. The rules:
    ///
    /// 1. A metric is only printed if specified here
    /// 2. A metric is not printed if not collected by Callgrind
    /// 3. The order matters
    /// 4. In case of duplicate specifications of the same metric the first one wins.
    ///
    /// Callgrind offers a lot of metrics, so the [`CallgrindMetrics`] enum contains groups of
    /// [`EventKind`]s, to avoid having to specify all [`EventKind`]s one-by-one (although still
    /// possible with [`CallgrindMetrics::SingleEvent`]).
    ///
    /// All command-line arguments of Callgrind and which metric they collect are described in full
    /// detail in the [Callgrind
    /// documentation](https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options).
    ///
    /// # Examples
    ///
    /// To enable printing all Callgrind metrics specify [`CallgrindMetrics::All`]. `All` Callgrind
    /// metrics include the cache misses ([`EventKind::I1mr`], ...). For example in a library
    /// benchmark:
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// use gungraun::{Callgrind, CallgrindMetrics, LibraryBenchmarkConfig, OutputFormat, main};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Callgrind::default().format([CallgrindMetrics::All])),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    ///
    /// The benchmark is executed with the Callgrind arguments set by gungraun which don't
    /// collect any other metrics than cache misses (`--cache-sim=yes`), so the output will look
    /// like this:
    ///
    /// ```text
    /// file::some_group::printing cache_misses:
    ///   Instructions:                        1353|1353                 (No change)
    ///   Dr:                                   255|255                  (No change)
    ///   Dw:                                   233|233                  (No change)
    ///   I1mr:                                  54|54                   (No change)
    ///   D1mr:                                  12|12                   (No change)
    ///   D1mw:                                   0|0                    (No change)
    ///   ILmr:                                  53|53                   (No change)
    ///   DLmr:                                   3|3                    (No change)
    ///   DLmw:                                   0|0                    (No change)
    ///   L1 Hits:                             1775|1775                 (No change)
    ///   LL Hits:                               10|10                   (No change)
    ///   RAM Hits:                              56|56                   (No change)
    ///   Total read+write:                    1841|1841                 (No change)
    ///   Estimated Cycles:                    3785|3785                 (No change)
    /// ```
    pub fn format<I, T>(&mut self, callgrind_metrics: T) -> &mut Self
    where
        I: Into<CallgrindMetrics>,
        T: IntoIterator<Item = I>,
    {
        let format = self
            .0
            .output_format
            .get_or_insert_with(|| __internal::InternalToolOutputFormat::Callgrind(Vec::new()));

        if let __internal::InternalToolOutputFormat::Callgrind(items) = format {
            items.extend(callgrind_metrics.into_iter().map(Into::into));
        }

        self
    }
}

impl Default for Callgrind {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::Callgrind))
    }
}

impl Dhat {
    /// Creates a new `Callgrind` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`] and [`Dhat::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Dhat;
    ///
    /// let config = Dhat::with_args(["mode=ad-hoc"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(Tool::DHAT, args))
    }

    /// Adds command-line arguments to the `Dhat` configuration.
    ///
    /// Valid arguments
    /// are <https://valgrind.org/docs/manual/dh-manual.html#dh-manual.options> and the core
    /// Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Dhat;
    ///
    /// let config = Dhat::default().args(["interval-size=10000"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// See also [`Callgrind::enable`]
    ///
    /// ```rust
    /// use gungraun::Dhat;
    ///
    /// let config = Dhat::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }

    /// Customize the format of the dhat output
    ///
    /// See also [`Callgrind::format`] for more details and [`DhatMetric`] for valid metrics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{Dhat, DhatMetric};
    ///
    /// let config = Dhat::default().format([DhatMetric::TotalBytes, DhatMetric::AtTGmaxBytes]);
    /// ```
    pub fn format<I, T>(&mut self, kinds: T) -> &mut Self
    where
        I: Into<DhatMetric>,
        T: IntoIterator<Item = I>,
    {
        let format = self
            .0
            .output_format
            .get_or_insert_with(|| __internal::InternalToolOutputFormat::DHAT(Vec::new()));

        if let __internal::InternalToolOutputFormat::DHAT(items) = format {
            items.extend(kinds.into_iter().map(Into::into));
        }

        self
    }

    /// Sets or unset the entry point for DHAT.
    ///
    /// The basic concept of this [`EntryPoint`] is almost the same as for
    /// [`Callgrind::entry_point`] and for additional details see there. For library benchmarks the
    /// default entry point is [`EntryPoint::Default`] and for binary benchmarks it's
    /// [`EntryPoint::None`].
    ///
    /// Note that the default entry point tries to match the benchmark function, so it doesn't make
    /// much sense to use [`EntryPoint::Default`] in binary benchmarks. The result of an incorrect
    /// entry point is usually that all metrics are `0`, which is an indicator that something has
    /// gone wrong.
    ///
    /// # Details
    ///
    /// The [`EntryPoint`] for [`Dhat`] works exactly the same way as the entry point for
    /// [`Callgrind`]. As a consequence, allocations and deallocations in the `setup` and
    /// `teardown` function are excluded from the final metrics. This behavior typically aligns with
    /// user expectations. However, DHAT has a unique characteristic: if the benchmarked function
    /// uses an array created in the setup function, the metrics will not capture the reads and
    /// writes to that array. To accurately measure these reads and writes, it is necessary to set
    /// the entry point to the setup function.
    ///
    /// Since there is no `--toggle-collect` argument, it's possible to define additional `frames`
    /// (the Gungraun specific DHAT equivalent of callgrind toggles) in the [`Dhat::frames`] method.
    ///
    /// The [`EntryPoint::Default`] matches the benchmark function and a [`EntryPoint::Custom`] is
    /// convenience for specifying [`EntryPoint::None`] and a frame in [`Dhat::frames`].
    ///
    /// # Examples
    ///
    /// Specifying no entry point in library benchmarks is the same as specifying
    /// [`EntryPoint::Default`]. It is used here nonetheless for demonstration purposes:
    ///
    /// ```rust
    /// # mod my_lib { pub fn to_be_benchmarked() -> Vec<i32> { vec![0] } }
    /// use std::hint::black_box;
    ///
    /// use gungraun::{
    ///     Dhat, EntryPoint, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main,
    /// };
    /// use my_lib::to_be_benchmarked;
    ///
    /// #[library_benchmark(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Dhat::default().entry_point(EntryPoint::Default))
    /// )]
    /// // DEFAULT ENTRY POINT
    /// fn some_bench() -> Vec<i32> {
    ///     black_box(to_be_benchmarked())
    /// }
    ///
    /// library_benchmark_group!(name = some_group, benchmarks = some_bench);
    /// # fn main() {
    /// main!(library_benchmark_groups = some_group);
    /// # }
    /// ```
    ///
    /// You most likely want to disable the entry point with [`EntryPoint::None`] if you're using
    /// DHAT ad-hoc profiling.
    #[cfg_attr(not(feature = "stubs"), doc = "```rust,ignore")]
    #[cfg_attr(feature = "stubs", doc = "```rust")]
    /// use gungraun::{
    ///     main, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group,
    ///     EntryPoint, Dhat
    /// };
    /// use std::hint::black_box;
    ///
    /// fn to_be_benchmarked() -> Vec<i32> {
    ///     gungraun::client_requests::dhat::ad_hoc_event(20);
    ///     // allocations worth a weight of `20`
    /// #   vec![1, 2, 3, 4, 5]
    /// }
    ///
    /// #[library_benchmark(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Dhat::with_args(["--mode=ad-hoc"])
    ///             .entry_point(EntryPoint::None)
    ///         )
    /// )]
    /// fn some_bench() -> Vec<i32> {
    ///     black_box(to_be_benchmarked())
    /// }
    ///
    /// library_benchmark_group!(name = some_group, benchmarks = some_bench);
    /// # fn main() {
    /// main!(library_benchmark_groups = some_group);
    /// # }
    /// ```
    pub fn entry_point(&mut self, entry_point: EntryPoint) -> &mut Self {
        self.0.entry_point = Some(entry_point);
        self
    }

    /// Adds one or multiple `frames` which will be included in the benchmark metrics.
    ///
    /// `Frames` are special to Gungraun and the DHAT equivalent to callgrind toggles
    /// (`--toggle-collect`) and like `--toggle-collect` this method accepts simple glob patterns
    /// with `*` and `?` wildcards. A `Frame` describes an entry in the call stack (See the
    /// example). Sometimes the [`Dhat::entry_point`] is not enough and it is required to specify
    /// additional frames. This is especially true in multi-threaded/multi-process applications.
    /// Like in callgrind, each thread/subprocess in DHAT is treated as a separate unit and thus
    /// requires `frames` in addition to the default entry point to include the interesting ones in
    /// the measurements.
    ///
    /// # Example
    ///
    /// To demonstrate a general workflow, below is a sanitized example output of `dh_view.html` of
    /// a benchmark of a multi-threaded program. Most of the program points, including the default
    /// entry point, are not shown here to safe some space. The spawned thread
    /// (`std::sys::pal::unix::thread::Thread::new::thread_start`) with the function call
    /// `benchmark_tests::find_primes` is the interesting one.
    ///
    /// ```text
    /// ▼ PP 1/1 (3 children) {
    ///     Total:     156,372 bytes (100%, 14,948.32/Minstr) in 76 blocks (100%, 7.27/Minstr), avg size 2,057.53 bytes, avg lifetime 2,907,942.57 instrs (27.8% of program duration)
    ///     At t-gmax: 52,351 bytes (100%) in 20 blocks (100%), avg size 2,617.55 bytes
    ///     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
    ///     Reads:     117,583 bytes (100%, 11,240.3/Minstr), 0.75/byte
    ///     Writes:    135,680 bytes (100%, 12,970.28/Minstr), 0.87/byte
    ///     Allocated at {
    ///       #0: [root]
    ///     }
    ///   }
    ///   ├─▼ PP 1.1/3 (12 children) {
    ///   │     Total:     154,468 bytes (98.78%, 14,766.31/Minstr) in 57 blocks (75%, 5.45/Minstr), avg size 2,709.96 bytes, avg lifetime 2,937,398.7 instrs (28.08% of program duration)
    ///   │     At t-gmax: 51,375 bytes (98.14%) in 15 blocks (75%), avg size 3,425 bytes
    ///   │     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
    ///   │     Reads:     116,367 bytes (98.97%, 11,124.06/Minstr), 0.75/byte
    ///   │     Writes:    134,872 bytes (99.4%, 12,893.03/Minstr), 0.87/byte
    ///   │     Allocated at {
    ///   │       #1: 0x48CC7A8: malloc (in /usr/lib/valgrind/vgpreload_dhat-amd64-linux.so)
    ///   │     }
    ///   │   }
    ///   │   ├── PP 1.1.1/12 {
    ///   │   │     Total:     81,824 bytes (52.33%, 7,821.93/Minstr) in 29 blocks (38.16%, 2.77/Minstr), avg size 2,821.52 bytes, avg lifetime 785,423.83 instrs (7.51% of program duration)
    ///   │   │     Max:       40,960 bytes in 3 blocks, avg size 13,653.33 bytes
    ///   │   │     At t-gmax: 40,960 bytes (78.24%) in 3 blocks (15%), avg size 13,653.33 bytes
    ///   │   │     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
    ///   │   │     Reads:     66,824 bytes (56.83%, 6,388.01/Minstr), 0.82/byte
    ///   │   │     Writes:    66,824 bytes (49.25%, 6,388.01/Minstr), 0.82/byte
    ///   │   │     Allocated at {
    ///   │   │       ^1: 0x48CC7A8: malloc (in /usr/lib/valgrind/vgpreload_dhat-amd64-linux.so)
    ///   │   │       #2: 0x40197C7: UnknownInlinedFun (alloc.rs:93)
    ///   │   │       #3: 0x40197C7: UnknownInlinedFun (alloc.rs:188)
    ///   │   │       #4: 0x40197C7: UnknownInlinedFun (alloc.rs:249)
    ///   │   │       #5: 0x40197C7: UnknownInlinedFun (mod.rs:476)
    ///   │   │       #6: 0x40197C7: with_capacity_in<alloc::alloc::Global> (mod.rs:422)
    ///   │   │       #7: 0x40197C7: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:190)
    ///   │   │       #8: 0x40197C7: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:815)
    ///   │   │       #9: 0x40197C7: with_capacity<u64> (mod.rs:495)
    ///   │   │       #10: 0x40197C7: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, benchmark_tests::find_primes::{closure_env#0}>> (spec_from_iter_nested.rs:31)
    ///   │   │       #11: 0x40197C7: <alloc::vec::Vec<T> as alloc::vec::spec_from_iter::SpecFromIter<T,I>>::from_iter (spec_from_iter.rs:34)
    ///   │   │       #12: 0x4016B97: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, benchmark_tests::find_primes::{closure_env#0}>> (mod.rs:3438)
    ///   │   │       #13: 0x4016B97: collect<core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, benchmark_tests::find_primes::{closure_env#0}>, alloc::vec::Vec<u64, alloc::alloc::Global>> (iterator.rs:2001)
    ///   │   │       #14: 0x4016B97: benchmark_tests::find_primes (lib.rs:25)
    ///   │   │       #15: 0x4019DA0: {closure#0} (lib.rs:32)
    ///   │   │       #16: 0x4019DA0: std::sys::backtrace::__rust_begin_short_backtrace (backtrace.rs:152)
    ///   │   │       #17: 0x4018BB4: {closure#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>> (mod.rs:559)
    ///   │   │       #18: 0x4018BB4: call_once<alloc::vec::Vec<u64, alloc::alloc::Global>, std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>> (unwind_safe.rs:272)
    ///   │   │       #19: 0x4018BB4: do_call<core::panic::unwind_safe::AssertUnwindSafe<std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panicking.rs:589)
    ///   │   │       #20: 0x4018BB4: try<alloc::vec::Vec<u64, alloc::alloc::Global>, core::panic::unwind_safe::AssertUnwindSafe<std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>>> (panicking.rs:552)
    ///   │   │       #21: 0x4018BB4: catch_unwind<core::panic::unwind_safe::AssertUnwindSafe<std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panic.rs:359)
    ///   │   │       #22: 0x4018BB4: {closure#1}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>> (mod.rs:557)
    ///   │   │       #23: 0x4018BB4: core::ops::function::FnOnce::call_once{{vtable.shim}} (function.rs:250)
    ///   │   │       #24: 0x404A2BA: call_once<(), dyn core::ops::function::FnOnce<(), Output=()>, alloc::alloc::Global> (boxed.rs:1966)
    ///   │   │       #25: 0x404A2BA: call_once<(), alloc::boxed::Box<dyn core::ops::function::FnOnce<(), Output=()>, alloc::alloc::Global>, alloc::alloc::Global> (boxed.rs:1966)
    ///   │   │       #26: 0x404A2BA: std::sys::pal::unix::thread::Thread::new::thread_start (thread.rs:97)
    ///   │   │       #27: 0x49C27EA: ??? (in /usr/lib/libc.so.6)
    ///   │   │       #28: 0x4A45FB3: clone (in /usr/lib/libc.so.6)
    ///   │   │     }
    ///   │   │   }
    ///
    ///   ...
    /// ```
    ///
    /// As can be seen, the call stack of the program point `PP 1.1.1/12` does not include a main
    /// function, benchmark function, and so forth because a thread is a completely separate unit.
    /// This enables us to exclude uninteresting threads by simply not specifying them here and
    /// include the interesting ones for example with:
    ///
    /// ```rust
    /// use gungraun::Dhat;
    ///
    /// Dhat::default().frames(["benchmark_tests::find_primes"]);
    /// ```
    pub fn frames<I, T>(&mut self, frames: T) -> &mut Self
    where
        I: Into<String>,
        T: IntoIterator<Item = I>,
    {
        let spec = self.dhat_spec_mut();
        let this = spec.frames.get_or_insert_with(Vec::new);
        this.extend(frames.into_iter().map(Into::into));

        self
    }

    /// Configures whether DHAT JSON output is sanitized after entry point and frame filtering.
    ///
    /// The metrics in Gungraun's DHAT output are tailored to the [`EntryPoint`] and additional
    /// [frame filtering][`Dhat::frames`]. By default, this tailoring is also applied to the DHAT
    /// output files on which the metrics are based. [`sanitize_output`][Dhat::sanitize_output]
    /// allows disabling sanitization with [`SanitizeOutput::No`], or keeping a backup of the
    /// original files with [`SanitizeOutput::KeepOrig`] while still applying sanitization to the
    /// DHAT output files.
    ///
    /// Sanitization is useful when inspecting the output with DHAT's `dh_view.html` data
    /// visualizer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{Dhat, SanitizeOutput};
    ///
    /// let config = Dhat::default().sanitize_output(SanitizeOutput::KeepOrig);
    /// ```
    pub fn sanitize_output(&mut self, sanitize_output: SanitizeOutput) -> &mut Self {
        self.0.sanitize_output = Some(sanitize_output);
        self
    }

    /// Configures the limits percentages over/below which a performance regression can be assumed.
    ///
    /// Same as [`Callgrind::soft_limits`] but for [`DhatMetric`]s.
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Dhat, DhatMetric};
    ///
    /// let config = Dhat::default().soft_limits([(DhatMetric::TotalBytes, 5f64)]);
    /// ```
    pub fn soft_limits<K, T>(&mut self, soft_limits: T) -> &mut Self
    where
        K: Into<DhatMetrics>,
        T: IntoIterator<Item = (K, f64)>,
    {
        let iter = soft_limits.into_iter().map(|(k, l)| (k.into(), l));

        if let Some(__internal::InternalToolRegressionConfig::Dhat(config)) =
            &mut self.0.regression_config
        {
            config.soft_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Dhat(
                __internal::InternalDhatRegressionConfig {
                    soft_limits: iter.collect(),
                    hard_limits: Vec::default(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// Sets hard limits above which a performance regression can be assumed.
    ///
    /// Same as [`Callgrind::hard_limits`] but for [`DhatMetric`]s.
    ///
    /// # Examples
    ///
    /// If in a benchmark configured like below, there are more than a total of `10_000` bytes
    /// allocated, a performance regression is registered failing the benchmark run.
    ///
    /// ```
    /// use gungraun::{Dhat, DhatMetric};
    ///
    /// let config = Dhat::default().hard_limits([(DhatMetric::TotalBytes, 10_000)]);
    /// ```
    ///
    /// or for a group of metrics but with a special value for `TotalBytes`:
    ///
    /// ```
    /// use gungraun::{Dhat, DhatMetric, DhatMetrics};
    ///
    /// let config = Dhat::default().hard_limits([
    ///     (DhatMetrics::Default, 10_000),
    ///     (DhatMetric::TotalBytes.into(), 5_000),
    /// ]);
    /// ```
    pub fn hard_limits<K, L, T>(&mut self, hard_limits: T) -> &mut Self
    where
        K: Into<DhatMetrics>,
        L: Into<Limit>,
        T: IntoIterator<Item = (K, L)>,
    {
        let iter = hard_limits.into_iter().map(|(k, l)| (k.into(), l.into()));

        if let Some(__internal::InternalToolRegressionConfig::Dhat(config)) =
            &mut self.0.regression_config
        {
            config.hard_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Dhat(
                __internal::InternalDhatRegressionConfig {
                    soft_limits: Vec::default(),
                    hard_limits: iter.collect(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// If set to true, then the benchmarks fail on the first encountered regression
    ///
    /// The default is `false` and the whole benchmark run fails with a regression error after all
    /// benchmarks have been run. This option does not enable regression checks by itself. Configure
    /// regression checks explicitly with [`Dhat::soft_limits`] or [`Dhat::hard_limits`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Dhat, DhatMetric};
    ///
    /// let config = Dhat::default()
    ///     .soft_limits([(DhatMetric::TotalBytes, 5f64)])
    ///     .fail_fast(true);
    /// ```
    pub fn fail_fast(&mut self, value: bool) -> &mut Self {
        if let Some(__internal::InternalToolRegressionConfig::Dhat(config)) =
            &mut self.0.regression_config
        {
            config.fail_fast = Some(value);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Dhat(
                __internal::InternalDhatRegressionConfig {
                    soft_limits: Vec::default(),
                    hard_limits: Vec::default(),
                    fail_fast: Some(value),
                },
            ));
        }
        self
    }

    fn dhat_spec_mut(&mut self) -> &mut __internal::InternalDhatSpec {
        if !matches!(self.0.options, __internal::InternalToolSpecOptions::Dhat(_)) {
            self.0.options =
                __internal::InternalToolSpecOptions::Dhat(__internal::InternalDhatSpec::default());
        }

        match &mut self.0.options {
            __internal::InternalToolSpecOptions::Dhat(spec) => spec,
            _ => unreachable!("Dhat should always use DhatSpec"),
        }
    }
}

impl Default for Dhat {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::DHAT))
    }
}

impl Drd {
    /// Creates a new `Drd` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`] and [`Drd::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Drd;
    ///
    /// let config = Drd::with_args(["exclusive-threshold=100"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(Tool::DRD, args))
    }

    /// Adds command-line arguments to the `Drd` configuration.
    ///
    /// Valid arguments are <https://valgrind.org/docs/manual/drd-manual.html#drd-manual.options>
    /// and the core Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Drd;
    ///
    /// let config = Drd::default().args(["exclusive-threshold=100"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// See also [`Callgrind::enable`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Drd;
    ///
    /// let config = Drd::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }

    /// Customize the format of the `DRD` output
    ///
    /// See also [`Callgrind::format`] for more details and [`ErrorMetric`] for valid metrics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{Drd, ErrorMetric};
    ///
    /// let config = Drd::default().format([ErrorMetric::Errors, ErrorMetric::SuppressedErrors]);
    /// ```
    pub fn format<I, T>(&mut self, kinds: T) -> &mut Self
    where
        I: Into<ErrorMetric>,
        T: IntoIterator<Item = I>,
    {
        let format = self
            .0
            .output_format
            .get_or_insert_with(|| __internal::InternalToolOutputFormat::DRD(Vec::new()));

        if let __internal::InternalToolOutputFormat::DRD(items) = format {
            items.extend(kinds.into_iter().map(Into::into));
        }

        self
    }
}

impl Default for Drd {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::DRD))
    }
}

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
    /// use gungraun::{FlamegraphConfig, FlamegraphKind};
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
    /// the negated differential flamegraph shows what will happen. Especially, this allows you to
    /// see vanished event lines (in blue) for example because the underlying code has improved
    /// and removed an unnecessary function call.
    ///
    /// See also [Differential Flame
    /// Graphs](https://www.brendangregg.com/blog/2014-11-09/differential-flame-graphs.html) from
    /// Brendan Gregg's Blog.
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{FlamegraphConfig, FlamegraphKind};
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
    /// use gungraun::{FlamegraphConfig, FlamegraphKind};
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
    /// use gungraun::{EventKind, FlamegraphConfig};
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
    /// use gungraun::{Direction, FlamegraphConfig};
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
    /// use gungraun::{Direction, FlamegraphConfig};
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
    /// use gungraun::FlamegraphConfig;
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
    /// use gungraun::FlamegraphConfig;
    ///
    /// let config = FlamegraphConfig::default().min_width(0f64);
    /// ```
    pub fn min_width(&mut self, min_width: f64) -> &mut Self {
        self.0.min_width = Some(min_width);
        self
    }
}

impl Helgrind {
    /// Creates a new `Helgrind` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`] and [`Helgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Helgrind;
    ///
    /// let config = Helgrind::with_args(["free-is-write=yes"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(
            Tool::Helgrind,
            args,
        ))
    }

    /// Adds command-line arguments to the `Helgrind` configuration.
    ///
    /// Valid arguments
    /// are <https://valgrind.org/docs/manual/hg-manual.html#hg-manual.options> and the core
    /// Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Helgrind;
    ///
    /// let config = Helgrind::default().args(["free-is-write=yes"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// See also [`Callgrind::enable`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Helgrind;
    ///
    /// let config = Helgrind::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }

    /// Customize the format of the `Helgrind` output
    ///
    /// See also [`Callgrind::format`] for more details and [`ErrorMetric`] for valid metrics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{ErrorMetric, Helgrind};
    ///
    /// let config = Helgrind::default().format([ErrorMetric::Errors, ErrorMetric::SuppressedErrors]);
    /// ```
    pub fn format<I, T>(&mut self, kinds: T) -> &mut Self
    where
        I: Into<ErrorMetric>,
        T: IntoIterator<Item = I>,
    {
        let format = self
            .0
            .output_format
            .get_or_insert_with(|| __internal::InternalToolOutputFormat::Helgrind(Vec::new()));

        if let __internal::InternalToolOutputFormat::Helgrind(items) = format {
            items.extend(kinds.into_iter().map(Into::into));
        }

        self
    }
}

impl Default for Helgrind {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::Helgrind))
    }
}

impl Massif {
    /// Creates a new `Massif` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`] and [`Massif::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Massif;
    ///
    /// let config = Massif::with_args(["threshold=2.0"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(Tool::Massif, args))
    }

    /// Adds command-line arguments to the `Massif` configuration.
    ///
    /// Valid arguments
    /// are <https://valgrind.org/docs/manual/ms-manual.html#ms-manual.options> and the core
    /// Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Massif;
    ///
    /// let config = Massif::default().args(["threshold=2.0"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// See also [`Callgrind::enable`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Massif;
    ///
    /// let config = Massif::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }
}

impl Default for Massif {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::Massif))
    }
}

impl Memcheck {
    /// Creates a new `Memcheck` configuration with initial command-line arguments.
    ///
    /// See also [`Callgrind::args`] and [`Memcheck::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Memcheck;
    ///
    /// let config = Memcheck::with_args(["free-is-write=yes"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(
            Tool::Memcheck,
            args,
        ))
    }

    /// Adds command-line arguments to the `Memcheck` configuration.
    ///
    /// Valid arguments
    /// are <https://valgrind.org/docs/manual/mc-manual.html#mc-manual.options> and the core
    /// Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>.
    ///
    /// See also [`Callgrind::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Memcheck;
    ///
    /// let config = Memcheck::default().args(["show-leak-kinds=all"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend_ignore_flag(args);
        self
    }

    /// Enable this tool. This is the default.
    ///
    /// See also [`Callgrind::enable`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Memcheck;
    ///
    /// let config = Memcheck::default().enable(false);
    /// ```
    pub fn enable(&mut self, value: bool) -> &mut Self {
        self.0.enable = Some(value);
        self
    }

    /// Customize the format of the `Memcheck` output
    ///
    /// See also [`Callgrind::format`] for more details and [`ErrorMetric`] for valid metrics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{ErrorMetric, Memcheck};
    ///
    /// let config = Memcheck::default().format([ErrorMetric::Errors, ErrorMetric::SuppressedErrors]);
    /// ```
    pub fn format<I, T>(&mut self, kinds: T) -> &mut Self
    where
        I: Into<ErrorMetric>,
        T: IntoIterator<Item = I>,
    {
        let format = self
            .0
            .output_format
            .get_or_insert_with(|| __internal::InternalToolOutputFormat::Memcheck(Vec::new()));

        if let __internal::InternalToolOutputFormat::Memcheck(items) = format {
            items.extend(kinds.into_iter().map(Into::into));
        }

        self
    }
}

impl Default for Memcheck {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::Memcheck))
    }
}

impl Perf {
    /// Creates a new `Perf` configuration with initial command-line arguments.
    ///
    /// See also [`Perf::args`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::with_args(["--all-user"]);
    /// ```
    pub fn with_args<I, T>(args: T) -> Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        Self(__internal::InternalToolSpec::with_args(Tool::Perf, args))
    }

    /// Adds command-line arguments to the `Perf` configuration.
    ///
    /// The command-line arguments are passed directly to the `perf` invocation. Valid arguments are
    /// from the `perf stat` documentation. Command-line arguments for `perf record` if you have
    /// enabled it with [`Self::record`] can be added with [`Self::record_args`]. Note that not all
    /// command-line arguments are supported, especially the ones which manipulate the output and
    /// output paths. Unsupported arguments will be ignored, printing a warning.
    ///
    /// Unlike Valgrind tools, argument flags are necessary and cannot be omitted ("--all-user" but
    /// not `all-user`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().args(["--all-user"]);
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.raw_tool_args.extend(args);
        self
    }

    /// Sets the statistical significance threshold for perf soft-limit checks.
    ///
    /// `alpha` is the [p-value] threshold used to determine whether a metric change is
    /// statistically significant before applying soft limits. When [`Self::soft_limits`] are
    /// configured, only statistically significant changes are considered regressions.
    ///
    /// The default value is `0.05`. For benchmarking contexts, `0.05` is the conventional default
    /// because it balances sensitivity and false-positive rate well. Higher values make the check
    /// more sensitive, so it can catch smaller or noisier changes sooner, but it also increases the
    /// chance of false positives. Lower values make the check more conservative, which reduces
    /// noise-driven regression reports, but it may miss subtle real regressions unless more samples
    /// are collected.
    ///
    /// If your CI is noisy and you get too many spurious regressions, tightening to `0.01` or
    /// `0.001` (more stringent, useful when false positives are costly) can help. Some literature
    /// suggests a value of `0.005` as a middle-ground. If you are doing exploratory profiling and
    /// want to catch smaller changes, loosening to `0.10` may be appropriate. See [Statistical
    /// significance][stat-sig] for more background.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().alpha(0.05);
    /// ```
    ///
    /// [p-value]: https://en.wikipedia.org/wiki/P-value
    /// [stat-sig]: https://en.wikipedia.org/wiki/Statistical_significance
    pub fn alpha(&mut self, value: f64) -> &mut Self {
        let perf_spec = self.perf_spec_mut();
        perf_spec.alpha = Some(value);

        if let Some(__internal::InternalToolRegressionConfig::Perf(config)) =
            &mut self.0.regression_config
        {
            config.alpha = Some(value);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Perf(
                __internal::InternalPerfRegressionConfig {
                    alpha: Some(value),
                    soft_limits: Vec::default(),
                    hard_limits: Vec::default(),
                    fail_fast: None,
                },
            ));
        }

        self
    }

    /// Disables or enables the default entry point for the benchmark.
    ///
    /// When set to `true`, Gungraun does not automatically start perf measurement when the
    /// benchmark function is entered. Instead, you manually bracket the measured region with
    /// [`perf_enable!()`] and [`perf_disable!()`]. This is useful when the benchmark body contains
    /// setup or teardown work that should not be measured.
    ///
    /// The `perf_enable!()` and `perf_disable!()` macros can also be called from production code
    /// that is executed in the benchmark process. To use them outside benchmark code, the
    /// dependency on `gungraun` must enable the `stubs` feature (or `perf_stubs` if only the perf
    /// macros are needed without Valgrind client-request stubs). The Gungraun [guide] contains more
    /// examples.
    ///
    /// The perf client-request macros operate on a single process-global control channel. They are
    /// not thread-safe, must not be nested, and every token returned by [`perf_enable!()`] must be
    /// passed to exactly one matching [`perf_disable!()`].
    ///
    /// When set to `false`, the default entry point is restored to [`EntryPoint::Default`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{
    ///     LibraryBenchmarkConfig, Perf, library_benchmark, library_benchmark_group, main,
    /// };
    ///
    /// #[library_benchmark(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Perf::default().disable_entry_point(true))
    /// )]
    /// fn some_bench() {
    ///     // setup code (not measured)
    ///     let token = gungraun::perf_enable!();
    ///     // benchmarked code
    ///     gungraun::perf_disable!(token);
    ///     // teardown code (not measured)
    /// }
    ///
    /// library_benchmark_group!(name = some_group, benchmarks = some_bench);
    /// # fn main() {
    /// # main!(library_benchmark_groups = some_group);
    /// # }
    /// ```
    ///
    /// [`perf_enable!()`]: crate::perf_enable
    /// [`perf_disable!()`]: crate::perf_disable
    /// [guide]: https://gungraun.github.io/gungraun/latest/html/index.html
    pub fn disable_entry_point(&mut self, yes: bool) -> &mut Self {
        if yes {
            self.0.entry_point = Some(EntryPoint::None);
        } else {
            self.0.entry_point = Some(EntryPoint::Default);
        }

        self
    }

    /// Adds a single perf event selector to measure.
    ///
    /// The event selector is passed directly to `perf` and determines which hardware or software
    /// events are counted. Each event set is executed in a separate perf invocation and passed to
    /// perf with `--event` as-is. Hence, `event_set` supports the same syntax as the perf
    /// stat/record `--event` event selector.
    ///
    /// These event sets are the same for `perf stat` and `perf record` (if activated with
    /// [`Self::record`]).
    ///
    /// # Examples
    ///
    /// Executes `perf stat` once with `--event=cycles,instructions`.
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().event_set("cycles,instructions");
    /// ```
    ///
    /// Executes `perf stat` twice. The first time with `--event=cache-misses` and a second time
    /// with `--event={instructions,cycles}` (using perf's group syntax):
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default()
    ///     .event_set("cache-misses")
    ///     .event_set("{instructions,cycles}");
    /// ```
    pub fn event_set<T>(&mut self, events: T) -> &mut Self
    where
        T: AsRef<str>,
    {
        let spec = self.perf_spec_mut();
        let events_spec = spec.events.get_or_insert_with(Vec::new);
        events_spec.push(events.as_ref().to_owned());

        self
    }

    /// Adds multiple perf event sets, equivalent to calling [`Self::event_set`] once for each item.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().event_sets(["cycles", "instructions"]);
    /// ```
    pub fn event_sets<I, T>(&mut self, events: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        let spec = self.perf_spec_mut();
        let events_spec = spec.events.get_or_insert_with(Vec::new);
        events_spec.extend(events.into_iter().map(|event| event.as_ref().to_owned()));

        self
    }

    /// If set to `true`, the benchmarks fail on the first encountered regression.
    ///
    /// The default is `false` and the whole benchmark run fails with a regression error after all
    /// benchmarks have been run. This option does not enable regression checks by itself. Configure
    /// regression checks explicitly with [`Perf::soft_limits`] or [`Perf::hard_limits`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{Perf, PerfMetric};
    ///
    /// let config = Perf::default()
    ///     .soft_limits([("*instructions*", 5.0)])
    ///     .fail_fast(true);
    /// ```
    pub fn fail_fast(&mut self, value: bool) -> &mut Self {
        if let Some(__internal::InternalToolRegressionConfig::Perf(config)) =
            &mut self.0.regression_config
        {
            config.fail_fast = Some(value);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Perf(
                __internal::InternalPerfRegressionConfig {
                    alpha: None,
                    soft_limits: Vec::default(),
                    hard_limits: Vec::default(),
                    fail_fast: Some(value),
                },
            ));
        }
        self
    }

    /// Sets patterns for perf metrics that must be nonzero.
    ///
    /// If a metric matching one of these patterns is exactly zero, the entire measurement record
    /// containing it is discarded. Each measurement record contains all metrics selected by one
    /// [`Self::event_set`]. By default, these patterns are: `task-clock*`, `cpu-clock*`, and
    /// `*instructions*`. Calling this method overrides the default patterns.
    ///
    /// Short-running benchmarks can occasionally produce zero values for metrics expected to be
    /// nonzero. In sampling mode, discarding these records mitigates one source of artificial
    /// low-end skew in the measured metrics.
    ///
    /// Configure only metrics that cannot legitimately be zero for the benchmark. A matching zero
    /// discards the entire measurement record.
    ///
    /// Patterns use [`simplematch`] wildcard syntax, including:
    ///
    /// - `*` (any sequence of characters),
    /// - `?` (a single character),
    /// - `\` to escape special characters,
    /// - character classes such as `[...]`, `[!...]`, and `[a-zA-Z]`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().non_zero_metrics(["cycles", "instructions"]);
    /// ```
    ///
    /// [`simplematch`]: https://crates.io/crates/simplematch
    pub fn non_zero_metrics<I, T>(&mut self, values: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        let spec = self.perf_spec_mut();
        spec.non_zero_metrics = Some(values.into_iter().map(|v| v.as_ref().to_owned()).collect());

        self
    }

    /// Sets the minimum percentage of time a PMU counter must be running.
    ///
    /// When perf multiplexes hardware counters because more events are requested than physical PMU
    /// slots exist, `pcnt_running` reports the fraction of the interval the counter was active.
    /// Gungraun discards sampled records whose `pcnt_running` falls below the `min_pcnt_running`
    /// threshold.
    ///
    /// The default is `100.0` (no multiplexing tolerated) and valid `min_pcnt_running` values are
    ///
    /// `0.0 <= min_pcnt_running <= 100.0`.
    ///
    /// Lower this value only if you intentionally request more events than the hardware can count
    /// simultaneously and you still want to keep multiplexed data. Usually, it is better to keep
    /// the default and split the amount of events into multiple sets using [`Perf::event_sets`]
    /// with each set having the number of available physical PMU slots. However, splitting into
    /// multiple sets requires perf to be run multiple times.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().min_pcnt_running(80.0);
    /// ```
    pub fn min_pcnt_running(&mut self, percent: f64) -> &mut Self {
        let spec = self.perf_spec_mut();
        spec.min_pcnt_running = Some(percent);

        self
    }

    /// Configures the soft limits over/below which a performance regression can be assumed.
    ///
    /// A soft limit consists of a metric pattern (See [`Self::non_zero_metrics`] for a description
    /// of the wildcard syntax) and a percentage over which a regression is assumed. If the limit is
    /// negative, then a regression is assumed to be below this limit. Only [statistically
    /// significant][stat-sig] changes are checked against the soft limits.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().soft_limits([("cycles", 5f64)]);
    /// ```
    ///
    /// [stat-sig]: Self::alpha
    pub fn soft_limits<K, T>(&mut self, soft_limits: T) -> &mut Self
    where
        K: Into<String>,
        T: IntoIterator<Item = (K, f64)>,
    {
        let iter = soft_limits.into_iter().map(|(k, l)| (k.into(), l));

        if let Some(__internal::InternalToolRegressionConfig::Perf(config)) =
            &mut self.0.regression_config
        {
            config.soft_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Perf(
                __internal::InternalPerfRegressionConfig {
                    alpha: None,
                    soft_limits: iter.collect(),
                    hard_limits: Vec::default(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// Sets hard limits above which a performance regression can be assumed.
    ///
    /// In contrast to [`Perf::soft_limits`], hard limits restrict a metric pattern (See
    /// [`Self::non_zero_metrics`] for a description of the wildcard syntax) in absolute numbers
    /// instead of a percentage. A hard limit only affects the `new` benchmark run.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{Limit, Perf};
    ///
    /// let config = Perf::default().hard_limits([("cycles", None, Limit::Int(10_000))]);
    /// ```
    pub fn hard_limits<K, L, U, T>(&mut self, hard_limits: T) -> &mut Self
    where
        K: Into<String>,
        L: Into<Limit>,
        U: Into<Option<Unit>>,
        T: IntoIterator<Item = (K, U, L)>,
    {
        let iter = hard_limits
            .into_iter()
            .map(|(k, u, l)| (k.into(), u.into(), l.into()));

        if let Some(__internal::InternalToolRegressionConfig::Perf(config)) =
            &mut self.0.regression_config
        {
            config.hard_limits.extend(iter);
        } else {
            self.0.regression_config = Some(__internal::InternalToolRegressionConfig::Perf(
                __internal::InternalPerfRegressionConfig {
                    alpha: None,
                    soft_limits: Vec::default(),
                    hard_limits: iter.collect(),
                    fail_fast: None,
                },
            ));
        }
        self
    }

    /// Sets the [`PerfRunMode`] for this perf configuration.
    ///
    /// The run mode controls how benchmark invocations are calibrated inside the `perf`
    /// measurement. See [`PerfRunMode`] for a description of each mode. The default is
    /// [`PerfRunMode::Direct`] which runs perf in normal mode without any special setup.
    ///
    /// For binary benchmarks, calibration-oriented run modes such as
    /// [`PerfRunMode::DefaultCalibrate`] and [`PerfRunMode::Calibrate`] are effectively ignored and
    /// fall back to [`PerfRunMode::Direct`], because the benchmark binary is invoked directly with
    /// command arguments.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// use gungraun::{LibraryBenchmarkConfig, Perf, PerfRunMode, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Perf::default().run_mode(PerfRunMode::DefaultCalibrate)),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn run_mode(&mut self, run_mode: PerfRunMode) -> &mut Self {
        let api_run_mode = match run_mode {
            PerfRunMode::DefaultCalibrate => __internal::InternalPerfRunMode::DefaultCalibrate,
            PerfRunMode::Calibrate(duration) => {
                __internal::InternalPerfRunMode::Calibrate(duration)
            }
            PerfRunMode::Direct => __internal::InternalPerfRunMode::Direct,
        };
        self.perf_spec_mut().run_mode = Some(api_run_mode);
        self
    }

    /// Configures whether to run a companion `perf record` capture in addition to `perf stat`.
    ///
    /// When enabled, the runner executes an additional `perf record` run with the benchmark,
    /// producing a sample-based profile that can be analyzed with `perf report`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().record(true);
    /// ```
    pub fn record(&mut self, yes: bool) -> &mut Self {
        self.perf_spec_mut().record = Some(yes);
        self
    }

    /// Adds command-line arguments to the optional `perf record` run.
    ///
    /// These arguments are passed only to the companion `perf record` invocation and not to
    /// `perf stat`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default()
    ///     .record(true)
    ///     .record_args(["--call-graph", "dwarf"]);
    /// ```
    pub fn record_args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        let spec = self.perf_spec_mut();
        spec.record_args.extend(args);
        self
    }

    /// Sets the sampling duration for `perf stat` in sampling mode.
    ///
    /// This duration is a wall-clock limit for continuously repeated `perf stat` sampling. It is
    /// independent from the duration supplied to [`PerfRunMode::Calibrate`], which controls a
    /// separate calibration pass.
    ///
    /// If the sampling duration is long enough for multiple benchmark runs, the first run is
    /// discarded to mitigate cold-start effects. However, there is always at least one record
    /// kept. For example, if a benchmark run takes 1s and the sampling duration is 2s, two runs
    /// fit within the window: the first is discarded, and one record is kept.
    ///
    /// Cold-start effects become less significant as benchmark runtime increases. With a 1s
    /// benchmark and a 1.5s sampling duration, only a single run fits within the window, so
    /// cold-start effects are present in the sole record. However, for longer-running benchmarks,
    /// these effects are typically negligible relative to the total measurement.
    ///
    /// A sampling duration above 1 second typically works well, but the optimal setting depends on
    /// your specific use-case and benchmark.
    ///
    /// For binary benchmarks, setup and teardown run only once before and after the sampling
    /// period, unlike library benchmarks where they run per sample.
    ///
    /// [`Self::sample_duration`] affects the main `perf stat` measurement, not the optional
    /// companion `perf record` capture (see [`Perf::record`]).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::time::Duration;
    ///
    /// use gungraun::Perf;
    ///
    /// let config = Perf::default().sample_duration(Duration::from_secs(1));
    /// ```
    pub fn sample_duration(&mut self, duration: Duration) -> &mut Self {
        let spec = self.perf_spec_mut();
        spec.sample_duration = Some(duration);
        self
    }

    fn perf_spec_mut(&mut self) -> &mut __internal::InternalPerfSpec {
        if !matches!(self.0.options, __internal::InternalToolSpecOptions::Perf(_)) {
            self.0.options =
                __internal::InternalToolSpecOptions::Perf(__internal::InternalPerfSpec::default());
        }

        match &mut self.0.options {
            __internal::InternalToolSpecOptions::Perf(spec) => spec,
            _ => unreachable!("Perf should always use PerfSpec"),
        }
    }
}

impl Default for Perf {
    fn default() -> Self {
        Self(__internal::InternalToolSpec::new(Tool::Perf))
    }
}

impl OutputFormat {
    /// Adjust, enable or disable the truncation of the description in the gungraun output
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
    ///   LL Hits:                      748|748             (No change)
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
    /// use gungraun::{LibraryBenchmarkConfig, OutputFormat, main};
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(
    /// #    name = some_group,
    /// #    benchmarks = some_func
    /// # );
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .output_format(OutputFormat::default().truncate_description(None)),
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
    /// In Callgrind, threads are treated as separate units (similar to subprocesses) and the
    /// metrics for them are dumped into an own file. Other Valgrind tools usually separate the
    /// output files only by subprocesses. To also show the metrics of any intermediate fragments
    /// and not just the total over all of them, set the value of this method to `true`.
    ///
    /// Temporarily setting `show_intermediate` to `true` can help to find misconfigurations in
    /// multi-thread/multi-process benchmarks.
    ///
    /// # Examples
    ///
    /// As opposed to Valgrind/Callgrind, `--trace-children=yes`, `--separate-threads=yes` and
    /// `--fair-sched=try` are the defaults in Gungraun, so in the following example it's not
    /// necessary to specify `--separate-threads` to track the metrics of the spawned thread.
    /// However, it is necessary to specify an additional toggle or else the metrics of the thread
    /// are all zero. We also set the [`super::EntryPoint`] to `None` to disable the default entry
    /// point (toggle) which is the benchmark function. So, with this setup we collect only the
    /// metrics of the method `my_lib::heavy_calculation` in the spawned thread and nothing else.
    ///
    /// ```rust
    /// use gungraun::{
    ///     Callgrind, EntryPoint, LibraryBenchmarkConfig, OutputFormat, library_benchmark,
    ///     library_benchmark_group, main,
    /// };
    /// # mod my_lib { pub fn heavy_calculation() -> u64 { 42 }}
    ///
    /// #[library_benchmark(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Callgrind::with_args(["--toggle-collect=my_lib::heavy_calculation"])
    ///             .entry_point(EntryPoint::None)
    ///         )
    ///         .output_format(OutputFormat::default().show_intermediate(true))
    /// )]
    /// fn bench_thread() -> u64 {
    ///     let handle = std::thread::spawn(|| my_lib::heavy_calculation());
    ///     handle.join().unwrap()
    /// }
    ///
    /// library_benchmark_group!(name = some_group, benchmarks = bench_thread);
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
    ///   LL Hits:                          0|N/A             (*********)
    ///   RAM Hits:                         0|N/A             (*********)
    ///   Total read+write:                 0|N/A             (*********)
    ///   Estimated Cycles:                 0|N/A             (*********)
    ///   ## pid: 633247 part: 1 thread: 2   |N/A
    ///   Command:            target/release/deps/my_benchmark-08fe8356975cd1af
    ///   Instructions:                  3905|N/A             (*********)
    ///   L1 Hits:                       4992|N/A             (*********)
    ///   LL Hits:                          0|N/A             (*********)
    ///   RAM Hits:                       464|N/A             (*********)
    ///   Total read+write:              5456|N/A             (*********)
    ///   Estimated Cycles:             21232|N/A             (*********)
    ///   ## Total
    ///   Instructions:                  3905|N/A             (*********)
    ///   L1 Hits:                       4992|N/A             (*********)
    ///   LL Hits:                          0|N/A             (*********)
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
    ///   LL Hits:                          0|N/A             (*********)
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
    /// use gungraun::OutputFormat;
    ///
    /// let output_format = OutputFormat::default().show_grid(true);
    /// ```
    ///
    /// Below is the output of a Gungraun run with DHAT as additional tool benchmarking a
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
    /// | LL Hits:                              17|17                   (No change)
    /// | RAM Hits:                            202|202                  (No change)
    /// | Total read+write:                   6639|6639                 (No change)
    /// | Estimated Cycles:                  13575|13575                (No change)
    /// |-## pid: 3186468 part: 1 thread: 1       |pid: 2721319 part: 1 thread: 1
    /// | Command:            target/release/thread 3
    /// | Instructions:                      35452|35452                (No change)
    /// | L1 Hits:                           77367|77367                (No change)
    /// | LL Hits:                             610|610                  (No change)
    /// | RAM Hits:                            784|784                  (No change)
    /// | Total read+write:                  78761|78761                (No change)
    /// | Estimated Cycles:                 107857|107857               (No change)
    /// |-## pid: 3186468 part: 1 thread: 2       |pid: 2721319 part: 1 thread: 2
    /// | Command:            target/release/thread 3
    /// | Instructions:                    2460507|2460507              (No change)
    /// | L1 Hits:                         2534939|2534939              (No change)
    /// | LL Hits:                              17|17                   (No change)
    /// | RAM Hits:                            186|186                  (No change)
    /// | Total read+write:                2535142|2535142              (No change)
    /// | Estimated Cycles:                2541534|2541534              (No change)
    /// |-## pid: 3186468 part: 1 thread: 3       |pid: 2721319 part: 1 thread: 3
    /// | Command:            target/release/thread 3
    /// | Instructions:                    3650414|3650414              (No change)
    /// | L1 Hits:                         3724275|3724275              (No change)
    /// | LL Hits:                              21|21                   (No change)
    /// | RAM Hits:                            130|130                  (No change)
    /// | Total read+write:                3724426|3724426              (No change)
    /// | Estimated Cycles:                3728930|3728930              (No change)
    /// |-## pid: 3186468 part: 1 thread: 4       |pid: 2721319 part: 1 thread: 4
    /// | Command:            target/release/thread 3
    /// | Instructions:                    4349846|4349846              (No change)
    /// | L1 Hits:                         4423438|4423438              (No change)
    /// | LL Hits:                              24|24                   (No change)
    /// | RAM Hits:                            125|125                  (No change)
    /// | Total read+write:                4423587|4423587              (No change)
    /// | Estimated Cycles:                4427933|4427933              (No change)
    /// |-## Total
    /// | Instructions:                   10500916|10500916             (No change)
    /// | L1 Hits:                        10766439|10766439             (No change)
    /// | LL Hits:                             689|689                  (No change)
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
    /// | LL Hits:                             601|689                  (-12.7721%) [-1.14642x]
    /// | RAM Hits:                           1189|1427                 (-16.6783%) [-1.20017x]
    /// | Total read+write:               10759049|10768555             (-0.08828%) [-1.00088x]
    /// | Estimated Cycles:               10801879|10819829             (-0.16590%) [-1.00166x]
    pub fn show_grid(&mut self, value: bool) -> &mut Self {
        self.0.show_grid = Some(value);
        self
    }

    /// Shows changes only when they are above the `tolerance` level
    ///
    /// Changes whose percentage is below the specified tolerance are not marked as changes.
    /// Negative tolerance values are converted to their absolute value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::OutputFormat;
    ///
    /// let output_format = OutputFormat::default().tolerance(1.5);
    /// ```
    ///
    /// Below is the output of an Gungraun run with the tolerance set.
    ///
    /// ```text
    /// my_benchmark::some_group::bench_with_tolerance_margin
    ///   Instructions:                     9975976|9976136              (Tolerance)
    ///   L1 Hits:                         10183337|10183517             (Tolerance)
    ///   LL Hits:                              641|654                  (-1.98777%) [-1.02028x]
    ///   RAM Hits:                            1211|1216                 (Tolerance)
    ///   Total read+write:                10185189|10185387             (Tolerance)
    ///   Estimated Cycles:                10228927|10229347             (Tolerance)
    /// ```
    pub fn tolerance(&mut self, value: f64) -> &mut Self {
        self.0.tolerance = Some(value);
        self
    }
}

impl Sandbox {
    /// Creates a new `Sandbox` builder.
    ///
    /// By default, benchmarks are not run in a `Sandbox` because setting up a `Sandbox` usually
    /// involves some user interaction, for example copying fixtures into it with
    /// [`Sandbox::fixtures`].
    ///
    /// The temporary directory is only created immediately before the benchmark is executed.
    ///
    /// # Examples
    ///
    /// Enables the sandbox for all binary benchmarks.
    ///
    /// ```rust
    /// use gungraun::{BinaryBenchmarkConfig, Sandbox, main};
    /// # use gungraun::binary_benchmark_group;
    /// # binary_benchmark_group!(name = my_group, benchmarks = |_group| {});
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    ///     binary_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// Enables the sandbox for all library benchmarks
    ///
    /// ```rust
    /// use gungraun::Sandbox;
    /// use gungraun::prelude::*;
    /// # #[library_benchmark] fn some_bench() {}
    /// # library_benchmark_group!(name = my_group, benchmarks = some_bench);
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    ///     library_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    pub fn new(enabled: bool) -> Self {
        Self(__internal::InternalSandbox {
            enabled: Some(enabled),
            ..Default::default()
        })
    }

    /// Specify the directories and/or files you want to copy into the root of the `Sandbox`
    ///
    /// The paths are interpreted relative to the workspace root as it is reported by `cargo`. In a
    /// multi-crate project this is the directory with the top-level `Cargo.toml`. Otherwise, it is
    /// simply the directory with your `Cargo.toml` file in it.
    ///
    /// # Examples
    ///
    /// Assuming you crate's binary is called `my-foo` taking a file path as the first argument and
    /// the fixtures directory is `$WORKSPACE_ROOT/benches/fixtures` containing a fixture
    /// `fix_1.txt`:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::{binary_benchmark_group, main};
    /// use gungraun::{BinaryBenchmarkConfig, Sandbox, binary_benchmark};
    ///
    /// #[binary_benchmark]
    /// #[bench::fix_1(
    ///      args = ("fix_1.txt"),
    ///      config = BinaryBenchmarkConfig::default()
    ///          .sandbox(Sandbox::new(true)
    ///              .fixtures(["benches/fixtures/fix_1.txt"])
    ///         )
    /// )]
    /// fn bench_with_fixtures(path: &str) -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
    ///         .arg(path)
    ///         .build()
    /// }
    ///
    /// # binary_benchmark_group!(name = my_group, benchmarks = bench_with_fixtures);
    /// # fn main() { main!(binary_benchmark_groups = my_group); }
    /// ```
    pub fn fixtures<I, T>(&mut self, paths: T) -> &mut Self
    where
        I: Into<PathBuf>,
        T: IntoIterator<Item = I>,
    {
        self.0.fixtures.extend(paths.into_iter().map(Into::into));
        self
    }
}
