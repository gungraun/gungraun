//! The command-line arguments of cargo bench as in ARGS of `cargo bench -- ARGS`

// spell-checker: ignore totalbytes totalblocks writeback writebackbehaviour

/// Default values for command-line arguments
///
/// This module contains constants that define the default behavior when corresponding command-line
/// arguments are not specified.
pub mod defaults {
    /// Default value for `--allow-aslr`
    ///
    /// When `false` (the default), Gungraun attempts to disable Address Space Layout Randomization
    /// (ASLR) for more consistent benchmark results by using `setarch` on Linux or `proccontrol`
    /// on FreeBSD.
    pub const ALLOW_ASLR: bool = false;

    /// Default value for `--env-clear`
    ///
    /// When `true` (the default), Gungraun clears most environment variables before running the
    /// benchmark. Only essential variables like `LD_PRELOAD`, `LD_LIBRARY_PATH` are preserved.
    pub const ENV_CLEAR: bool = true;
}

use std::ffi::OsString;
use std::fmt::Display;
use std::hash::Hash;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::Result;
use clap::builder::{BoolishValueParser, PathBufValueParser, TypedValueParser};
use clap::{ArgAction, CommandFactory, Parser};
use indexmap::{IndexMap, IndexSet, indexset};
use simplematch::{DoWild, Options};
use strum::IntoEnumIterator;

use super::cachegrind::regression::CachegrindRegressionConfig;
use super::callgrind::regression::CallgrindRegressionConfig;
use super::dhat::regression::DhatRegressionConfig;
use super::format::{ListFormat, OutputFormatKind};
use super::tool::regression::ToolRegressionConfig;
use crate::api::{
    CachegrindMetric, CachegrindMetrics, CallgrindMetrics, DhatMetric, DhatMetrics, ErrorMetric,
    EventKind, RawToolArgs, Tool,
};
use crate::metrics::logic::TypeChecker;
use crate::metrics::model::Metric;
use crate::runner::common::CapturedOutput;
use crate::summary::model::{BaselineName, SummaryFormat};
use crate::util;

const DOWILD_OPTIONS: Options<u8> = Options::new().enable_escape(true).enable_classes(true);

// Utility for complex types intended to be used during the parsing of the command-line arguments
type Limits<T> = (IndexMap<T, f64>, IndexMap<T, Metric>);
type ParsedMetrics<T> = Result<Vec<(T, Option<Metric>)>, String>;

/// A filter for benchmarks
///
/// # Developer Notes
///
/// This enum is used instead of a plain `String` for possible future usages to filter by benchmark
/// ids, group name, file name etc.
#[derive(Debug, Clone)]
pub enum BenchmarkFilter {
    /// The name of the benchmark
    WildcardPattern(String),
}

/// The `NoCapture` options for the command-line argument --nocapture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoCapture {
    /// Don't capture any output
    True,
    /// Capture all output
    False,
    /// Don't capture `stderr`
    Stderr,
    /// Don't capture `stdout`
    Stdout,
}

/// An internal enum for the value of the --truncate-description argument
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncateDescription {
    /// Truncate the description to this value
    To(usize),
    /// Do not truncate the description
    None,
}

// TODO: Add cli args for perf, check current cli args like --tools, --default-tool if they support
// perf, Update the documentation of the args which need it
/// The command line arguments the user provided after `--` when running cargo bench
///
/// These arguments are not the command line arguments passed to `gungraun-runner`. We collect
/// the command line arguments in the `gungraun::main!` macro without the binary as first
/// argument, that's why `no_binary_name` is set to `true`.
#[expect(clippy::partial_pub_fields, clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "High-precision, one-shot and consistent benchmarking framework/harness for Rust

Boolish command line arguments take also one of `y`, `yes`, `t`, `true`, `on`, `1`
instead of `true` and one of `n`, `no`, `f`, `false`, `off`, and `0` instead of
`false`",
    after_help = "  Exit codes:
      0: Success
      1: All other errors
      2: Parsing command-line arguments failed
      3: One or more regressions occurred
    ",
    long_about = None,
    no_binary_name = true,
    override_usage= "cargo bench ... -- [OPTIONS | FILTER]",
    max_term_width = 101
)]
pub struct CommandLineArgs {
    ////////////////////////////////////////////////////////////////////////////////////////////////
    // The following arguments are accepted by the rust libtest harness and ignored by us
    //
    // Further details in <https://doc.rust-lang.org/rustc/tests/index.html#cli-arguments> or by
    // running `cargo test -- --help`
    // `--bench` also shows up as last argument set by `cargo bench` even if not explicitly given
    ////////////////////////////////////////////////////////////////////////////////////////////////
    #[arg(action = ArgAction::SetTrue, hide = true, long = "bench", required = false)]
    _bench: bool,

