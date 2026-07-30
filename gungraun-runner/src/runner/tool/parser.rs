//! The parser module

use std::cmp::Ordering;
use std::path::PathBuf;

use anyhow::Result;
use log::debug;

use super::config::ToolConfig;
use super::error_metric_parser::ErrorMetricLogfileParser;
use super::generic_parser::GenericLogfileParser;
use super::path::ToolOutputPath;
use crate::api::EntryPoint;
use crate::runner::dhat::json_parser::JsonParser;
use crate::runner::dhat::logfile_parser::DhatLogfileParser;
use crate::runner::tool::config::ToolConfigOptions;
use crate::runner::{cachegrind, callgrind, perf};
use crate::summary::model::ToolMetrics;

/// The combined header of output and log files
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// The path to the executed command with command-line arguments
    pub command: String,
    /// Some output files contain a description (desc:) field
    pub desc: Vec<String>,
    /// The parent pid of the profile
    pub parent_pid: Option<i32>,
    /// The part number (currently only Callgrind)
    pub part: Option<u64>,
    /// The pid of the profile
    pub pid: i32,
    /// The thread number (currently only Callgrind)
    pub thread: Option<usize>,
}

/// The output of a [`Parser`]
#[derive(Debug, Clone, PartialEq)]
pub struct ParserOutput {
    /// Details about the profile run if present. A vector separated by lines
    pub details: Vec<String>,
    /// The [`Header`] containing some basic information about the profile
    pub header: Header,
    /// The extracted metrics from a profile or logfile
    pub metrics: ToolMetrics,
    /// The path to the profile or logfile which was parsed
    pub path: PathBuf,
}

/// Needs to be implemented by a parser to be able to be used in the [`parser_factory`]
pub trait Parser: std::fmt::Debug + Send {
    /// Returns the [`ToolOutputPath`].
    fn get_output_path(&self) -> &ToolOutputPath;

    /// Parse all files of the stored [`ToolOutputPath`]
    fn parse(&self) -> Result<Vec<ParserOutput>> {
        self.parse_with(self.get_output_path())
    }

    /// Parse all "old" or "base" files of the [`ToolOutputPath`]
    fn parse_base(&self) -> Result<Vec<ParserOutput>> {
        self.parse_with(&self.get_output_path().to_base_path())
    }

    /// Parse a single file
    fn parse_single(&self, path: PathBuf) -> Result<ParserOutput>;

    /// Returns a sorted vector of parser results.
    fn parse_with(&self, output_path: &ToolOutputPath) -> Result<Vec<ParserOutput>> {
        debug!("{}: Parsing file '{}'", output_path.tool.id(), output_path);
        let Ok(paths) = output_path.sanitized_paths() else {
            return Ok(vec![]);
        };

        let mut parser_results = Vec::with_capacity(paths.len());
        for path in paths {
            let parsed = self.parse_single(path)?;
            let position = parser_results
                .binary_search_by(|probe: &ParserOutput| probe.compare_target_ids(&parsed))
                .unwrap_or_else(|e| e);

            parser_results.insert(position, parsed);
        }

        Ok(parser_results)
    }
}

impl ParserOutput {
    /// Compare by target ids `pid`, `part` and `thread`
    ///
    /// Same as in [`crate::runner::callgrind::parser::CallgrindProperties::compare_target_ids`]
    ///
    /// Highest precedence takes `pid`. Second is `part` and third is `thread` all sorted ascending.
    /// See also [Callgrind
    /// Format](https://valgrind.org/docs/manual/cl-format.html#cl-format.reference.grammar)
    pub fn compare_target_ids(&self, other: &Self) -> Ordering {
        self.header.pid.cmp(&other.header.pid).then_with(|| {
            self.header
                .thread
                .cmp(&other.header.thread)
                .then_with(|| self.header.part.cmp(&other.header.part))
        })
    }
}

/// Return an appropriate parser for a tool
pub fn parser_factory(
    tool_config: &ToolConfig,
    root_dir: PathBuf,
    output_path: &ToolOutputPath,
) -> Box<dyn Parser> {
    match &tool_config.options {
        ToolConfigOptions::Callgrind => Box::new(callgrind::summary_parser::SummaryParser {
            output_path: output_path.clone(),
        }),
        ToolConfigOptions::Cachegrind => Box::new(cachegrind::summary_parser::SummaryParser {
            output_path: output_path.clone(),
        }),
        ToolConfigOptions::DHAT(dhat_config) => {
            if tool_config.entry_point == EntryPoint::None && dhat_config.frames.is_empty() {
                Box::new(DhatLogfileParser::new(
                    output_path.to_log_output(),
                    root_dir,
                ))
            } else {
                Box::new(JsonParser::new(
                    output_path.clone(),
                    tool_config.entry_point.clone(),
                    dhat_config.frames.clone(),
                    tool_config.sanitize_output,
                ))
            }
        }
        ToolConfigOptions::Memcheck | ToolConfigOptions::DRD | ToolConfigOptions::Helgrind => {
            Box::new(ErrorMetricLogfileParser {
                output_path: output_path.to_log_output(),
                root_dir,
            })
        }
        ToolConfigOptions::Perf(perf_config) => Box::new(perf::json_parser::JsonParser {
            output_path: output_path.clone(),
            min_pcnt_running: perf_config.min_pcnt_running,
            non_zero_metrics: perf_config.non_zero_metrics.clone(),
        }),
        _ => Box::new(GenericLogfileParser {
            output_path: output_path.to_log_output(),
            root_dir,
        }),
    }
}
