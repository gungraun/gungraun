use std::ffi::OsString;
use std::path::PathBuf;

use derive_more::AsRef;
use gungraun_macros::IntoInner;
use gungraun_runner::api::Tool;

use crate::__internal;

/// The main configuration of a library benchmark.
///
/// # Examples
///
/// ```rust
/// # use gungraun::{library_benchmark, library_benchmark_group};
/// use gungraun::{Callgrind, LibraryBenchmarkConfig, main};
/// # #[library_benchmark]
/// # fn some_func() {}
/// # library_benchmark_group!(name = some_group, benchmarks = some_func);
/// # fn main() {
/// main!(
///     config = LibraryBenchmarkConfig::default()
///         .tool(Callgrind::with_args(["toggle-collect=something"])),
///     library_benchmark_groups = some_group
/// );
/// # }
/// ```
#[derive(Debug, Default, IntoInner, AsRef, Clone)]
pub struct LibraryBenchmarkConfig(__internal::InternalLibraryBenchmarkConfig);

impl LibraryBenchmarkConfig {
    /// Change the default tool to something different than Callgrind
    ///
    /// Any [`Tool`] is valid, however using Cachegrind also requires to use client requests
    /// to produce correct metrics. The guide fully describes how to use Cachegrind instead of
    /// Callgrind.
    ///
    /// # Example for dhat
    ///
    /// ```rust
    /// # mod lib { pub fn some_func(value: u64) -> u64 { value + 2 }}
    /// use gungraun::{
    ///     LibraryBenchmarkConfig, Tool, library_benchmark, library_benchmark_group, main,
    /// };
    ///
    /// #[library_benchmark]
    /// fn bench_me() -> u64 {
    ///     lib::some_func(10)
    /// }
    ///
    /// library_benchmark_group!(name = my_group, benchmarks = bench_me);
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().default_tool(Tool::DHAT),
    ///     library_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// # Example for using Cachegrind as default tool on the fly
    ///
    /// `--instr-at-start=no` is required to only measure the metrics between the two client
    /// request calls.
    #[cfg_attr(not(feature = "stubs"), doc = "```rust,ignore")]
    #[cfg_attr(feature = "stubs", doc = "```rust")]
    /// # mod lib { pub fn some_func(value: u64) -> u64 { value + 2 }}
    /// use gungraun::client_requests::cachegrind as cr;
    /// use gungraun::{
    ///     Cachegrind, LibraryBenchmarkConfig, Tool, library_benchmark,
    ///     library_benchmark_group, main,
    /// };
    ///
    /// #[library_benchmark(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .default_tool(Tool::Cachegrind)
    ///         .tool(Cachegrind::with_args(["--instr-at-start=no"]))
    /// )]
    /// fn bench_me() -> u64 {
    ///     cr::start_instrumentation();
    ///     let r = lib::some_func(10);
    ///     cr::stop_instrumentation();
    ///     r
    /// }
    ///
    /// library_benchmark_group!(name = my_group, benchmarks = bench_me);
    ///
    /// # fn main() {
    /// main!(library_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn default_tool<T>(&mut self, tool: T) -> &mut Self
    where
        T: Into<Tool>,
    {
        self.0.default_tool = Some(tool.into());
        self
    }