    #[arg(hide = true, long = "color", num_args = 0.., required = false)]
    _color: Vec<String>,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "ensure-time", required = false)]
    _ensure_time: bool,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "exact", required = false)]
    _exact: bool,

    #[arg(
        action = ArgAction::SetTrue,
        hide = true,
        long = "exclude-should-panic",
        required = false
    )]
    _exclude_should_panic: bool,

    #[arg(
        action = ArgAction::SetTrue,
        hide = true,
        long = "fail-fast",
        required = false
    )]
    _fail_fast: bool,

    #[arg(
        action = ArgAction::SetTrue,
        hide = true,
        long = "force-run-in-process",
        required = false
    )]
    _force_run_in_process: bool,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "include-ignored", required = false)]
    _include_ignored: bool,

    #[arg(hide = true, long = "logfile", num_args = 0.., required = false)]
    _logfile: Vec<String>,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "quiet", required = false, short = 'q')]
    _quiet: bool,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "report-time", required = false)]
    _report_time: bool,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "show-output", required = false)]
    _show_output: bool,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "shuffle", required = false)]
    _shuffle: bool,

    #[arg(hide = true, long = "shuffle-seed", num_args = 0.., required = false)]
    _shuffle_seed: Vec<String>,

    #[arg(hide = true, long = "skip", num_args = 0.., required = false)]
    _skip: Vec<String>,

    #[arg(action = ArgAction::SetTrue, hide = true, long = "test", required = false)]
    _test: bool,

    #[arg(hide = true, long = "test-threads", num_args = 0.., required = false)]
    _test_threads: Vec<String>,

    #[arg(hide = true, num_args = 0.., required = false, short = 'Z')]
    _unstable_options: Vec<String>,

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // End of ignored libtest arguments
    ////////////////////////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // Display order:
    // 20 : List
    // 100: General args
    // 150: Valgrind runner
    // 200: Baselines
    // 300: Output format
    // 400: Metrics
    // 450: Tools
    // 500: Tool arguments
    // 600: Limits, Regression
    ////////////////////////////////////////////////////////////////////////////////////////////////
    #[rustfmt::skip]
    /// Allow ASLR (Address Space Layout Randomization)
    ///
    /// If possible, ASLR is disabled on platforms that support it (linux, freebsd) because ASLR
    /// could noise up the Callgrind cache simulation results a bit. Setting this option to `true`
    /// runs all benchmarks with ASLR enabled.
    ///
    /// See also [kernel.org: randomize_va_space]
    ///
    /// [kernel.org: randomize_va_space]:
    /// https://docs.kernel.org/admin-guide/sysctl/kernel.html#randomize-va-space
    #[arg(
        default_missing_value = "true",
        display_order = 100,
        env = "GUNGRAUN_ALLOW_ASLR",
        long = "allow-aslr",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
    )]
    pub allow_aslr: Option<bool>,

    #[rustfmt::skip]
    /// Compare benchmark results against a previously saved baseline
    ///
    /// This option compares the current benchmark run against a named baseline from a previous run
    /// without modifying the saved baseline. Baselines store benchmark results for future
    /// comparisons, useful for tracking performance over time or comparing against fixed reference
    /// points like a release tag or main branch.
    ///
    /// If this option is specified but no baseline with that name exists yet, Gungraun creates a
    /// new baseline with the current results instead of comparing.
    ///
    /// See also: `--save-baseline` to create or update a baseline, `--load-baseline` to compare
    /// existing baselines without running benchmarks.
    ///
    /// Examples:
    ///   * `--baseline` (uses the default baseline name "default")
    ///   * `--baseline=main` (compares against baseline saved as "main")
    ///   * `--baseline=v1.0` (compares against baseline saved as "v1.0")
    #[arg(
        default_missing_value = "default",
        display_order = 200,
        env = "GUNGRAUN_BASELINE",
        long = "baseline",
        num_args = 0..=1,
        require_equals = true,
    )]
    pub baseline: Option<BaselineName>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to the experimental BBV
    ///
    /// <https://valgrind.org/docs/manual/bbv-manual.html#bbv-manual.usage>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --bbv-args=--interval-size=10000
    ///   * --bbv-args='--interval-size=10000 --instr-count-only=yes'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_BBV_ARGS",
        long = "bbv-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub bbv_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Cachegrind
    ///
    /// <https://valgrind.org/docs/manual/cg-manual.html#cg-manual.cgopts>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --cachegrind-args=--instr-at-start=no
    ///   * --cachegrind-args='--branch-sim=yes --instr-at-start=no'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_CACHEGRIND_ARGS",
        long = "cachegrind-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub cachegrind_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    #[expect(clippy::doc_markdown)]
    /// Set performance regression limits for specific Cachegrind metrics
    ///
    /// This is a `,` separate list of CachegrindMetric=limit or CachegrindMetrics=limit
    /// (key=value) pairs. See the description of --callgrind-limits for the details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetrics.html>
    /// respectively <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetric.html>
    /// for valid metrics and group members.
    ///
    /// See the guide
    /// (<https://gungraun.github.io/gungraun/latest/html/regressions.html>) for all
    /// details or replace the format spec in `--callgrind-limits` with the following:
    ///
    /// group ::= "@" ( "default"
    ///               | "all"
    ///               | ("cachemisses" | "misses" | "ms")
    ///               | ("cachemissrates" | "missrates" | "mr")
    ///               | ("cachehits" | "hits" | "hs")
    ///               | ("cachehitrates" | "hitrates" | "hr")
    ///               | ("cachesim" | "cs")
    ///               | ("branchsim" | "bs")
    ///               )
    /// event ::= CachegrindMetric
    ///
    /// Examples:
    ///   * --cachegrind-limits='ir=0.0%'
    ///   * --cachegrind-limits='ir=10000,EstimatedCycles=10%'
    ///   * --cachegrind-limits='@all=10%,ir=10000,EstimatedCycles=10%'
    #[arg(
        display_order = 600,
        env = "GUNGRAUN_CACHEGRIND_LIMITS",
        long = "cachegrind-limits",
        num_args = 1,
        value_parser = parse_cachegrind_limits,
        verbatim_doc_comment,
    )]
    pub cachegrind_limits: Option<ToolRegressionConfig>,

    #[rustfmt::skip]
    /// Define the Cachegrind metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of Cachegrind metric groups and event kinds which are allowed
    /// to appear in the terminal output of Cachegrind.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetrics.html>
    /// respectively
    /// <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetric.html> for valid
    /// metrics and group members.
    ///
    /// The `group` names, their abbreviations if present and `event` kinds are exactly the same as
    /// described in the `--cachegrind-limits` option.
    ///
    /// Examples:
    ///   * --cachegrind-metrics='ir' to show only `Instructions`
    ///   * --cachegrind-metrics='@all' to show all possible Cachegrind metrics
    ///   * --cachegrind-metrics='@default,@mr' to show cache miss rates in addition to the defaults
    #[arg(
        display_order = 400,
        env = "GUNGRAUN_CACHEGRIND_METRICS",
        long = "cachegrind-metrics",
        num_args = 1..,
        required = false,
        value_parser = parse_cachegrind_metrics,
        verbatim_doc_comment,
    )]
    pub cachegrind_metrics: Option<IndexSet<CachegrindMetric>>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Callgrind
    ///
    /// <https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options> and the core valgrind
    /// command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>. Note that not all
    /// command-line arguments are supported especially the ones which change output paths.
    /// Unsupported arguments will be ignored printing a warning.
    ///
    /// Examples:
    ///   * --callgrind-args=--dump-instr=yes
    ///   * --callgrind-args='--dump-instr=yes --collect-systime=yes'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_CALLGRIND_ARGS",
        long = "callgrind-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub callgrind_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    #[expect(clippy::doc_markdown)]
    /// Set performance regression limits for specific `EventKinds`
    ///
    /// This is a `,` separate list of EventKind=limit or CallgrindMetrics=limit (key=value) pairs
    /// with the limit being a soft limit if the number suffixed with a `%` or a hard limit if it
    /// is a bare number. It is possible to specify hard and soft limits in one go with the `|`
    /// operator (e.g. `ir=10%|10000`). Groups (CallgrindMetrics) are prefixed with `@`. List of
    /// allowed groups and events with their abbreviations:
    ///
    /// group ::= "@" ( "default"
    ///               | "all"
    ///               | ("cachemisses" | "misses" | "ms")
    ///               | ("cachemissrates" | "missrates" | "mr")
    ///               | ("cachehits" | "hits" | "hs")
    ///               | ("cachehitrates" | "hitrates" | "hr")
    ///               | ("cachesim" | "cs")
    ///               | ("cacheuse" | "cu")
    ///               | ("systemcalls" | "syscalls" | "sc")
    ///               | ("branchsim" | "bs")
    ///               | ("writebackbehaviour" | "writeback" | "wb")
    ///               )
    /// event ::= EventKind
    ///
    /// See the guide (<https://gungraun.github.io/gungraun/latest/html/regressions.html>)
    /// for more details, the docs of `CallgrindMetrics`
    /// (<https://docs.rs/gungraun/latest/gungraun/enum.CallgrindMetrics.html>) and
    /// `EventKind` <https://docs.rs/gungraun/latest/gungraun/enum.EventKind.html> for a
    /// list of metrics and groups with their members.
    ///
    /// A performance regression check for an `EventKind` fails if the limit is exceeded. If
    /// limits are defined and one or more regressions have occurred during the benchmark run,
    /// the whole benchmark is considered to have failed and the program exits with error and
    /// exit code `3`.
    ///
    /// Examples:
    ///   * --callgrind-limits='ir=5.0%'
    ///   * --callgrind-limits='ir=10000,EstimatedCycles=10%'
    ///   * --callgrind-limits='@all=10%,ir=5%|10000'
    #[arg(
        display_order = 600,
        env = "GUNGRAUN_CALLGRIND_LIMITS",
        long = "callgrind-limits",
        num_args = 1,
        value_parser = parse_callgrind_limits,
        verbatim_doc_comment,
    )]
    pub callgrind_limits: Option<ToolRegressionConfig>,

    #[rustfmt::skip]
    /// Define the Callgrind metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of Callgrind metric groups and event kinds which are allowed
    /// to appear in the terminal output of Callgrind. Group names need to be prefixed with '@'.
    /// The order matters and the Callgrind metrics are shown in their insertion order of this
    /// option. More precisely, in case of duplicate metrics, the first specified one wins.
    ///
    /// The `group` names, their abbreviations if present and `event` kinds are exactly the same as
    /// described in the `--callgrind-limits` option.
    ///
    /// For a list of valid metrics, groups and their members see the docs of `CallgrindMetrics`
    /// (<https://docs.rs/gungraun/latest/gungraun/enum.CallgrindMetrics.html>) and
    /// `EventKind` <https://docs.rs/gungraun/latest/gungraun/enum.EventKind.html>.
    ///
    /// Note that setting the metrics here does not imply that these metrics are actually
    /// collected. This option just sets the order and appearance of metrics in case they are
    /// collected. To activate the collection of specific metrics you need to use
    /// `--callgrind-args`.
    ///
    /// Examples:
    ///   * --callgrind-metrics='ir' to show only `Instructions`
    ///   * --callgrind-metrics='@all' to show all possible Callgrind metrics
    ///   * --callgrind-metrics='@default,@mr' to show cache miss rates in addition to the defaults
    #[arg(
        display_order = 400,
        env = "GUNGRAUN_CALLGRIND_METRICS",
        long = "callgrind-metrics",
        num_args = 1..,
        required = false,
        value_parser = parse_callgrind_metrics,
        verbatim_doc_comment,
    )]
    pub callgrind_metrics: Option<IndexSet<EventKind>>,

    #[rustfmt::skip]
    /// The default tool used to run the benchmarks
    ///
    /// The standard tool to run the benchmarks is Callgrind but can be overridden with this
    /// option. Any Valgrind tool can be used:
    ///   * callgrind
    ///   * cachegrind
    ///   * dhat
    ///   * memcheck
    ///   * helgrind
    ///   * drd
    ///   * massif
    ///   * exp-bbv
    ///
    /// This argument matches the tool case-insensitive. Note that using Cachegrind with this
    /// option to benchmark library functions needs adjustments to the benchmarking functions with
    /// client-requests to measure the counts correctly. If you want to switch permanently to
    /// Cachegrind, it is usually better to activate the `cachegrind` feature of gungraun in
    /// your Cargo.toml. However, setting a tool with this option overrides Cachegrind set with the
    /// gungraun feature. See the guide for all details.
    #[arg(
        display_order = 450,
        env = "GUNGRAUN_DEFAULT_TOOL",
        long = "default-tool",
        num_args = 1,
        verbatim_doc_comment,
    )]
    pub default_tool: Option<Tool>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to DHAT
    ///
    /// <https://valgrind.org/docs/manual/dh-manual.html#dh-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --dhat-args=--mode=ad-hoc
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_DHAT_ARGS",
        long = "dhat-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub dhat_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Set performance regression limits for specific DHAT metrics
    ///
    /// This is a `,` separate list of DhatMetrics=limit or DhatMetric=limit (key=value) pairs. See
    /// the description of --callgrind-limits for the details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetrics.html> respectively
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetric.html> for valid metrics
    /// and group members.
    ///
    /// See the guide
    /// (<https://gungraun.github.io/gungraun/latest/html/regressions.html>) for all
    /// details or replace the format spec in `--callgrind-limits` with the following:
    ///
    /// group ::= "@" ( "default" | "all" )
    /// event ::=   ( "totalunits" | "tun" )
    ///           | ( "totalevents" | "tev" )
    ///           | ( "totalbytes" | "tb" )
    ///           | ( "totalblocks" | "tbk" )
    ///           | ( "attgmaxbytes" | "gb" )
    ///           | ( "attgmaxblocks" | "gbk" )
    ///           | ( "attendbytes" | "eb" )
    ///           | ( "attendblocks" | "ebk" )
    ///           | ( "readsbytes" | "rb" )
    ///           | ( "writesbytes" | "wb" )
    ///           | ( "totallifetimes" | "tl" )
    ///           | ( "maximumbytes" | "mb" )
    ///           | ( "maximumblocks" | "mbk" )
    ///
    /// `events` with a long name have their allowed abbreviations placed in the same parentheses.
    ///
    /// Examples:
    ///   * --dhat-limits='totalbytes=0.0%'
    ///   * --dhat-limits='totalbytes=10000,totalblocks=5%'
    ///   * --dhat-limits='@all=10%,totalbytes=5000,totalblocks=5%'
    #[arg(
        display_order = 600,
        env = "GUNGRAUN_DHAT_LIMITS",
        long = "dhat-limits",
        num_args = 1,
        value_parser = parse_dhat_limits,
        verbatim_doc_comment,
    )]
    pub dhat_limits: Option<ToolRegressionConfig>,

    #[rustfmt::skip]
    /// Define the DHAT metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of DHAT metric groups and event kinds which are allowed to
    /// appear in the terminal output of DHAT.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetrics.html> respectively
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetric.html> for valid metrics
    /// and group members.
    ///
    /// The `group` names, their abbreviations if present and `event` kinds are exactly the same as
    /// described in the `--dhat-limits` option.
    ///
    /// Examples:
    ///   * --dhat-metrics='totalbytes' to show only `Total Bytes`
    ///   * --dhat-metrics='@all' to show all possible DHAT metrics
    ///   * --dhat-metrics='@default,mb' to show maximum bytes in addition to the defaults
    #[arg(
        display_order = 400,
        env = "GUNGRAUN_DHAT_METRICS",
        long = "dhat-metrics",
        num_args = 1..,
        required = false,
        value_parser = parse_dhat_metrics,
        verbatim_doc_comment,
    )]
    pub dhat_metrics: Option<IndexSet<DhatMetric>>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to DRD
    ///
    /// <https://valgrind.org/docs/manual/drd-manual.html#drd-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --drd-args=--exclusive-threshold=100
    ///   * --drd-args='--exclusive-threshold=100 --free-is-write=yes'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_DRD_ARGS",
        long = "drd-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub drd_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Define the DRD error metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of error metrics which are allowed to appear in the terminal
    /// output of DRD. The `group` and `event` are the same as for `--memcheck-metrics`.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid error
    /// metrics.
    ///
    /// Since this is a very small set of metrics, there is only one `group`: `@all`
    ///
    /// Examples:
    ///   * --drd-metrics='errors' to show only `Errors`
    ///   * --drd-metrics='@all' to show all possible error metrics (the default)
    ///   * --drd-metrics='err,ctx' to show only errors and contexts
    #[arg(
        display_order = 400,
        env = "GUNGRAUN_DRD_METRICS",
        long = "drd-metrics",
        num_args = 1..,
        required = false,
        value_parser = parse_drd_metrics,
        verbatim_doc_comment,
    )]
    pub drd_metrics: Option<IndexSet<ErrorMetric>>,

    #[rustfmt::skip]
    /// Control whether environment variables are cleared before running a benchmark
    ///
    /// By default (`true`), environment variables are cleared to ensure reproducible benchmark
    /// results across different environments. Set to `false` to preserve all environment variables
    /// of the `cargo bench` process.
    ///
    /// Examples:
    ///   * `--env-clear` (default: clear environment)
    ///   * `--env-clear=false` (preserve environment)
    #[arg(
        default_missing_value = "true",
        display_order = 100,
        env = "GUNGRAUN_ENV_CLEAR",
        long = "env-clear",
        num_args = 0..=1,
        value_parser = BoolishValueParser::new(),
        verbatim_doc_comment,
    )]
    pub env_clear: Option<bool>,

    #[rustfmt::skip]
    /// Set environment variables for benchmarks ignoring the clearing of environment variables
    ///
    /// Environment variables can be specified in two forms:
    /// - `KEY=VALUE`: Set `KEY` to `VALUE` explicitly
    /// - `KEY`: Resolve `KEY` from the current environment and pass its value
    ///
    /// Multiple key-value pairs can be specified in a single invocation using space-separated
    /// values (posix-style quoting of values is supported). The `--envs` argument can also be
    /// specified multiple times to accumulate environment variables.
    ///
    /// These variables are cumulative to any environment variables configured via
    /// `LibraryBenchmarkConfig::env` or `BinaryBenchmarkConfig::env`.
    ///
    /// Examples:
    ///   * `--envs=FOO=bar` (set FOO to "bar")
    ///   * `--envs=FOO` (pass the original value of FOO from current environment)
    ///   * `--envs='FOO=bar BAZ=qux'` (set multiple variables and once)
    ///   * `--envs=FOO=bar --envs=BAZ=qux` (accumulate multiple times)
    #[arg(
        action = ArgAction::Append,
        display_order = 100,
        env = "GUNGRAUN_ENVS",
        long = "envs",
        num_args = 1,
        require_equals = true,
        required = false,
        value_parser = parse_envs,
        verbatim_doc_comment,
    )]
    pub envs: Vec<Vec<(OsString, OsString)>>,

    #[rustfmt::skip]
    /// If specified, only run benchmarks matching this wildcard pattern
    ///
    /// The wildcard pattern can contain `*` to match any amount of characters, `?` to match a
    /// single character and simple classes `[...]` like `[abc] `to match the characters `a` or `b`
    /// or `c`. Character classes can contain ranges, so `[abc]` could be rewritten as `[a-c]` and
    /// they can be negated with `[!...]` to not match the contained characters.
    ///
    /// This pattern matches the whole module path of benchmarks. A list of all benchmarks with
    /// their module path as recognized by this option can be obtained by running `--list`. The
    /// general structure of the module path of a benchmark is:
    ///
    /// `FILENAME::GROUP::FUNCTION::ID`
    ///
    /// Examples:
    ///   * `*::my_benchmark_id` runs all benchmarks with the id `my_benchmark_id`
    ///   * `gungraun_benchmarks::*` runs all benchmarks in the file `gungraun_benchmarks`
    ///   * `my_file::some_group::*` runs all benchmarks in the file `my_file` and the group
    ///     `some_group`
    #[arg(
        env = "GUNGRAUN_FILTER",
        name = "FILTER",
        num_args = 0..=1,
        verbatim_doc_comment,
    )]
    pub filter: Option<BenchmarkFilter>,

    /// Hidden libtest-compat shim that controls the format of the `--list` output
    ///
    /// Only `terse` actually changes behavior (it suppresses the trailing blank line and
    /// `0 tests, N benchmarks` summary so the listing matches what `cargo nextest` expects).
    /// All other values, including `pretty`, `json`, `junit`, the empty string, and unknown
    /// values, fall back to `pretty` to preserve compatibility with consumers that pass libtest
    /// values gungraun doesn't natively emit. Hidden because it has no effect outside `--list`.
    #[arg(
        default_missing_value = "pretty",
        default_value = "pretty",
        hide = true,
        long = "format",
        num_args = 0..=1,
        required = false,
        value_parser = parse_list_format,
    )]
    pub format: ListFormat,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Helgrind
    ///
    /// <https://valgrind.org/docs/manual/hg-manual.html#hg-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --helgrind-args=--free-is-write=yes
    ///   * --helgrind-args='--conflict-cache-size=100000 --free-is-write=yes'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_HELGRIND_ARGS",
        long = "helgrind-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub helgrind_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Define the Helgrind error metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of error metrics which are allowed to appear in the terminal
    /// output of Helgrind. The `group` and `event` are the same as for `--memcheck-metrics`.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid error
    /// metrics.
    ///
    /// Examples:
    ///   * --helgrind-metrics='errors' to show only `Errors`
    ///   * --helgrind-metrics='@all' to show all possible error metrics (the default)
    ///   * --helgrind-metrics='err,ctx' to show only errors and contexts
    #[arg(
        display_order = 400,
        env = "GUNGRAUN_HELGRIND_METRICS",
        long = "helgrind-metrics",
        num_args = 1..,
        required = false,
        value_parser = parse_helgrind_metrics,
        verbatim_doc_comment,
    )]
    pub helgrind_metrics: Option<IndexSet<ErrorMetric>>,

    #[rustfmt::skip]
    /// Specify the home directory of gungraun benchmark output files
    ///
    /// All output files are by default stored under the `$PROJECT_ROOT/target/gungraun` directory.
    /// This option lets you customize this home directory, and it will be created if it doesn't
    /// exist.
    #[arg(
        display_order = 100,
        env = "GUNGRAUN_HOME",
        long = "home",
        num_args = 1,
    )]
    pub home: Option<PathBuf>,

    /// Hidden libtest-compat shim consulted only by `--list`
    ///
    /// `cargo nextest` performs a second `--list --format terse --ignored` pass to discover the
    /// set of ignored tests. Gungraun has no per-benchmark ignore concept, so when this flag is
    /// paired with `--list` we emit an empty list (the contract nextest documents for harnesses
    /// without ignored tests). Ignored otherwise.
    #[arg(action = ArgAction::SetTrue, hide = true, long = "ignored", required = false)]
    pub ignored: bool,

    #[rustfmt::skip]
    /// Print a list of all benchmarks. With this argument no benchmarks are executed.
    ///
    /// The output format is intended to be the same as the output format of the libtest harness.
    /// However, future changes of the output format by cargo might not be incorporated into
    /// gungraun. As a consequence, it is not considered safe to rely on the output in
    /// scripts.
    ///
    /// Combine with `--format terse` for `cargo nextest`-compatible output (per-benchmark lines
    /// only, no trailing blank line or summary). `--list --format terse --ignored` emits an
    /// empty list because gungraun has no ignored-benchmark concept.
    #[arg(
        action = ArgAction::Set,
        default_missing_value = "true",
        default_value = "false",
        display_order = 20,
        env = "GUNGRAUN_LIST",
        long = "list",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
    )]
    pub list: bool,

    #[rustfmt::skip]
    /// Load an existing baseline instead of running new benchmarks
    ///
    /// This option loads benchmark results from a previously saved baseline and uses them as the
    /// "new" data for comparison against another baseline. This allows comparing two existing
    /// baselines without re-running any benchmarks.
    ///
    /// This option requires `--baseline` to be specified, which provides the "old" baseline to
    /// compare against.
    ///
    /// This is useful for:
    /// - Re-comparing existing baselines with different comparison targets
    /// - Comparing two previously saved baselines against each other
    /// - Avoiding expensive benchmark re-runs when only analysis is needed
    ///
    /// See also: `--baseline` to compare against a baseline while running new benchmarks,
    /// `--save-baseline` to create or update a baseline.
    ///
    /// Examples:
    ///   * `--load-baseline --baseline=main` (loads "default", compares against "main")
    ///   * `--load-baseline=feature --baseline=main` (loads "feature", compares against "main")
    ///   * `--load-baseline=v1.1 --baseline=v1.0` (loads "v1.1", compares against "v1.0")
    #[clap(
        id = "LOAD_BASELINE",
        long = "load-baseline",
        requires = "baseline",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "default",
        env = "GUNGRAUN_LOAD_BASELINE",
        display_order = 200
    )]
    pub load_baseline: Option<BaselineName>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Massif
    ///
    /// <https://valgrind.org/docs/manual/ms-manual.html#ms-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --massif-args=--heap=no
    ///   * --massif-args='--heap=no --threshold=2.0'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_MASSIF_ARGS",
        long = "massif-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub massif_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Memcheck
    ///
    /// <https://valgrind.org/docs/manual/mc-manual.html#mc-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --memcheck-args=--leak-check=full
    ///   * --memcheck-args='--leak-check=yes --show-leak-kinds=all'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_MEMCHECK_ARGS",
        long = "memcheck-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub memcheck_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Define the Memcheck error metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of error metrics which are allowed to appear in the terminal
    /// output of Memcheck.
    ///
    /// Since this is a very small set of metrics, there is only one `group`: `@all`
    ///
    /// group ::= "@all"
    /// event ::=   ( "errors" | "err" )
    ///           | ( "contexts" | "ctx" )
    ///           | ( "suppressederrors" | "serr")
    ///           | ( "suppressedcontexts" | "sctx" )
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid
    /// metrics.
    ///
    /// Examples:
    ///   * --memcheck-metrics='errors' to show only `Errors`
    ///   * --memcheck-metrics='@all' to show all possible error metrics (the default)
    ///   * --memcheck-metrics='err,ctx' to show only errors and contexts
    #[arg(
        display_order = 400,
        env = "GUNGRAUN_MEMCHECK_METRICS",
        long = "memcheck-metrics",
        num_args = 1..,
        required = false,
        value_parser = parse_memcheck_metrics,
        verbatim_doc_comment,
    )]
    pub memcheck_metrics: Option<IndexSet<ErrorMetric>>,

    #[rustfmt::skip]
    /// Don't capture terminal output of benchmarks
    ///
    /// Possible values are one of [true, false, stdout, stderr].
    ///
    /// This option is currently restricted to the Callgrind run of benchmarks. The output of
    /// additional tool runs like DHAT, Memcheck, ... is still captured, to prevent showing the
    /// same output of benchmarks multiple times. Use `GUNGRAUN_LOG=info` to also show
    /// captured and logged output.
    ///
    /// If no value is given, the default missing value is `true` and doesn't capture stdout and
    /// stderr. Besides `true` or `false` you can specify the special values `stdout` or `stderr`.
    /// If `--nocapture=stdout` is given, the output to `stdout` won't be captured and the output
    /// to `stderr` will be discarded. Likewise, if `--nocapture=stderr` is specified, the output
    /// to `stderr` won't be captured and the output to `stdout` will be discarded.
    #[arg(
        alias = "no-capture",
        default_missing_value = "true",
        default_value = "false",
        display_order = 300,
        env = "GUNGRAUN_NOCAPTURE",
        long = "nocapture",
        num_args = 0..=1,
        require_equals = true,
        required = false,
        value_parser = parse_nocapture,
    )]
    pub nocapture: NoCapture,

    #[rustfmt::skip]
    /// Suppress the summary showing regressions and execution time at the end of a benchmark run
    ///
    /// Note, that a summary is only printed if the `--output-format` is not JSON.
    ///
    /// The summary described by `--nosummary` is different from `--save-summary` and they do not
    /// affect each other.
    #[arg(
        alias = "no-summary",
        action = ArgAction::Set,
        default_missing_value = "true",
        default_value = "false",
        display_order = 300,
        env = "GUNGRAUN_NOSUMMARY",
        long = "nosummary",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
    )]
    pub nosummary: bool,

    #[rustfmt::skip]
    /// The terminal output format in default human-readable format or in machine-readable json
    /// format
    ///
    /// # The JSON Output Format
    ///
    /// The json terminal output schema is the same as the schema with the `--save-summary`
    /// argument when saving to a `summary.json` file. All other output than the json output goes
    /// to stderr and only the summary output goes to stdout. When not printing pretty json, each
    /// line is a dictionary summarizing a single benchmark. You can combine all lines (benchmarks)
    /// into an array for example with `jq`
    ///
    /// `cargo bench -- --output-format=json | jq -s`
    ///
    /// which transforms `{...}\n{...}` into `[{...},{...}]`
    #[arg(
        default_value = "default",
        display_order = 300,
        env = "GUNGRAUN_OUTPUT_FORMAT",
        long = "output-format",
        num_args = 1,
        required = false,
        value_enum,
    )]
    pub output_format: OutputFormatKind,

    #[rustfmt::skip]
    /// Number of benchmarks to run in parallel.
    ///
    /// A value of `1` runs benchmarks serially which is the default if this option is not
    /// specified. Passing `auto` lets the runner choose the parallelism level based on available
    /// hardware which is the number of available logical cores.
    ///
    /// Note that benchmark groups are used as synchronization points and only benchmarks within the
    /// same group are executed in parallel.
    ///
    /// Valgrind and gungraun perform disk I/O even if your benchmarks don't. This is usually a
    /// bottleneck, so running with parallelism of 10 may provide similar speedup as 5. Actual
    /// results depend on the hardware and if your benchmarks are performing disk I/O, too.
    ///
    /// Examples:
    ///   * --parallel=4
    ///   * --parallel=auto
    #[arg(
        default_missing_value = "auto",
        default_value = "1",
        display_order = 100,
        env = "GUNGRAUN_PARALLEL",
        long = "parallel",
        num_args = 0..=1,
        require_equals = true,
        required = false,
        value_parser = parse_parallel,
    )]
    pub parallel: usize,

    #[rustfmt::skip]
    /// Fail the entire benchmark run on the first performance regression
    ///
    /// When enabled, this option causes Gungraun to stop immediately when a performance regression
    /// is detected, rather than continuing to run all benchmarks and reporting regressions at the
    /// end. The program exits with exit code `3` to indicate that one or more regressions
    /// occurred.
    ///
    /// Performance regressions are defined by limits set via `--callgrind-limits`,
    /// `--cachegrind-limits`, `--dhat-limits`, and similar options. Without this option, Gungraun
    /// completes all benchmarks and reports all regressions in a summary at the end.
    ///
    /// See also: `--callgrind-limits`, `--cachegrind-limits`, `--dhat-limits` for defining
    /// regression limits.
    ///
    /// Examples:
    ///   * `--regression-fail-fast` (fail on first regression)
    ///   * `--regression-fail-fast=false` (continue running, report at end - default)
    #[arg(
        default_missing_value = "true",
        display_order = 600,
        env = "GUNGRAUN_REGRESSION_FAIL_FAST",
        long = "regression-fail-fast",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
    )]
    pub regression_fail_fast: Option<bool>,

    #[rustfmt::skip]
    /// Save benchmark results as a named baseline for future comparisons
    ///
    /// If a baseline with this name already exists, Gungraun first compares against it before
    /// overwriting with the new results. If this option is used together with `--baseline`,
    /// Gungraun compares against the baseline selected by `--baseline` and saves the new results
    /// with the name selected by `--save-baseline`.
    ///
    /// This option is useful for creating reference measurements (like from the main branch or a
    /// release tag) that you can later compare against using `--baseline`.
    ///
    /// This option conflicts with `--load-baseline`. See `--baseline` to compare against a saved
    /// baseline without modifying it and `--load-baseline` to compare existing baselines without
    /// running benchmarks.
    ///
    /// Examples:
    ///   * `--save-baseline` (uses the default baseline name "default")
    ///   * `--save-baseline=main` (saves as baseline "main")
    ///   * `--save-baseline=v1.0` (saves as baseline "v1.0")
    ///   * `--save-baseline=pr_1234 --baseline=main` (compares against "main", saves as `pr_1234`)
    #[arg(
        conflicts_with = "LOAD_BASELINE",
        default_missing_value = "default",
        display_order = 200,
        env = "GUNGRAUN_SAVE_BASELINE",
        long = "save-baseline",
        num_args = 0..=1,
        require_equals = true,
    )]
    pub save_baseline: Option<BaselineName>,

    #[rustfmt::skip]
    /// Save a machine-readable summary of each benchmark run to a JSON file
    ///
    /// This option saves a structured JSON summary of each benchmark run alongside the usual
    /// benchmark output. The summary file contains benchmark results, metrics, detected
    /// regressions, and other metadata in a machine-readable format.
    ///
    /// The summary file is saved as `summary.json` in the benchmark's output directory next to the
    /// other usual benchmark output.
    ///
    /// Available formats:
    /// - `json`: Compact JSON without newlines (space-efficient)
    /// - `pretty-json`: Pretty-printed JSON with indentation (human-readable)
    ///
    /// See also `--output-format` for printing JSON summaries to the terminal instead of saving to
    /// a file.
    ///
    /// Examples:
    ///   * `--save-summary` (saves as compact JSON)
    ///   * `--save-summary=json` (saves as compact JSON)
    ///   * `--save-summary=pretty-json` (saves as pretty-printed JSON)
    #[arg(
        default_missing_value = "json",
        display_order = 300,
        env = "GUNGRAUN_SAVE_SUMMARY",
        long = "save-summary",
        num_args = 0..=1,
        require_equals = true,
        value_enum,
    )]
    pub save_summary: Option<SummaryFormat>,

    #[rustfmt::skip]
    /// Separate gungraun benchmark output files by target
    ///
    /// The default output path for files created by Gungraun and Valgrind during the
    /// benchmark is
    ///
    /// `target/gungraun/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`.
    ///
    /// This can be problematic if you're running the benchmarks not only for a single target
    /// because you end up comparing the benchmark runs with the wrong targets. Setting this option
    /// changes the default output path to
    ///
    /// `target/gungraun/$TARGET/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`
    ///
    /// Although not as comfortable and strict, you could achieve a separation by target also with
    /// baselines and a combination of `--save-baseline=$TARGET` and `--baseline=$TARGET` if you
    /// prefer having all files of a single $BENCH in the same directory.
    #[arg(
        action = ArgAction::Set,
        default_missing_value = "true",
        default_value = "false",
        display_order = 100,
        env = "GUNGRAUN_SEPARATE_TARGETS",
        long = "separate-targets",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
    )]
    pub separate_targets: bool,

    #[rustfmt::skip]
    /// Show an ascii grid in the benchmark terminal output
    ///
    /// A matter of taste but the guiding lines can also be helpful reading benchmark output when
    /// running multiple tools with multiple threads and subprocesses for example by using
    /// `--show-intermediate`.
    #[arg(
        default_missing_value = "true",
        display_order = 300,
        env = "GUNGRAUN_SHOW_GRID",
        long = "show-grid",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
    )]
    pub show_grid: Option<bool>,

    #[rustfmt::skip]
    /// Show intermediate metrics from parts, subprocesses, threads, ... (Default: false)
    ///
    /// In Callgrind, threads are treated as separate units (similar to subprocesses) and the
    /// metrics for them are dumped into an own file. Other Valgrind tools usually separate the
    /// output files only by subprocesses. Use this option, to also show the metrics of any
    /// intermediate fragments and not just the total over all of them.
    ///
    /// Temporarily setting `show_intermediate` to `true` can help to find misconfigurations in
    /// multi-thread/multi-process benchmarks.
    #[arg(
        default_missing_value = "true",
        display_order = 300,
        env = "GUNGRAUN_SHOW_INTERMEDIATE",
        long = "show-intermediate",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
    )]
    pub show_intermediate: Option<bool>,

    #[rustfmt::skip]
    /// Show only the comparison between different benchmarks when using `compare_by_id`
    ///
    /// If you're only interested in the comparisons between different benchmarks but not the metric
    /// differences between the self comparisons of the new and old benchmark run, use this option.
    /// This option is only useful if `compare_by_id` is used in the `library_benchmark_group!` or
    /// `binary_benchmark_group!`. Note, that it does not prevent any benchmarks to be run,
    /// especially benchmarks which are not compared to another benchmark. Such benchmarks have only
    /// the usual benchmark headline printed.
    #[arg(
        default_missing_value = "true",
        display_order = 300,
        env = "GUNGRAUN_SHOW_ONLY_COMPARISON",
        long = "show-only-comparison",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        verbatim_doc_comment,
    )]
    pub show_only_comparison: Option<bool>,

    #[rustfmt::skip]
    /// Show changes only when they are above the `tolerance` level
    ///
    /// If no value is specified, the default value of `0.000_009_999_999_999_999_999` is based on
    /// the number of decimal places of the percentages displayed in the terminal output in case of
    /// differences.
    ///
    /// Negative tolerance values are converted to their absolute value.
    ///
    /// Examples:
    ///   * --tolerance (applies the default value)
    ///   * --tolerance=0.1 (set the tolerance level to `0.1`)
    #[arg(
        default_missing_value = "0.000009999999999999999",
        display_order = 300,
        env = "GUNGRAUN_TOLERANCE",
        long = "tolerance",
        num_args = 0..=1,
        require_equals = true,
        verbatim_doc_comment,
    )]
    pub tolerance: Option<f64>,

    #[rustfmt::skip]
    /// Specify an alternative executable to run a tool invocation
    ///
    /// By default, Gungraun runs the selected tool directly. This option allows specifying an
    /// alternative runner executable that will be invoked instead, with the selected tool binary
    /// passed as an argument to the runner.
    ///
    /// When specified, the runner is invoked as:
    ///   `<RUNNER> [RUNNER_ARGS...] <TOOL_BIN> [TOOL_ARGS...] <BENCHMARK> [BENCHMARK_ARGS...]`
    ///
    /// The runner receives extra environment variables that provide context:
    /// - `GUNGRAUN_TR_DEST_DIR`: The destination directory for tool output files
    /// - `GUNGRAUN_TR_HOME`: The gungraun home (`--home`) directory
    /// - `GUNGRAUN_TR_WORKSPACE_ROOT`: The project's workspace root directory
    /// - `GUNGRAUN_ALLOW_ASLR`: `yes` or `no` (the default) based on `--allow-aslr` setting
    ///
    /// Environment variables in `--tool-runner-args` are interpolated using `${VAR}` syntax.
    /// The interpolation priority is: `GUNGRAUN_TR_*` variables first, then `--envs` variables,
    /// then the system environment.
    ///
    /// This is useful for running benchmarks in containers or other environments where the tool is
    /// not available on the host. See the online guide for detailed examples.
    ///
    /// Examples:
    ///   * --tool-runner=docker
    ///   * --tool-runner=/path/to/wrapper --tool-runner-args='--some-flag=${GUNGRAUN_ALLOW_ASLR}'
    #[arg(
        display_order = 150,
        env = "GUNGRAUN_TOOL_RUNNER",
        long = "tool-runner",
        num_args = 1,
        value_parser = PathBufValueParser::new().try_map(parse_path_resolved),
        verbatim_doc_comment,
    )]
    pub tool_runner: Option<PathBuf>,

    #[rustfmt::skip]
    /// Additional arguments to pass to the tool runner executable
    ///
    /// This option is only effective when `--tool-runner` is specified. The arguments are passed
    /// to the runner executable after `--tool-runner` and before the tool path.
    ///
    /// Environment variable interpolation is supported using the `${VAR}` syntax. Variables are
    /// resolved in this order:
    /// 1. `GUNGRAUN_TR_*` variables set by Gungraun (see `--tool-runner` for the list)
    /// 2. Variables specified via `--envs` and `LibraryBenchmarkConfig::envs` or
    ///    `BinaryBenchmarkConfig::envs`
    /// 3. System environment variables
    ///
    /// The interpolation allows passing dynamic values to the runner based on Gungraun's
    /// configuration. For example, `${GUNGRAUN_ALLOW_ASLR}` interpolation is useful for passing
    /// the ASLR setting to container setups.
    ///
    /// Examples:
    ///   * --tool-runner=sudo --tool-runner-args='--user=foo'
    ///   * --tool-runner=wrapper '--tool-runner-args=--allow-aslr=${GUNGRAUN_ALLOW_ASLR}'
    #[arg(
        action = ArgAction::Append,
        display_order = 150,
        env = "GUNGRAUN_TOOL_RUNNER_ARGS",
        long = "tool-runner-args",
        num_args = 1,
        required = false,
        requires = "tool_runner",
        value_parser = parse_raw_args,
        verbatim_doc_comment,
    )]
    pub tool_runner_args: Vec<RawArgs>,

    #[rustfmt::skip]
    /// Override the destination directory path for tool runner output files
    ///
    /// This option is only effective when `--tool-runner` is specified. By default, tool output
    /// files are written to paths under the gungraun home directory or in temporary directories.
    /// This option allows substituting this path with a custom directory.
    ///
    /// When specified, any occurrence of this path prefix in tool arguments will be replaced with
    /// the directory path specified by `--tool-runner-dest`.
    ///
    /// WARNING: Make sure the directory of this argument exists, is empty and doesn't point to a
    /// directory with important files in it! This directory is managed by Gungraun and Gungraun
    /// might delete **all** files in this directory. More details can be found in the online
    /// guide.
    ///
    /// Examples:
    ///   * `--tool-runner-dest=/tmp/results`
    #[arg(
        display_order = 150,
        env = "GUNGRAUN_TOOL_RUNNER_DEST",
        long = "tool-runner-dest",
        num_args = 1,
        requires = "tool_runner",
        verbatim_doc_comment,
    )]
    pub tool_runner_dest: Option<PathBuf>,

    #[rustfmt::skip]
    /// Override the workspace root path for the tool runner
    ///
    /// This option is only effective when `--tool-runner` is specified. It allows substituting the
    /// workspace root path prefix in the benchmark executable path and all other tool arguments.
    ///
    /// This can be useful for container setups where the workspace is mounted at a different
    /// location inside the container.
    ///
    /// Examples:
    ///   * `--tool-runner-root=/workspace`
    #[arg(
        display_order = 150,
        env = "GUNGRAUN_TOOL_RUNNER_ROOT",
        long = "tool-runner-root",
        num_args = 1,
        requires = "tool_runner",
        verbatim_doc_comment,
    )]
    pub tool_runner_root: Option<PathBuf>,

    #[rustfmt::skip]
    /// A comma separated list of tools to run additionally to Callgrind or another default tool
    ///
    /// The tools specified here take precedence over the tools in the benchmarks. The Valgrind
    /// tools which are allowed here are the same as the ones listed in the documentation of
    /// --default-tool.
    ///
    /// Examples
    ///   * --tools dhat
    ///   * --tools memcheck,drd
    #[arg(
        display_order = 450,
        env = "GUNGRAUN_TOOLS",
        long = "tools",
        num_args = 1..,
        value_delimiter = ',',
        verbatim_doc_comment,
    )]
    pub tools: Vec<Tool>,

    #[rustfmt::skip]
    /// Adjust, enable or disable the truncation of the description in the Gungraun output
    ///
    /// The default is to truncate the description to the size of 50 ascii characters. A false
    /// value disables the truncation entirely and a value will truncate the description to the
    /// given amount of characters excluding the ellipsis.
    ///
    /// To clarify which part of the output is meant by `DESCRIPTION`:
    ///
    /// ```text
    /// benchmark_file::group_name::function_name id:DESCRIPTION
    ///   Instructions:              352135|352135          (No change)
    ///   ...
    /// ```
    ///
    /// Examples:
    ///   * --truncate-description=no (disables truncation)
    ///   * --truncate-description=100 (set the truncation to 100 ascii chars)
    ///   * --truncate-description (this is the default and sets the size of 50 ascii chars)
    #[arg(
        default_missing_value = "50",
        display_order = 300,
        env = "GUNGRAUN_TRUNCATE_DESCRIPTION",
        long = "truncate-description",
        num_args = 0..=1,
        require_equals = true,
        value_parser = parse_truncate_description,
        verbatim_doc_comment,
    )]
    pub truncate_description: Option<TruncateDescription>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to all tools
    ///
    /// The core Valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options> which are
    /// recognized by all tools. More specific arguments for example set with --callgrind-args
    /// override the arguments with the same name specified with this option.
    ///
    /// Examples:
    ///   * --valgrind-args=--time-stamp=yes
    ///   * --valgrind-args='--error-exitcode=202 --num-callers=50'
    #[arg(
        display_order = 500,
        env = "GUNGRAUN_VALGRIND_ARGS",
        long = "valgrind-args",
        num_args = 1,
        value_parser = parse_tool_args,
        verbatim_doc_comment,
    )]
    pub valgrind_args: Option<RawToolArgs>,

    // TODO: Add perf_bin similar to this valgrind_bin
    #[rustfmt::skip]
    /// Specify the path to the Valgrind executable
    ///
    /// By default, Gungraun searches for `valgrind` in the system PATH. This option
    /// allows specifying an alternative Valgrind executable. When used with
    /// `--tool-runner`, this path is passed to the runner as the Valgrind binary
    /// to invoke.
    ///
    /// Note: The specified path is not validated for existence. If the path is invalid, the
    /// benchmark will fail when attempting to execute Valgrind.
    ///
    /// Examples:
    ///   * `--valgrind-bin=/usr/local/bin/valgrind`
    ///   * `--valgrind-bin=/doesnotexist` (used with `--tool-runner` for container setups)
    #[arg(
        display_order = 100,
        env = "GUNGRAUN_VALGRIND_BIN",
        long = "valgrind-bin",
        num_args = 1,
        verbatim_doc_comment,
    )]
    pub valgrind_bin: Option<PathBuf>,

    /// Override the Cargo workspace root
    ///
    /// By default, Gungraun queries `cargo metadata` to detect the workspace root. This is usually
    /// correct, but this option provides the workspace root explicitly and avoids that part of the
    /// Cargo metadata query.
    ///
    /// To run without invoking `cargo` at all, also provide an explicit output location with
    /// `--home`/`GUNGRAUN_HOME` or set `CARGO_TARGET_DIR`. If neither is provided, Gungraun still
    /// queries `cargo metadata` to detect Cargo's effective target directory.
    ///
    /// This can be useful when `gungraun-runner` executes in an environment where `cargo` is not
    /// available, such as a container, VM, or emulator image.
    ///
    /// Examples:
    ///   * `--workspace-root=/project`
    ///   * `GUNGRAUN_WORKSPACE_ROOT=/project GUNGRAUN_HOME=/tmp/gungraun cargo bench`
    #[arg(
        display_order = 100,
        env = "GUNGRAUN_WORKSPACE_ROOT",
        long = "workspace-root",
        num_args = 1,
        verbatim_doc_comment
    )]
    pub workspace_root: Option<PathBuf>,
}

