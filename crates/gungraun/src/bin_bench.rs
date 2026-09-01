use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use derive_more::AsRef;
use gungraun_macros::IntoInner;

use crate::{__internal, DelayKind, ExitWith, Stdin, Stdio, Tool};

/// [low level api](`crate::binary_benchmark_group`) only: This struct mirrors the `#[bench]` and
/// `#[benches]` attribute of a [`crate::binary_benchmark`]
#[derive(Debug, Clone)]
pub struct Bench {
    /// All [`Command`]s
    pub commands: Vec<__internal::InternalCommandKind>,
    /// An optional [`BinaryBenchmarkConfig`]
    ///
    /// This field stores the internal representation of the [`BinaryBenchmarkConfig`]. Use
    /// `BinaryBenchmarkConfig::into` to generate the internal configuration from a
    /// [`BinaryBenchmarkConfig`]
    pub config: Option<__internal::InternalBinaryBenchmarkConfig>,
    /// The [`BenchmarkId`] used to uniquely identify this benchmark within a [`BinaryBenchmark`]
    pub id: BenchmarkId,
    /// The `setup` function to be executed before the [`Command`] is executed
    pub setup: __internal::InternalBinAssistantKind,
    /// The `teardown` function to be executed after the [`Command`] is executed
    pub teardown: __internal::InternalBinAssistantKind,
}

/// [low level api](`crate::binary_benchmark_group`) only: Create a new benchmark id
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BenchmarkId(String);

/// [low level api](`crate::binary_benchmark_group`) only: Mirror the [`crate::binary_benchmark`]
/// attribute
///
/// A `BinaryBenchmark` can be created in two ways. Either with [`BinaryBenchmark::new`]. Or via the
/// [`crate::binary_benchmark_attribute`] macro used with a function annotated with the
/// [`crate::binary_benchmark`] attribute. So, you can start with the high-level api using the
/// attribute and then go on in the low-level api.
///
/// # Examples
///
/// For examples using [`BinaryBenchmark::new`], see there. Here's an example using the
/// [`crate::binary_benchmark_attribute`]
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use gungraun::{Bench, binary_benchmark, binary_benchmark_attribute, binary_benchmark_group};
///
/// #[binary_benchmark]
/// #[bench::foo("foo")]
/// fn bench_binary(arg: &str) -> gungraun::Command {
///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
///         .arg(arg)
///         .build()
/// }
///
/// binary_benchmark_group!(
///     name = my_group,
///     benchmarks = |group: &mut BinaryBenchmarkGroup| {
///         let mut binary_benchmark = binary_benchmark_attribute!(bench_binary);
///
///         // Continue and add another `Bench` to the `BinaryBenchmark`
///         binary_benchmark.bench(
///             Bench::new("bar")
///                 .command(gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo")).arg("bar")),
///         );
///
///         // Finally, add the `BinaryBenchmark` to the group
///         group.binary_benchmark(binary_benchmark);
///     }
/// );
/// # fn main() {}
/// ```
#[derive(Debug, Clone)]
pub struct BinaryBenchmark {
    /// All [`Bench`]es which were added to this [`BinaryBenchmark`]
    pub benches: Vec<Bench>,
    /// An optional [`BinaryBenchmarkConfig`] which is applied to all [`Command`]s within this
    /// [`BinaryBenchmark`]
    pub config: Option<__internal::InternalBinaryBenchmarkConfig>,
    /// An id which has to be unique within the same [`BinaryBenchmarkGroup`]
    ///
    /// In the high-level api this is the name of the function which is annotated by
    /// [`crate::binary_benchmark`]
    pub id: BenchmarkId,
    /// The default `setup` function for all [`Bench`]es within this [`BinaryBenchmark`]. It can be
    /// overwritten in a [`Bench`]
    pub setup: Option<fn()>,
    /// The default `teardown` function for all [`Bench`]es within this [`BinaryBenchmark`]. It can
    /// be overwritten in a [`Bench`]
    pub teardown: Option<fn()>,
}

/// The configuration of a binary benchmark
///
/// The [`BinaryBenchmarkConfig`] can be specified at multiple levels and configures the benchmarks
/// at this level. For example a [`BinaryBenchmarkConfig`] at (`main`)[`crate::main`] level
/// configures all benchmarks. A configuration at [`group`](crate::binary_benchmark_group) level
/// configures all benchmarks in this group inheriting the configuration of the `main` level and if
/// not specified otherwise overwrites the values of the `main` configuration if the option is
/// specified in both [`BinaryBenchmarkConfig`]s. The deeper levels are the
/// (`#[binary_benchmark] attribute`)[`crate::binary_benchmark`], then `#[bench]` and the
/// `#[benches]` attribute.
///
/// # Examples
///
/// ```rust
/// # use gungraun::binary_benchmark_group;
/// # binary_benchmark_group!(name = some_group,
/// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
/// use gungraun::{BinaryBenchmarkConfig, Callgrind, main};
///
/// main!(
///     config = BinaryBenchmarkConfig::default()
///         .tool(Callgrind::with_args(["toggle-collect=something"])),
///     binary_benchmark_groups = some_group
/// );
/// ```
#[derive(Debug, Default, Clone, IntoInner, AsRef)]
pub struct BinaryBenchmarkConfig(__internal::InternalBinaryBenchmarkConfig);

/// [low level api](`crate::binary_benchmark_group`) only: The top level struct to add binary
/// benchmarks to
///
/// This struct doesn't need to be instantiated by yourself. It is passed as mutable reference to
/// the expression in `benchmarks`.
///
/// ```rust
/// use gungraun::binary_benchmark_group;
///
/// binary_benchmark_group!(
///     name = my_group,
///     benchmarks = |group: &mut BinaryBenchmarkGroup| {
///         // Access the BinaryBenchmarkGroup with the identifier `group` to add benchmarks to the
///         // group.
///         //
///         // group.binary_benchmark(/* BinaryBenchmark::new(...) */);
///     }
/// );
/// ```
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BinaryBenchmarkGroup {
    /// All [`BinaryBenchmark`]s
    pub binary_benchmarks: Vec<BinaryBenchmark>,
}

/// Provide the [`Command`] to be benchmarked
///
/// `Command` is a builder for the binary which is going to be benchmarked providing fine-grained
/// control over how the `Command` for the Valgrind benchmark should be executed.
///
/// The default configuration is created with [`Command::new`] providing a path to an executable.
/// Adding a crate's binary is usually done with `env!("CARGO_BIN_EXE_<name>")` where `<name>` is
/// the name of the binary. The builder methods allow the configuration to be changed prior to
/// [`Command::build`]. The [`Command`] can be reused to build multiple processes.
///
/// # Examples
///
/// Suppose your crate's binary is called `my-echo`:
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use gungraun::Command;
/// let command = Command::new(env!("CARGO_BIN_EXE_my-echo"));
/// ```
///
/// However, an gungraun benchmark is not limited to a crate's binaries, it can be any
/// executable in the `$PATH`, or an absolute path to a binary installed on your system. The
/// following will create a `Command` for the system's `echo` from the `$PATH`:
///
/// ```rust
/// use gungraun::Command;
/// let command = Command::new("echo");
/// ```
#[derive(Debug, Default, Clone, PartialEq, IntoInner, AsRef)]
pub struct Command(__internal::InternalCommand);

