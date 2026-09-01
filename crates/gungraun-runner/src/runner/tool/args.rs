//! The module containing all elements for [`ValgrindArgs`]

use std::ffi::OsString;
use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Result, anyhow};
use log::warn;
use nix::NixPath;

use super::path::ToolOutputPath;
use crate::api::{RawToolArgs, Tool};
use crate::error::Error;
use crate::runner::perf::args::PerfArgs;
use crate::util::{bool_to_yesno, yesno_to_bool};

/// Default value for Callgrind and Cachegrind cache simulation.
pub const DEFAULT_CACHE_SIM: bool = true;
/// Default value for combining Callgrind dump files.
pub const DEFAULT_COMBINE_DUMPS: bool = false;
/// Default value for Callgrind position compression.
pub const DEFAULT_COMPRESS_POS: bool = false;
/// Default value for Callgrind string compression.
pub const DEFAULT_COMPRESS_STRINGS: bool = false;
/// Default Valgrind level-one data cache configuration.
pub const DEFAULT_D1: &str = "32768,8,64";
/// Default value for including instruction information in Callgrind dumps.
pub const DEFAULT_DUMP_INSTR: bool = false;
/// Default value for including source-line information in Callgrind dumps.
pub const DEFAULT_DUMP_LINE: bool = true;
/// Default error exit code for Valgrind tools that report errors.
pub const DEFAULT_ERROR_EXIT_CODE_ERROR_TOOL: &str = "201";
/// Default error exit code for Valgrind tools that do not report errors.
pub const DEFAULT_ERROR_EXIT_CODE_OTHER_TOOL: &str = "0";
/// Default Valgrind scheduler fairness mode.
pub const DEFAULT_FAIR_SCHED: FairSched = FairSched::Try;
/// Default Valgrind level-one instruction cache configuration.
pub const DEFAULT_I1: &str = "32768,8,64";
/// Default Valgrind last-level cache configuration.
pub const DEFAULT_LL: &str = "8388608,16,64";
/// Default value for producing separate Callgrind dumps per thread.
pub const DEFAULT_SEPARATE_THREADS: bool = true;
/// Default value for tracing child processes under Valgrind.
pub const DEFAULT_TRACE_CHILDREN: bool = true;
/// Default value for verbose Valgrind output.
pub const DEFAULT_VERBOSE: bool = false;
/// Default Valgrind debugger server mode.
pub const DEFAULT_VGDB: Vgdb = Vgdb::No;

/// The possible values of the --fair-sched cli arg
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FairSched {
    /// Corresponds to `yes`
    Yes,
    /// Corresponds to `no`
    No,
    /// Corresponds to `try`
    Try,
}

/// Normalizes per-tool command-line argument construction for Valgrind and perf.
///
/// `ToolArgs` dispatches to [`ValgrindArgs`] or [`PerfArgs`] depending on the active tool, exposing
/// a unified interface for setting output paths, events, and serializing the final argument vector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolArgs {
    /// Valgrind tool arguments.
    Valgrind(ValgrindArgs),
    /// perf tool arguments.
    Perf(PerfArgs),
}

/// A Valgrind tool selectable by the runner.
///
/// Each variant maps one-to-one to [`crate::api::Tool`] and determines the `--tool` argument passed
/// to Valgrind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValgrindTool {
    /// The Callgrind profiler.
    Callgrind,
    /// The Cachegrind cache simulator.
    Cachegrind,
    /// The DHAT heap profiler.
    DHAT,
    /// The Memcheck memory error detector.
    Memcheck,
    /// The Helgrind thread error detector.
    Helgrind,
    /// The DRD thread error detector.
    DRD,
    /// The Massif heap profiler.
    Massif,
    /// The Basic Block Vector generator.
    BBV,
}

/// The possible values for --vgdb
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vgdb {
    /// Corresponds to `yes`
    Yes,
    /// Corresponds to `no`
    No,
    /// Corresponds to `full`
    Full,
}

/// The arguments to pass to the Valgrind tool
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValgrindArgs {
    /// The error exit code for error checking tools like `Memcheck`
    pub error_exitcode: String,
    /// The --fair-sched argument
    pub fair_sched: FairSched,
    /// The logfile paths argument --log-file
    pub log_path: Option<OsString>,
    /// All other arguments
    pub other: Vec<String>,
    /// The output paths argument like --callgrind-out-file, ...
    pub output_paths: Vec<OsString>,
    /// The [`ValgrindTool`]
    pub tool: ValgrindTool,
    /// The --trace-children argument
    pub trace_children: bool,
    /// If --verbose is set to true of false
    pub verbose: bool,
    /// The --vgdb argument
    pub vgdb: Vgdb,
    /// The xtree paths argument --xtree-leak-file
    pub xleak_path: Option<OsString>,
    /// The xtree paths argument --xtree-memory-file
    pub xtree_path: Option<OsString>,
}

