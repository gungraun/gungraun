//! The module containing the command line arguments for Cachegrind

use anyhow::Result;

use crate::api::{RawToolArgs, Tool};
use crate::error::Error;
use crate::runner::tool::args::{ToolArgsLike, ValgrindArgs, ValgrindTool, defaults};
use crate::util::{bool_to_yesno, yesno_to_bool};

/// The command-line arguments
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachegrindArgs {
    cache_sim: bool,
    d1: String,
    i1: String,
    ll: String,
    valgrind: ValgrindArgs,
}

impl ToolArgsLike for CachegrindArgs {
    fn try_from_raw_tool_args(tool: Tool, raw_tool_args: &[&RawToolArgs]) -> Result<Self> {
        debug_assert_eq!(tool, Tool::Cachegrind);

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
                None | Some(_) => self.valgrind.try_update(std::iter::once(arg))?,
            }
        }
        Ok(())
    }
}

impl Default for CachegrindArgs {
    fn default() -> Self {
        Self {
            i1: defaults::I1.into(),
            d1: defaults::D1.into(),
            ll: defaults::LL.into(),
            cache_sim: defaults::CACHE_SIM,
            valgrind: ValgrindArgs::new(ValgrindTool::Cachegrind),
        }
    }
}

impl From<CachegrindArgs> for ValgrindArgs {
    fn from(value: CachegrindArgs) -> Self {
        let mut valgrind = value.valgrind;
        let other = vec![
            format!("--I1={}", &value.i1),
            format!("--D1={}", &value.d1),
            format!("--LL={}", &value.ll),
            format!("--cache-sim={}", bool_to_yesno(value.cache_sim)),
        ];
        valgrind.other.extend(other);

        valgrind
    }
}

#[cfg(test)]
mod tests {
    use bon::builder;
    use rstest::rstest;

    use super::*;
    use crate::runner::tool::args::FairSched;

    fn default_cachegrind_other_args() -> Vec<String> {
        strings([
            "--I1=32768,8,64",
            "--D1=32768,8,64",
            "--LL=8388608,16,64",
            "--cache-sim=yes",
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
        let mut args = ValgrindArgs::new(ValgrindTool::Cachegrind);
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
    pub fn cachegrind_args_f(
        i1: Option<&str>,
        d1: Option<&str>,
        ll: Option<&str>,
        cache_sim: Option<bool>,
        valgrind: Option<ValgrindArgs>,
    ) -> CachegrindArgs {
        let mut args = CachegrindArgs::default();
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
        if let Some(value) = valgrind {
            args.valgrind = value;
        }

        args
    }

    #[rstest]
    #[case::i1(&["--I1=some"], cachegrind_args_f().i1("some").fx())]
    #[case::d1(&["--D1=some"], cachegrind_args_f().d1("some").fx())]
    #[case::ll(&["--LL=some"], cachegrind_args_f().ll("some").fx())]
    #[case::cache_sim(&["--cache-sim=no"], cachegrind_args_f().cache_sim(false).fx())]
    #[case::valgrind_special(
        &["--fair-sched=no"],
        cachegrind_args_f()
            .valgrind(valgrind_args_f().fair_sched(FairSched::No).fx())
            .fx()
    )]
    #[case::valgrind_other(
        &["--some-arg=yes"],
        cachegrind_args_f()
            .valgrind(valgrind_args_f().other(vec!["--some-arg=yes".to_owned()]).fx())
            .fx()
    )]
    fn test_try_from_raw_tool_args(#[case] args: &[&str], #[case] expected: CachegrindArgs) {
        let actual = CachegrindArgs::try_from_raw_tool_args(
            Tool::Cachegrind,
            &[&RawToolArgs::from_iter_ignore_flag(args)],
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_from_cachegrind_args_defaults() {
        let expected = valgrind_args_f()
            .other(default_cachegrind_other_args())
            .fx();

        let actual = ValgrindArgs::from(cachegrind_args_f().fx());

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_from_cachegrind_args_when_tool_specific_values() {
        let args = cachegrind_args_f()
            .i1("i1")
            .d1("d1")
            .ll("ll")
            .cache_sim(false)
            .fx();
        let expected = valgrind_args_f()
            .other(strings(["--I1=i1", "--D1=d1", "--LL=ll", "--cache-sim=no"]))
            .fx();

        let actual = ValgrindArgs::from(args);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_from_cachegrind_args_when_valgrind_args_then_appends_tool_specific_last() {
        let args = cachegrind_args_f()
            .i1("tool")
            .valgrind(
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
            ]))
            .fx();

        let actual = ValgrindArgs::from(args);

        assert_eq!(actual, expected);
    }
}