/// Provide the [`crate::Delay`] to specify the event for [`crate::Command`] execution start.
///
/// The default configuration is created with [`Delay::new`] providing a [`crate::DelayKind`] to
/// specify the event type and parameters for the `Delay`.
///
/// Additionally, the `Delay` can be created using `from*()` methods.
/// - [`Delay::from(duration)`](Delay::from)
/// - [`Delay::from_tcp_socket(addr)`](Delay::from_tcp_socket)
/// - [`Delay::from_udp_request(addr, request)`](Delay::from_udp_request)
/// - [`Delay::from_path(path)`](Delay::from_path)
///
/// # Examples
///
/// Suppose your command needs to start 60 seconds after the benchmark started:
///
/// ```rust
/// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
/// use std::time::Duration;
///
/// use gungraun::{Command, Delay, DelayKind};
///
/// let command = Command::new(env!("CARGO_BIN_EXE_my-echo")).delay(Delay::new(
///     DelayKind::DurationElapse(Duration::from_secs(60)),
/// ));
///
/// let command_from =
///     Command::new(env!("CARGO_BIN_EXE_my-echo")).delay(Delay::from(Duration::from_secs(60)));
///
/// let command_duration =
///     Command::new(env!("CARGO_BIN_EXE_my-echo")).delay(Duration::from_secs(60));
/// ```
///
/// However, an gungraun [`Delay`] is not limited to a duration, it can be any
/// path creation event, a successful TCP connect or as well a received UDP response.
///
/// ```rust
/// use gungraun::{Command, Delay, DelayKind};
///
/// let command = Command::new("echo").delay(Delay::new(DelayKind::PathExists(
///     "/your/path/to/wait/for".into(),
/// )));
///
/// let command_from = Command::new("echo").delay(Delay::from_path("/your/path/to/wait/for"));
/// ```
///
/// ```rust
/// use std::net::SocketAddr;
/// use std::time::Duration;
///
/// use gungraun::{Command, Delay, DelayKind};
///
/// let command = Command::new("echo").delay(
///     Delay::new(DelayKind::TcpConnect(
///         "127.0.0.1:31000".parse::<SocketAddr>().unwrap(),
///     ))
///     .timeout(Duration::from_secs(3))
///     .poll(Duration::from_millis(50)),
/// );
///
/// let command_from = Command::new("echo").delay(
///     Delay::from_tcp_socket("127.0.0.1:31000".parse::<SocketAddr>().unwrap())
///         .timeout(Duration::from_secs(3))
///         .poll(Duration::from_millis(50)),
/// );
/// ```
///
/// ```rust
/// use std::net::SocketAddr;
/// use std::time::Duration;
///
/// use gungraun::{Command, Delay, DelayKind};
///
/// let command = Command::new("echo").delay(
///     Delay::new(DelayKind::UdpResponse(
///         "127.0.0.1:34000".parse::<SocketAddr>().unwrap(),
///         vec![1],
///     ))
///     .timeout(Duration::from_secs(3))
///     .poll(Duration::from_millis(50)),
/// );
///
/// let command_from = Command::new("echo").delay(
///     Delay::from_udp_request("127.0.0.1:34000".parse::<SocketAddr>().unwrap(), vec![1])
///         .timeout(Duration::from_secs(3))
///         .poll(Duration::from_millis(50)),
/// );
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, IntoInner, AsRef)]
pub struct Delay(__internal::InternalDelay);

impl Bench {
    /// Creates a new `Bench` with a unique [`BenchmarkId`].
    ///
    /// If the provided [`BenchmarkId`] is invalid, `gungraun` exits with an error.
    ///
    /// # Scope of uniqueness of the [`BenchmarkId`]
    ///
    /// The id needs to be unique within the same [`BinaryBenchmark`]
    ///
    /// # Examples
    ///
    /// The [`BenchmarkId`] can be created from any &str-like
    ///
    /// ```
    /// use gungraun::Bench;
    ///
    /// let bench = Bench::new("my_unique_id");
    /// ```
    ///
    /// but you can also provide the [`BenchmarkId`]
    ///
    /// ```
    /// use gungraun::{Bench, BenchmarkId};
    ///
    /// let bench = Bench::new(BenchmarkId::new("my_unique_id"));
    /// ```
    pub fn new<T>(id: T) -> Self
    where
        T: Into<BenchmarkId>,
    {
        Self {
            id: id.into(),
            config: None,
            commands: vec![],
            setup: __internal::InternalBinAssistantKind::None,
            teardown: __internal::InternalBinAssistantKind::None,
        }
    }

    /// Adds a [`BinaryBenchmarkConfig`] for this `Bench`.
    ///
    /// A `Bench` without a `BinaryBenchmarkConfig` behaves like having specified the default
    /// [`BinaryBenchmarkConfig`]. This [`BinaryBenchmarkConfig`] overwrites the values of a
    /// [`BinaryBenchmarkConfig`] specified at a higher level. See there for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::{Bench, BinaryBenchmarkConfig};
    ///
    /// let bench = Bench::new("some_id").config(BinaryBenchmarkConfig::default().env("FOO", "BAR"));
    /// ```
    pub fn config<T>(&mut self, config: T) -> &mut Self
    where
        T: Into<__internal::InternalBinaryBenchmarkConfig>,
    {
        self.config = Some(config.into());
        self
    }

    /// Adds a [`Command`] to this `Bench`.
    ///
    /// A `Bench` with multiple `Commands` behaves exactly as the
    /// [`#[benches]`](crate::binary_benchmark) attribute
    ///
    /// # Errors
    ///
    /// It is an error if a `Bench` does not contain any `Commands`, so this method or
    /// [`Bench::commands`] has to be called at least once.
    ///
    /// # Examples
    ///
    /// Suppose the crate's binary is called `my-echo`:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{Bench, Command};
    ///
    /// let bench = Bench::new("some_id")
    ///     .command(Command::new(env!("CARGO_BIN_EXE_my-echo")))
    ///     .clone();
    ///
    /// assert_eq!(bench.commands.len(), 1);
    /// ```
    pub fn command<T>(&mut self, command: T) -> &mut Self
    where
        T: Into<__internal::InternalCommand>,
    {
        self.commands
            .push(__internal::InternalCommandKind::Default(Box::new(
                command.into(),
            )));
        self
    }

    /// Adds multiple [`Command`]s to this `Bench`.
    ///
    /// See also [`Bench::command`].
    ///
    /// # Examples
    ///
    /// Suppose the crate's binary is called `my-echo`
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{Bench, Command};
    ///
    /// let mut command = Command::new(env!("CARGO_BIN_EXE_my-echo"));
    ///
    /// let echo_foo = command.clone().arg("foo").build();
    /// let echo_bar = command.arg("bar").build();
    ///
    /// let mut bench = Bench::new("some_id");
    /// bench.commands([echo_foo, echo_bar]).clone();
    ///
    /// assert_eq!(bench.commands.len(), 2);
    /// ```
    pub fn commands<I, T>(&mut self, commands: T) -> &mut Self
    where
        I: Into<__internal::InternalCommand>,
        T: IntoIterator<Item = I>,
    {
        self.commands.extend(
            commands
                .into_iter()
                .map(|c| __internal::InternalCommandKind::Default(Box::new(c.into()))),
        );
        self
    }

    /// Adds a `setup` function to be executed before the [`Command`] is executed.
    ///
    /// This `setup` function overwrites the `setup` function of [`BinaryBenchmark`]. In the
    /// presence of a [`Sandbox`][crate::Sandbox], this function is executed in the sandbox.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Bench;
    ///
    /// fn my_setup() {
    ///     println!("Place everything in this function you want to be executed prior to the Command");
    /// }
    ///
    /// let mut bench = Bench::new("some_id");
    /// bench.setup(my_setup);
    ///
    /// assert!(bench.setup.is_some())
    /// ```
    ///
    /// Overwrite the setup function from a [`BinaryBenchmark`]
    ///
    /// ```rust
    /// use gungraun::{Bench, BinaryBenchmark};
    /// fn binary_benchmark_setup() {
    ///     println!("setup in BinaryBenchmark")
    /// }
    ///
    /// fn bench_setup() {
    ///     println!("setup in Bench")
    /// }
    ///
    /// BinaryBenchmark::new("bench_binary")
    ///     .setup(binary_benchmark_setup)
    ///     .bench(Bench::new("some_id").setup(bench_setup));
    /// ```
    pub fn setup(&mut self, setup: fn()) -> &mut Self {
        self.setup = __internal::InternalBinAssistantKind::Default(setup);
        self
    }

