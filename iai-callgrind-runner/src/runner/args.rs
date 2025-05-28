use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use clap::builder::BoolishValueParser;
use clap::{ArgAction, Parser};

use super::format::OutputFormatKind;
use super::summary::{BaselineName, SummaryFormat};
use crate::api::{EventKind, RawArgs, RegressionConfig};

/// A filter for benchmarks
///
/// # Developer Notes
///
/// This enum is used instead of a plain `String` for possible future usages to filter by benchmark
/// ids, group name, file name etc.
#[derive(Debug, Clone)]
pub enum BenchmarkFilter {
    /// The name of the benchmark
    Name(String),
}

impl BenchmarkFilter {
    /// Return true if the haystack contains the filter
    pub fn apply(&self, haystack: &str) -> bool {
        let Self::Name(name) = self;
        haystack.contains(name)
    }
}

impl FromStr for BenchmarkFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(BenchmarkFilter::Name(s.to_owned()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoCapture {
    True,
    False,
    Stderr,
    Stdout,
}

impl NoCapture {
    pub fn apply(self, command: &mut Command) {
        match self {
            NoCapture::True | NoCapture::False => {}
            NoCapture::Stderr => {
                command.stdout(Stdio::null()).stderr(Stdio::inherit());
            }
            NoCapture::Stdout => {
                command.stdout(Stdio::inherit()).stderr(Stdio::null());
            }
        }
    }
}

/// The command line arguments the user provided after `--` when running cargo bench
///
/// These arguments are not the command line arguments passed to `iai-callgrind-runner`. We collect
/// the command line arguments in the `iai-callgrind::main!` macro without the binary as first
/// argument, that's why `no_binary_name` is set to `true`.
#[allow(clippy::partial_pub_fields)]
#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "High-precision and consistent benchmarking framework/harness for Rust

Boolish command line arguments take also one of `y`, `yes`, `t`, `true`, `on`, `1`
instead of `true` and one of `n`, `no`, `f`, `false`, `off`, and `0` instead of
`false`",
    long_about = None,
    no_binary_name = true,
    override_usage= "cargo bench ... [BENCHNAME] -- [OPTIONS]"
)]
pub struct CommandLineArgs {
    /// `--bench` usually shows up as last argument set by cargo and not by us.
    ///
    /// This argument is useless, so we sort it out and never make use of it.
    #[arg(long = "bench", hide = true, action = ArgAction::SetTrue, required = false)]
    _bench: bool,

    /// If specified, only run benches containing this string in their names
    ///
    /// Note that a benchmark name might differ from the benchmark file name.
    #[arg(name = "BENCHNAME", num_args = 0..=1, env = "IAI_CALLGRIND_FILTER")]
    pub filter: Option<BenchmarkFilter>,