/// Common parsing behavior for Valgrind tool arguments.
pub trait ToolArgsLike: Sized {
    /// Try to create new arguments from multiple [`RawToolArgs`].
    fn try_from_raw_tool_args(tool: Tool, raw_tool_args: &[&RawToolArgs]) -> Result<Self>;

    /// Try to update these arguments from the contents of an iterator.
    fn try_update<'a, T>(&mut self, args: T) -> Result<()>
    where
        T: Iterator<Item = &'a String>;
}

impl Display for FairSched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Try => "try",
        };
        write!(f, "{string}")
    }
}

impl FromStr for FairSched {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "no" => Ok(Self::No),
            "yes" => Ok(Self::Yes),
            "try" => Ok(Self::Try),
            _ => Err(anyhow!(
                "Invalid argument for --fair-sched. Valid arguments are: 'yes', 'no', 'try'"
            )),
        }
    }
}

impl From<ValgrindTool> for Tool {
    fn from(value: ValgrindTool) -> Self {
        match value {
            ValgrindTool::Callgrind => Self::Callgrind,
            ValgrindTool::Cachegrind => Self::Cachegrind,
            ValgrindTool::DHAT => Self::DHAT,
            ValgrindTool::Memcheck => Self::Memcheck,
            ValgrindTool::Helgrind => Self::Helgrind,
            ValgrindTool::DRD => Self::DRD,
            ValgrindTool::Massif => Self::Massif,
            ValgrindTool::BBV => Self::BBV,
        }
    }
}

impl ToolArgs {
    /// Sets the output path argument to the [`ToolOutputPath`] for the active tool.
    pub fn set_output_arg(
        &mut self,
        output_path: &ToolOutputPath,
        tool_runner_dest: Option<&Path>,
    ) {
        match self {
            Self::Valgrind(valgrind_args) => {
                valgrind_args.set_output_arg(output_path, tool_runner_dest);
            }
            Self::Perf(perf_args) => perf_args.set_output_arg(output_path, tool_runner_dest),
        }
    }

    /// Sets the log file argument to the [`ToolOutputPath`] for Valgrind tools.
    ///
    /// This is a no-op for perf.
    pub fn set_log_arg(&mut self, output_path: &ToolOutputPath, tool_runner_dest: Option<&Path>) {
        match self {
            Self::Valgrind(valgrind_args) => {
                valgrind_args.set_log_arg(output_path, tool_runner_dest);
            }
            Self::Perf(_) => {}
        }
    }

    /// Sets the xtree file argument to the [`ToolOutputPath`] for Valgrind tools that support it.
    ///
    /// This is a no-op for perf.
    pub fn set_xtree_arg(&mut self, output_path: &ToolOutputPath, tool_runner_dest: Option<&Path>) {
        match self {
            Self::Valgrind(valgrind_args) => {
                valgrind_args.set_xtree_arg(output_path, tool_runner_dest);
            }
            Self::Perf(_) => {}
        }
    }

    /// Sets the xleak file argument to the [`ToolOutputPath`] for Valgrind tools that support it.
    ///
    /// This is a no-op for perf.
    pub fn set_xleak_arg(&mut self, output_path: &ToolOutputPath, tool_runner_dest: Option<&Path>) {
        match self {
            Self::Valgrind(valgrind_args) => {
                valgrind_args.set_xleak_arg(output_path, tool_runner_dest);
            }
            Self::Perf(_) => {}
        }
    }

    /// Adds a list of perf events to the command line arguments.
    ///
    /// This is a no-op for Valgrind tools.
    pub fn add_events(&mut self, events: &str) {
        match self {
            Self::Valgrind(_) => {}
            Self::Perf(perf_args) => perf_args.add_events(events),
        }
    }

    /// Enables sampling mode for `perf stat`.
    ///
    /// This is a no-op for Valgrind tools.
    pub fn use_sampling(&mut self, yes: bool) {
        match self {
            Self::Valgrind(_) => {}
            Self::Perf(perf_args) => perf_args.use_sampling(yes),
        }
    }

    /// Returns `true` if the active tool is `perf record`.
    pub fn is_perf_record(&self) -> bool {
        match self {
            Self::Perf(perf_args) => perf_args.is_record(),
            Self::Valgrind(_) => false,
        }
    }

