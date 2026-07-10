//! Build and filter command-line arguments for `perf stat` and `perf record`.
//!
//! This module keeps Gungraun-managed arguments, such as output paths, control file descriptors,
//! and event selection, separate from user-provided raw perf arguments.

use std::ffi::OsString;
use std::fmt::Display;
use std::path::Path;

use log::warn;

use crate::api::{PERF_ACK_FD_WRITE, PERF_CTL_FD_READ, RawToolArgs, Tool};
use crate::runner::tool::args::ToolArgsLike;
use crate::runner::tool::path::ToolOutputPath;

/// The default event list used when no perf events are configured explicitly.
pub const DEFAULT_PERF_EVENTS: &str = "instructions:u,cycles:u,task-clock,cpu-clock,faults,\
                                       context-switches,branch-misses,cache-misses";
/// A supported perf subcommand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PerfTool {
    /// The `perf stat` subcommand.
    #[default]
    Stat,
    /// The `perf record` subcommand.
    Record,
}

/// Shared arguments for any supported perf subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfArgs {
    args_after_events: Vec<String>,
    control: String,
    delay: String,
    events: Vec<String>,
    other: Vec<String>,
    output_path: Option<OsString>,
    tool: PerfTool,
}

/// Arguments specific to `perf stat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfStatArgs {
    big_num: bool,
    json: bool,
    perf_args: PerfArgs,
}

/// Arguments specific to `perf record`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfRecordArgs {
    exclude_perf: bool,
    /// The filter arguments stored as `--filter=<filter>`
    filter: Vec<String>,
    perf_args: PerfArgs,
}

impl PerfArgs {
    /// Ignore event selector arguments common between perf stat and perf record
    fn is_ignored_event_argument(arg: &str) -> bool {
        matches!(arg, "-e" | "--event" | "--pfm-events")
    }

    /// Ignored output file arguments common between perf stat and perf record
    fn is_ignored_outfile_argument(arg: &str) -> bool {
        matches!(arg, "-o" | "--output")
    }

    /// Ignored arguments common between perf stat and perf record
    fn is_ignored_value_argument(arg: &str) -> bool {
        matches!(
            arg,
            "--control" | "-p" | "--pid" | "-t" | "--tid" | "-D" | "--delay"
        )
    }

    /// Returns `true` when these arguments target `perf record`.
    pub fn is_record(&self) -> bool {
        matches!(self.tool, PerfTool::Record)
    }

    /// Creates default arguments for a perf subcommand.
    ///
    /// The defaults configure Gungraun's fixed control file descriptors and delay perf until it is
    /// explicitly enabled by the runner.
    pub fn new(tool: PerfTool) -> Self {
        Self {
            tool,
            other: Vec::default(),
            events: vec![],
            output_path: None,
            control: format!("fd:{PERF_CTL_FD_READ},{PERF_ACK_FD_WRITE}"),
            delay: "-1".to_owned(),
            args_after_events: Vec::default(),
        }
    }

    /// Serializes these arguments into the order expected by the perf command line.
    pub fn to_vec(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = vec![self.tool.to_string().into()];

        vec.push(format!("--control={}", self.control).into());
        vec.push(format!("--delay={}", &self.delay).into());

        vec.extend(self.events.iter().map(|e| format!("--event={e}").into()));

        vec.extend(self.args_after_events.iter().map(OsString::from));
        vec.extend(self.other.iter().map(OsString::from));

        if let Some(out_path) = &self.output_path {
            let mut arg = OsString::from("--output=");
            arg.push(out_path);
            vec.push(arg);
        }

        vec
    }

    /// Sets the managed perf output argument.
    ///
    /// `perf stat` writes to `output_path` directly. `perf record` writes its data file variant so
    /// the analyzer can consume the recorded perf data separately from perf's log output.
    pub fn set_output_arg(
        &mut self,
        output_path: &ToolOutputPath,
        tool_runner_dest: Option<&Path>,
    ) {
        let path = match self.tool {
            PerfTool::Stat => match tool_runner_dest {
                Some(dest) => dest.join(output_path.file_name()).into(),
                None => output_path.to_path().into(),
            },
            PerfTool::Record => {
                let data_output = output_path.to_data_output();
                match tool_runner_dest {
                    Some(dest) => dest.join(data_output.file_name()).into(),
                    None => data_output.to_path().into(),
                }
            }
        };

        self.output_path = Some(path);
    }

