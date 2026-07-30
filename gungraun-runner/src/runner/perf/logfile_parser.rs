//! Parsing helpers for perf log files emitted by Gungraun.
//!
//! Perf runs write a small textual log alongside the JSON output. The log starts with a header
//! section containing some general [`Header`] fields, followed by an empty line and the remaining
//! log body. The log body may contain any user logged data.
//!
//! For batched perf modes, the body may also contain a repetition marker line prefixed with
//! [`crate::api::PERF_REPETITIONS_MARKER`]. If no repetition marker is present, parsing treats that
//! as "no logged repetitions" and returns `0`.

use std::path::Path;

use anyhow::{Result, anyhow};
use log::{debug, trace};

use crate::api::PERF_REPETITIONS_MARKER;
use crate::runner::tool::parser::Header;

/// Parsed metadata extracted from a perf log file.
///
/// Perf log parsing yields the structured header used for benchmark output and the repetition count
/// written by Gungraun's perf harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfLogData {
    /// Structured header parsed from the log file prologue.
    pub header: Header,
    /// Repetition count parsed from the first perf repetition marker.
    ///
    /// If no repetition marker is present, this is `0`.
    pub repetitions: usize,
}

/// Parse the structured header section from a perf log file.
///
/// The header is read until the first empty line. The `Command`, `Pid`, and `Part` fields are
/// required. Other header lines are ignored.
///
/// # Errors
///
/// Returns an error if any required field is missing or cannot be parsed.
pub fn parse_header<I>(path: &Path, lines: I) -> Result<Header>
where
    I: Iterator<Item = std::io::Result<String>>,
{
    debug!("Parsing header from perf log file: {}", path.display());

    let mut pid = None;
    let mut command = None;
    let mut part = None;
    for line in lines {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            trace!("Found empty line. Stopping header parsing");
            // The header is separated from the body by at least one empty line. The first empty
            // line is removed from the iterator.
            break;
        } else if let Some((key, value)) = line.split_once(':') {
            let (key, value) = (key.trim(), value.trim());
            // These unwraps are safe. If there is a key, there is also a value present
            match key.to_ascii_lowercase().as_str() {
                "command" => {
                    trace!("Found command: {value}");
                    command = Some(value.to_owned());
                }
                "pid" => {
                    trace!("Found pid: {value}");
                    pid = Some(value.to_owned());
                }
                "part" => {
                    trace!("Found part: {value}");
                    part = Some(value.to_owned());
                }
                _ => {
                    trace!("Ignoring: {value}");
                    // Ignore other header lines
                }
            }
        } else {
            trace!("Malformed header line found: {line}");
            // Some malformed header line which we ignore
        }
    }

    let command = command.ok_or_else(|| {
        anyhow!(
            "Error parsing header of perf logfile '{}': A command should be present",
            path.display()
        )
    })?;

    let pid = pid
        .ok_or_else(|| {
            anyhow!(
                "Error parsing header of perf logfile '{}': A pid should be present",
                path.display()
            )
        })
        .and_then(|p| p.parse().map_err(Into::into))?;

    let part = part
        .ok_or_else(|| {
            anyhow!(
                "Error parsing header of perf logfile '{}': A part should be present",
                path.display()
            )
        })
        .and_then(|p| p.parse::<u64>().map_err(Into::into))?;

    Ok(Header {
        command,
        pid,
        parent_pid: None,
        thread: None,
        part: Some(part),
        desc: vec![],
    })
}

/// Parse a perf log file into its header metadata and repetition count.
///
/// The input is expected to contain a header section first, terminated by an empty line, followed
/// by the remaining log body. The repetition count is extracted from the first line prefixed with
/// [`PERF_REPETITIONS_MARKER`].
///
/// If no repetition marker is present, the parsed repetition count is `0`.
///
/// # Errors
///
/// Returns an error if the header is malformed or if a repetition marker is present but its value
/// is not a valid `usize`.
///
/// [`PERF_REPETITIONS_MARKER`]: crate::api::PERF_REPETITIONS_MARKER
pub fn parse_perf_log<I>(path: &Path, mut lines: I) -> Result<PerfLogData>
where
    I: Iterator<Item = std::io::Result<String>>,
{
    let header = parse_header(path, &mut lines)?;
    let repetitions = parse_repetitions(path, lines)?;

    Ok(PerfLogData {
        header,
        repetitions,
    })
}

fn parse_repetitions<I>(path: &Path, lines: I) -> Result<usize>
where
    I: Iterator<Item = std::io::Result<String>>,
{
    debug!("Parsing perf log file '{}' for repetitions", path.display());

    let mut repetitions = None;

    for line in lines {
        let line = line?;
        let line = line.trim();
        let Some(value) = line.strip_prefix(PERF_REPETITIONS_MARKER) else {
            continue;
        };

        let value = value.trim();
        debug!("Found repetitions: '{value}'");

        let parsed = value.parse::<usize>().map_err(|error| {
            anyhow!(
                "Error parsing logfile '{}': invalid perf repetition count: {error}",
                path.display()
            )
        })?;

        repetitions = Some(parsed);
        break;
    }

    // Missing repetition markers are tolerated and treated as "no logged repetitions".
    Ok(repetitions.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use std::string::ToString;

    use super::*;

    fn parse(lines: &[&str]) -> Result<PerfLogData> {
        parse_perf_log(
            Path::new("perf.log"),
            lines.iter().map(|l| Ok(l.to_string())),
        )
    }

    #[test]
    fn parse_perf_log_reads_header_and_repetitions() {
        let data = parse(&[
            "Command: bench",
            "Pid: 123",
            "Part: 1",
            "",
            "some body line",
            "gungraun::__perf_repetitions: 42",
        ])
        .unwrap();

        assert_eq!(data.header.command, "bench");
        assert_eq!(data.header.pid, 123);
        assert_eq!(data.repetitions, 42);
    }

    #[test]
    fn parse_perf_log_when_missing_repetitions_then_0() {
        let data = parse(&["Command: bench", "Pid: 123", "Part: 1", "", "body"]).unwrap();

        assert_eq!(data.header.command, "bench");
        assert_eq!(data.header.pid, 123);
        assert_eq!(data.repetitions, 0);
    }

    #[test]
    fn parse_perf_log_when_duplicate_repetitions_then_uses_first() {
        let data = parse(&[
            "Command: bench",
            "Pid: 123",
            "Part: 1",
            "",
            "gungraun::__perf_repetitions: 1",
            "gungraun::__perf_repetitions: 2",
        ])
        .unwrap();

        assert_eq!(data.repetitions, 1);
    }

    #[test]
    fn parse_perf_log_when_zero_repetitions() {
        let data = parse(&[
            "Command: bench",
            "Pid: 123",
            "Part: 1",
            "",
            "gungraun::__perf_repetitions: 0",
        ])
        .unwrap();

        assert_eq!(data.repetitions, 0);
    }

    #[test]
    fn parse_perf_log_rejects_invalid_repetitions() {
        let error = parse(&[
            "Command: bench",
            "Pid: 123",
            "Part: 1",
            "",
            "gungraun::__perf_repetitions: dynamic",
        ])
        .unwrap_err();

        assert!(error.to_string().contains("invalid perf repetition count"));
    }
}