    /// Serializes the active tool arguments into a vector suitable for
    /// [`std::process::Command::args`].
    pub fn to_vec(&self) -> Vec<OsString> {
        match self {
            Self::Valgrind(valgrind_args) => valgrind_args.to_vec(),
            Self::Perf(perf_args) => perf_args.to_vec(),
        }
    }
}
impl ValgrindArgs {
    /// Create a new `ValgrindArgs` with the defaults for this tool.
    pub fn new(tool: ValgrindTool) -> Self {
        Self {
            tool,
            output_paths: Vec::default(),
            log_path: Option::default(),
            xtree_path: Option::default(),
            xleak_path: Option::default(),
            error_exitcode: match tool {
                ValgrindTool::Memcheck | ValgrindTool::Helgrind | ValgrindTool::DRD => {
                    DEFAULT_ERROR_EXIT_CODE_ERROR_TOOL.to_owned()
                }
                ValgrindTool::Callgrind
                | ValgrindTool::Massif
                | ValgrindTool::DHAT
                | ValgrindTool::BBV
                | ValgrindTool::Cachegrind => DEFAULT_ERROR_EXIT_CODE_OTHER_TOOL.to_owned(),
            },
            verbose: DEFAULT_VERBOSE,
            other: Vec::default(),
            trace_children: DEFAULT_TRACE_CHILDREN,
            fair_sched: DEFAULT_FAIR_SCHED,
            vgdb: DEFAULT_VGDB,
        }
    }

    /// Set the output file argument depending on the tool of this `ValgrindArgs`
    pub fn set_output_arg(
        &mut self,
        output_path: &ToolOutputPath,
        tool_runner_dest: Option<&Path>,
    ) {
        if !self.tool.has_output_file() {
            return;
        }

        match self.tool {
            ValgrindTool::Callgrind => {
                let arg = self.generate_file_arg(
                    "--callgrind-out-file=",
                    output_path,
                    tool_runner_dest,
                    None,
                );
                self.output_paths.push(arg);
            }
            ValgrindTool::Massif => {
                let arg = self.generate_file_arg(
                    "--massif-out-file=",
                    output_path,
                    tool_runner_dest,
                    None,
                );
                self.output_paths.push(arg);
            }
            ValgrindTool::DHAT => {
                let arg =
                    self.generate_file_arg("--dhat-out-file=", output_path, tool_runner_dest, None);
                self.output_paths.push(arg);
            }
            ValgrindTool::BBV => {
                let bb_arg = self.generate_file_arg(
                    "--bb-out-file=",
                    output_path,
                    tool_runner_dest,
                    Some("bb"),
                );
                let pc_arg = self.generate_file_arg(
                    "--pc-out-file=",
                    output_path,
                    tool_runner_dest,
                    Some("pc"),
                );
                self.output_paths.push(bb_arg);
                self.output_paths.push(pc_arg);
            }
            ValgrindTool::Cachegrind => {
                let arg = self.generate_file_arg(
                    "--cachegrind-out-file=",
                    output_path,
                    tool_runner_dest,
                    None,
                );

                self.output_paths.push(arg);
            }
            // The other tools don't have an outfile
            _ => {}
        }
    }

    /// Set the logfile argument
    pub fn set_log_arg(&mut self, output_path: &ToolOutputPath, tool_runner_dest: Option<&Path>) {
        let arg = self.generate_file_arg(
            "--log-file=",
            &output_path.to_log_output(),
            tool_runner_dest,
            None,
        );
        self.log_path = Some(arg);
    }

    /// Set the xtree-memory-file argument for tools which support it
    pub fn set_xtree_arg(&mut self, output_path: &ToolOutputPath, tool_runner_dest: Option<&Path>) {
        if let Some(output_path) = output_path.to_xtree_output() {
            let arg = self.generate_file_arg(
                "--xtree-memory-file=",
                &output_path,
                tool_runner_dest,
                None,
            );
            self.xtree_path = Some(arg);
        }
    }

    /// Set the xtree-leak-file argument for tools which support it
    pub fn set_xleak_arg(&mut self, output_path: &ToolOutputPath, tool_runner_dest: Option<&Path>) {
        if let Some(output_path) = output_path.to_xleak_output() {
            let arg =
                self.generate_file_arg("--xtree-leak-file=", &output_path, tool_runner_dest, None);
            self.xleak_path = Some(arg);
        }
    }