    /// Pass Valgrind arguments to all tools
    ///
    /// Only core [valgrind
    /// arguments](https://valgrind.org/docs/manual/manual-core.html#manual-core.options) are
    /// allowed.
    ///
    /// These arguments can be overwritten by tool specific arguments for example with
    /// [`crate::Callgrind::args`]
    ///
    /// # Examples
    ///
    /// Specify `--trace-children=no` for all configured tools (including Callgrind):
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark_group, library_benchmark};
    /// # #[library_benchmark] fn bench_me() {}
    /// # library_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = bench_me
    /// # );
    /// use gungraun::{Dhat, LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .valgrind_args(["--trace-children=no"])
    ///         .tool(Dhat::default()),
    ///     library_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// Overwrite the Valgrind argument `--num-callers=25` for `DHAT` with `--num-callers=30`:
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark_group, library_benchmark};
    /// # #[library_benchmark] fn bench_me() {}
    /// # library_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = bench_me
    /// # );
    /// use gungraun::{Dhat, LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .valgrind_args(["--num-callers=25"])
    ///         .tool(Dhat::with_args(["--num-callers=30"])),
    ///     library_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    pub fn valgrind_args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: AsRef<str>,
        T: IntoIterator<Item = I>,
    {
        self.0.valgrind_args.extend_ignore_flag(args);
        self
    }

    /// Clears the environment variables before running a benchmark (Default: true).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// use gungraun::{LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().env_clear(false),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn env_clear(&mut self, value: bool) -> &mut Self {
        self.0.env_clear = Some(value);
        self
    }

    /// Sets the directory of the library benchmark (Default: Unchanged).
    ///
    /// Unchanged means, in the case of running with the [`Sandbox`][crate::Sandbox] enabled, the
    /// root of the sandbox. In the case of running without sandboxing enabled, this will be the
    /// directory which `cargo bench` sets. If running the benchmark within the sandbox, and the
    /// path is relative then this new directory must be contained in the sandbox.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::prelude::*;
    /// # #[library_benchmark] fn bench() {}
    /// # library_benchmark_group!(name = my_group, benchmarks = bench);
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().current_dir("/tmp"),
    ///     library_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// and the following will change the current directory to `fixtures` assuming it is contained
    /// in the root of the sandbox
    ///
    /// ```rust
    /// use gungraun::Sandbox;
    /// use gungraun::prelude::*;
    /// # #[library_benchmark] fn bench() {}
    /// # library_benchmark_group!(name = my_group, benchmarks = bench);
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .sandbox(Sandbox::new(true))
    ///         .current_dir("fixtures"),
    ///     library_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    pub fn current_dir<T>(&mut self, value: T) -> &mut Self
    where
        T: Into<PathBuf>,
    {
        self.0.current_dir = Some(value.into());
        self
    }

    /// Adds environment variables which will be available in library benchmarks.
    ///
    /// These environment variables are available independently of the setting of
    /// [`LibraryBenchmarkConfig::env_clear`].
    ///
    /// # Examples
    ///
    /// An example for a custom environment variable, available in all benchmarks:
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// use gungraun::{LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().env("FOO", "BAR"),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn env<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.0.envs.push((key.into(), Some(value.into())));
        self
    }

    /// Adds multiple environment variables which will be available in library benchmarks.
    ///
    /// See also [`LibraryBenchmarkConfig::env`] for more details.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// use gungraun::{LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .envs([("MY_CUSTOM_VAR", "SOME_VALUE"), ("FOO", "BAR")]),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn envs<K, V, T>(&mut self, envs: T) -> &mut Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
        T: IntoIterator<Item = (K, V)>,
    {
        self.0
            .envs
            .extend(envs.into_iter().map(|(k, v)| (k.into(), Some(v.into()))));
        self
    }

    /// Specify a pass-through environment variable
    ///
    /// Usually, the environment variables before running a library benchmark are cleared
    /// but specifying pass-through variables makes this environment variable available to
    /// the benchmark as it actually appeared in the root environment.
    ///
    /// Pass-through environment variables are ignored if they don't exist in the root
    /// environment.
    ///
    /// # Examples
    ///
    /// Here, we chose to pass through the original value of the `HOME` variable:
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// use gungraun::{LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().pass_through_env("HOME"),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn pass_through_env<K>(&mut self, key: K) -> &mut Self
    where
        K: Into<OsString>,
    {
        self.0.envs.push((key.into(), None));
        self
    }

    /// Specify multiple pass-through environment variables
    ///
    /// See also [`LibraryBenchmarkConfig::pass_through_env`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{library_benchmark, library_benchmark_group};
    /// # #[library_benchmark]
    /// # fn some_func() {}
    /// # library_benchmark_group!(name = some_group, benchmarks = some_func);
    /// use gungraun::{LibraryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().pass_through_envs(["HOME", "USER"]),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn pass_through_envs<K, T>(&mut self, envs: T) -> &mut Self
    where
        K: Into<OsString>,
        T: IntoIterator<Item = K>,
    {
        self.0
            .envs
            .extend(envs.into_iter().map(|k| (k.into(), None)));
        self
    }

    /// Configures library benchmarks to run in a [`Sandbox`] (Default: false).
    ///
    /// If specified and enabled, the selected benchmark is run in a temporary directory. This also
    /// includes benchmark-level `setup` and `teardown` functions.
    ///
    /// See the [`Sandbox`] documentation for more details.
    ///
    /// # Examples
    ///
    /// To enable the sandbox for all library benchmarks:
    ///
    /// ```rust
    /// use gungraun::Sandbox;
    /// use gungraun::prelude::*;
    /// # #[library_benchmark] fn bench() {}
    /// # library_benchmark_group!(name = my_group, benchmarks = bench);
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    ///     library_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// [`Sandbox`]: crate::Sandbox
    pub fn sandbox<T>(&mut self, sandbox: T) -> &mut Self
    where
        T: Into<__internal::InternalSandbox>,
    {
        self.0.sandbox = Some(sandbox.into());
        self
    }

    /// Adds a configuration for a Valgrind tool.
    ///
    /// Valid configurations are [`crate::Callgrind`], [`crate::Cachegrind`], [`crate::Dhat`],
    /// [`crate::Memcheck`], [`crate::Helgrind`], [`crate::Drd`], [`crate::Massif`] and
    /// [`crate::Bbv`].
    ///
    /// # Example
    ///
    /// Run DHAT in addition to callgrind.
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
    pub fn tool<T>(&mut self, tool: T) -> &mut Self
    where
        T: Into<__internal::InternalToolSpec>,
    {
        self.0.tool_specs.update(tool.into());
        self
    }

    /// Override previously defined configurations of Valgrind tools
    ///
    /// Usually, if specifying tool configurations with [`LibraryBenchmarkConfig::tool`] these tools
    /// are appended to the configuration of a [`LibraryBenchmarkConfig`] of higher levels.
    /// Specifying a tool with this method overrides previously defined configurations.
    ///
    /// # Examples
    ///
    /// The following will run `DHAT` and `Massif` (and the default Callgrind) for all benchmarks in
    /// `main!` except for `some_func` which will just run `Memcheck` (and Callgrind).
    ///
    /// ```rust
    /// use gungraun::{
    ///     Dhat, LibraryBenchmarkConfig, Massif, Memcheck, library_benchmark, library_benchmark_group,
    ///     main,
    /// };
    ///
    /// #[library_benchmark(config = LibraryBenchmarkConfig::default()
    ///     .tool_override(Memcheck::default())
    /// )]
    /// fn some_func() {}
    ///
    /// library_benchmark_group!(name = some_group, benchmarks = some_func);
    ///
    /// # fn main() {
    /// main!(
    ///     config = LibraryBenchmarkConfig::default()
    ///         .tool(Dhat::default())
    ///         .tool(Massif::default()),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn tool_override<T>(&mut self, tool: T) -> &mut Self
    where
        T: Into<__internal::InternalToolSpec>,
    {
        self.0
            .tool_specs_override
            .get_or_insert_with(__internal::InternalToolSpecs::default)
            .update(tool.into());
        self
    }

    /// Configures the [`crate::OutputFormat`] of the terminal output of Gungraun.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{main, LibraryBenchmarkConfig, OutputFormat};
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
    ///         .output_format(OutputFormat::default()
    ///             .truncate_description(Some(200))
    ///         ),
    ///     library_benchmark_groups = some_group
    /// );
    /// # }
    pub fn output_format<T>(&mut self, output_format: T) -> &mut Self
    where
        T: Into<__internal::InternalOutputFormat>,
    {
        self.0.output_format = Some(output_format.into());
        self
    }
}
