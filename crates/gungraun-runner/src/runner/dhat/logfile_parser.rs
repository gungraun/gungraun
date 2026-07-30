//! Module containing the dhat log file parser

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow};
use log::debug;
use regex::Regex;

use crate::api::DhatMetric;
use crate::metrics::model::Metrics;
use crate::runner::tool::logfile_parser::{
    EMPTY_LINE_RE, EXTRACT_FIELDS_RE, STRIP_PREFIX_RE, parse_header,
};
use crate::runner::tool::parser::{Parser, ParserOutput};
use crate::runner::tool::path::ToolOutputPath;
use crate::summary::model::ToolMetrics;

// The different regex have to consider --time-stamp=yes
static METRICS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?<bytes>[0-9]+)\s*(?<unit>bytes|units)(?:\s*in\s*(?<blocks>[0-9]+))?.*$")
        .expect("Regex should compile")
});
static FIXUP_NUMBERS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("([0-9]),([0-9])").expect("Regex should compile"));

#[derive(Debug, PartialEq, Eq)]
enum State {
    HeaderSpace,
    Body,
    Fields,
    Footer,
}

/// The dhat logfile parser
#[derive(Debug)]
pub struct DhatLogfileParser {
    /// The [`ToolOutputPath`]
    pub output_path: ToolOutputPath,
    /// The path to the root/project directory used to make paths relative
    pub root_dir: PathBuf,
}

impl DhatLogfileParser {
    /// Creates a new `DhatLogfileParser`.
    pub fn new(output_path: ToolOutputPath, root_dir: PathBuf) -> Self {
        Self {
            output_path,
            root_dir,
        }
    }

    /// Parse a single line of the logfile
    ///
    /// A return value of `false` indicates parsing is complete.
    fn parse_line(
        line: &str,
        state: &mut State,
        metrics: &mut Metrics<DhatMetric>,
        details: &mut Vec<String>,
    ) -> Result<bool> {
        match &state {
            State::HeaderSpace if EMPTY_LINE_RE.is_match(line) => {}
            State::HeaderSpace | State::Body => {
                if *state == State::HeaderSpace {
                    *state = State::Body;
                }

                if let Some(caps) = EXTRACT_FIELDS_RE.captures(line) {
                    let key = caps.name("key").unwrap().as_str();

                    // Total: ... is the first line of the fields we're interested in
                    if key.to_ascii_lowercase().as_str() == "total" {
                        *state = State::Fields;
                        return Self::parse_line(line, state, metrics, details);
                    }
                }

                if let Some(caps) = STRIP_PREFIX_RE.captures(line) {
                    let rest_of_line = caps.name("rest").unwrap().as_str();

                    details.push(rest_of_line.to_owned());
                } else {
                    details.push(line.to_owned());
                }
            }
            State::Fields => {
                // The original metrics lines look like this in heap mode:
                //
                // ==2960865== Total:     156,362 bytes in 78 blocks
                // ==2960865== At t-gmax: 48,821 bytes in 13 blocks
                // ==2960865== At t-end:  0 bytes in 0 blocks
                // ==2960865== Reads:     119,827 bytes
                // ==2960865== Writes:    136,997 bytes
                //
                // or in copy mode
                //
                // ==2960865== Total:     156,362 bytes in 78 blocks
                //
                // or in ad-hoc mode
                //
                // ==2960865== Total:     50 units in 2 events
                //
                // The prefix with the pid can be different but the `EXTRACT_FIELDS_RE` takes
                // care of that.
                //
                // The metric lines with bytes and blocks need to be parsed into two separate
                // metric kinds
                if let Some(fields_caps) = EXTRACT_FIELDS_RE.captures(line) {
                    let key = fields_caps.name("key").unwrap().as_str();
                    let value = fields_caps.name("value").unwrap().as_str();
                    let value = FIXUP_NUMBERS_RE.replace_all(value, "$1$2");

                    if let Some(metrics_caps) = METRICS_RE.captures(&value) {
                        let num_bytes = metrics_caps.name("bytes").unwrap().as_str().parse()?;
                        let num_blocks = metrics_caps
                            .name("blocks")
                            .and_then(|s| s.as_str().parse().ok());

                        let unit = metrics_caps.name("unit").unwrap().as_str();

                        match key {
                            "Total" if unit == "units" => {
                                metrics.insert(DhatMetric::TotalUnits, num_bytes);
                                metrics.insert(
                                    DhatMetric::TotalEvents,
                                    num_blocks
                                        .ok_or_else(|| anyhow!("Error parsing total events"))?,
                                );
                            }
                            "Total" => {
                                metrics.insert(DhatMetric::TotalBytes, num_bytes);
                                metrics.insert(
                                    DhatMetric::TotalBlocks,
                                    num_blocks
                                        .ok_or_else(|| anyhow!("Error parsing total blocks"))?,
                                );
                            }
                            "At t-gmax" => {
                                metrics.insert(DhatMetric::AtTGmaxBytes, num_bytes);
                                metrics.insert(
                                    DhatMetric::AtTGmaxBlocks,
                                    num_blocks
                                        .ok_or_else(|| anyhow!("Error parsing At t-gmax blocks"))?,
                                );
                            }
                            "At t-end" => {
                                metrics.insert(DhatMetric::AtTEndBytes, num_bytes);
                                metrics.insert(
                                    DhatMetric::AtTEndBlocks,
                                    num_blocks
                                        .ok_or_else(|| anyhow!("Error parsing At t-end blocks"))?,
                                );
                            }
                            "Reads" => {
                                let metric_kind = DhatMetric::ReadsBytes;
                                metrics.insert(metric_kind, num_bytes);
                            }
                            "Writes" => {
                                let metric_kind = DhatMetric::WritesBytes;
                                metrics.insert(metric_kind, num_bytes);
                            }
                            _ => {
                                debug!("Ignoring invalid dhat metric kind: {key}");
                            }
                        }
                    }
                } else {
                    *state = State::Footer;
                }
            }
            State::Footer => {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

impl Parser for DhatLogfileParser {
    fn parse_single(&self, path: PathBuf) -> Result<ParserOutput> {
        let file = File::open(&path)
            .with_context(|| format!("Error opening log file '{}'", path.display()))?;

        let mut iter = BufReader::new(file)
            .lines()
            .skip_while(|l| l.as_ref().is_ok_and(|l| l.trim().is_empty()));

        let header = parse_header(&path, &mut iter)?;

        let mut metrics = Metrics::empty();
        let mut details = vec![];

        let mut state = State::HeaderSpace;
        for line in iter {
            let line = line?;
            if !Self::parse_line(&line, &mut state, &mut metrics, &mut details)? {
                break;
            }
        }

        // Remove the last empty lines from the details
        while let Some(last) = details.last() {
            if last.trim().is_empty() {
                details.pop();
            } else {
                break;
            }
        }

        Ok(ParserOutput {
            header,
            path,
            details,
            metrics: ToolMetrics::Dhat(metrics),
        })
    }

    fn get_output_path(&self) -> &ToolOutputPath {
        &self.output_path
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::some_bytes_in_blocks("156362 bytes in 78 blocks")]
    #[case::zero_bytes_in_blocks("0 bytes in 0 blocks")]
    #[case::some_bytes("156362 bytes")]
    #[case::zero_bytes("0 bytes")]
    #[case::zero_bytes("20 units in 1 events")]
    fn test_metrics_re_when_match(#[case] haystack: &str) {
        assert!(METRICS_RE.is_match(haystack));
    }
}