    /// Adds a `teardown` function to be executed after the [`Command`] is executed.
    ///
    /// This `teardown` function overwrites the `teardown` function of [`BinaryBenchmark`]. In the
    /// presence of a [`Sandbox`][crate::Sandbox], this function is executed in the sandbox.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Bench;
    ///
    /// fn my_teardown() {
    ///     println!(
    ///         "Place everything in this function you want to be executed after the execution of the \
    ///          Command"
    ///     );
    /// }
    ///
    /// let mut bench = Bench::new("some_id");
    /// bench.teardown(my_teardown);
    ///
    /// assert!(bench.teardown.is_some())
    /// ```
    ///
    /// Overwrite the teardown function from a [`BinaryBenchmark`]
    ///
    /// ```rust
    /// use gungraun::{Bench, BinaryBenchmark};
    /// fn binary_benchmark_teardown() {
    ///     println!("teardown in BinaryBenchmark")
    /// }
    ///
    /// fn bench_teardown() {
    ///     println!("teardown in Bench")
    /// }
    ///
    /// BinaryBenchmark::new("bench_binary")
    ///     .teardown(binary_benchmark_teardown)
    ///     .bench(Bench::new("some_id").teardown(bench_teardown));
    /// ```
    pub fn teardown(&mut self, teardown: fn()) -> &mut Self {
        self.teardown = __internal::InternalBinAssistantKind::Default(teardown);
        self
    }

    /// Add each line of a file as [`Command`] to this [`Bench`] using a `generator` function.
    ///
    /// This method mirrors the `file` parameter of the `#[benches]` attribute as far as possible.
    /// In the low-level api you can achieve the same or more quickly yourself and this method
    /// exists for the sake of completeness (and convenience).
    ///
    /// The file has to exist at the time you're using this method and the file has to be encoded in
    /// UTF-8. The `generator` function tells us how to convert each line of the file into a
    /// [`Command`].
    ///
    /// # Notable differences to high-level api
    ///
    /// If the file path in the high-level api is relative we interpret the path relative to the
    /// workspace root (and make it absolute). In this method we use the path AS IS.
    ///
    /// # Errors
    ///
    /// If the file is empty, cannot be opened for reading or a line in the file cannot be converted
    /// to a String. Also, the error from the `generator` is propagated. The errors containing the
    /// line number use a 0-indexed line number.
    ///
    /// # Examples
    ///
    /// Suppose your cargo's binary is named `my-echo` and you want to convert a file with inputs
    /// `benches/inputs` into commands and each line is the only argument for your `my-echo` binary:
    ///
    /// // FIXME: Use prelude where possible
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{Bench, BinaryBenchmark, binary_benchmark_group};
    ///
    /// binary_benchmark_group!(
    ///     name = my_group,
    ///     benchmarks = |group: &mut BinaryBenchmarkGroup| {
    ///         group.binary_benchmark(
    ///             BinaryBenchmark::new("some_id").bench(
    ///                 Bench::new("bench_id")
    ///                     .file("benches/inputs", |line| {
    ///                         Ok(gungraun::Command::new(env!("CARGO_BIN_EXE_my-echo"))
    ///                             .arg(line)
    ///                             .build())
    ///                     })
    ///                     .unwrap(),
    ///             ),
    ///         )
    ///     }
    /// );
    /// # fn main() {}
    /// ```
    pub fn file<T>(
        &mut self,
        path: T,
        generator: fn(String) -> Result<Command, String>,
    ) -> Result<&mut Self, String>
    where
        T: AsRef<Path>,
    {
        let path = path.as_ref();
        let file = File::open(path).map_err(|error| {
            format!(
                "{}: Error opening file '{}': {error}",
                self.id,
                path.display(),
            )
        })?;

        let reader = BufReader::new(file);
        let mut has_lines = false;
        for (index, line) in reader.lines().enumerate() {
            has_lines = true;

            let line = line.map_err(|error| {
                format!(
                    "{}: Error reading line {index} in file '{}': {error}",
                    self.id,
                    path.display()
                )
            })?;

            let command = generator(line).map_err(|error| {
                format!(
                    "{}: Error generating command from line {index} in file '{}': {error}",
                    self.id,
                    path.display()
                )
            })?;
            self.commands
                .push(__internal::InternalCommandKind::Default(Box::new(
                    command.into(),
                )));
        }

        if !has_lines {
            return Err(format!("{}: Empty file '{}'", self.id, path.display()));
        }
        Ok(self)
    }
}

impl From<&mut Self> for Bench {
    fn from(value: &mut Self) -> Self {
        value.clone()
    }
}

impl From<&Self> for Bench {
    fn from(value: &Self) -> Self {
        value.clone()
    }
}

impl PartialEq for Bench {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.commands == other.commands
            && self.config == other.config
            && self.setup == other.setup
            && self.teardown == other.teardown
    }
}

impl BenchmarkId {
    /// Convenience method to create a `BenchmarkId` with a parameter in the [low level
    /// api](crate::binary_benchmark_group)
    ///
    /// The `parameter` is simply appended to the `id` with an underscore, so
    /// `BenchmarkId::with_parameter("some", 1)` is equivalent to `BenchmarkId::new("some_1")`
    ///
    /// # Examples
    ///
    /// ```
    /// use gungraun::BenchmarkId;
    ///
    /// let new_id = BenchmarkId::new("prefix_1");
    /// let with_parameter = BenchmarkId::with_parameter("prefix", 1);
    /// assert_eq!(new_id, with_parameter);
    /// ```
    ///
    /// ```rust
    /// # use gungraun::main;
    /// use std::ffi::OsStr;
    ///
    /// use gungraun::{Bench, BenchmarkId, BinaryBenchmark, Command, binary_benchmark_group};
    ///
    /// binary_benchmark_group!(
    ///     name = low_level_group,
    ///     benchmarks = |group: &mut BinaryBenchmarkGroup| {
    ///         let mut binary_benchmark = BinaryBenchmark::new("some_id");
    ///         for arg in 0..10 {
    ///             let id = BenchmarkId::with_parameter("prefix", arg);
    ///             binary_benchmark.bench(
    ///                 Bench::new(id).command(Command::new("echo").arg(arg.to_string()).build()),
    ///             );
    ///         }
    ///         group.binary_benchmark(binary_benchmark);
    ///     }
    /// );
    /// # fn main() {
    /// # main!(binary_benchmark_groups = low_level_group);
    /// # }
    /// ```
    pub fn with_parameter<T, P>(id: T, parameter: P) -> Self
    where
        T: AsRef<str>,
        P: Display,
    {
        Self(format!("{}_{parameter}", id.as_ref()))
    }

    /// Creates a new `BenchmarkId`.
    ///
    /// `BenchmarkId`s can be created from any string-like input. See [`BenchmarkId::validate`] for
    /// ids which are considered valid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::BenchmarkId;
    ///
    /// let id = BenchmarkId::new("my_id");
    ///
    /// assert!(id.validate().is_ok());
    /// ```
    pub fn new<T>(id: T) -> Self
    where
        T: Into<String>,
    {
        Self(id.into())
    }