    /// The raw arguments to pass through to Callgrind
    ///
    /// This is a space separated list of command-line-arguments specified as if they were
    /// passed directly to valgrind.
    ///
    /// Examples:
    ///   * --callgrind-args=--dump-instr=yes
    ///   * --callgrind-args='--dump-instr=yes --collect-systime=yes'
    #[arg(
        long = "callgrind-args",
        value_parser = parse_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "IAI_CALLGRIND_CALLGRIND_ARGS"
    )]
    pub callgrind_args: Option<RawArgs>,

    /// Save a machine-readable summary of each benchmark run in json format next to the usual
    /// benchmark output
    #[arg(
        long = "save-summary",
        value_enum,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "json",
        env = "IAI_CALLGRIND_SAVE_SUMMARY"
    )]
    pub save_summary: Option<SummaryFormat>,

    /// Allow ASLR (Address Space Layout Randomization)
    ///
    /// If possible, ASLR is disabled on platforms that support it (linux, freebsd) because ASLR
    /// could noise up the callgrind cache simulation results a bit. Setting this option to true
    /// runs all benchmarks with ASLR enabled.
    ///
    /// See also <https://docs.kernel.org/admin-guide/sysctl/kernel.html?highlight=randomize_va_space#randomize-va-space>
    #[arg(
        long = "allow-aslr",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        env = "IAI_CALLGRIND_ALLOW_ASLR",
    )]
    pub allow_aslr: Option<bool>,

    /// Set performance regression limits for specific `EventKinds`
    ///
    /// This is a `,` separate list of EventKind=limit (key=value) pairs with the limit being a
    /// positive or negative percentage. If positive, a performance regression check for this
    /// `EventKind` fails if the limit is exceeded. If negative, the regression check fails if the
    /// value comes below the limit. The `EventKind` is matched case-insensitive. For a list of
    /// valid `EventKinds` see the docs: <https://docs.rs/iai-callgrind/latest/iai_callgrind/enum.EventKind.html>
    ///
    /// Examples: --regression='ir=0.0' or --regression='ir=0, EstimatedCycles=10'
    #[arg(
        long = "regression",
        num_args = 1,
        value_parser = parse_regression_config,
        env = "IAI_CALLGRIND_REGRESSION",
    )]
    pub regression: Option<RegressionConfig>,

    /// If true, the first failed performance regression check fails the whole benchmark run
    ///
    /// This option requires `--regression=...` or `IAI_CALLGRIND_REGRESSION=...` to be present.
    #[arg(
        long = "regression-fail-fast",
        requires = "regression",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        env = "IAI_CALLGRIND_REGRESSION_FAIL_FAST",
    )]
    pub regression_fail_fast: Option<bool>,

    /// Compare against this baseline if present and then overwrite it
    #[arg(
        long = "save-baseline",
        default_missing_value = "default",
        num_args = 0..=1,
        require_equals = true,
        conflicts_with_all = &["baseline", "LOAD_BASELINE"],
        env = "IAI_CALLGRIND_SAVE_BASELINE",
    )]
    pub save_baseline: Option<BaselineName>,

    /// Compare against this baseline if present but do not overwrite it
    #[arg(
        long = "baseline",
        default_missing_value = "default",
        num_args = 0..=1,
        require_equals = true,
        env = "IAI_CALLGRIND_BASELINE"
    )]
    pub baseline: Option<BaselineName>,

    /// Load this baseline as the new data set instead of creating a new one
    #[clap(
        id = "LOAD_BASELINE",
        long = "load-baseline",
        requires = "baseline",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "default",
        env = "IAI_CALLGRIND_LOAD_BASELINE"
    )]
    pub load_baseline: Option<BaselineName>,

    /// The terminal output format in default human-readable format or in machine-readable json
    /// format
    ///
    /// # The JSON Output Format
    ///
    /// The json terminal output schema is the same as the schema with the `--save-summary`
    /// argument when saving to a `summary.json` file. All other output than the json output goes
    /// to stderr and only the summary output goes to stdout. When not printing pretty json, each
    /// line is a dictionary summarizing a single benchmark. You can combine all lines
    /// (benchmarks) into an array for example with `jq`
    ///
    /// `cargo bench -- --output-format=json | jq -s`
    ///
    /// which transforms `{...}\n{...}` into `[{...},{...}]`
    #[arg(
        long = "output-format",
        value_enum,
        required = false,
        default_value = "default",
        num_args = 1,
        env = "IAI_CALLGRIND_OUTPUT_FORMAT"
    )]
    pub output_format: OutputFormatKind,

    /// Separate iai-callgrind benchmark output files by target
    ///
    /// The default output path for files created by iai-callgrind and valgrind during the
    /// benchmark is
    ///
    /// `target/iai/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`.
    ///
    /// This can be problematic if you're running the benchmarks not only for a
    /// single target because you end up comparing the benchmark runs with the wrong targets.
    /// Setting this option changes the default output path to
    ///
    /// `target/iai/$TARGET/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`
    ///
    /// Although not as comfortable and strict, you could achieve a separation by target also with
    /// baselines and a combination of `--save-baseline=$TARGET` and `--baseline=$TARGET` if you
    /// prefer having all files of a single $BENCH in the same directory.
    #[arg(
        long = "separate-targets",
        default_missing_value = "true",
        default_value = "false",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        action = ArgAction::Set,
        env = "IAI_CALLGRIND_SEPARATE_TARGETS",
    )]
    pub separate_targets: bool,

    /// Specify the home directory of iai-callgrind benchmark output files
    ///
    /// All output files are per default stored under the `$PROJECT_ROOT/target/iai` directory.
    /// This option lets you customize this home directory, and it will be created if it
    /// doesn't exist.
    #[arg(long = "home", num_args = 1, env = "IAI_CALLGRIND_HOME")]
    pub home: Option<PathBuf>,

    /// Don't capture terminal output of benchmarks
    ///
    /// Possible values are one of [true, false, stdout, stderr].
    ///
    /// This option is currently restricted to the `callgrind` run of benchmarks. The output of
    /// additional tool runs like DHAT, Memcheck, ... is still captured, to prevent showing the
    /// same output of benchmarks multiple times. Use `IAI_CALLGRIND_LOG=info` to also show
    /// captured and logged output.
    ///
    /// If no value is given, the default missing value is `true` and doesn't capture stdout and
    /// stderr. Besides `true` or `false` you can specify the special values `stdout` or `stderr`.
    /// If `--nocapture=stdout` is given, the output to `stdout` won't be captured and the output
    /// to `stderr` will be discarded. Likewise, if `--nocapture=stderr` is specified, the
    /// output to `stderr` won't be captured and the output to `stdout` will be discarded.
    #[arg(
        long = "nocapture",
        required = false,
        default_missing_value = "true",
        default_value = "false",
        num_args = 0..=1,
        require_equals = true,
        value_parser = parse_nocapture,
        env = "IAI_CALLGRIND_NOCAPTURE"
    )]
    pub nocapture: NoCapture,
}