/// A wrapper type for raw command-line arguments
///
/// Stores a list of raw string arguments without special processing or validation. Used for
/// arguments passed through to external executables without modification, particularly for
/// `--tool-runner-args`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawArgs(Vec<String>);

impl CommandLineArgs {
    /// Parses command-line arguments and exits on parsing or validation errors.
    pub fn parse_validated_from<I, T>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::try_parse_validated_from(iter).unwrap_or_else(|error| error.exit())
    }

    /// Parses command-line arguments and validates relationships between parsed options.
    pub fn try_parse_validated_from<I, T>(iter: I) -> clap::error::Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::try_parse_from(iter).and_then(|args| args.validate().map(|()| args))
    }

    fn validate(&self) -> clap::error::Result<()> {
        if let (Some(save_baseline), Some(baseline)) = (&self.save_baseline, &self.baseline) {
            if save_baseline == baseline {
                let mut cmd = Self::command();
                return Err(cmd.error(
                    clap::error::ErrorKind::ValueValidation,
                    format!(
                        "--save-baseline and --baseline cannot use the same baseline name; use \
                         only --save-baseline={save_baseline} instead"
                    ),
                ));
            }
        }

        Ok(())
    }
}

impl BenchmarkFilter {
    /// Return `true` if the filter matches the haystack
    pub fn apply(&self, haystack: &str) -> bool {
        let Self::WildcardPattern(pattern) = self;
        pattern.as_str().dowild_with(haystack, DOWILD_OPTIONS)
    }
}

