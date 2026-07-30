//! The module containing the Cachegrind summary parser
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use log::{debug, trace};

use super::parser::parse_header;
use crate::error::Error;
use crate::runner::tool::logfile_parser;
use crate::runner::tool::parser::{Header, Parser, ParserOutput};
use crate::runner::tool::path::ToolOutputPath;
use crate::summary::model::ToolMetrics;

/// Parse the `summary:` line in the cachegrind output file
///
/// The format of the `.out` file is fully described
/// [here](https://valgrind.org/docs/manual/cg-manual.html#cg-manual.impl-details.file-format)
///
/// Note that unlike the callgrind output file the cachegrind file does not contain the pid or ppid.
/// Since we need them, it's necessary to additionally parse the matching log output file. Although
/// very unlikely, it is still possible that the pid and ppid can't be parsed from the log file, so
/// a `0` for pid and `None` for ppid can be interpreted as failure and shown as such in the
/// terminal output.
#[derive(Debug)]
pub struct SummaryParser {
    /// The [`ToolOutputPath`]
    pub output_path: ToolOutputPath,
}

impl Parser for SummaryParser {
    fn parse_single(&self, path: PathBuf) -> Result<ParserOutput> {
        debug!(
            "Parsing cachegrind output file '{}' for the summary",
            path.display()
        );

        let mut iter = BufReader::new(File::open(&path)?).lines();

        let properties = parse_header(&mut iter)
            .map_err(|error| Error::ParseError(path.clone(), error.to_string()))?;

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
        }

        let (pid, parent_pid) = if let Some(logfile) = self.output_path.log_path_of(&path) {
            let file = File::open(&logfile)
                .with_context(|| format!("Error opening log file '{}'", logfile.display()))?;

            let iter = BufReader::new(file).lines();
            let header = logfile_parser::parse_header(&logfile, iter)?;
            (header.pid, header.parent_pid)
        } else {
            (0i32, None)
        };

        if let Some(metrics) = metrics {
            let header = Header {
                command: properties.cmd,
                pid,
                parent_pid,
                thread: None,
                part: None,
                desc: properties.desc,
            };
            Ok(ParserOutput {
                path,
                header,
                details: vec![],
                metrics: ToolMetrics::Cachegrind(metrics),
            })
        } else {
            Err(Error::ParseError(path, "No summary line found".to_owned()).into())
        }
    }

    fn get_output_path(&self) -> &ToolOutputPath {
        &self.output_path
    }
}