/// This function parses a space separated list of raw argument strings into [`crate::api::RawArgs`]
fn parse_args(value: &str) -> Result<RawArgs, String> {
    shlex::split(value)
        .ok_or_else(|| "Failed to split callgrind args".to_owned())
        .map(RawArgs::new)
}

fn parse_regression_config(value: &str) -> Result<RegressionConfig, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("No limits found: At least one limit must be specified".to_owned());
    }

    let regression_config = if value.eq_ignore_ascii_case("default") {
        RegressionConfig::default()
    } else {
        let mut limits = vec![];

        for split in value.split(',') {
            let split = split.trim();

            if let Some((key, value)) = split.split_once('=') {
                let (key, value) = (key.trim(), value.trim());
                let event_kind = EventKind::from_str_ignore_case(key)
                    .ok_or_else(|| -> String { format!("Unknown event kind: '{key}'") })?;

                let pct = value.parse::<f64>().map_err(|error| -> String {
                    format!("Invalid percentage for '{key}': {error}")
                })?;
                limits.push((event_kind, pct));
            } else {
                return Err(format!("Invalid format of key/value pair: '{split}'"));
            }
        }

        RegressionConfig {
            limits,
            ..Default::default()
        }
    };

    Ok(regression_config)
}

impl From<&CommandLineArgs> for Option<RegressionConfig> {
    fn from(value: &CommandLineArgs) -> Self {
        let mut config = value.regression.clone();
        if let Some(config) = config.as_mut() {
            config.fail_fast = value.regression_fail_fast;
        }
        config
    }
}