impl FromStr for BenchmarkFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::WildcardPattern(s.to_owned()))
    }
}

impl NoCapture {
    /// Apply the `NoCapture` option to the [`Command`]
    pub fn apply(
        self,
        command: &mut Command,
        captured_output: Option<&CapturedOutput>,
    ) -> Result<()> {
        match (self, captured_output) {
            (Self::True, Some(captured_output)) => {
                // Both go to the same file, here chosen to be stdout
                command
                    .stdout(captured_output.stdout.try_clone()?)
                    .stderr(captured_output.stdout.try_clone()?);
            }
            (Self::False, Some(captured_output)) => {
                command
                    .stdout(captured_output.stdout.try_clone()?)
                    .stderr(captured_output.stderr.try_clone()?);
            }
            (Self::Stderr, Some(captured_output)) => {
                command
                    .stdout(Stdio::null())
                    .stderr(captured_output.stderr.try_clone()?);
            }
            (Self::Stdout, Some(captured_output)) => {
                command
                    .stdout(captured_output.stdout.try_clone()?)
                    .stderr(Stdio::null());
            }
            (Self::True, None) => {
                command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
            }
            (Self::False, None) => {
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
            }
            (Self::Stderr, None) => {
                command.stdout(Stdio::null()).stderr(Stdio::inherit());
            }
            (Self::Stdout, None) => {
                command.stdout(Stdio::inherit()).stderr(Stdio::null());
            }
        }

        Ok(())
    }
}