    /// Returns ok if this [`BenchmarkId`] is valid
    ///
    /// An id should be short, descriptive besides being unique. The requirements for the uniqueness
    /// differ for the structs where a `BenchmarkId` is used and is further described there.
    ///
    /// We use a minimal subset of rust's identifiers. A valid `BenchmarkId` starts with an ascii
    /// alphabetic letter `[a-zA-Z]` or underscore `[_]`. All following characters can be an ascii
    /// alphabetic letter, underscore or a digit `[0-9]`. At least one valid character must be
    /// present.
    ///
    /// The `BenchmarkId` is used by `gungraun` as file and directory name for the output files
    /// of the benchmarks. Therefore, the limit for an id is chosen to be 120 bytes. This is a
    /// calculation with some headroom. There are file systems which do not even allow 255 bytes. If
    /// you're working on such a peculiar file system, you have to restrict your ids to even fewer
    /// bytes which is `floor(MAX_LENGTH/2) - 1`. Leaving the maximum bytes aside, the best IDs are
    /// simple, short and descriptive.
    ///
    /// Usually, it is not necessary to call this function, since we already check the validity of
    /// benchmark ids prior to the execution of the benchmark runner. But if your ids come from an
    /// untrusted source you can use this method for more immediate feedback.
    ///
    /// # Errors
    ///
    /// This function will return an error describing the source of the error if the id is invalid
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::BenchmarkId;
    ///
    /// assert!(BenchmarkId::new("").validate().is_err());
    /// assert!(BenchmarkId::new("0a").validate().is_err());
    /// assert!(BenchmarkId::new("id with spaces").validate().is_err());
    /// assert!(BenchmarkId::new("path/to").validate().is_err());
    /// assert!(BenchmarkId::new("no::module::too").validate().is_err());
    ///
    /// assert!(BenchmarkId::new("_").validate().is_ok());
    /// assert!(BenchmarkId::new("abc").validate().is_ok());
    /// assert!(BenchmarkId::new("a9").validate().is_ok());
    /// assert!(BenchmarkId::new("z_").validate().is_ok());
    /// assert!(BenchmarkId::new("some_id").validate().is_ok());
    /// ```
    #[expect(clippy::missing_panics_doc)]
    pub fn validate(&self) -> Result<(), String> {
        const MAX_LENGTH_ID: usize = 255;
        if self.0.is_empty() {
            return Err("Invalid id: Cannot be empty".to_owned());
        }

        let mut bytes = self.0.bytes();
        // This unwrap is safe, since we just checked that the string is not empty
        let first = bytes.next().unwrap();

        if first.is_ascii_alphabetic() || first == b'_' {
            for (index, byte) in (1..).zip(bytes) {
                if index > MAX_LENGTH_ID {
                    return Err(format!(
                        "Invalid id '{}': Maximum length of {MAX_LENGTH_ID} bytes reached",
                        self.0,
                    ));
                }
                if byte.is_ascii() {
                    if !(byte.is_ascii_alphanumeric() || byte == b'_') {
                        return Err(format!(
                            "Invalid id '{}' at position {index}: Invalid character '{}'",
                            self.0,
                            char::from(byte)
                        ));
                    }
                } else {
                    return Err(format!(
                        "Invalid id '{}' at position {index}: Encountered non-ascii character",
                        self.0
                    ));
                }
            }
        } else if first.is_ascii() {
            return Err(format!(
                "Invalid id '{}': As first character is '{}' not allowed",
                self.0,
                char::from(first)
            ));
        } else {
            return Err(format!(
                "Invalid id '{}': Encountered non-ascii character as first character",
                self.0
            ));
        }
        Ok(())
    }
}

impl Display for BenchmarkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<T> From<T> for BenchmarkId
where
    T: AsRef<str>,
{
    fn from(value: T) -> Self {
        Self(value.as_ref().to_owned())
    }
}

impl BinaryBenchmark {
    /// Creates a new `BinaryBenchmark`.
    ///
    /// A `BinaryBenchmark` is the equivalent of the
    /// [`#[binary_benchmark]`](`crate::binary_benchmark`) attribute in the low-level api and needs
    /// a [`BenchmarkId`]. In the high-level api the id is derived from the function name.
    ///
    /// The [`BenchmarkId`] is used together with the id of each [`Bench`] to create a directory
    /// name. This usually limits the combined length of the ids to `255 - 1` but can be less
    /// depending on the file system. See [`BenchmarkId`] for more details
    ///
    /// # Scope of uniqueness of the [`BenchmarkId`]
    ///
    /// The id needs to be unique within the same [`crate::binary_benchmark_group`]. It is
    /// recommended to use an id which is unique within the whole benchmark file, although doing
    /// otherwise does currently not incur any negative consequence.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{BenchmarkId, BinaryBenchmark};
    ///
    /// let binary_benchmark = BinaryBenchmark::new("some_id");
    /// assert_eq!(binary_benchmark.id, BenchmarkId::new("some_id"));
    /// ```
    pub fn new<T>(id: T) -> Self
    where
        T: Into<BenchmarkId>,
    {
        Self {
            id: id.into(),
            config: None,
            benches: vec![],
            setup: None,
            teardown: None,
        }
    }

    /// Adds a [`BinaryBenchmarkConfig`] to this `BinaryBenchmark`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{BinaryBenchmark, BinaryBenchmarkConfig};
    ///
    /// let binary_benchmark = BinaryBenchmark::new("some_id")
    ///     .config(BinaryBenchmarkConfig::default().env("FOO", "BAR"))
    ///     .clone();
    ///
    /// assert_eq!(
    ///     binary_benchmark.config,
    ///     Some(BinaryBenchmarkConfig::default().env("FOO", "BAR").into())
    /// );
    /// ```
    pub fn config<T>(&mut self, config: T) -> &mut Self
    where
        T: Into<__internal::InternalBinaryBenchmarkConfig>,
    {
        self.config = Some(config.into());
        self
    }

    /// Adds a [`Bench`] to this `BinaryBenchmark`.
    ///
    /// Adding a [`Bench`] which doesn't contain a [`Command`] is an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{Bench, BinaryBenchmark, Command};
    ///
    /// // Each `Bench` needs at least one `Command`!
    /// let mut bench = Bench::new("some_id");
    /// bench.command(Command::new(env!("CARGO_BIN_EXE_my-echo")));
    ///
    /// let binary_benchmark = BinaryBenchmark::new("bench_binary")
    ///     .bench(bench.clone())
    ///     .clone();
    ///
    /// assert_eq!(binary_benchmark.benches[0], bench);
    /// ```
    pub fn bench<T>(&mut self, bench: T) -> &mut Self
    where
        T: Into<Bench>,
    {
        self.benches.push(bench.into());
        self
    }

    /// Adds a `setup` function to this `BinaryBenchmark`.
    ///
    /// This `setup` function is used in all [`Bench`]es of this `BinaryBenchmark` if not overridden
    /// by the `Bench`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Bench;
    /// fn my_setup() {
    ///     println!(
    ///         "Place everything in this function you want to be executed before the execution of \
    ///          the Command"
    ///     );
    /// }
    ///
    /// let bench = Bench::new("some_id").setup(my_setup).clone();
    ///
    /// assert!(bench.setup.is_some())
    /// ```
    ///
    /// Overwrite the setup function from this `BinaryBenchmark` in a [`Bench`]
    ///
    /// ```rust
    /// use gungraun::{Bench, BinaryBenchmark};
    /// fn binary_benchmark_setup() {
    ///     println!("setup in BinaryBenchmark")
    /// }
    ///
    /// fn bench_setup() {
    ///     println!("setup in Bench")
    /// }
    ///
    /// BinaryBenchmark::new("bench_binary")
    ///     .setup(binary_benchmark_setup)
    ///     .bench(Bench::new("some_id").setup(bench_setup));
    /// ```
    pub fn setup(&mut self, setup: fn()) -> &mut Self {
        self.setup = Some(setup);
        self
    }

    /// Adds a `teardown` function to this `BinaryBenchmark`.
    ///
    /// This `teardown` function is used in all [`Bench`]es of this `BinaryBenchmark` if not
    /// overridden by the `Bench`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::Bench;
    /// fn my_teardown() {
    ///     println!(
    ///         "Place everything in this function you want to be executed after the execution of the \
    ///          Command"
    ///     );
    /// }
    ///
    /// let bench = Bench::new("some_id").teardown(my_teardown).clone();
    ///
    /// assert!(bench.teardown.is_some())
    /// ```
    ///
    /// Overwrite the teardown function from this `BinaryBenchmark` in a [`Bench`]
    ///
    /// ```rust
    /// use gungraun::{Bench, BinaryBenchmark};
    /// fn binary_benchmark_teardown() {
    ///     println!("teardown in BinaryBenchmark")
    /// }
    ///
    /// fn bench_teardown() {
    ///     println!("teardown in Bench")
    /// }
    ///
    /// BinaryBenchmark::new("bench_binary")
    ///     .teardown(binary_benchmark_teardown)
    ///     .bench(Bench::new("some_id").teardown(bench_teardown));
    /// ```
    pub fn teardown(&mut self, teardown: fn()) -> &mut Self {
        self.teardown = Some(teardown);
        self
    }
}

impl From<&mut Self> for BinaryBenchmark {
    fn from(value: &mut Self) -> Self {
        value.clone()
    }
}

impl From<&Self> for BinaryBenchmark {
    fn from(value: &Self) -> Self {
        value.clone()
    }
}

impl PartialEq for BinaryBenchmark {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.config == other.config
            && self.benches == other.benches
            && (self.setup.is_some() && other.setup.is_some()
                || self.setup.is_none() && other.setup.is_none())
            && (self.teardown.is_some() && other.teardown.is_some()
                || self.teardown.is_none() && other.teardown.is_none())
    }
}

