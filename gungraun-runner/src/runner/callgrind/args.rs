//! The module containing the command line arguments for callgrind
use std::collections::VecDeque;

use anyhow::Result;
use log::warn;

use crate::api::{RawToolArgs, Tool};
use crate::error::Error;
use crate::runner::tool::args::{ToolArgsLike, ValgrindArgs, ValgrindTool, defaults};
use crate::util::{bool_to_yesno, yesno_to_bool};

/// The command-line arguments
#[expect(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallgrindArgs {
    cache_sim: bool,
    /// --combine-dumps is currently not supported by the Callgrind parsers, so we print a warning
    combine_dumps: bool,
    compress_pos: bool,
    compress_strings: bool,
    d1: String,
    dump_instr: bool,
    dump_line: bool,
    i1: String,
    ll: String,
    separate_threads: bool,
    toggle_collect: VecDeque<String>,
    valgrind_args: ValgrindArgs,
}

impl ToolArgsLike for CallgrindArgs {
    fn try_from_raw_tool_args(tool: Tool, raw_tool_args: &[&RawToolArgs]) -> Result<Self> {
        debug_assert_eq!(tool, Tool::Callgrind);

        let mut default = Self::default();
        default.try_update(raw_tool_args.iter().flat_map(|s| s.as_slice()))?;
        Ok(default)
    }