    /// Enables sampling mode for `perf stat`.
    ///
    /// When enabled, this adds `-r 0` so perf repeats until the runner stops it. Other perf
    /// subcommands are unchanged.
    pub fn use_sampling(&mut self, yes: bool) {
        match self.tool {
            PerfTool::Stat if yes => {
                self.other.extend(["-r".to_owned(), "0".to_owned()]);
            }
            _ => {}
        }
    }

    /// Adds a perf event list to the command line arguments.
    pub fn add_events(&mut self, events: &str) {
        self.events.push(events.to_owned());
    }

    /// Applies raw user arguments while dropping arguments managed by Gungraun.
    ///
    /// Output paths, control file descriptors, delays, process IDs, and event selection are owned
    /// by the runner. Unknown or unmanaged arguments are preserved in insertion order.
    fn update<'a, T>(&mut self, args: T)
    where
        T: Iterator<Item = &'a String>,
    {
        let mut args = args.peekable();

        while let Some(current) = args.next() {
            let arg = current.trim();
            match arg.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some((flag, _)) if Self::is_ignored_event_argument(flag) => {
                    warn!("Ignoring perf argument '{flag}': Setting events is managed by Gungraun");
                }
                Some((flag, _)) if Self::is_ignored_outfile_argument(flag) => {
                    warn!(
                        "Ignoring perf argument '{flag}': Output files of tools are managed by \
                         Gungraun",
                    );
                }
                Some((flag, _)) if Self::is_ignored_value_argument(flag) => {
                    warn!("Ignoring perf argument '{flag}'");
                }
                // value argument
                Some((flag, _)) if flag.starts_with('-') => {
                    self.other.push(arg.to_owned());
                }
                None if Self::is_ignored_event_argument(arg) => {
                    let _ = args.next();
                    warn!("Ignoring perf argument '{arg}': Setting events is managed by Gungraun");
                }
                None if Self::is_ignored_outfile_argument(arg) => {
                    let _ = args.next();
                    warn!(
                        "Ignoring perf argument '{arg}': Output files of tools are managed by \
                         Gungraun",
                    );
                }
                None if Self::is_ignored_value_argument(arg) => {
                    let _ = args.next();
                    warn!("Ignoring perf argument '{arg}'");
                }
                // value argument
                None if arg.starts_with('-')
                    && args.peek().is_some_and(|a| !a.starts_with('-')) =>
                {
                    self.other.push(arg.to_owned());
                    self.other.push(args.next().unwrap().to_owned());
                }
                // non-value argument
                None if arg.starts_with('-') => {
                    self.other.push(arg.to_owned());
                }
                // positional
                None | Some(_) => {
                    warn!("Ignoring positional argument '{arg}'");
                }
            }
        }
    }
}

impl From<PerfRecordArgs> for PerfArgs {
    fn from(value: PerfRecordArgs) -> Self {
        let mut perf_args = value.perf_args;

        if value.exclude_perf {
            perf_args
                .args_after_events
                .push("--exclude_perf".to_owned());
        }
        perf_args.args_after_events.extend(value.filter);

        perf_args
    }
}

impl From<PerfStatArgs> for PerfArgs {
    fn from(value: PerfStatArgs) -> Self {
        let mut perf_args = value.perf_args;

        if value.big_num {
            perf_args.other.push("--big-num".into());
        } else {
            perf_args.other.push("--no-big-num".into());
        }

        if value.json {
            perf_args.other.push("-j".into());
        }

        perf_args
    }
}

impl PerfRecordArgs {
    fn is_ignored_value_argument(arg: &str) -> bool {
        matches!(arg, "-u" | "--uid")
    }
}

impl ToolArgsLike for PerfRecordArgs {
    fn try_from_raw_tool_args(tool: Tool, raw_tool_args: &[&RawToolArgs]) -> anyhow::Result<Self> {
        debug_assert_eq!(tool, Tool::Perf);

        let mut tool_args = Self::default();
        tool_args.try_update(raw_tool_args.iter().flat_map(|args| args.as_slice()))?;
        Ok(tool_args)
    }