impl BinaryBenchmarkConfig {
    /// Change the default tool to something different than Callgrind
    ///
    /// See also [`crate::LibraryBenchmarkConfig::default_tool`].
    ///
    /// # Example for using Cachegrind on the fly
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{BinaryBenchmarkConfig, Tool, binary_benchmark, binary_benchmark_group, main};
    ///
    /// #[binary_benchmark(
    ///     config = BinaryBenchmarkConfig::default()
    ///         .default_tool(Tool::Cachegrind)
    /// )]
    /// fn bench_me() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_echo"))
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_me);
    ///
    /// # fn main() {
    /// main!(binary_benchmark_groups = my_group);
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
    /// [`crate::Callgrind::args`].
    ///
    /// # Examples
    ///
    /// Specify `--trace-children=no` for all configured tools (including callgrind):
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {}
    /// # );
    /// use gungraun::{BinaryBenchmarkConfig, Dhat, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default()
    ///         .valgrind_args(["--trace-children=no"])
    ///         .tool(Dhat::default()),
    ///     binary_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// Overwrite the Valgrind argument `--num-callers=25` for `DHAT` with `--num-callers=30`:
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, Dhat, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default()
    ///         .valgrind_args(["--num-callers=25"])
    ///         .tool(Dhat::with_args(["--num-callers=30"])),
    ///     binary_benchmark_groups = my_group
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

    /// Adds an environment variable to the [`Command`].
    ///
    /// These environment variables are available independently of the setting of
    /// [`BinaryBenchmarkConfig::env_clear`].
    ///
    /// # Examples
    ///
    /// An example for a custom environment variable "FOO=BAR":
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().env("FOO", "BAR"),
    ///     binary_benchmark_groups = my_group
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

    /// Adds multiple environment variables to the [`Command`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().envs([("FOO", "BAR"), ("BAR", "BAZ")]),
    ///     binary_benchmark_groups = my_group
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
    /// Usually, the environment variables before running a binary benchmark are cleared
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
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().pass_through_env("HOME"),
    ///     binary_benchmark_groups = my_group
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
    /// See also [`crate::BinaryBenchmarkConfig::pass_through_env`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::binary_benchmark_group;
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().pass_through_envs(["HOME", "USER"]),
    ///     binary_benchmark_groups = my_group
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

    /// If false, don't clear the environment variables before running the benchmark (Default: true)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().env_clear(false),
    ///     binary_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    pub fn env_clear(&mut self, value: bool) -> &mut Self {
        self.0.env_clear = Some(value);
        self
    }

    /// Sets the directory of the benchmarked binary (Default: Unchanged).
    ///
    /// Unchanged means, in the case of running with the sandbox enabled, the root of the sandbox.
    /// In the case of running without sandboxing enabled, this will be the directory which `cargo
    /// bench` sets. If running the benchmark within the sandbox, and the path is relative then this
    /// new directory must be contained in the sandbox.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().current_dir("/tmp"),
    ///     binary_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// and the following will change the current directory to `fixtures` assuming it is
    /// contained in the root of the sandbox
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, Sandbox, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default()
    ///         .sandbox(Sandbox::new(true))
    ///         .current_dir("fixtures"),
    ///     binary_benchmark_groups = my_group
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

    /// Sets the expected exit status [`ExitWith`] of a benchmarked binary.
    ///
    /// By default, the benchmarked binary is expected to succeed which is the equivalent of
    /// [`ExitWith::Success`]. But, if a benchmark is expected to fail, setting this option is
    /// required.
    ///
    /// # Examples
    ///
    /// If the benchmark is expected to fail with a specific exit code, for example `100`:
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, ExitWith, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().exit_with(ExitWith::Code(100)),
    ///     binary_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    ///
    /// If a benchmark is expected to fail, but the exit code doesn't matter:
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, ExitWith, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().exit_with(ExitWith::Failure),
    ///     binary_benchmark_groups = my_group
    /// );
    /// # }
    /// ```
    pub fn exit_with<T>(&mut self, value: T) -> &mut Self
    where
        T: Into<__internal::InternalExitWith>,
    {
        self.0.exit_with = Some(value.into());
        self
    }

    /// Adds a configuration for a Valgrind tool.
    ///
    /// Valid configurations are [`crate::Callgrind`], [`crate::Cachegrind`], [`crate::Dhat`],
    /// [`crate::Memcheck`], [`crate::Helgrind`], [`crate::Drd`], [`crate::Massif`] and
    /// [`crate::Bbv`].
    ///
    /// # Examples
    ///
    /// Run DHAT in addition to callgrind.
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, Dhat, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().tool(Dhat::default()),
    ///     binary_benchmark_groups = my_group
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
    /// See also [`crate::LibraryBenchmarkConfig::tool_override`] for more details.
    ///
    /// # Example
    ///
    /// The following will run `DHAT` and `Massif` (and the default Callgrind) for all benchmarks
    /// in `main!` except for `foo` which will just run `Memcheck` (and Callgrind).
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{
    ///     BinaryBenchmarkConfig, Dhat, Massif, Memcheck, binary_benchmark, binary_benchmark_group,
    ///     main,
    /// };
    ///
    /// #[binary_benchmark]
    /// #[bench::some(
    ///     config = BinaryBenchmarkConfig::default()
    ///         .tool_override(Memcheck::default())
    /// )]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default()
    ///         .tool(Dhat::default())
    ///         .tool(Massif::default()),
    ///     binary_benchmark_groups = my_group
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

    /// Configures benchmarks to run in a [`Sandbox`] (Default: false).
    ///
    /// If specified, we create a temporary directory in which the `setup` and `teardown` functions
    /// of the `#[binary_benchmark]` (`#[bench]`, `#[benches]`) and the [`Command`] itself are run.
    ///
    /// See the [`Sandbox`] documentation for more details.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::{binary_benchmark_group};
    /// # binary_benchmark_group!(
    /// #    name = my_group,
    /// #    benchmarks = |_group: &mut BinaryBenchmarkGroup| {});
    /// use gungraun::{BinaryBenchmarkConfig, Sandbox, main};
    ///
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    ///     binary_benchmark_groups = my_group
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

    /// Configures the [`crate::OutputFormat`] of the terminal output of Gungraun.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun::{BinaryBenchmarkConfig, OutputFormat, main};
    /// # use gungraun::{binary_benchmark, binary_benchmark_group};
    /// # #[binary_benchmark]
    /// # fn some_func() -> gungraun::Command { gungraun::Command::new("some/path") }
    /// # binary_benchmark_group!(
    /// #    name = some_group,
    /// #    benchmarks = some_func
    /// # );
    /// # fn main() {
    /// main!(
    ///     config = BinaryBenchmarkConfig::default()
    ///         .output_format(OutputFormat::default().truncate_description(Some(200))),
    ///     binary_benchmark_groups = some_group
    /// );
    /// # }
    /// ```
    pub fn output_format<T>(&mut self, output_format: T) -> &mut Self
    where
        T: Into<__internal::InternalOutputFormat>,
    {
        self.0.output_format = Some(output_format.into());
        self
    }

    /// Execute the `setup` in parallel to the [`Command`].
    ///
    /// See also [`Command::setup_parallel`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use std::net::{SocketAddr, TcpListener};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// use gungraun::{
    ///     BinaryBenchmarkConfig, Command, Delay, DelayKind, binary_benchmark, binary_benchmark_group,
    ///     main,
    /// };
    ///
    /// fn setup_tcp_server() {
    ///     thread::sleep(Duration::from_millis(300));
    ///     let _listener =
    ///         TcpListener::bind("127.0.0.1:31000".parse::<SocketAddr>().unwrap()).unwrap();
    ///     thread::sleep(Duration::from_secs(1));
    /// }
    ///
    /// #[binary_benchmark]
    /// #[bench::delay(
    ///     setup = setup_tcp_server(),
    ///     config = BinaryBenchmarkConfig::default()
    ///         .setup_parallel(true)
    /// )]
    /// fn bench_binary() -> gungraun::Command {
    ///     Command::new(env!("CARGO_BIN_EXE_my-echo"))
    ///         .delay(
    ///             Delay::new(DelayKind::TcpConnect(
    ///                 "127.0.0.1:31000".parse::<SocketAddr>().unwrap(),
    ///             ))
    ///             .timeout(Duration::from_millis(500)),
    ///         )
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = delay, benchmarks = bench_binary);
    /// # fn main() {
    /// main!(binary_benchmark_groups = delay);
    /// # }
    /// ```
    pub fn setup_parallel(&mut self, setup_parallel: bool) -> &mut Self {
        self.0.setup_parallel = Some(setup_parallel);
        self
    }
}