    /// Convert into a vector of arguments usable as input for [`std::process::Command::args`]
    pub fn to_vec(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = vec![];

        vec.push(format!("--tool={}", self.tool.id()).into());
        vec.push(format!("--error-exitcode={}", self.error_exitcode).into());
        vec.push(format!("--trace-children={}", bool_to_yesno(self.trace_children)).into());
        vec.push(format!("--fair-sched={}", self.fair_sched).into());
        vec.push(format!("--vgdb={}", self.vgdb).into());
        if self.verbose {
            vec.push("--verbose".into());
        }

        vec.extend(self.other.iter().map(OsString::from));
        vec.extend_from_slice(&self.output_paths);
        if let Some(log_arg) = self.log_path.as_ref() {
            vec.push(log_arg.clone());
        }
        if let Some(xtree_arg) = self.xtree_path.as_ref() {
            vec.push(xtree_arg.clone());
        }
        if let Some(xleak_arg) = self.xleak_path.as_ref() {
            vec.push(xleak_arg.clone());
        }

        vec
    }

    fn generate_file_arg(
        &self,
        arg: &str,
        output_path: &ToolOutputPath,
        tool_runner_dest: Option<&Path>,
        extra_modifier: Option<&str>,
    ) -> OsString {
        let output_path = match (self.trace_children, extra_modifier) {
            (true, Some(modifier)) => output_path.with_modifiers([modifier, "#%p"]),
            (true, None) => output_path.with_modifiers(["#%p"]),
            (false, Some(modifier)) => output_path.with_modifiers([modifier, "#0"]),
            (false, None) => output_path.with_modifiers(["#0"]),
        };

        let path = match tool_runner_dest {
            Some(dest) => dest.join(output_path.file_name()),
            None => output_path.to_path(),
        };

        let mut file_arg = OsString::with_capacity(arg.len().saturating_add(path.len()));
        file_arg.push(arg);
        file_arg.push(path);
        file_arg
    }
}

impl ToolArgsLike for ValgrindArgs {
    fn try_from_raw_tool_args(tool: Tool, raw_tool_args: &[&RawToolArgs]) -> Result<Self> {
        let valgrind_tool = ValgrindTool::try_from(tool).map_err(anyhow::Error::msg)?;
        let mut tool_args = Self::new(valgrind_tool);

        tool_args.try_update(raw_tool_args.iter().flat_map(|args| args.as_slice()))?;

        Ok(tool_args)
    }

    fn try_update<'a, T: Iterator<Item = &'a String>>(&mut self, args: T) -> Result<()> {
        for arg in args {
            let arg = arg.trim();
            match arg.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("--error-exitcode", value)) => {
                    value.clone_into(&mut self.error_exitcode);
                }
                Some((key @ "--trace-children", value)) => {
                    self.trace_children = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some(("--fair-sched", value)) => {
                    self.fair_sched = FairSched::from_str(value)?;
                }
                Some(("--vgdb", value)) => {
                    self.vgdb = Vgdb::from_str(value)?;
                }
                Some((arg, _)) if is_ignored_outfile_argument(arg) => warn!(
                    "Ignoring {} argument '{arg}': Output/Log files of tools are managed by \
                     Gungraun",
                    self.tool.id()
                ),
                Some((arg, _)) if is_ignored_argument(arg) => {
                    warn!("Ignoring {} argument '{arg}'", self.tool.id());
                }
                None if matches!(arg, "-v" | "--verbose") => self.verbose = true,
                None if is_ignored_argument(arg) => {
                    warn!("Ignoring {} argument '{arg}'", self.tool.id());
                }
                None | Some(_) => self.other.push(arg.to_owned()),
            }
        }

        Ok(())
    }
}

impl ValgrindTool {
    fn id(self) -> String {
        Tool::from(self).id()
    }

    fn has_output_file(self) -> bool {
        Tool::from(self).has_output_file()
    }
}

impl TryFrom<Tool> for ValgrindTool {
    type Error = String;

    fn try_from(value: Tool) -> std::result::Result<Self, Self::Error> {
        match value {
            Tool::Callgrind => Ok(Self::Callgrind),
            Tool::Cachegrind => Ok(Self::Cachegrind),
            Tool::DHAT => Ok(Self::DHAT),
            Tool::Memcheck => Ok(Self::Memcheck),
            Tool::Helgrind => Ok(Self::Helgrind),
            Tool::DRD => Ok(Self::DRD),
            Tool::Massif => Ok(Self::Massif),
            Tool::BBV => Ok(Self::BBV),
            Tool::Perf => Err("Invalid valgrind tool: perf".to_owned()),
        }
    }
}

impl Display for Vgdb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Full => "full",
        };
        write!(f, "{string}")
    }
}