    fn try_update<'a, T>(&mut self, args: T) -> anyhow::Result<()>
    where
        T: Iterator<Item = &'a String>,
    {
        let mut args = args.peekable();
        let mut remainder = vec![];

        while let Some(arg) = args.next() {
            let trimmed = arg.trim();
            match trimmed.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some((flag, _)) if Self::is_ignored_value_argument(flag) => {
                    warn!("Ignoring perf argument: '{flag}'");
                }
                // TODO: STOPPED HERE, and thinking about fp vs dwarf as default but possibly dwarf
                // only for flamegraphs, otherwise no --call-graph or -g options, That's also where
                // I stopped in man perf record sorting out perf record ignored arguments
                Some(("--filter", _)) => {
                    self.filter.push(trimmed.to_owned());
                }
                None if Self::is_ignored_value_argument(trimmed) => {
                    warn!("Ignoring perf argument: '{arg}'");
                }
                None if trimmed == "--exclude-perf" => self.exclude_perf = true,
                None if trimmed == "--filter" && args.peek().is_some() => {
                    self.filter.push(format!(
                        "--filter={}",
                        args.next().expect("A next element should be present")
                    ));
                }
                // value argument
                None if arg.starts_with('-')
                    && args.peek().is_some_and(|a| !a.starts_with('-')) =>
                {
                    remainder.push(arg);
                    remainder.push(args.next().unwrap());
                }
                _ => remainder.push(arg),
            }
        }

        self.perf_args.update(remainder.into_iter());

        Ok(())
    }
}

impl PerfStatArgs {
    fn is_ignored_non_value_argument(arg: &str) -> bool {
        matches!(
            arg,
            "--append"
                | "-B"
                | "--big-num"
                | "-v"
                | "--verbose"
                | "--interval-clear"
                | "-T"
                | "--transaction"
                | "--quiet"
        )
    }

    fn is_ignored_repeat_argument(arg: &str) -> bool {
        matches!(arg, "-r" | "--repeat")
    }

    fn is_ignored_subcommand(arg: &str) -> bool {
        matches!(arg, "record" | "report")
    }

    fn is_ignored_value_argument(arg: &str) -> bool {
        matches!(
            arg,
            "--log-fd"
                | "--control"
                | "-p"
                | "--pid"
                | "-t"
                | "--tid"
                | "-b"
                | "--bpf-prog"
                | "-D"
                | "--delay"
                | "-I"
                | "--interval-print"
                | "--interval-count"
                | "-x"
                | "--field-separator"
        )
    }
}

impl ToolArgsLike for PerfStatArgs {
    fn try_from_raw_tool_args(tool: Tool, raw_tool_args: &[&RawToolArgs]) -> anyhow::Result<Self> {
        debug_assert_eq!(tool, Tool::Perf);

        let mut tool_args = Self::default();
        tool_args.try_update(raw_tool_args.iter().flat_map(|args| args.as_slice()))?;
        Ok(tool_args)
    }

    fn try_update<'a, T>(&mut self, args: T) -> anyhow::Result<()>
    where
        T: Iterator<Item = &'a String>,
    {
        let mut args = args.peekable();
        let mut remainder = vec![];

        while let Some(arg) = args.next() {
            let trimmed = arg.trim();
            match trimmed.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some((flag, _)) if Self::is_ignored_repeat_argument(arg) => {
                    warn!(
                        "Ignoring perf argument '{flag}': Repetitions are managed by Gungraun \
                         using sampling"
                    );
                }
                Some((flag, _)) if Self::is_ignored_value_argument(flag) => {
                    warn!("Ignoring perf argument '{flag}'");
                }
                None if Self::is_ignored_repeat_argument(arg) => {
                    warn!(
                        "Ignoring perf argument '{arg}': Repetitions are managed by Gungraun \
                         using sampling"
                    );
                }
                None if Self::is_ignored_value_argument(arg) => {
                    let _ = args.next();
                    warn!("Ignoring perf argument '{arg}'");
                }
                None if Self::is_ignored_non_value_argument(arg) => {
                    warn!("Ignoring perf argument '{arg}'");
                }
                None if Self::is_ignored_subcommand(arg) => {
                    warn!(
                        "Ignoring perf argument: '{arg}' and all following arguments: Perf stat \
                         subcommands are not allowed"
                    );
                    break;
                }
                // value argument
                None if arg.starts_with('-')
                    && args.peek().is_some_and(|a| !a.starts_with('-')) =>
                {
                    remainder.push(arg);
                    remainder.push(args.next().unwrap());
                }
                _ => remainder.push(arg),
            }
        }