    fn try_update<'a, T: Iterator<Item = &'a String>>(&mut self, args: T) -> Result<()> {
        for arg in args {
            let trimmed = arg.trim();
            match trimmed.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("--I1", value)) => value.clone_into(&mut self.i1),
                Some(("--D1", value)) => value.clone_into(&mut self.d1),
                Some(("--LL", value)) => value.clone_into(&mut self.ll),
                Some((key @ "--cache-sim", value)) => {
                    self.cache_sim = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some(("--toggle-collect", value)) => {
                    self.toggle_collect.push_back(value.to_owned());
                }
                Some((key @ "--dump-instr", value)) => {
                    self.dump_instr = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some((key @ "--dump-line", value)) => {
                    self.dump_line = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some((key @ "--separate-threads", value)) => {
                    self.separate_threads = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some((
                    key @ ("--combine-dumps" | "--compress-strings" | "--compress-pos"),
                    value,
                )) => {
                    warn!("Ignoring unsupported callgrind argument: '{key}={value}'");
                }
                None | Some(_) => self.valgrind_args.try_update(std::iter::once(arg))?,
            }
        }
        Ok(())
    }
}

impl Default for CallgrindArgs {
    fn default() -> Self {
        Self {
            // Set some reasonable cache sizes. The exact sizes matter less than having fixed sizes,
            // since otherwise callgrind would take them from the CPU and make benchmark runs
            // even more incomparable between machines.
            i1: defaults::I1.into(),
            d1: defaults::D1.into(),
            ll: defaults::LL.into(),
            cache_sim: defaults::CACHE_SIM,
            compress_pos: defaults::COMPRESS_POS,
            compress_strings: defaults::COMPRESS_STRINGS,
            combine_dumps: defaults::COMBINE_DUMPS,
            dump_line: defaults::DUMP_LINE,
            dump_instr: defaults::DUMP_INSTR,
            toggle_collect: VecDeque::default(),
            separate_threads: defaults::SEPARATE_THREADS,
            valgrind_args: ValgrindArgs::new(ValgrindTool::Callgrind),
        }
    }
}

impl From<CallgrindArgs> for ValgrindArgs {
    fn from(value: CallgrindArgs) -> Self {
        let mut valgrind = value.valgrind_args;
        let other = vec![
            format!("--I1={}", &value.i1),
            format!("--D1={}", &value.d1),
            format!("--LL={}", &value.ll),
            format!("--cache-sim={}", bool_to_yesno(value.cache_sim)),
            format!(
                "--compress-strings={}",
                bool_to_yesno(value.compress_strings)
            ),
            format!("--compress-pos={}", bool_to_yesno(value.compress_pos)),
            format!("--dump-line={}", bool_to_yesno(value.dump_line)),
            format!("--dump-instr={}", bool_to_yesno(value.dump_instr)),
            format!("--combine-dumps={}", bool_to_yesno(value.combine_dumps)),
            format!(
                "--separate-threads={}",
                bool_to_yesno(value.separate_threads)
            ),
        ];
        valgrind.other.extend(other);
        valgrind.other.extend(
            value
                .toggle_collect
                .into_iter()
                .map(|s| format!("--toggle-collect={s}")),
        );

        valgrind
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bon::builder;
    use rstest::rstest;

    use super::*;
    use crate::runner::tool::args::FairSched;

    fn default_callgrind_other_args() -> Vec<String> {
        strings([
            "--I1=32768,8,64",
            "--D1=32768,8,64",
            "--LL=8388608,16,64",
            "--cache-sim=yes",
            "--compress-strings=no",
            "--compress-pos=no",
            "--dump-line=yes",
            "--dump-instr=no",
            "--combine-dumps=no",
            "--separate-threads=yes",
        ])
    }

    fn strings<const N: usize>(args: [&str; N]) -> Vec<String> {
        args.into_iter().map(str::to_owned).collect()
    }

    #[builder(finish_fn = "fx")]
    pub fn valgrind_args_f(
        i1: Option<&str>,
        fair_sched: Option<FairSched>,
        other: Option<Vec<String>>,
    ) -> ValgrindArgs {
        let mut args = ValgrindArgs::new(ValgrindTool::Callgrind);
        if let Some(value) = i1 {
            args.other.push(format!("--I1={value}"));
        }
        if let Some(value) = fair_sched {
            args.fair_sched = value;
        }
        if let Some(value) = other {
            args.other.extend(value);
        }

        args
    }

    #[builder(finish_fn = "fx")]
    pub fn callgrind_args_f(
        i1: Option<&str>,
        d1: Option<&str>,
        ll: Option<&str>,
        cache_sim: Option<bool>,
        toggle_collect: Option<VecDeque<String>>,
        dump_instr: Option<bool>,
        dump_line: Option<bool>,
        separate_threads: Option<bool>,
        valgrind_args: Option<ValgrindArgs>,
    ) -> CallgrindArgs {
        let mut args = CallgrindArgs::default();
        if let Some(value) = i1 {
            args.i1 = value.to_owned();
        }
        if let Some(value) = d1 {
            args.d1 = value.to_owned();
        }
        if let Some(value) = ll {
            args.ll = value.to_owned();
        }
        if let Some(value) = cache_sim {
            args.cache_sim = value;
        }
        if let Some(value) = toggle_collect {
            args.toggle_collect = value;
        }
        if let Some(value) = dump_instr {
            args.dump_instr = value;
        }
        if let Some(value) = dump_line {
            args.dump_line = value;
        }
        if let Some(value) = separate_threads {
            args.separate_threads = value;
        }
        if let Some(value) = valgrind_args {
            args.valgrind_args = value;
        }

        args
    }

    #[rstest]
    #[case::i1(&["--I1=some"], callgrind_args_f().i1("some").fx())]
    #[case::d1(&["--D1=some"], callgrind_args_f().d1("some").fx())]
    #[case::ll(&["--LL=some"], callgrind_args_f().ll("some").fx())]
    #[case::cache_sim(&["--cache-sim=no"], callgrind_args_f().cache_sim(false).fx())]
    #[case::toggle_collect(
        &["--toggle-collect=main"],
        callgrind_args_f()
            .toggle_collect(VecDeque::from(["main".to_owned()]))
            .fx()
    )]
    #[case::dump_instr(&["--dump-instr=yes"], callgrind_args_f().dump_instr(true).fx())]
    #[case::dump_line(&["--dump-line=no"], callgrind_args_f().dump_line(false).fx())]
    #[case::separate_threads(
        &["--separate-threads=no"],
        callgrind_args_f().separate_threads(false).fx()
    )]
    #[case::combine_dumps_is_ignored(&["--combine-dumps=yes"], callgrind_args_f().fx())]
    #[case::compress_strings_is_ignored(
        &["--compress-strings=yes"],
        callgrind_args_f().fx()
    )]
    #[case::compress_pos_is_ignored(&["--compress-pos=yes"], callgrind_args_f().fx())]
    #[case::valgrind_special(
        &["--fair-sched=no"],
        callgrind_args_f()
            .valgrind_args(valgrind_args_f().fair_sched(FairSched::No).fx())
            .fx()
    )]
    #[case::valgrind_other(
        &["--some-arg=yes"],
        callgrind_args_f()
            .valgrind_args(valgrind_args_f().other(vec!["--some-arg=yes".to_owned()]).fx())
            .fx()
    )]
    fn test_try_from_raw_tool_args(#[case] args: &[&str], #[case] expected: CallgrindArgs) {
        let actual = CallgrindArgs::try_from_raw_tool_args(
            Tool::Callgrind,
            &[&RawToolArgs::from_iter_ignore_flag(args)],
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_from_callgrind_args_when_defaults() {
        let expected = valgrind_args_f().other(default_callgrind_other_args()).fx();

        let actual = ValgrindArgs::from(callgrind_args_f().fx());

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_from_callgrind_args_when_tool_specific_values() {
        let args = callgrind_args_f()
            .i1("i1")
            .d1("d1")
            .ll("ll")
            .cache_sim(false)
            .dump_line(false)
            .dump_instr(true)
            .separate_threads(false)
            .fx();
        let expected = valgrind_args_f()
            .other(strings([
                "--I1=i1",
                "--D1=d1",
                "--LL=ll",
                "--cache-sim=no",
                "--compress-strings=no",
                "--compress-pos=no",
                "--dump-line=no",
                "--dump-instr=yes",
                "--combine-dumps=no",
                "--separate-threads=no",
            ]))
            .fx();

        let actual = ValgrindArgs::from(args);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_from_callgrind_args_when_toggle_collect() {
        let args = callgrind_args_f()
            .toggle_collect(VecDeque::from(["main".to_owned(), "helper".to_owned()]))
            .fx();
        let mut other = default_callgrind_other_args();
        other.extend(strings([
            "--toggle-collect=main",
            "--toggle-collect=helper",
        ]));
        let expected = valgrind_args_f().other(other).fx();

        let actual = ValgrindArgs::from(args);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_from_callgrind_args_when_valgrind_args_then_appends_tool_specific_last() {
        let args = callgrind_args_f()
            .i1("tool")
            .valgrind_args(
                valgrind_args_f()
                    .fair_sched(FairSched::No)
                    .other(strings(["--unknown=yes", "--I1=generic"]))
                    .fx(),
            )
            .fx();
        let expected = valgrind_args_f()
            .fair_sched(FairSched::No)
            .other(strings([
                "--unknown=yes",
                "--I1=generic",
                "--I1=tool",
                "--D1=32768,8,64",
                "--LL=8388608,16,64",
                "--cache-sim=yes",
                "--compress-strings=no",
                "--compress-pos=no",
                "--dump-line=yes",
                "--dump-instr=no",
                "--combine-dumps=no",
                "--separate-threads=yes",
            ]))
            .fx();

        let actual = ValgrindArgs::from(args);

        assert_eq!(actual, expected);
    }
}