fn parse_nocapture(value: &str) -> Result<NoCapture, String> {
    // Taken from clap source code
    const TRUE_LITERALS: [&str; 6] = ["y", "yes", "t", "true", "on", "1"];
    const FALSE_LITERALS: [&str; 6] = ["n", "no", "f", "false", "off", "0"];

    let lowercase: String = value.to_lowercase();

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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::api::EventKind::*;
    use crate::api::RawArgs;

    #[rstest]
    #[case::empty("", &[])]
    #[case::single_key_value("--some=yes", &["--some=yes"])]
    #[case::two_key_value("--some=yes --other=no", &["--some=yes", "--other=no"])]
    #[case::single_escaped("--some='yes and no'", &["--some=yes and no"])]
    #[case::double_escaped("--some='\"yes and no\"'", &["--some=\"yes and no\""])]
    #[case::multiple_escaped("--some='yes and no' --other='no and yes'", &["--some=yes and no", "--other=no and yes"])]
    fn test_parse_callgrind_args(#[case] value: &str, #[case] expected: &[&str]) {
        let actual = parse_args(value).unwrap();
        assert_eq!(actual, RawArgs::from_iter(expected));
    }

    #[rstest]
    #[case::regression_default("default", vec![])]
    #[case::regression_default_case_insensitive("DefAulT", vec![])]
    #[case::regression_only("Ir=10", vec![(Ir, 10f64)])]
    #[case::regression_case_insensitive("EstIMATedCycles=10", vec![(EstimatedCycles, 10f64)])]
    #[case::multiple_regression("Ir=10,EstimatedCycles=5", vec![(Ir, 10f64), (EstimatedCycles, 5f64)])]
    #[case::multiple_regression_with_whitespace("Ir= 10 ,  EstimatedCycles = 5", vec![(Ir, 10f64), (EstimatedCycles, 5f64)])]
    fn test_parse_regression_config(
        #[case] regression_var: &str,
        #[case] expected_limits: Vec<(EventKind, f64)>,
    ) {
        let expected = RegressionConfig {
            limits: expected_limits,
            fail_fast: None,
        };

        let actual = parse_regression_config(regression_var).unwrap();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::regression_wrong_format_of_key_value_pair(
        "Ir:10",
        "Invalid format of key/value pair: 'Ir:10'"
    )]
    #[case::regression_unknown_event_kind("WRONG=10", "Unknown event kind: 'WRONG'")]
    #[case::regression_invalid_percentage(
        "Ir=10.0.0",
        "Invalid percentage for 'Ir': invalid float literal"
    )]
    #[case::regression_empty_limits("", "No limits found: At least one limit must be specified")]
    fn test_try_regression_config_from_env_then_error(
        #[case] regression_var: &str,
        #[case] expected_reason: &str,
    ) {
        assert_eq!(
            &parse_regression_config(regression_var).unwrap_err(),
            expected_reason,
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_callgrind_args_env() {
        let test_arg = "--just-testing=yes";
        std::env::set_var("IAI_CALLGRIND_CALLGRIND_ARGS", test_arg);
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.callgrind_args,
            Some(RawArgs::new(vec![test_arg.to_owned()]))
        );
    }

    #[test]
    fn test_callgrind_args_not_env() {
        let test_arg = "--just-testing=yes";
        let result = CommandLineArgs::parse_from([format!("--callgrind-args={test_arg}")]);
        assert_eq!(
            result.callgrind_args,
            Some(RawArgs::new(vec![test_arg.to_owned()]))
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_callgrind_args_cli_takes_precedence_over_env() {
        let test_arg_yes = "--just-testing=yes";
        let test_arg_no = "--just-testing=no";
        std::env::set_var("IAI_CALLGRIND_CALLGRIND_ARGS", test_arg_yes);
        let result = CommandLineArgs::parse_from([format!("--callgrind-args={test_arg_no}")]);
        assert_eq!(
            result.callgrind_args,
            Some(RawArgs::new(vec![test_arg_no.to_owned()]))
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_save_summary_env() {
        std::env::set_var("IAI_CALLGRIND_SAVE_SUMMARY", "json");
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
        std::env::set_var("IAI_CALLGRIND_ALLOW_ASLR", "yes");
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
        std::env::set_var("IAI_CALLGRIND_SEPARATE_TARGETS", "yes");
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
        std::env::set_var("IAI_CALLGRIND_HOME", "/tmp/my_iai_home");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.home, Some(PathBuf::from("/tmp/my_iai_home")));
    }

    #[test]
    fn test_home_cli() {
        let result = CommandLineArgs::parse_from(["--home=/test_me".to_owned()]);
        assert_eq!(result.home, Some(PathBuf::from("/test_me")));
    }

    #[test]
    fn test_home_cli_when_no_value_then_error() {
        let result = CommandLineArgs::try_parse_from(["--home=".to_owned()]);
        assert!(result.is_err());
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
        std::env::set_var("IAI_CALLGRIND_NOCAPTURE", "true");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.nocapture, NoCapture::True);
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
}