        self.perf_args.update(remainder.into_iter());

        Ok(())
    }
}

impl Default for PerfStatArgs {
    fn default() -> Self {
        Self {
            big_num: false,
            json: true,
            perf_args: PerfArgs::new(PerfTool::Stat),
        }
    }
}

impl Default for PerfRecordArgs {
    fn default() -> Self {
        Self {
            exclude_perf: false,
            filter: vec![],
            perf_args: PerfArgs::new(PerfTool::Record),
        }
    }
}

impl Display for PerfTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stat => f.write_str("stat"),
            Self::Record => f.write_str("record"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn perf_stat_args(args: &[&str]) -> anyhow::Result<PerfStatArgs> {
        PerfStatArgs::try_from_raw_tool_args(Tool::Perf, &[&RawToolArgs::from_iter(args)])
    }

    #[rstest]
    #[case::output_equals(&["--output=custom.json"])]
    #[case::output_short_space(&["-o", "custom.log"])]
    #[case::output_long_space(&["--output", "custom.log"])]
    fn test_ignores_managed_output_arguments(#[case] input: &[&str]) {
        let args = perf_stat_args(input).unwrap();

        assert!(args.perf_args.output_path.is_none());
        assert!(args.perf_args.other.is_empty());
    }

    #[rstest]
    #[case::delay_equals(&["--delay=10"])]
    #[case::control_equals(&["--control=fd:1,2"])]
    #[case::pid_long_equals(&["--pid=123"])]
    #[case::pid_short_equals(&["-p=123"])]
    #[case::log_fd_equals(&["--log-fd=1"])]
    fn test_ignores_managed_value_arguments_with_equals_form(#[case] input: &[&str]) {
        let args = perf_stat_args(input).unwrap();
        assert_eq!(PerfStatArgs::default(), args);
    }

    #[rstest]
    #[case::delay_space(&["--delay", "10"])]
    #[case::control_space(&["--control", "fd:1,2"])]
    #[case::pid_long_space(&["--pid", "123"])]
    #[case::pid_short_space(&["-p", "123"])]
    #[case::log_fd_space(&["--log-fd", "1"])]
    fn test_ignores_managed_value_arguments_with_space_separated_values(#[case] input: &[&str]) {
        let args = perf_stat_args(input).unwrap();
        assert_eq!(PerfStatArgs::default(), args);
    }

    #[rstest]
    #[case::unknown_then_output_space(
        &["--metric-only", "--output", "custom.json"],
        &["--metric-only"]
    )]
    #[case::unknown_then_output_equals(
        &["--metric-only", "--output=custom.json"],
        &["--metric-only"]
    )]
    fn test_keeps_unknown_arguments_without_consuming_following_managed_arguments(
        #[case] input: &[&str],
        #[case] expected_other: &[&str],
    ) {
        let args = perf_stat_args(input).unwrap();

        assert_eq!(expected_other, args.perf_args.other);
        assert!(args.perf_args.output_path.is_none());
    }

    #[rstest]
    #[case::event_equals(&["--event=branches"])]
    #[case::event_long_space(&["--event", "branches"])]
    #[case::event_short_space(&["-e", "branches"])]
    fn test_ignores_raw_event_arguments(#[case] input: &[&str]) {
        let args = perf_stat_args(input).unwrap();

        assert_eq!(PerfStatArgs::default(), args);
    }

    #[rstest]
    #[case::short_event("-e")]
    #[case::long_event("--event")]
    fn test_ignores_event_argument_without_value(#[case] input: &str) {
        let args = perf_stat_args(&[input]).unwrap();

        assert_eq!(PerfStatArgs::default(), args);
    }
}
