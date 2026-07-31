//! The module containing the Callgrind summary/total parser
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::Result;
use log::{debug, trace};

use super::model::Metrics;
use super::parser::{CallgrindParser, CallgrindProperties, parse_header};
use crate::error::Error;
use crate::runner::tool::parser::{Header, Parser, ParserOutput};
use crate::runner::tool::path::ToolOutputPath;
use crate::summary::model::ToolMetrics::Callgrind;

/// Parse the `total:` line in the callgrind output or `summary:` if total is not present
///
/// The format is described [here](https://valgrind.org/docs/manual/cl-format.html)
///
/// The summary would usually be the first choice, but there are bugs in the summary line if
/// callgrind client requests are used, which is why the total is used as primary metric and then
/// the summary.
///
/// Regarding the summary:
///
/// For the visualization to be able to show cost percentage, a sum of the cost of the full run has
/// to be known. Usually, it is assumed that this is the sum of all cost lines in a file. But
/// sometimes, this is not correct. The "summary:" line in the header gives the full cost for the
/// profile run.
///
/// This header line specifies a summary cost, which should be equal or larger than a total over all
/// self costs. It may be larger as the cost lines may not represent all cost of the program run.
#[derive(Debug)]
pub struct SummaryParser {
    /// The [`ToolOutputPath`]
    pub output_path: ToolOutputPath,
}

impl SummaryParser {
    /// Creates a new `SummaryParser`.
    pub fn new(output_path: &ToolOutputPath) -> Self {
        Self {
            output_path: output_path.clone(),
        }
    }
}

impl CallgrindParser for SummaryParser {
    type Output = Metrics;

    fn parse_single(&self, path: &Path) -> Result<(CallgrindProperties, Self::Output)> {
        debug!(
            "Parsing callgrind output file '{}' for a summary or totals",
            path.display()
        );

        let mut iter = BufReader::new(File::open(path)?).lines();

        let properties = parse_header(&mut iter)
            .map_err(|error| Error::ParseError(path.to_owned(), error.to_string()))?;

        let mut metrics = None;
        for line in iter {
            let line = line?;

            if let Some(suffix) = line.strip_prefix("summary:") {
                trace!("Found line with summary: '{line}'");

                let mut inner = properties.metrics_prototype.clone();
                inner.add_iter_str(suffix.split_ascii_whitespace())?;
                metrics = Some(inner);

                trace!("Updated counters to '{metrics:?}'");
            }

            if let Some(suffix) = line.strip_prefix("totals:") {
                trace!("Found line with totals: '{line}'");

                let mut inner = properties.metrics_prototype.clone();
                inner.add_iter_str(suffix.split_ascii_whitespace())?;
                metrics = Some(inner);

                trace!("Updated counters to '{metrics:?}'");
                break;
            }
        }

        if let Some(metrics) = metrics {
            Ok((properties, metrics))
        } else {
            Err(Error::ParseError(
                path.to_owned(),
                "No summary or totals line found".to_owned(),
            )
            .into())
        }
    }
}

impl Parser for SummaryParser {
    fn parse_single(&self, path: PathBuf) -> Result<ParserOutput> {
        CallgrindParser::parse_single(self, &path).map(|(props, metrics)| ParserOutput {
            path: path.clone(),
            header: Header {
                command: props.cmd.expect("A command should be present"),
                pid: props.pid.expect("A pid should be present"),
                parent_pid: None,
                thread: props.thread,
                part: props.part,
                desc: props.desc,
            },
            details: vec![],
            metrics: Callgrind(metrics),
        })
    }

    fn get_output_path(&self) -> &ToolOutputPath {
        &self.output_path
    }
}