impl FromStr for Vgdb {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "no" => Ok(Self::No),
            "yes" => Ok(Self::Yes),
            "full" => Ok(Self::Full),
            _ => Err(anyhow!(
                "Invalid argument for --vgdb. Valid arguments are: 'yes', 'no', 'full'"
            )),
        }
    }
}

/// Returns `true` if this is a generic ignored argument.
pub fn is_ignored_argument(arg: &str) -> bool {
    matches!(
        arg,
        "-h" | "--help"
            | "--help-dyn-options"
            | "--help-debug"
            | "--version"
            | "-q"
            | "--quiet"
            | "--tool"
    )
}

/// Returns `true` if this is an ignored argument related to output or logfiles.
pub fn is_ignored_outfile_argument(arg: &str) -> bool {
    matches!(
        arg,
        "--dhat-out-file"
            | "--massif-out-file"
            | "--callgrind-out-file"
            | "--cachegrind-out-file"
            | "--bb-out-file"
            | "--pc-out-file"
            | "--log-file"
            | "--log-fd"
            | "--log-socket"
            | "--xml"
            | "--xml-file"
            | "--xml-fd"
            | "--xml-socket"
            | "--xml-user-comment"
            | "--xtree-leak-file"
            | "--xtree-memory-file"
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use rstest::rstest;

    use super::*;
    use crate::fixtures::valgrind_args_f;

    fn assert_contains_args<const N: usize>(actual: &[OsString], expected: [&str; N]) {
        for expected in expected {
            assert!(
                actual.iter().any(|arg| arg.to_string_lossy() == expected),
                "expected serialized arg {expected}"
            );
        }
    }

    fn strings<const N: usize>(args: [&str; N]) -> Vec<String> {
        args.into_iter().map(str::to_owned).collect()
    }

    #[rstest]
    #[case::error_exitcode(
        &["--error-exitcode=99"],
        valgrind_args_f().error_exitcode("99").fx()
    )]
    #[case::trace_children(
        &["--trace-children=no"],
        valgrind_args_f().trace_children(false).fx()
    )]
    #[case::fair_sched(
        &["--fair-sched=no"],
        valgrind_args_f().fair_sched(FairSched::No).fx()
    )]
    #[case::long_verbose(&["--verbose"], valgrind_args_f().verbose(true).fx())]
    #[case::short_verbose(&["-v"], valgrind_args_f().verbose(true).fx())]
    #[case::vgdb(&["--vgdb=yes"], valgrind_args_f().vgdb(Vgdb::Yes).fx())]
    #[case::vgdb(&["--vgdb=no"], valgrind_args_f().vgdb(Vgdb::No).fx())]
    #[case::vgdb(&["--vgdb=full"], valgrind_args_f().vgdb(Vgdb::Full).fx())]
    #[case::outfile_is_ignored(&["--log-file=some"], valgrind_args_f().fx())]
    #[case::other(
        &["--some-arg=yes"],
        valgrind_args_f()
            .other(strings(["--some-arg=yes"]))
            .fx()
    )]
    fn test_try_from_raw_tool_args(#[case] args: &[&str], #[case] expected: ValgrindArgs) {
        let actual =
            ValgrindArgs::try_from_raw_tool_args(Tool::Memcheck, &[&RawToolArgs::from_iter(args)])
                .unwrap();

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::trace_children(&["--trace-children=something"])]
    #[case::fair_sched(&["--fair-sched=something"])]
    #[case::vgdb(&["--vgdb=something"])]
    fn test_try_from_raw_tool_args_when_invalid_then_error(#[case] input: &[&str]) {
        ValgrindArgs::try_from_raw_tool_args(Tool::Memcheck, &[&RawToolArgs::from_iter(input)])
            .unwrap_err();
    }

    #[test]
    fn test_to_vec() {
        let args = valgrind_args_f()
            .error_exitcode("99")
            .fair_sched(FairSched::No)
            .trace_children(false)
            .fx();

        let actual = args.to_vec();

        assert_contains_args(
            &actual,
            [
                "--tool=memcheck",
                "--error-exitcode=99",
                "--trace-children=no",
                "--fair-sched=no",
                "--vgdb=no",
            ],
        );
    }

    #[test]
    fn test_to_vec_when_verbose_and_other_args() {
        let args = valgrind_args_f()
            .verbose(true)
            .other(strings(["--some-arg=yes", "--another-some-arg"]))
            .fx();

        let actual = args.to_vec();

        assert_contains_args(
            &actual,
            ["--verbose", "--some-arg=yes", "--another-some-arg"],
        );
    }
}