impl BinaryBenchmarkGroup {
    /// Adds a [`BinaryBenchmark`] to this group.
    ///
    /// This can be a binary benchmark created with [`BinaryBenchmark::new`] or a
    /// [`crate::binary_benchmark`] attributed function addable with the
    /// [`crate::binary_benchmark_attribute`] macro.
    ///
    /// It is an error to add a [`BinaryBenchmark`] without having added a [`Bench`] to it.
    ///
    /// # Examples
    ///
    /// Adds a [`BinaryBenchmark`] to this group.
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{Bench, BinaryBenchmark, BinaryBenchmarkGroup, binary_benchmark_group};
    ///
    /// fn setup_my_group(group: &mut BinaryBenchmarkGroup) {
    ///     group.binary_benchmark(
    ///         BinaryBenchmark::new("bench_binary").bench(
    ///             Bench::new("foo")
    ///                 .command(gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo")).arg("foo")),
    ///         ),
    ///     );
    /// }
    ///
    /// binary_benchmark_group!(
    ///     name = my_group,
    ///     benchmarks = |group: &mut BinaryBenchmarkGroup| setup_my_group(group)
    /// );
    /// # fn main() {}
    /// ```
    ///
    /// Or, add a `#[binary_benchmark]` annotated function to this group:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{binary_benchmark, binary_benchmark_attribute, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// #[bench::foo("foo")]
    /// fn bench_binary(arg: &str) -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-foo"))
    ///         .arg(arg)
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(
    ///     name = my_group,
    ///     benchmarks = |group: &mut BinaryBenchmarkGroup| {
    ///         group.binary_benchmark(binary_benchmark_attribute!(bench_binary))
    ///     }
    /// );
    /// # fn main() {}
    /// ```
    pub fn binary_benchmark<T>(&mut self, binary_benchmark: T) -> &mut Self
    where
        T: Into<BinaryBenchmark>,
    {
        self.binary_benchmarks.push(binary_benchmark.into());
        self
    }

    /// Adds multiple [`BinaryBenchmark`]s at once.
    pub fn binary_benchmarks<I, T>(&mut self, binary_benchmarks: T) -> &mut Self
    where
        I: Into<BinaryBenchmark>,
        T: IntoIterator<Item = I>,
    {
        self.binary_benchmarks
            .extend(binary_benchmarks.into_iter().map(Into::into));
        self
    }
}

impl Command {
    /// Create a new [`Command`] which is run under Valgrind.
    ///
    /// Use [`env!("CARGO_BIN_EXE_<name>)`][env-cargo-exe]
    /// to provide the path to an executable of your project instead of `target/release/<name>`.
    ///
    /// This `Command` is a builder for the binary which is going to be benchmarked but is not
    /// executed right away. We simply gather all the information to be finally able to execute the
    /// command under Valgrind, later (after we collected all the commands in this benchmark file).
    /// As opposed to [`std::process::Command`], the build is finalized with [`Command::build`].
    ///
    /// # Relative paths
    ///
    /// Relative paths are interpreted relative to the current directory and if not running the
    /// benchmarks in a [`Sandbox`], depends on where `cargo bench` sets the current directory.
    /// Usually, it's best to use [`Path::canonicalize`] to resolve the relative path to a binary in
    /// your project's directory. In case you're running the benchmark in a [`Sandbox`], the path is
    /// interpreted relative to the root directory of the `Sandbox`. Gungraun tries to resolve
    /// simple names like `Command::new("echo")` searching the `$PATH`. To disambiguate between
    /// simple names and relative paths, use `./`. For example `echo` is searched in the `$PATH` and
    /// `./echo` is interpreted relative.
    ///
    /// # Examples
    ///
    /// Assume the project's binary or one of your project's binaries name is `my-echo`:
    ///
    /// ```
    /// # macro_rules! env { ($m:tt) => {{ "/home/my_project/target/release/my-echo" }} }
    /// use gungraun::Command;
    ///
    /// let command = Command::new(env!("CARGO_BIN_EXE_my-echo"));
    /// ```
    ///
    /// or use your system's echo from the `$PATH` with
    ///
    /// ```
    /// use gungraun::Command;
    ///
    /// let command = Command::new("echo").arg("foo").build();
    /// ```
    ///
    /// [env-cargo-exe]:
    /// https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
    /// [`Sandbox`]: crate::Sandbox
    pub fn new<T>(path: T) -> Self
    where
        T: AsRef<OsStr>,
    {
        Self(__internal::InternalCommand {
            path: PathBuf::from(path.as_ref()),
            ..Default::default()
        })
    }

    /// Delay the execution of the [`Command`]
    ///
    /// This option allows you to delay the [`Command`] execution till a certain event has happened.
    /// Supported events are:
    ///  - Timer expired
    ///  - File path exists
    ///  - TCP/UDP connect succeeded
    ///
    /// [`Delay`] can be used in combination with [`Command::setup_parallel`] to wait for an event
    /// that is triggered within the `setup()` function. E.g. the setup starts a server that is
    /// needed by the [`Command`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{Command, Delay, DelayKind, binary_benchmark, binary_benchmark_group, main};
    ///
    /// fn start_server() {
    ///     // action to start the server, creates pid file
    ///     std::fs::File::create("/tmp/my-server.pid").unwrap();
    /// }
    ///
    /// #[binary_benchmark]
    /// #[bench::server(setup = start_server)]
    /// fn bench_binary() -> Command {
    ///     Command::new(env!("CARGO_BIN_EXE_my-echo"))
    ///         .setup_parallel(true)
    ///         .delay(Delay::new(DelayKind::PathExists(
    ///             "/tmp/my-server.pid".into(),
    ///         )))
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn delay<T: Into<Delay>>(&mut self, delay: T) -> &mut Self {
        self.0.delay = Some(delay.into().into());
        self
    }

    /// Execute the `setup()` in parallel to the [`Command`].
    ///
    /// This option changes the execution flow in a way that the [`Command`] is executed parallel
    /// to the `setup` instead of waiting for the `setup` to complete.
    ///
    /// This can be combined with the usage of [`Delay`] to further control the timing when the
    /// [`Command`] is executed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use std::net::{SocketAddr, TcpListener};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// use gungraun::{Command, Delay, DelayKind, binary_benchmark, binary_benchmark_group, main};
    ///
    /// fn setup_tcp_server() {
    ///     thread::sleep(Duration::from_millis(300));
    ///     let _listener =
    ///         TcpListener::bind("127.0.0.1:31000".parse::<SocketAddr>().unwrap()).unwrap();
    ///     thread::sleep(Duration::from_secs(1));
    /// }
    ///
    /// #[binary_benchmark]
    /// #[bench::delay(setup = setup_tcp_server())]
    /// fn bench_binary() -> gungraun::Command {
    ///     Command::new(env!("CARGO_BIN_EXE_my-echo"))
    ///         .setup_parallel(true)
    ///         .delay(
    ///             Delay::new(DelayKind::TcpConnect(
    ///                 "127.0.0.1:31000".parse::<SocketAddr>().unwrap(),
    ///             ))
    ///             .timeout(Duration::from_millis(500)),
    ///         )
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = delay, benchmarks = bench_binary);
    /// # fn main() {
    /// main!(binary_benchmark_groups = delay);
    /// # }
    /// ```
    pub fn setup_parallel(&mut self, setup_parallel: bool) -> &mut Self {
        self.0.config.setup_parallel = Some(setup_parallel);
        self
    }

