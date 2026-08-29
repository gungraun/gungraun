//! Version 7 Gungraun summary types and parsing helpers.
//!
//! This module re-exports the version 7 summary model and provides parsing helpers that assume the
//! input already matches schema version 7.

use std::fs;
use std::path::Path;

pub use gungraun_runner::api::{
    CachegrindMetric, DhatMetric, ErrorMetric, EventKind, PerfMetric, Tool,
};
pub use gungraun_runner::metrics::model::{
    AnnotatedMetric, Metric, MetricKind, MetricsDiff, MetricsSummary, PerfQualities,
};
pub use gungraun_runner::summary::model::{
    BenchmarkKind, BenchmarkSummary, Diffs, FlamegraphSummary, Profile, ProfileData, ProfileInfo,
    ProfilePart, ProfileTotal, Profiles, SummaryFormat, SummaryOutput, ToolMetricSummary,
    ToolRegression,
};

use crate::error::{Error, Result};

/// The version string stored in version 7 summary JSON files.
pub const SCHEMA_VERSION: &str = "7";

/// Parse a version 7 summary JSON file.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// let summary = gungraun_summary::v7::parse(Path::new("target/summary.json")).unwrap();
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

/// Parse a version 7 summary JSON buffer.
///
/// # Errors
///
/// Returns [`Error::ParseError`] if the buffer is not valid JSON or does not deserialize into a
/// [`BenchmarkSummary`].
pub fn parse_slice(buffer: &[u8]) -> Result<BenchmarkSummary> {
    serde_json::from_slice(buffer).map_err(|error| Error::ParseError(error.to_string()))
}
