//! Version 6 Gungraun summary types and parsing helpers.
//!
//! This module is a self-contained snapshot of the version 6 summary model: the version 6
//! summary-model types live in the local [`model`] module (re-exported here), including the
//! `ErrorTool` enum variants used by Memcheck, Helgrind and DRD. Unchanged shared types are
//! re-exported from `gungraun-runner`. The parsing helpers assume the input already matches
//! schema version 6.

mod model;

use std::fs;
use std::path::Path;

pub use gungraun_runner::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind};
pub use gungraun_runner::metrics::model::Metric;
pub use model::{
    BenchmarkKind, BenchmarkSummary, Diffs, FlamegraphSummary, MetricKind, MetricsDiff,
    MetricsSummary, Profile, ProfileData, ProfileInfo, ProfilePart, ProfileTotal, Profiles,
    SCHEMA_VERSION, SummaryFormat, SummaryOutput, ToolMetricSummary, ToolRegression, ValgrindTool,
};

use crate::error::{Error, Result};

/// Parse a version 6 summary JSON file.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// let summary = gungraun_summary::v6::parse(Path::new("target/summary.json")).unwrap();
/// println!("{}", summary.function_name);
/// ```
///
/// # Errors
///
/// Returns [`Error::ParseError`] if the file cannot be read or if the JSON does
/// not deserialize into a [`BenchmarkSummary`].
pub fn parse(path: &Path) -> Result<BenchmarkSummary> {
    fs::read(path)
        .map_err(|error| Error::ParseError(format!("'{}': {error}", path.display())))
        .and_then(|buffer| parse_slice(&buffer))
}

/// Parse a version 6 summary JSON buffer.
///
/// # Errors
///
/// Returns [`Error::ParseError`] if the buffer is not valid JSON or does not deserialize into a
/// [`BenchmarkSummary`].
pub fn parse_slice(buffer: &[u8]) -> Result<BenchmarkSummary> {
    serde_json::from_slice(buffer).map_err(|error| Error::ParseError(error.to_string()))
}