    /// Adds an argument to pass to the [`Command`]
    ///
    /// This option works exactly the same way as [`std::process::Command::arg`]. To pass multiple
    /// arguments see [`Command::args`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use gungraun::main;
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::{binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-echo"))
    ///         .arg("foo")
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn arg<T>(&mut self, arg: T) -> &mut Self
    where
        T: Into<OsString>,
    {
        self.0.args.push(arg.into());
        self
    }

    /// Adds multiple arguments to pass to the [`Command`]
    ///
    /// This option works exactly the same way as [`std::process::Command::args`].
    ///
    /// # Examples
    ///
    /// The following will execute `my-echo foo`.
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-echo"))
    ///         .arg("foo")
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn args<I, T>(&mut self, args: T) -> &mut Self
    where
        I: Into<OsString>,
        T: IntoIterator<Item = I>,
    {
        self.0.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Configuration for the process's standard input ([`Stdin`])
    ///
    /// This method takes an [`Stdin`], [`Stdio`] and everything that implements `Into<Stdin>`. The
    /// [`Stdin`] enum mirrors most of the possibilities of [`std::process::Stdio`] but also some
    /// additional possibilities most notably [`Stdin::Setup`] (see there for more details).
    ///
    /// By default, the stdin is not inherited from the parent and any attempt by the child process
    /// to read from the stdin stream will result in the stream immediately closing.
    ///
    /// The options you might be interested in the most are [`Stdin::File`], which mirrors the
    /// behaviour of [`std::process::Stdio`] if `Stdio` is a [`std::fs::File`], and
    /// [`Stdin::Setup`], which is special to `gungraun` and lets you pipe the output of
    /// the `setup` function into the Stdin of this [`Command`]. If you need to delay the `Command`
    /// when using [`Stdin::Setup`], you can do so with [`Command::delay`].
    ///
    /// # Implementation details
    ///
    /// As the [`Command`] itself is not executed immediately, the [`std::process::Stdio`] is not
    /// created either. We only use the information from here to create the [`std::process::Stdio`]
    /// later after we collected all commands. Setting the Stdin to `Inherit` is discouraged and
    /// won't have the effect you might expect, since the benchmark runner (the parent) uses the
    /// Stdin for its own purposes and closes it before this [`Command`] is executed.
    ///
    /// # Examples
    ///
    /// Pipe the content of a file (`benches/fixture.txt`) into the stdin of this [`Command`]:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{Stdio, binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .stdin(Stdio::File("benches/fixture.txt".into()))
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    ///
    /// Pipe the Stdout of setup into the Stdin of this [`Command`]:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{Pipe, Stdin, binary_benchmark, binary_benchmark_group};
    ///
    /// fn setup_pipe() {
    ///     // All output to Stdout of this function will be piped into the Stdin of `my-exe`
    ///     println!("This string will be piped into the stdin of my-exe");
    /// }
    ///
    /// #[binary_benchmark(setup = setup_pipe())]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .stdin(Stdin::Setup(Pipe::Stdout))
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn stdin<T>(&mut self, stdin: T) -> &mut Self
    where
        T: Into<Stdin>,
    {
        self.0.stdin = Some(stdin.into());
        self
    }

    /// Configuration for the [`Command`]s standard output (Stdout) handle.
    ///
    /// The output of benchmark commands and functions are usually captured by the benchmark runner.
    /// This can be changed for example with the `--nocapture` option or here. Any option specified
    /// here takes precedence over the changes which `--nocapture` makes to the Stdout of the
    /// command.
    ///
    /// # Examples
    ///
    /// To see the output of this [`Command`] regardless of `--nocapture` in the benchmark output
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{Stdio, binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .stdout(Stdio::Inherit)
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    ///
    /// To pipe the Stdout into a file `/tmp/benchmark.output`:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use std::path::PathBuf;
    ///
    /// use gungraun::{Stdio, binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .stdout(Stdio::File("/tmp/benchmark.output".into()))
    ///         // or
    ///         .stdout(PathBuf::from("/tmp/benchmark.output"))
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn stdout<T>(&mut self, stdio: T) -> &mut Self
    where
        T: Into<Stdio>,
    {
        self.0.stdout = Some(stdio.into());
        self
    }

    /// Configuration for the [`Command`]s standard error output (Stderr) handle.
    ///
    /// This option is similar to [`Command::stdout`] but configures the Stderr. See there for more
    /// details.
    ///
    /// # Examples
    ///
    /// To see the error output of this [`Command`] regardless of `--nocapture` in the benchmark
    /// output
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{Stdio, binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .stderr(Stdio::Inherit)
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    ///
    /// To pipe the Stderr into a file `/tmp/benchmark.output`:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use std::path::PathBuf;
    ///
    /// use gungraun::{Stdio, binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .stderr(Stdio::File("/tmp/benchmark.output".into()))
    ///         // or
    ///         .stderr(PathBuf::from("/tmp/benchmark.output"))
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn stderr<T>(&mut self, stdio: T) -> &mut Self
    where
        T: Into<Stdio>,
    {
        self.0.stderr = Some(stdio.into());
        self
    }

    /// Adds an environment variable available for this [`Command`].
    ///
    /// These environment variables are available independently of the setting of
    /// [`BinaryBenchmarkConfig::env_clear`] and additive to environment variables added with
    /// [`BinaryBenchmarkConfig::env`].
    ///
    /// # Examples
    ///
    /// An example for a custom environment variable "FOO=BAR":
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .env("FOO", "BAR")
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn env<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.0.config.envs.push((key.into(), Some(value.into())));
        self
    }

    /// Adds multiple environment variables available for this [`Command`].
    ///
    /// See [`Command::env`] for more details.
    ///
    /// # Examples
    ///
    /// Adds the custom environment variables "FOO=BAR" and `BAR=BAZ`:.
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .envs([("FOO", "BAR"), ("BAR", "BAZ")])
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.0
            .config
            .envs
            .extend(vars.into_iter().map(|(k, v)| (k.into(), Some(v.into()))));
        self
    }

    /// Sets the directory of the benchmarked binary (Default: Unchanged).
    ///
    /// See also [`BinaryBenchmarkConfig::current_dir`]
    ///
    /// # Examples
    ///
    /// To set the working directory of your [`Command`] to `/tmp`:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .current_dir("/tmp")
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    ///
    /// and the following will change the current directory to `fixtures` located in the root of the
    /// [`BinaryBenchmarkConfig::sandbox`]
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{BinaryBenchmarkConfig, Sandbox, binary_benchmark, binary_benchmark_group};
    ///
    /// fn setup_sandbox() {
    ///     std::fs::create_dir("fixtures").unwrap();
    /// }
    ///
    /// #[binary_benchmark(
    ///     setup = setup_sandbox(),
    ///     config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true))
    /// )]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .current_dir("fixtures")
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn current_dir<T>(&mut self, value: T) -> &mut Self
    where
        T: Into<PathBuf>,
    {
        self.0.config.current_dir = Some(value.into());
        self
    }

    /// Sets the expected exit status [`ExitWith`] of this [`Command`].
    ///
    /// See also [`BinaryBenchmarkConfig::exit_with`]. This setting overwrites the setting of the
    /// [`BinaryBenchmarkConfig`].
    ///
    /// # Examples
    ///
    /// If the command is expected to exit with a specific code, for example `100`:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{ExitWith, binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .exit_with(ExitWith::Code(100))
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    ///
    /// If a command is expected to fail, but the exit code doesn't matter:
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// # use gungraun::main;
    /// use gungraun::{ExitWith, binary_benchmark, binary_benchmark_group};
    ///
    /// #[binary_benchmark]
    /// fn bench_binary() -> gungraun::Command {
    ///     gungraun::Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///         .exit_with(ExitWith::Failure)
    ///         .build()
    /// }
    ///
    /// binary_benchmark_group!(name = my_group, benchmarks = bench_binary);
    /// # fn main() {
    /// # main!(binary_benchmark_groups = my_group);
    /// # }
    /// ```
    pub fn exit_with(&mut self, exit_with: ExitWith) -> &mut Self {
        self.0.config.exit_with = Some(exit_with);
        self
    }

    /// Finalize and build this [`Command`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    /// use gungraun::Command;
    ///
    /// let command: Command = Command::new(env!("CARGO_BIN_EXE_my-exe"))
    ///     .arg("foo")
    ///     .build();
    /// ```
    #[must_use]
    pub fn build(&mut self) -> Self {
        self.clone()
    }
}

impl From<&mut Self> for Command {
    fn from(value: &mut Self) -> Self {
        value.clone()
    }
}

impl From<&Self> for Command {
    fn from(value: &Self) -> Self {
        value.clone()
    }
}

impl Delay {
    /// Instantiate a [`Delay`] which will wait until a path exists ([`Path::exists`]).
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    ///
    /// use gungraun::{Command, Delay};
    ///
    /// let command = Command::new("echo").delay(Delay::from_path("/your/path/to/wait/for"));
    /// ```
    pub fn from_path<T>(path: T) -> Self
    where
        T: Into<PathBuf>,
    {
        Self(__internal::InternalDelay {
            kind: DelayKind::PathExists(path.into()),
            ..Default::default()
        })
    }

    /// Instantiate a [`Delay`] which will wait until successful TCP connect
    /// ([`std::net::TcpStream::connect`]).
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    ///
    /// use std::net::SocketAddr;
    ///
    /// use gungraun::{Command, Delay};
    ///
    /// let command = Command::new("echo").delay(Delay::from_tcp_socket(
    ///     "127.0.0.1:31000".parse::<SocketAddr>().unwrap(),
    /// ));
    /// ```
    pub fn from_tcp_socket<T>(addr: T) -> Self
    where
        T: Into<SocketAddr>,
    {
        Self(__internal::InternalDelay {
            kind: DelayKind::TcpConnect(addr.into()),
            ..Default::default()
        })
    }

    /// Instantiate a [`Delay`] which will wait until a UDP response is received after
    /// sending the UDP request. The poll duration is also used as the reconnect and request resend
    /// interval. ([`std::net::UdpSocket::bind`], [`std::net::UdpSocket::connect`],
    /// [`std::net::UdpSocket::recv`]).
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    ///
    /// use std::net::SocketAddr;
    ///
    /// use gungraun::{Command, Delay};
    ///
    /// let command = Command::new("echo").delay(Delay::from_udp_request(
    ///     "127.0.0.1:34000".parse::<SocketAddr>().unwrap(),
    ///     vec![1],
    /// ));
    /// ```
    pub fn from_udp_request<T, U>(addr: T, req: U) -> Self
    where
        T: Into<SocketAddr>,
        U: Into<Vec<u8>>,
    {
        Self(__internal::InternalDelay {
            kind: DelayKind::UdpResponse(addr.into(), req.into()),
            ..Default::default()
        })
    }

    /// Instantiate a [`Delay`] waiting until an event has happened.
    ///
    /// The possible events are defined in [`DelayKind`].
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    ///
    /// use std::time::Duration;
    ///
    /// use gungraun::{Command, Delay, DelayKind};
    ///
    /// let command = Command::new("echo").delay(Delay::new(DelayKind::DurationElapse(
    ///     Duration::from_secs(15),
    /// )));
    /// ```
    pub fn new(kind: DelayKind) -> Self {
        Self(__internal::InternalDelay {
            kind,
            ..Default::default()
        })
    }

    /// Update the [`Delay`] poll interval.
    ///
    /// The poll interval should be considered together with the [`Delay::timeout`], and ideally
    /// should have a value of `n * timeout duration`.
    ///
    /// In case the poll interval is set to a value `>=` timeout duration it is attempted to set
    /// the poll interval to a value of `timeout duration - 5ms`.
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    ///
    /// use std::net::SocketAddr;
    /// use std::time::Duration;
    ///
    /// use gungraun::{Command, Delay};
    ///
    /// let command = Command::new("echo").delay(
    ///     Delay::from_udp_request("127.0.0.1:34000".parse::<SocketAddr>().unwrap(), vec![1])
    ///         .poll(Duration::from_millis(150)),
    /// );
    /// ```
    #[must_use]
    pub fn poll<T: Into<Duration>>(mut self, duration: T) -> Self {
        self.0.poll = Some(duration.into());
        self
    }

    /// Update the [`Delay`] timeout interval.
    ///
    /// The timeout duration should be considered together with the poll interval. For further
    /// details please refer to [`Delay::poll`]. The minimum timeout duration is `10ms`.
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    ///
    /// use std::net::SocketAddr;
    /// use std::time::Duration;
    ///
    /// use gungraun::{Command, Delay};
    /// let command = Command::new("echo").delay(
    ///     Delay::from_tcp_socket("127.0.0.1:31000".parse::<SocketAddr>().unwrap())
    ///         .timeout(Duration::from_secs(5)),
    /// );
    /// ```
    #[must_use]
    pub fn timeout<T: Into<Duration>>(mut self, duration: T) -> Self {
        self.0.timeout = Some(duration.into());
        self
    }
}

impl<T> From<T> for Delay
where
    T: Into<Duration>,
{
    /// Instantiate a [`Delay`] which will wait until the duration has elapsed.
    ///
    /// ```rust
    /// # macro_rules! env { ($m:tt) => {{ "/some/path" }} }
    ///
    /// use std::time::Duration;
    ///
    /// use gungraun::{Command, Delay};
    ///
    /// let command = Command::new("echo").delay(Delay::from(Duration::from_secs(10)));
    /// ```
    fn from(duration: T) -> Self {
        Self(__internal::InternalDelay {
            kind: DelayKind::DurationElapse(duration.into()),
            ..Default::default()
        })
    }
}

impl From<BenchmarkId> for String {
    fn from(value: BenchmarkId) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty("")]
    #[case::simple_invalid("-")]
    #[case::when_0_char_minus_1("a\x2f")]
    #[case::when_9_char_plus_1("a\x3a")]
    #[case::when_big_a_char_minus_1("\x40")]
    #[case::when_big_z_char_plus_1("\x5b")]
    #[case::when_low_a_char_minus_1("\x60")]
    #[case::when_low_z_char_plus_1("\x7b")]
    #[case::invalid_2nd("a-")]
    #[case::invalid_3rd("ab-")]
    #[case::all_invalid("---")]
    #[case::number_as_first("0")]
    // This would be a valid rust identifier, but we don't allow the whole set
    #[case::non_ascii_1st("µ")]
    #[case::non_ascii_2nd("aµ")]
    #[case::non_ascii_3rd("aaµ")]
    #[case::non_ascii_middle("aµa")]
    fn test_benchmark_id_validate_when_error(#[case] id: &str) {
        let id = BenchmarkId::new(id);
        assert!(id.validate().is_err());
    }

    #[rstest]
    #[case::lowercase_a("a")]
    #[case::lowercase_b("b")]
    #[case::lowercase_m("m")]
    #[case::lowercase_y("y")]
    #[case::lowercase_z("z")]
    #[case::uppercase_a("A")]
    #[case::uppercase_b("B")]
    #[case::uppercase_n("N")]
    #[case::uppercase_y("Y")]
    #[case::uppercase_z("Z")]
    #[case::zero_2nd("a0")]
    #[case::one_2nd("a1")]
    #[case::eight_2nd("a8")]
    #[case::nine_2nd("a9")]
    #[case::number_middle("b4t")]
    #[case::underscore("_")]
    #[case::only_underscore("___")]
    #[case::underscore_last("a_")]
    #[case::mixed_all("auAEwer9__2xcd")]
    fn test_benchmark_id_validate(#[case] id: &str) {
        let id = BenchmarkId::new(id);
        id.validate().unwrap();
    }

    #[rstest]
    #[case::empty("", "Invalid id: Cannot be empty")]
    #[case::non_ascii_first(
        "µ",
        "Invalid id 'µ': Encountered non-ascii character as first character"
    )]
    #[case::multibyte_middle(
        "aµ",
        "Invalid id 'aµ' at position 1: Encountered non-ascii character"
    )]
    #[case::non_ascii_middle("a-", "Invalid id 'a-' at position 1: Invalid character '-'")]
    #[case::invalid_first("-", "Invalid id '-': As first character is '-' not allowed")]
    fn test_benchmark_id_validate_error_message(#[case] id: &str, #[case] message: &str) {
        let id = BenchmarkId::new(id);
        assert_eq!(
            id.validate()
                .expect_err("Validation should return an error"),
            message
        );
    }
}
