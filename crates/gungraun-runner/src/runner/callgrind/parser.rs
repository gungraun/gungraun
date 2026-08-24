//! Module containing the basic callgrind parser elements
use std::cmp::Ordering;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Result, anyhow, bail};
use log::{trace, warn};
use serde::{Deserialize, Serialize};
use simplematch::DoWild;

use super::model::{Metrics, Positions};
use crate::api::EventKind;
use crate::runner::DEFAULT_TOGGLE;
use crate::runner::tool::parser::ParserOutput;
use crate::runner::tool::path::ToolOutputPath;

/// The properties and header data of a callgrind output file
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallgrindProperties {
    /// The executed command with command-line arguments
    pub cmd: Option<String>,
    /// The "creator" of this output file
    pub creator: Option<String>,
    /// The `desc:` fields
    pub desc: Vec<String>,
    /// The prototype for all metrics in this file
    pub metrics_prototype: Metrics,
    /// The part number
    pub part: Option<u64>,
    /// The pid
    pub pid: Option<i32>,
    /// The prototype for all positions in this file
    pub positions_prototype: Positions,
    /// The thread
    pub thread: Option<usize>,
}

/// The `Sentinel` function to search for in the haystack
///
/// # Developer notes
///
/// Refactor: This struct was named `Sentinel` but the usage changed and it would better be named
/// `Needle` since it is used to seek for a specific function in the haystack of functions of the
/// output file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sentinel(String);

/// A callgrind specific parser trait
pub trait CallgrindParser {
    /// The output of the parser
    type Output;

    /// Parse all callgrind output files of this [`ToolOutputPath`]
    fn parse(
        &self,
        output: &ToolOutputPath,
    ) -> Result<Vec<(PathBuf, CallgrindProperties, Self::Output)>> {
        let paths = output.sanitized_paths()?;
        let mut results: Vec<(PathBuf, CallgrindProperties, Self::Output)> =
            Vec::with_capacity(paths.len());
        for path in paths {
            let parsed = self.parse_single(&path).map(|(p, c)| (path, p, c))?;

            let position = results
                .binary_search_by(|probe| probe.1.compare_target_ids(&parsed.1))
                .unwrap_or_else(|e| e);

            results.insert(position, parsed);
        }

        Ok(results)
    }

    /// Parse a single callgrind output file
    fn parse_single(&self, path: &Path) -> Result<(CallgrindProperties, Self::Output)>;
}

impl CallgrindProperties {
    /// Compare by target ids `pid`, `part` and `thread`
    ///
    /// Highest precedence takes `pid`. Second is `part` and third is `thread` all sorted ascending.
    /// See also [Callgrind
    /// Format](https://valgrind.org/docs/manual/cl-format.html#cl-format.reference.grammar)
    pub fn compare_target_ids(&self, other: &Self) -> Ordering {
        self.pid.cmp(&other.pid).then_with(|| {
            self.thread
                .cmp(&other.thread)
                .then_with(|| self.part.cmp(&other.part))
        })
    }
}

impl From<ParserOutput> for CallgrindProperties {
    fn from(value: ParserOutput) -> Self {
        Self {
            metrics_prototype: Metrics::default(),
            positions_prototype: Positions::default(),
            pid: Some(value.header.pid),
            thread: value.header.thread,
            part: value.header.part,
            desc: value.header.desc,
            cmd: Some(value.header.command),
            creator: None,
        }
    }
}

impl Sentinel {
    /// Creates a new Sentinel.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gungraun_runner::runner::callgrind::parser::Sentinel;
    ///
    /// let _ = Sentinel::new("main");
    /// let _ = Sentinel::new("main::*");
    /// ```
    pub fn new<T>(value: T) -> Self
    where
        T: Into<String>,
    {
        Self(value.into())
    }

    /// Creates a new `Sentinel` from this module path.
    pub fn from_path(module: &str, function: &str) -> Self {
        Self::new(format!("{module}::{function}"))
    }

    /// Creates a new `Sentinel` from the segments of a module path.
    pub fn from_segments<I, T>(segments: T) -> Self
    where
        I: AsRef<str>,
        T: AsRef<[I]>,
    {
        let joined = if let Some((first, suffix)) = segments.as_ref().split_first() {
            suffix.iter().fold(first.as_ref().to_owned(), |mut a, b| {
                a.push_str("::");
                a.push_str(b.as_ref());
                a
            })
        } else {
            String::new()
        };
        Self::new(joined)
    }

    /// Returns `true` if this `Sentinel` matches the function in the `haystack`.
    pub fn matches(&self, haystack: &str) -> bool {
        self.0.as_str().dowild(haystack)
    }
}

