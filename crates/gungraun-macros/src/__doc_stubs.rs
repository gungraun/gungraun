// Internal mock stubs for `gungraun-macros` doc tests.
//
// This file is included via `include!()` inside `mod gungraun` blocks in
// `#[library_benchmark]` and `#[binary_benchmark]` doc tests. It provides
// the minimal mock types, macros, and functions that the macro expansion
// expects to find in the `gungraun` crate.

// These macros are defined outside `mod gungraun` in the doctest with
// `#[macro_export]` and re-exported here so they are available under the
// `gungraun::` path.
pub use crate::perf_log;
pub use crate::perf_enable;
pub use crate::perf_disable;

// --- shared stubs ---
pub mod client_requests {
    pub mod cachegrind {
        pub fn start_instrumentation() {}
        pub fn stop_instrumentation() {}
    }
}

pub struct LibraryBenchmarkConfig {}

// --- binary-benchmark stubs ---
pub mod prelude {}

#[derive(Clone)]
pub struct Command {}

impl Command {
    pub fn new(_a: &str) -> Self {
        Self {}
    }
    pub fn stdout(&mut self, _a: Stdio) -> &mut Self {
        self
    }
    pub fn arg<T>(&mut self, _a: T) -> &mut Self
    where
        T: Into<std::path::PathBuf>,
    {
        self
    }
    pub fn build(&mut self) -> Self {
        self.clone()
    }
}

pub enum Stdio {
    Inherit,
    File(std::path::PathBuf),
}

#[derive(Clone)]
pub struct Sandbox {}

impl Sandbox {
    pub fn new(_a: bool) -> Self {
        Self {}
    }
    pub fn fixtures(&mut self, _a: [&str; 2]) -> &mut Self {
        self
    }
}

impl From<&mut Sandbox> for Sandbox {
    fn from(value: &mut Sandbox) -> Self {
        value.clone()
    }
}

#[derive(Default)]
pub struct BinaryBenchmarkConfig {}

impl BinaryBenchmarkConfig {
    pub fn sandbox<T: Into<Sandbox>>(&mut self, _a: T) -> &mut Self {
        self
    }
}

impl From<&mut BinaryBenchmarkConfig> for BinaryBenchmarkConfig {
    fn from(_value: &mut BinaryBenchmarkConfig) -> Self {
        BinaryBenchmarkConfig {}
    }
}

// --- __internal (library + binary) ---
pub mod __internal {
    pub const PERF_REPETITIONS_MARKER: &str = "gungraun::__perf_repetitions:";

    // Library benchmark internal types
    #[derive(Clone, Copy)]
    pub enum InternalBenchRunMode {
        Default,
        PerfDynamic,
        PerfCalibrate,
        PerfOverhead(usize),
        PerfRepeat(usize),
        PerfOnce,
    }

    pub enum InternalLibFunctionKind {
        None,
        Default(fn(InternalBenchRunMode)),
        Iter(fn(InternalBenchRunMode, Option<usize>) -> usize),
    }

    pub struct InternalMacroLibBench {
        pub id_display: Option<&'static str>,
        pub args_display: Option<&'static str>,
        pub consts_display: Option<&'static str>,
        pub func: InternalLibFunctionKind,
        pub config: Option<fn() -> InternalLibraryBenchmarkConfig>,
    }

    pub struct InternalLibraryBenchmarkConfig {}

    // Binary benchmark internal types
    pub enum InternalBinFunctionKind {
        None,
        Default(fn() -> super::Command),
    }

    pub enum InternalBinAssistantKind {
        None,
        Default(fn()),
    }

    pub struct InternalMacroBinBench {
        pub id_display: Option<&'static str>,
        pub args_display: Option<&'static str>,
        pub consts_display: Option<&'static str>,
        pub func: InternalBinFunctionKind,
        pub config: Option<fn() -> InternalBinaryBenchmarkConfig>,
        pub setup: InternalBinAssistantKind,
        pub teardown: InternalBinAssistantKind,
    }

    pub struct InternalBinaryBenchmarkConfig {}

    impl From<&mut super::BinaryBenchmarkConfig> for InternalBinaryBenchmarkConfig {
        fn from(_value: &mut super::BinaryBenchmarkConfig) -> Self {
            InternalBinaryBenchmarkConfig {}
        }
    }

    pub mod stats {
        use std::time::Duration;

        pub fn calibrate_linear<I, O, S, W, T>(
            _max_calibration_time: Duration,
            _setup: S,
            _work: W,
            _teardown: T,
        ) -> u64
        where
            S: Fn() -> I,
            W: Fn(I) -> O,
            T: Fn(O),
        {
            1
        }
    }

    pub mod perf {
        pub fn calibrate() {}
    }
}