impl From<TruncateDescription> for Option<usize> {
    fn from(value: TruncateDescription) -> Self {
        match value {
            TruncateDescription::To(to) => Some(to),
            TruncateDescription::None => None,
        }
    }
}

impl RawArgs {
    /// Returns a slice of the underlying argument strings
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Return `true` if there are no elements in this `RawArgs`
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return the amount of elements
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

// Convert the `metric` if it is present
//
// Used for example for hard limits to convert u64 values to f64 values if required.
fn convert_metric<T: Display + TypeChecker + Copy>(
    metric_kind: T,
    metric: Option<Metric>,
) -> Result<(T, Option<Metric>), String> {
    if let Some(metric) = metric {
        metric
            .try_convert(metric_kind)
            .ok_or_else(|| {
                format!(
                    "Invalid hard limit for '{metric_kind}': Expected an integer (e.g. '10'). If \
                     you wanted this value to be a soft limit use the '%' suffix (e.g. '4.0%' or \
                     '4%')"
                )
            })
            .map(|(t, m)| (t, Some(m)))
    } else {
        Ok((metric_kind, None))
    }
}

/// Same as `parse_callgrind_limits` but for Cachegrind
fn parse_cachegrind_limits(value: &str) -> Result<ToolRegressionConfig, String> {
    let (soft_limits, hard_limits) = parse_limits(value, |key, metric| {
        let metrics = key
            .parse::<CachegrindMetrics>()
            .map_err(|error| error.to_string())?;
        IndexSet::from(metrics)
            .into_iter()
            .map(|metric_kind| convert_metric(metric_kind, metric))
            .collect::<ParsedMetrics<CachegrindMetric>>()
    })?;

    let config = ToolRegressionConfig::Cachegrind(CachegrindRegressionConfig {
        soft_limits: soft_limits.into_iter().collect(),
        hard_limits: hard_limits.into_iter().collect(),
        ..Default::default()
    });

    Ok(config)
}

/// Parse the Cachegrind metrics
fn parse_cachegrind_metrics(value: &str) -> Result<IndexSet<CachegrindMetric>, String> {
    parse_tool_metrics(value, |item| {
        item.parse::<CachegrindMetrics>()
            .map(IndexSet::from)
            .map_err(|error| error.to_string())
    })
}

/// Parse the Callgrind limits from the command-line
///
/// This method (and the other `parse_dhat_limits`, ...) parses soft and hard limits in one go. The
/// format is described in the --help message above in [`CommandLineArgs`].
///
/// In order to avoid back and forth conversions between `api::ToolRegressionConfig` and
/// `tool::ToolRegressionConfig` we parse the `tool::ToolRegressionConfig` directly.
fn parse_callgrind_limits(value: &str) -> Result<ToolRegressionConfig, String> {
    let (soft_limits, hard_limits) = parse_limits(value, |key, metric| {
        let metrics = key
            .parse::<CallgrindMetrics>()
            .map_err(|error| error.to_string())?;
        IndexSet::from(metrics)
            .into_iter()
            .map(|event_kind| convert_metric(event_kind, metric))
            .collect::<ParsedMetrics<EventKind>>()
    })?;

    let config = ToolRegressionConfig::Callgrind(CallgrindRegressionConfig {
        soft_limits: soft_limits.into_iter().collect(),
        hard_limits: hard_limits.into_iter().collect(),
        ..Default::default()
    });

    Ok(config)
}

/// Parse the Callgrind metrics
fn parse_callgrind_metrics(value: &str) -> Result<IndexSet<EventKind>, String> {
    parse_tool_metrics(value, |item| {
        item.parse::<CallgrindMetrics>()
            .map(IndexSet::from)
            .map_err(|error| error.to_string())
    })
}

/// Same as `parse_callgrind_limits` but for dhat
fn parse_dhat_limits(value: &str) -> Result<ToolRegressionConfig, String> {
    let (soft_limits, hard_limits) = parse_limits(value, |key, metric| {
        let metrics = key
            .parse::<DhatMetrics>()
            .map_err(|error| error.to_string())?;
        IndexSet::from(metrics)
            .into_iter()
            .map(|metric_kind| convert_metric(metric_kind, metric))
            .collect::<ParsedMetrics<DhatMetric>>()
    })?;

    let config = ToolRegressionConfig::Dhat(DhatRegressionConfig {
        soft_limits: soft_limits.into_iter().collect(),
        hard_limits: hard_limits.into_iter().collect(),
        ..Default::default()
    });

    Ok(config)
}

/// Parse the DHAT metrics
fn parse_dhat_metrics(value: &str) -> Result<IndexSet<DhatMetric>, String> {
    parse_tool_metrics(value, |item| {
        item.parse::<DhatMetrics>()
            .map(IndexSet::from)
            .map_err(|error| error.to_string())
    })
}

/// Parse the DRD metrics as error metrics
fn parse_drd_metrics(value: &str) -> Result<IndexSet<ErrorMetric>, String> {
    parse_tool_metrics(value, parse_error_metrics)
}

/// Parse environment variable `key=value` pairs and resolve standalone keys
fn parse_envs(value: &str) -> Result<Vec<(OsString, OsString)>, String> {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
        .or_else(|| trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
        .unwrap_or(trimmed);

    let splits = shlex::split(trimmed)
        .ok_or_else(|| format!("Failed splitting '{value}' for POSIX shell environment"))?;

    let mut result = vec![];
    for split in splits {
        if let Some((key, equals_value)) = split.split_once('=') {
            if key.is_empty() {
                return Err(format!("Empty key for value: '{equals_value}'"));
            }

            result.push((OsString::from(key), OsString::from(equals_value)));
        } else if let Some(env_value) = std::env::var_os(&split) {
            result.push((OsString::from(split), env_value));
        } else {
            // do nothing
        }
    }

    Ok(result)
}

fn parse_error_metrics(item: &str) -> Result<IndexSet<ErrorMetric>, String> {
    if let Some(prefix) = item.strip_prefix('@') {
        if prefix == "all" {
            Ok(ErrorMetric::iter().fold(IndexSet::new(), |mut acc, elem| {
                acc.insert(elem);
                acc
            }))
        } else {
            Err(format!("Invalid error metric group: '{item}"))
        }
    } else {
        let metric = item
            .parse::<ErrorMetric>()
            .map_err(|error| error.to_string())?;
        Ok(indexset! { metric })
    }
}

/// Parse the helgrind metrics as error metrics
fn parse_helgrind_metrics(value: &str) -> Result<IndexSet<ErrorMetric>, String> {
    parse_tool_metrics(value, parse_error_metrics)
}

/// Parse the value of the hidden `--format` libtest-compat shim.
///
/// Only `terse` actually changes behavior. `pretty`, `json`, `junit`, the empty string and any
/// unknown value all map to [`ListFormat::Pretty`] so we keep silently accepting values we don't
/// natively support (matching the previous `Vec<String>` accept-all behavior).
#[expect(
    clippy::unnecessary_wraps,
    reason = "clap value_parser requires Result return type"
)]
fn parse_list_format(value: &str) -> Result<ListFormat, String> {
    match value {
        "terse" => Ok(ListFormat::Terse),
        _ => Ok(ListFormat::Pretty),
    }
}

fn parse_limits<T: Eq + Hash>(
    value: &str,
    parse_metrics: fn(&str, Option<Metric>) -> ParsedMetrics<T>,
) -> Result<Limits<T>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("No limits found: At least one limit must be present".to_owned());
    }

    let mut soft_limits = IndexMap::new();
    let mut hard_limits = IndexMap::new();

    for item in value.split(',') {
        let item = item.trim();

        if let Some((key, value)) = item.split_once('=') {
            let (key, value) = (key.trim(), value.trim());
            for split in value.split('|') {
                let split = split.trim();

                if let Some(prefix) = split.strip_suffix('%') {
                    let pct = prefix.parse::<f64>().map_err(|error| -> String {
                        format!("Invalid soft limit for '{key}': {error}")
                    })?;
                    let metric_kinds = parse_metrics(key, None)?;
                    for (metric_kind, _) in metric_kinds {
                        soft_limits.insert(metric_kind, pct);
                    }
                } else {
                    let metric = split.parse::<Metric>().map_err(|error| -> String {
                        format!("Invalid hard limit for '{key}': {error}")
                    })?;
                    let metric_kinds = parse_metrics(key, Some(metric))?;
                    for (metric_kind, new_metric) in metric_kinds {
                        if let Some(new_metric) = new_metric {
                            hard_limits.insert(metric_kind, new_metric);
                        } else {
                            hard_limits.insert(metric_kind, metric);
                        }
                    }
                }
            }
        } else {
            return Err(format!("Invalid format of key=value pair: '{item}'"));
        }
    }

    Ok((soft_limits, hard_limits))
}