impl AsRef<Self> for Sentinel {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl Default for Sentinel {
    fn default() -> Self {
        Self::new(DEFAULT_TOGGLE)
    }
}

impl Display for Sentinel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Eq for Sentinel {}

impl PartialEq for Sentinel {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl From<Sentinel> for String {
    fn from(value: Sentinel) -> Self {
        value.0.as_str().to_owned()
    }
}

/// Parse the callgrind output files header
pub fn parse_header<I>(iter: &mut I) -> Result<CallgrindProperties>
where
    I: Iterator<Item = std::io::Result<String>>,
{
    let first_non_empty = loop {
        match iter.next().transpose()? {
            Some(line) if line.trim().is_empty() => {}
            Some(line) => break line,
            None => bail!("Empty file"),
        }
    };

    if !first_non_empty.contains("callgrind format") {
        warn!("Missing file format specifier. Assuming callgrind format.");
    }

    let mut positions_prototype: Option<Positions> = None;
    let mut metrics_prototype: Option<Metrics> = None;
    let mut pid: Option<i32> = None;
    let mut thread: Option<usize> = None;
    let mut part: Option<u64> = None;
    let mut desc: Vec<String> = vec![];
    let mut cmd: Option<String> = None;
    let mut creator: Option<String> = None;

    for line in std::iter::once(Ok(first_non_empty)).chain(iter) {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match line.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
            Some(("version", version)) if version != "1" => {
                bail!(
                    "Version mismatch: Requires callgrind format version '1' but was '{version}'"
                );
            }
            Some(("pid", value)) => {
                trace!("Using pid '{value}' from line: '{line}'");
                pid = Some(value.parse::<i32>()?);
            }
            Some(("thread", value)) => {
                trace!("Using thread '{value}' from line: '{line}'");
                thread = Some(value.parse::<usize>()?);
            }
            Some(("part", value)) => {
                trace!("Using part '{value}' from line: '{line}'");
                part = Some(value.parse::<u64>()?);
            }
            Some(("desc", value)) if !value.starts_with("Option:") => {
                trace!("Using description '{value}' from line: '{line}'");
                desc.push(value.to_owned());
            }
            Some(("cmd", value)) => {
                trace!("Using cmd '{value}' from line: '{line}'");
                cmd = Some(value.to_owned());
            }
            Some(("creator", value)) => {
                trace!("Using creator '{value}' from line: '{line}'");
                creator = Some(value.to_owned());
            }
            Some(("positions", positions)) => {
                trace!("Using positions '{positions}' from line: '{line}'");
                positions_prototype = Some(Positions::try_from_iter_str(
                    positions.split_ascii_whitespace(),
                )?);
            }
            // The events line is the last line in the header which is mandatory (according to
            // the source code of callgrind_annotate). The summary line is usually the last line,
            // but it is only optional. So, we break out of the loop here and stop the parsing.
            Some(("events", events)) => {
                trace!("Using events '{events}' from line: '{line}'");
                metrics_prototype = Some(
                    events
                        .split_ascii_whitespace()
                        .map(EventKind::from_str)
                        .collect::<Result<Metrics>>()?,
                );
                break;
            }
            // None is actually a malformed header line we just ignore here
            // Some(_) includes `^event:` lines
            None | Some(_) => {}
        }
    }

    Ok(CallgrindProperties {
        metrics_prototype: metrics_prototype
            .ok_or_else(|| anyhow!("Header field 'events' must be present"))?,
        positions_prototype: positions_prototype.unwrap_or_default(),
        pid,
        thread,
        part,
        desc,
        cmd,
        creator,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Cursor, ErrorKind};

    use rstest::rstest;

    use super::*;

    /// These are some non-exhaustive real world examples which a sentinel should be able to match
    #[rstest]
    #[case::main_binary("*::main", "by_binary::main")]
    #[case::below_main_exact("(below_main)", "(below_main)")]
    #[case::below_main_with_glob("?below_main?", "(below_main)")]
    #[case::exit("*exit*", "exit")]
    #[case::with_at_sign("__cpu_indicator_init*", "__cpu_indicator_init@GCC_4.8.0")]
    #[case::simple_function(
        "*::stack_overflow::*",
        "std::sys::unix::stack_overflow::imp::make_handler"
    )]
    #[case::generic(
        "std::sync::once_lock::OnceLock<*>*",
        "std::sync::once_lock::OnceLock<T>::initialize"
    )]
    #[case::generic_with_as(
        "<* as core::fmt::Write>::write_str",
        "<std::io::Write::write_fmt::Adapter<T> as core::fmt::Write>::write_str"
    )]
    #[case::generic_with_as_reference(
        "<&*>::write_fmt",
        "<&std::io::stdio::Stdout as std::io::Write>::write_fmt"
    )]
    #[case::generic_match_all(
        "*::write_fmt",
        "<&std::io::stdio::Stdout as std::io::Write>::write_fmt"
    )]
    #[case::hex("0x*", "0x00000000000083f0")]
    fn test_sentinel_from_glob_matches(#[case] input: &str, #[case] haystack: &str) {
        assert!(Sentinel::new(input).matches(haystack));
    }

    #[test]
    fn parse_header_when_format_banner_is_present() {
        let mut lines = [
            Ok(String::from("# callgrind format")),
            Ok(String::from("pid: 123")),
            Ok(String::from("cmd: bench command")),
            Ok(String::from("events: Ir Dr Dw")),
        ]
        .into_iter();

        let header = parse_header(&mut lines).unwrap();

        assert_eq!(header.pid, Some(123));
        assert_eq!(header.cmd.as_deref(), Some("bench command"));
        assert_eq!(header.metrics_prototype.0.len(), 3);
    }

    #[test]
    fn parse_header_when_format_banner_is_missing() {
        let mut lines = [
            Ok(String::new()),
            Ok(String::from("pid: 123")),
            Ok(String::from("cmd: bench command")),
            Ok(String::from("events: Ir Dr Dw")),
        ]
        .into_iter();

        let header = parse_header(&mut lines).unwrap();

        assert_eq!(header.pid, Some(123));
        assert_eq!(header.cmd.as_deref(), Some("bench command"));
        assert_eq!(header.metrics_prototype.0.len(), 3);
    }

    #[test]
    fn parse_header_when_invalid_utf8_then_error_but_not_panic() {
        // Invalid utf8
        let bytes = b"\n\xFF\npid: 123\nevents: Ir Dr Dw\n".to_vec();
        let mut lines = BufReader::new(Cursor::new(bytes)).lines();

        let err = parse_header(&mut lines).unwrap_err();
        let io_err = err.downcast_ref::<std::io::Error>().unwrap();

        assert_eq!(io_err.kind(), ErrorKind::InvalidData);
    }
}