/// Parse the Memcheck metrics as error metrics
fn parse_memcheck_metrics(value: &str) -> Result<IndexSet<ErrorMetric>, String> {
    parse_tool_metrics(value, parse_error_metrics)
}

/// Parse --nocapture
fn parse_nocapture(value: &str) -> Result<NoCapture, String> {
    // Taken from clap source code
    const TRUE_LITERALS: [&str; 6] = ["y", "yes", "t", "true", "on", "1"];
    const FALSE_LITERALS: [&str; 6] = ["n", "no", "f", "false", "off", "0"];

    let lowercase = value.to_lowercase();

    if TRUE_LITERALS.contains(&lowercase.as_str()) {
        Ok(NoCapture::True)
    } else if FALSE_LITERALS.contains(&lowercase.as_str()) {
        Ok(NoCapture::False)
    } else if lowercase == "stdout" {
        Ok(NoCapture::Stdout)
    } else if lowercase == "stderr" {
        Ok(NoCapture::Stderr)
    } else {
        Err(format!("Invalid value: {value}"))
    }
}

/// Parse --parallel
fn parse_parallel(value: &str) -> Result<usize, String> {
    let lowercase = value.to_lowercase();

    if lowercase == "auto" {
        Ok(num_cpus::get())
    } else if let Ok(num) = lowercase.parse::<usize>() {
        if num > 0 {
            Ok(num)
        } else {
            Err(format!("Value must be greater than 0 but was '{value}'"))
        }
    } else {
        Err(format!("Invalid value: {value}"))
    }
}

fn parse_path_resolved(value: PathBuf) -> Result<PathBuf, String> {
    util::resolve_binary_path(value, None).map_err(|error| error.to_string())
}

/// This function parses a space separated list of raw argument strings into [`RawArgs`]
fn parse_raw_args(value: &str) -> Result<RawArgs, String> {
    let value = if value.is_empty() {
        return Err(String::from("Empty arguments"));
    } else if value.len() >= 2 {
        match (&value.as_bytes()[0], &value.as_bytes()[value.len() - 1]) {
            (b'\'', b'\'') | (b'"', b'"') => &value[1..value.len() - 1],
            _ => value,
        }
    } else {
        value
    };

    shlex::split(value)
        .ok_or_else(|| "Failed to split args".to_owned())
        .map(RawArgs)
}

/// This function parses a space separated list of raw argument strings into
/// [`crate::api::RawToolArgs`]
fn parse_tool_args(value: &str) -> Result<RawToolArgs, String> {
    parse_raw_args(value).map(|r| RawToolArgs::new_ignore_flag(r.0))
}

/// Utility function to parse the --callgrind-metrics, ...
fn parse_tool_metrics<T: Eq + Hash>(
    value: &str,
    parse_metrics: fn(&str) -> Result<IndexSet<T>, String>,
) -> Result<IndexSet<T>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("No metric found: At least one metric must be present".to_owned());
    }

    let mut format = IndexSet::new();

    for item in value.split(',') {
        let item = item.trim();
        let metrics = parse_metrics(item)?;
        format.extend(metrics);
    }

    Ok(format)
}

fn parse_truncate_description(value: &str) -> Result<TruncateDescription, String> {
    // Almost the same as the BoolishValueParser but without `1` and `0` which are interpreted as
    // values. The FALSE_LITERALS also contain `none` as special value.
    const TRUE_LITERALS: [&str; 5] = ["y", "yes", "t", "true", "on"];
    const FALSE_LITERALS: [&str; 6] = ["n", "no", "none", "f", "false", "off"];

    let lowercase = value.to_lowercase();

    if TRUE_LITERALS.contains(&lowercase.as_str()) {
        Ok(TruncateDescription::To(50))
    } else if FALSE_LITERALS.contains(&lowercase.as_str()) {
        Ok(TruncateDescription::None)
    } else if let Ok(parsed) = lowercase.parse::<usize>() {
        Ok(TruncateDescription::To(parsed))
    } else {
        Err(format!("Invalid value: {value}"))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    use rstest::rstest;
    use tempfile::{NamedTempFile, tempdir};

    use super::*;
    use crate::api::EventKind::*;
    use crate::api::RawToolArgs;

    #[rstest]
    #[case::single_key_value("--some=yes", &["--some=yes"])]
    #[case::two_key_value("--some=yes --other=no", &["--some=yes", "--other=no"])]
    #[case::single_escaped("--some='yes and no'", &["--some=yes and no"])]
    #[case::double_escaped("--some='\"yes and no\"'", &["--some=\"yes and no\""])]
    #[case::multiple_escaped(
        "--some='yes and no' --other='no and yes'",
        &["--some=yes and no", "--other=no and yes"]
    )]
    fn test_parse_tool_args(#[case] value: &str, #[case] expected: &[&str]) {
        let actual = parse_tool_args(value).unwrap();
        assert_eq!(actual, RawToolArgs::from_iter_ignore_flag(expected));
    }

    #[test]
    fn test_parse_tool_args_when_empty_then_error() {
        parse_tool_args("").unwrap_err();
    }

    #[test]
    fn test_save_baseline_with_baseline_is_allowed() {
        let result = CommandLineArgs::try_parse_validated_from([
            "--save-baseline=pr_1234",
            "--baseline=main_2025_01_02",
        ])
        .unwrap();

        assert_eq!(
            result.save_baseline,
            Some(BaselineName("pr_1234".to_owned()))
        );
        assert_eq!(
            result.baseline,
            Some(BaselineName("main_2025_01_02".to_owned()))
        );
    }

    #[test]
    fn test_save_baseline_with_same_baseline_is_rejected() {
        let error =
            CommandLineArgs::try_parse_validated_from(["--save-baseline=main", "--baseline=main"])
                .unwrap_err();

        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
        assert!(
            error
                .to_string()
                .contains("use only --save-baseline=main instead")
        );
    }

    #[test]
    fn test_save_baseline_with_load_baseline_is_rejected() {
        CommandLineArgs::try_parse_from(["--save-baseline=pr_1234", "--load-baseline=main"])
            .unwrap_err();
    }

    #[rstest]
    #[case::single_soft("Ir=10%", vec![(Ir, 10f64)], vec![])]
    #[case::single_hard("Ir=20", vec![], vec![(Ir, 20.into())])]
    #[case::soft_and_hard("Ir=20|10%", vec![(Ir, 10f64)], vec![(Ir, 20.into())])]
    #[case::soft_and_hard_separated("Ir=20, Ir=10%", vec![(Ir, 10f64)], vec![(Ir, 20.into())])]
    #[case::soft_overwrite("Ir=20%, Ir=10%", vec![(Ir, 10f64)], vec![])]
    #[case::hard_overwrite("Ir=20, Ir=10", vec![], vec![(Ir, 10.into())])]
    #[case::group_wb_soft("@wb=10%", vec![(ILdmr, 10f64), (DLdmr, 10f64), (DLdmw, 10f64)], vec![])]
    #[case::group_writeback_soft(
        "@writeback=10%",
        vec![(ILdmr, 10f64), (DLdmr, 10f64), (DLdmw, 10f64)],
        vec![]
    )]
    #[case::group_writebackbehaviour_soft(
        "@writebackbehaviour=10%",
        vec![(ILdmr, 10f64), (DLdmr, 10f64), (DLdmw, 10f64)],
        vec![]
    )]
    #[case::group_hr_hard_int(
        "@hr=10",
        vec![],
        vec![(L1HitRate, 10f64.into()), (LLHitRate, 10f64.into()), (RamHitRate, 10f64.into())]
    )]
    #[case::group_hr_hard_float(
        "@hr=10.0",
        vec![],
        vec![(L1HitRate, 10f64.into()), (LLHitRate, 10f64.into()), (RamHitRate, 10f64.into())]
    )]
    #[case::case_insensitive(
        "EstIMATedCycles=10%",
        vec![(EstimatedCycles, 10f64)],
        vec![]
    )]
    #[case::multiple_soft(
        "Ir=10%,EstimatedCycles=5%",
        vec![(Ir, 10f64), (EstimatedCycles, 5f64)],
        vec![]
    )]
    #[case::multiple_hard(
        "Ir=20,EstimatedCycles=50",
        vec![],
        vec![(Ir, 20.into()), (EstimatedCycles, 50.into())]
    )]
    #[case::with_whitespace(
        "Ir= 10% , EstimatedCycles = 5%",
        vec![(Ir, 10f64), (EstimatedCycles, 5f64)],
        vec![]
    )]
    fn test_parse_callgrind_limits(
        #[case] regression_var: &str,
        #[case] expected_soft_limits: Vec<(EventKind, f64)>,
        #[case] expected_hard_limits: Vec<(EventKind, Metric)>,
    ) {
        if let ToolRegressionConfig::Callgrind(CallgrindRegressionConfig {
            soft_limits,
            hard_limits,
            ..
        }) = parse_callgrind_limits(regression_var).unwrap()
        {
            assert_eq!(soft_limits, expected_soft_limits);
            assert_eq!(hard_limits, expected_hard_limits);
        } else {
            panic!("Wrong regression config");
        }
    }

    #[rstest]
    #[case::regression_wrong_format_of_key_value_pair(
        "Ir:10",
        "Invalid format of key=value pair: 'Ir:10'"
    )]
    #[case::regression_unknown_event_kind("WRONG=10", "Unknown event kind: 'WRONG'")]
    #[case::float_instead_of_integer(
        "Ir=10.0",
        "Invalid hard limit for 'Instructions': Expected an integer (e.g. '10'). If you wanted \
         this value to be a soft limit use the '%' suffix (e.g. '4.0%' or '4%')"
    )]
    #[case::regression_invalid_percentage(
        "Ir=10.0.0",
        "Invalid hard limit for 'Ir': Invalid metric: invalid float literal"
    )]
    #[case::invalid_soft_limit("Ir=abc%", "Invalid soft limit for 'Ir': invalid float literal")]
    #[case::regression_empty_limits("", "No limits found: At least one limit must be present")]
    fn test_parse_callgrind_limits_then_error(
        #[case] regression_var: &str,
        #[case] expected_reason: &str,
    ) {
        assert_eq!(
            &parse_callgrind_limits(regression_var).unwrap_err(),
            expected_reason,
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_callgrind_args_env() {
        let test_arg = "--just-testing=yes";
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_CALLGRIND_ARGS", test_arg);
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.callgrind_args,
            Some(RawToolArgs::new_ignore_flag(vec![test_arg.to_owned()]))
        );
    }

    #[rstest]
    #[case::without_flag("--callgrind-args=foo", &["--foo"])]
    #[case::with_flag("--callgrind-args=--foo", &["--foo"])]
    #[case::without_flag_and_quotes("--callgrind-args='foo'", &["--foo"])]
    #[case::with_flag_and_quotes("--callgrind-args='--foo'", &["--foo"])]
    #[case::with_equals("--callgrind-args=--foo=bar", &["--foo=bar"])]
    #[case::two_flags("--callgrind-args='--foo=bar --bar=baz'", &["--foo=bar", "--bar=baz"])]
    #[case::two_without_flags("--callgrind-args='foo=bar bar=baz'", &["--foo=bar", "--bar=baz"])]
    fn test_callgrind_args_not_env(#[case] input: &str, #[case] expected: &[&str]) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(
            result.callgrind_args,
            Some(RawToolArgs::new_ignore_flag(
                expected.iter().map(ToOwned::to_owned)
            ))
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_callgrind_args_cli_takes_precedence_over_env() {
        let test_arg_yes = "--just-testing=yes";
        let test_arg_no = "--just-testing=no";
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_CALLGRIND_ARGS", test_arg_yes);
        }
        let result = CommandLineArgs::parse_from([format!("--callgrind-args={test_arg_no}")]);
        assert_eq!(
            result.callgrind_args,
            Some(RawToolArgs::new_ignore_flag(vec![test_arg_no.to_owned()]))
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_save_summary_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_SAVE_SUMMARY", "json");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.save_summary, Some(SummaryFormat::Json));
    }

    #[rstest]
    #[case::default("", SummaryFormat::Json)]
    #[case::json("json", SummaryFormat::Json)]
    #[case::pretty_json("pretty-json", SummaryFormat::PrettyJson)]
    fn test_save_summary_cli(#[case] value: &str, #[case] expected: SummaryFormat) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--save-summary".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--save-summary={value}")])
        };
        assert_eq!(result.save_summary, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_allow_aslr_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_ALLOW_ASLR", "yes");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.allow_aslr, Some(true));
    }

    #[rstest]
    #[case::default("", true)]
    #[case::yes("yes", true)]
    #[case::no("no", false)]
    fn test_allow_aslr_cli(#[case] value: &str, #[case] expected: bool) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--allow-aslr".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--allow-aslr={value}")])
        };
        assert_eq!(result.allow_aslr, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_separate_targets_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_SEPARATE_TARGETS", "yes");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert!(result.separate_targets);
    }

    #[rstest]
    #[case::default("", true)]
    #[case::yes("yes", true)]
    #[case::no("no", false)]
    fn test_separate_targets_cli(#[case] value: &str, #[case] expected: bool) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--separate-targets".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--separate-targets={value}")])
        };
        assert_eq!(result.separate_targets, expected);
    }

    #[test]
    #[serial_test::serial]
    fn test_home_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_HOME", "/tmp/my_gungraun_home");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.home, Some(PathBuf::from("/tmp/my_gungraun_home")));
    }

    #[test]
    fn test_home_cli() {
        let result = CommandLineArgs::parse_from(["--home=/test_me".to_owned()]);
        assert_eq!(result.home, Some(PathBuf::from("/test_me")));
    }

    #[test]
    fn test_home_cli_when_no_value_then_error() {
        let result = CommandLineArgs::try_parse_from(["--home=".to_owned()]);
        result.unwrap_err();
    }

    #[rstest]
    #[case::default("", NoCapture::True)]
    #[case::yes("true", NoCapture::True)]
    #[case::no("false", NoCapture::False)]
    #[case::stdout("stdout", NoCapture::Stdout)]
    #[case::stderr("stderr", NoCapture::Stderr)]
    fn test_nocapture_cli(#[case] value: &str, #[case] expected: NoCapture) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--nocapture".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--nocapture={value}")])
        };
        assert_eq!(result.nocapture, expected);
    }

    #[test]
    #[serial_test::serial]
    fn test_nocapture_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_NOCAPTURE", "true");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.nocapture, NoCapture::True);
    }

    #[rstest]
    #[case::single("drd", &[Tool::DRD])]
    #[case::two("drd,callgrind", &[Tool::DRD, Tool::Callgrind])]
    fn test_tools_cli(#[case] tools: &str, #[case] expected: &[Tool]) {
        let actual = CommandLineArgs::parse_from([format!("--tools={tools}")]);
        assert_eq!(actual.tools, expected);
    }

    #[rstest]
    #[case::y("y", true)]
    #[case::yes("yes", true)]
    #[case::t("t", true)]
    #[case::true_value("true", true)]
    #[case::on("on", true)]
    #[case::one("1", true)]
    #[case::n("n", false)]
    #[case::no("no", false)]
    #[case::f("f", false)]
    #[case::false_value("false", false)]
    #[case::off("off", false)]
    #[case::zero("0", false)]
    fn test_boolish(#[case] value: &str, #[case] expected: bool) {
        let result = CommandLineArgs::parse_from(&[format!("--allow-aslr={value}")]);
        assert_eq!(result.allow_aslr, Some(expected));
    }

    #[rstest]
    #[case::include_ignored("--include-ignored", "")]
    #[case::ignored("--ignored", "")]
    #[case::force_run_in_process("--force-run-in-process", "")]
    #[case::exclude_should_panic("--exclude-should-panic", "")]
    #[case::test("--test", "")]
    #[case::bench("--bench", "")]
    #[case::logfile_without_arg("--logfile", "")]
    #[case::logfile_with_arg("--logfile", "/some/path")]
    #[case::test_threads("--test-threads", "")]
    #[case::skip_without_arg("--skip", "")]
    #[case::skip_with_arg("--skip", "some::test")]
    #[case::quiet_short("-q", "")]
    #[case::quiet_long("--quiet", "")]
    #[case::exact("--exact", "")]
    #[case::color_without_arg("--color", "")]
    #[case::color_with_arg("--color", "auto")]
    #[case::format_without_arg("--format", "")]
    #[case::format_with_arg("--format", "terse")]
    #[case::show_output("--show-output", "")]
    #[case::z_without_arg("-Z", "")]
    #[case::z_with_arg("-Z", "unstable-options")]
    #[case::report_time("--report-time", "")]
    #[case::ensure_time("--ensure-time", "")]
    #[case::shuffle("--shuffle", "")]
    #[case::shuffle_seed_without_arg("--shuffle-seed", "")]
    #[case::shuffle_seed_with_arg("--shuffle-seed", "123")]
    fn test_when_libtest_arg_then_no_exit_with_error(#[case] arg: &str, #[case] value: &str) {
        let result = if value.is_empty() {
            CommandLineArgs::try_parse_from([arg])
        } else {
            CommandLineArgs::try_parse_from(&[format!("{arg}={value}")])
        };

        result.unwrap();
    }

    #[rstest]
    #[case::parse_terse("terse", ListFormat::Terse)]
    #[case::parse_pretty("pretty", ListFormat::Pretty)]
    #[case::parse_empty("", ListFormat::Pretty)]
    #[case::parse_json("json", ListFormat::Pretty)]
    #[case::parse_junit("junit", ListFormat::Pretty)]
    #[case::parse_unknown("nonsense", ListFormat::Pretty)]
    fn test_parse_list_format(#[case] input: &str, #[case] expected: ListFormat) {
        assert_eq!(parse_list_format(input).unwrap(), expected);
    }

    #[test]
    fn test_format_default_when_unset_is_pretty() {
        let actual = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(actual.format, ListFormat::Pretty);
    }

    #[test]
    fn test_format_bare_flag_is_pretty() {
        let actual = CommandLineArgs::parse_from(["--format"]);
        assert_eq!(actual.format, ListFormat::Pretty);
    }

    #[test]
    fn test_format_equals_empty_is_pretty() {
        let actual = CommandLineArgs::parse_from(["--format="]);
        assert_eq!(actual.format, ListFormat::Pretty);
    }

    #[test]
    fn test_format_equals_terse_is_terse() {
        let actual = CommandLineArgs::parse_from(["--format=terse"]);
        assert_eq!(actual.format, ListFormat::Terse);
    }

    #[test]
    fn test_format_space_terse_is_terse() {
        let actual = CommandLineArgs::parse_from(["--format", "terse"]);
        assert_eq!(actual.format, ListFormat::Terse);
    }

    #[test]
    fn test_format_unknown_value_is_pretty() {
        let actual = CommandLineArgs::parse_from(["--format=json"]);
        assert_eq!(actual.format, ListFormat::Pretty);
    }

    #[test]
    fn test_list_format_terse_nextest_invocation() {
        let actual = CommandLineArgs::parse_from(["--list", "--format", "terse"]);
        assert!(actual.list);
        assert!(!actual.ignored);
        assert_eq!(actual.format, ListFormat::Terse);
    }

    #[test]
    fn test_list_format_terse_ignored_nextest_invocation() {
        let actual = CommandLineArgs::parse_from(["--list", "--format", "terse", "--ignored"]);
        assert!(actual.list);
        assert!(actual.ignored);
        assert_eq!(actual.format, ListFormat::Terse);
    }

    #[test]
    fn test_format_bare_flag_followed_by_other_flag_is_pretty() {
        // `--format` followed by another flag must not gobble the flag as a value.
        let actual = CommandLineArgs::parse_from(["--format", "--list"]);
        assert!(actual.list);
        assert_eq!(actual.format, ListFormat::Pretty);
    }

    #[rstest]
    #[case::one("ir", indexset!{ Ir })]
    #[case::one_with_spaces("  ir ", indexset!{ Ir })]
    #[case::two("ir,i1mr", indexset!{ Ir, I1mr })]
    #[case::two_with_spaces("ir,   i1mr", indexset!{ Ir, I1mr })]
    #[case::group("@writebackbehaviour", indexset!{ ILdmr, DLdmr, DLdmw })]
    #[case::group_abbreviation("@wb", indexset!{ ILdmr, DLdmr, DLdmw })]
    #[case::group_and_single_then_no_change("@wb,ildmr", indexset!{ ILdmr, DLdmr, DLdmw })]
    #[case::single_and_group_then_overwrite("dldmw,@wb", indexset!{ DLdmw, ILdmr, DLdmr })]
    #[case::all("@all", CallgrindMetrics::All.into())]
    fn test_parse_callgrind_metrics(#[case] input: &str, #[case] expected: IndexSet<EventKind>) {
        assert_eq!(parse_callgrind_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::empty("")]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    #[case::wrong_delimiter("ir;dr")]
    fn test_parse_callgrind_metrics_then_error(#[case] input: &str) {
        parse_callgrind_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_callgrind_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--callgrind-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_callgrind_metrics_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_CALLGRIND_METRICS", "ir");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.callgrind_metrics,
            Some(IndexSet::from([EventKind::Ir]))
        );
    }

    // Just test the very basics. The details are tested in `test_parse_callgrind_metrics`
    #[rstest]
    #[case::one("ir", indexset!{ CachegrindMetric::Ir })]
    #[case::all("@all", CachegrindMetrics::All.into())]
    fn test_parse_cachegrind_metrics(
        #[case] input: &str,
        #[case] expected: IndexSet<CachegrindMetric>,
    ) {
        assert_eq!(parse_cachegrind_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_cachegrind_metrics_then_error(#[case] input: &str) {
        parse_cachegrind_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_cachegrind_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--cachegrind-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_cachegrind_metrics_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_CACHEGRIND_METRICS", "ir");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.cachegrind_metrics,
            Some(IndexSet::from([CachegrindMetric::Ir]))
        );
    }

    #[rstest]
    #[case::one("totalbytes", indexset!{ DhatMetric::TotalBytes })]
    #[case::all("@all", DhatMetrics::All.into())]
    fn test_parse_dhat_metrics(#[case] input: &str, #[case] expected: IndexSet<DhatMetric>) {
        assert_eq!(parse_dhat_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_dhat_metrics_then_error(#[case] input: &str) {
        parse_dhat_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_dhat_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--dhat-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_dhat_metrics_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_DHAT_METRICS", "totalbytes");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.dhat_metrics,
            Some(IndexSet::from([DhatMetric::TotalBytes]))
        );
    }

    #[rstest]
    #[case::one("errors", indexset!{ ErrorMetric::Errors })]
    #[case::all("@all", indexset! {
        ErrorMetric::Errors,
        ErrorMetric::Contexts,
        ErrorMetric::SuppressedErrors,
        ErrorMetric::SuppressedContexts
    })]
    fn test_parse_drd_metrics(#[case] input: &str, #[case] expected: IndexSet<ErrorMetric>) {
        assert_eq!(parse_drd_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_drd_metrics_then_error(#[case] input: &str) {
        parse_drd_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_drd_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--drd-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_drd_metrics_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_DRD_METRICS", "errors");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.drd_metrics,
            Some(IndexSet::from([ErrorMetric::Errors]))
        );
    }

    #[rstest]
    #[case::one("errors", indexset!{ ErrorMetric::Errors })]
    #[case::all("@all", indexset! {
        ErrorMetric::Errors,
        ErrorMetric::Contexts,
        ErrorMetric::SuppressedErrors,
        ErrorMetric::SuppressedContexts
    })]
    fn test_parse_memcheck_metrics(#[case] input: &str, #[case] expected: IndexSet<ErrorMetric>) {
        assert_eq!(parse_memcheck_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_memcheck_metrics_then_error(#[case] input: &str) {
        parse_memcheck_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_memcheck_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--memcheck-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_memcheck_metrics_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_MEMCHECK_METRICS", "errors");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.memcheck_metrics,
            Some(IndexSet::from([ErrorMetric::Errors]))
        );
    }

    #[rstest]
    #[case::one("errors", indexset!{ ErrorMetric::Errors })]
    #[case::all("@all", indexset! {
        ErrorMetric::Errors,
        ErrorMetric::Contexts,
        ErrorMetric::SuppressedErrors,
        ErrorMetric::SuppressedContexts
    })]
    fn test_parse_helgrind_metrics(#[case] input: &str, #[case] expected: IndexSet<ErrorMetric>) {
        assert_eq!(parse_helgrind_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_helgrind_metrics_then_error(#[case] input: &str) {
        parse_helgrind_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_helgrind_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--helgrind-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_helgrind_metrics_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_HELGRIND_METRICS", "errors");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.helgrind_metrics,
            Some(IndexSet::from([ErrorMetric::Errors]))
        );
    }

    #[rstest]
    #[case::default("--tolerance", f64::from_bits(0.000_01f64.to_bits() - 1))]
    #[case::some_value("--tolerance=1.0", 1.0)]
    fn test_arg_tolerance(#[case] input: &str, #[case] expected: f64) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.tolerance, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_tolerance_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_TOLERANCE", "2.0");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.tolerance, Some(2.0));
    }

    #[rstest]
    #[case::when_no_equals("--show-intermediate", true)]
    #[case::when_true("--show-intermediate=true", true)]
    #[case::when_false("--show-intermediate=false", false)]
    fn test_arg_show_intermediate(#[case] input: &str, #[case] expected: bool) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.show_intermediate, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_show_intermediate_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_SHOW_INTERMEDIATE", "yes");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.show_intermediate, Some(true));
    }

    #[rstest]
    #[case::when_no_equals("--show-grid", true)]
    #[case::when_true("--show-grid=true", true)]
    #[case::when_false("--show-grid=false", false)]
    fn test_arg_show_grid(#[case] input: &str, #[case] expected: bool) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.show_grid, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_show_grid_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_SHOW_GRID", "yes");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.show_grid, Some(true));
    }

    #[rstest]
    #[case::missing_value("--truncate-description", TruncateDescription::To(50))]
    #[case::some_value("--truncate-description=20", TruncateDescription::To(20))]
    #[case::when_false("--truncate-description=false", TruncateDescription::None)]
    #[case::when_no("--truncate-description=no", TruncateDescription::None)]
    fn test_arg_truncate_description(#[case] input: &str, #[case] expected: TruncateDescription) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.truncate_description, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_truncate_description_when_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_TRUNCATE_DESCRIPTION", "no");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.truncate_description, Some(TruncateDescription::None));
    }

    #[test]
    fn test_arg_tool_runner() {
        let file = tempfile::Builder::new()
            .permissions(Permissions::from_mode(0o755))
            .tempfile()
            .unwrap();
        let result =
            CommandLineArgs::try_parse_from([format!("--tool-runner={}", file.path().display())])
                .unwrap();

        assert_eq!(result.tool_runner, Some(file.path().to_path_buf()));
    }

    #[test]
    fn test_arg_tool_runner_when_directory_then_error() {
        let dir = tempdir().unwrap();
        let result =
            CommandLineArgs::try_parse_from([format!("--tool-runner='{}'", dir.path().display())]);
        result.unwrap_err();
    }

    #[test]
    fn test_arg_tool_runner_when_not_executable_then_error() {
        let file = NamedTempFile::new().unwrap();
        let result =
            CommandLineArgs::try_parse_from([format!("--tool-runner={}", file.path().display())]);
        result.unwrap_err();
    }

    #[rstest]
    #[case::positional_one(&["--tool-runner-args=foo"], &["foo"])]
    #[case::positional_one_with_quotes(&["--tool-runner-args='foo'"], &["foo"])]
    #[case::flag_one(&["--tool-runner-args=--foo"], &["--foo"])]
    #[case::flag_one_with_quotes(&["--tool-runner-args='--foo'"], &["--foo"])]
    #[case::flag_one_with_equals(&["--tool-runner-args=--foo=some"], &["--foo=some"])]
    #[case::flag_two(&["--tool-runner-args='--foo --bar'"], &["--foo", "--bar"])]
    fn test_tool_runner_args(#[case] input: &[&str], #[case] expected: &[&str]) {
        let result = CommandLineArgs::try_parse_from(
            input
                .iter()
                .chain(std::iter::once(&"--tool-runner=/bin/cat")),
        )
        .map_err(|e| e.to_string())
        .unwrap();
        assert_eq!(
            result.tool_runner_args,
            vec![RawArgs(expected.iter().map(ToString::to_string).collect())]
        );
    }

    #[test]
    fn test_tool_runner_args_when_twice() {
        let result = CommandLineArgs::try_parse_from([
            "--tool-runner-args=--foo",
            "--tool-runner-args=--bar",
            "--tool-runner=/bin/cat",
        ])
        .unwrap();
        assert_eq!(
            result.tool_runner_args,
            vec![
                RawArgs(vec!["--foo".to_owned()]),
                RawArgs(vec!["--bar".to_owned()])
            ]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_env_clear_default() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var("GUNGRAUN_ENV_CLEAR");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.env_clear, None);
    }

    #[test]
    #[serial_test::serial]
    fn test_env_clear_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_ENV_CLEAR", "yes");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.env_clear, Some(true));
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var("GUNGRAUN_ENV_CLEAR");
        }
    }

    #[rstest]
    #[case::yes("yes", true)]
    #[case::no("no", false)]
    #[case::true_val("true", true)]
    #[case::false_val("false", false)]
    #[case::on("on", true)]
    #[case::off("off", false)]
    #[case::one("1", true)]
    #[case::zero("0", false)]
    #[case::default("", true)]
    fn test_env_clear_cli(#[case] value: &str, #[case] expected: bool) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--env-clear".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--env-clear={value}")])
        };
        assert_eq!(result.env_clear, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_all_missing_vars() {
        let result =
            CommandLineArgs::try_parse_from(["--envs='NONEXISTENT1 NONEXISTENT2'"]).unwrap();

        assert_eq!(result.envs.len(), 1);
        assert_eq!(result.envs[0], vec![]);
    }

    #[test]
    fn test_envs_arg_empty_string() {
        let result = CommandLineArgs::try_parse_from(["--envs=''"]).unwrap();
        assert_eq!(result.envs.len(), 1);
        assert_eq!(result.envs[0], vec![]);
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_from_config_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("GUNGRAUN_ENVS", "FROM_CONFIG=yes");
        }
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("FROM_CONFIG"), OsString::from("yes"))]
        );
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var("GUNGRAUN_ENVS");
        }
    }

    #[test]
    fn test_envs_arg_missing_env_var() {
        let result = CommandLineArgs::try_parse_from(["--envs=NONEXISTENT_VAR_789"]).unwrap();
        assert_eq!(result.envs[0], vec![]);
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_mixed_resolution() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("MIXED_TEST_VAR", "from_env");
        }
        let result =
            CommandLineArgs::try_parse_from(["--envs='KEY=val MIXED_TEST_VAR OTHER=set'"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![
                (OsString::from("KEY"), OsString::from("val")),
                (OsString::from("MIXED_TEST_VAR"), OsString::from("from_env")),
                (OsString::from("OTHER"), OsString::from("set")),
            ]
        );
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var("MIXED_TEST_VAR");
        }
    }

    #[test]
    fn test_envs_arg_multiple_delimited() {
        let result = CommandLineArgs::try_parse_from(["--envs='A=1 B=2 C=3'"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![
                (OsString::from("A"), OsString::from("1")),
                (OsString::from("B"), OsString::from("2")),
                (OsString::from("C"), OsString::from("3")),
            ]
        );
    }

    #[test]
    fn test_envs_arg_multiple_invocations() {
        let result = CommandLineArgs::try_parse_from(["--envs=A=1", "--envs=B=2"]).unwrap();
        assert_eq!(
            result.envs,
            vec![
                vec![(OsString::from("A"), OsString::from("1"))],
                vec![(OsString::from("B"), OsString::from("2"))],
            ]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_partial_resolve() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("PARTIAL_EXISTS", "yes");
        }
        let result =
            CommandLineArgs::try_parse_from(["--envs='PARTIAL_EXISTS NONEXISTENT_XYZ'"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("PARTIAL_EXISTS"), OsString::from("yes"))]
        );
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var("PARTIAL_EXISTS");
        }
    }

    #[test]
    fn test_envs_arg_path_with_colons() {
        let result = CommandLineArgs::try_parse_from(["--envs=PATH=/usr/bin:/bin"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("PATH"), OsString::from("/usr/bin:/bin"))]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_resolve_from_env() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("RESOLVE_ME_VAR", "env_value");
        }
        let result = CommandLineArgs::try_parse_from(["--envs=RESOLVE_ME_VAR"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(
                OsString::from("RESOLVE_ME_VAR"),
                OsString::from("env_value")
            )]
        );
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var("RESOLVE_ME_VAR");
        }
    }

    #[rstest]
    #[case::simple(
        &["--envs=KEY=value"],
        vec![(OsString::from("KEY"), OsString::from("value"))]
    )]
    #[case::with_equals_in_value(
        &["--envs=URL=http://example.com"],
        vec![(OsString::from("URL"), OsString::from("http://example.com"))]
    )]
    #[case::empty_value(
        &["--envs=EMPTY="],
        vec![(OsString::from("EMPTY"), OsString::from(""))]
    )]
    #[case::multiple_equals(
        &["--envs=A=B=C"],
        vec![(OsString::from("A"), OsString::from("B=C"))]
    )]
    #[case::with_single_quotes(
        &["--envs='A=foo bar'"],
        vec![(OsString::from("A"), OsString::from("foo"))]
    )]
    #[case::with_single_quotes_value(
        &["--envs=A='foo bar'"],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::with_single_quotes_all(
        &["--envs='A='foo bar''"],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::with_double_quotes(
        &["--envs=\"A=foo bar\""],
        vec![(OsString::from("A"), OsString::from("foo"))]
    )]
    #[case::with_double_quotes_value(
        &["--envs=A=\"foo bar\""],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::with_double_quotes_all(
        &["--envs=\"A=\"foo bar\"\""],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::multiple_with_quotes(
        &["--envs=\"A='foo bar' B=baz\""],
        vec![
            (OsString::from("A"), OsString::from("foo bar")),
            (OsString::from("B"), OsString::from("baz"))
        ]
    )]
    fn test_envs_arg_single(#[case] args: &[&str], #[case] expected: Vec<(OsString, OsString)>) {
        let result = CommandLineArgs::try_parse_from(args).unwrap();
        let expected: Vec<(OsString, OsString)> = expected.into_iter().collect();
        assert_eq!(result.envs[0], expected);
    }

    #[test]
    fn test_envs_arg_unicode() {
        let result = CommandLineArgs::try_parse_from(["--envs=CAFÉ=café"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("CAFÉ"), OsString::from("café"))]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_parse_env_from_env_var() {
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var("TEST_PARSE_ENV_VAR", "resolved_value");
        }
        let result = parse_envs("TEST_PARSE_ENV_VAR").unwrap();
        assert_eq!(
            result,
            vec![(
                OsString::from("TEST_PARSE_ENV_VAR"),
                OsString::from("resolved_value")
            )]
        );
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var("TEST_PARSE_ENV_VAR");
        }
    }

    #[test]
    fn test_parse_envs_empty() {
        let result = parse_envs("").unwrap();
        assert_eq!(result, vec![]);
    }

    #[test]
    #[serial_test::serial]
    fn test_parse_envs_missing_env_var() {
        let result = parse_envs("NONEXISTENT_VAR_XYZ123").unwrap();
        assert_eq!(result, vec![]);
    }

    #[rstest]
    #[case::empty_key("=value", "Empty key for value: 'value'")]
    #[case::just_equals("=", "Empty key for value: ''")]
    #[case::shlex_error_wrong_quoting(
        "key='value",
        "Failed splitting 'key='value' for POSIX shell environment"
    )]
    fn test_parse_envs_when_error(#[case] input: &str, #[case] expected: &str) {
        let err = parse_envs(input).unwrap_err();
        assert_eq!(err, expected);
    }

    #[rstest]
    #[case::whitespace_only("      ", vec![])]
    #[case::leading_trailing("  A=1  ", vec![(OsString::from("A"), OsString::from("1"))])]
    #[case::multiple_spaces("A=1  B=2", vec![
        (OsString::from("A"), OsString::from("1")),
        (OsString::from("B"), OsString::from("2"))
    ])]
    fn test_parse_envs_whitespace(
        #[case] input: &str,
        #[case] expected: Vec<(OsString, OsString)>,
    ) {
        let result = parse_envs(input).unwrap();
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case::simple("KEY=value", "KEY", "value")]
    #[case::value_with_equals("URL=http://example.com", "URL", "http://example.com")]
    #[case::multiple_equals("A=B=C=D", "A", "B=C=D")]
    #[case::empty_value("KEY=", "KEY", "")]
    #[case::with_colons("PATH=/usr/bin:/bin", "PATH", "/usr/bin:/bin")]
    fn test_parse_envs_with_equals(
        #[case] input: &str,
        #[case] expected_key: &str,
        #[case] expected_value: &str,
    ) {
        let result = parse_envs(input).unwrap();
        assert_eq!(
            result,
            vec![(OsString::from(expected_key), OsString::from(expected_value))]
        );
    }
}
