//! Utilities and version-aware parsing helpers for Gungraun summary JSON.
//!
//! This module is the entrypoint for callers that do not know the summary schema version ahead of
//! time. The parse functions first inspect the summary's `version` field and then dispatch to the
//! matching versioned parser, such as [`crate::v6`].
//!
//! Use [`parse`] when reading from a file path and [`parse_slice`] when the summary JSON is already
//! available in memory.
//!
//! If you already know that the input is version 6, prefer the convenience parsers from the
//! [`crate::v6`] directly.
//!
//! # Examples
//!
//! Parse from a file path:
//!
//! ```no_run
//! use std::path::Path;
//!
//! use gungraun_summary::util::{SummaryByVersion, parse};
//!
//! match parse(Path::new("target/summary.json"))? {
//!     SummaryByVersion::V6(summary) => {
//!         assert_eq!(summary.version, "6");
//!     }
//!     _ => unreachable!("no other summary versions are currently supported"),
//! }
//! # Ok::<(), gungraun_summary::error::Error>(())
//! ```
//!
//! Parse from an in-memory JSON buffer:
//!
//! ```
//! use gungraun_summary::util::{SummaryByVersion, parse_slice};
//!
//! let summary = br#"{
//!   "baselines": [null, null],
//!   "benchmark_exe": "/project/bin",
//!   "benchmark_file": "/project/benches/example.rs",
//!   "details": null,
//!   "function_name": "some_benchmark_function",
//!   "id": null,
//!   "kind": "LibraryBenchmark",
//!   "module_path": "example::group::some_benchmark_function",
//!   "package_dir": "/project",
//!   "profiles": [],
//!   "project_root": "/project",
//!   "summary_output": null,
//!   "version": "6"
//! }"#;
//!
//! match parse_slice(summary)? {
//!     SummaryByVersion::V6(summary) => {
//!         assert_eq!(summary.version, "6");
//!     }
//!     _ => unreachable!("no other summary versions are currently supported"),
//! }
//! # Ok::<(), gungraun_summary::error::Error>(())
//! ```

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::v6;

/// A parsed summary tagged with the schema version used to deserialize it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SummaryByVersion {
    /// A summary parsed according to schema version 6.
    V6(v6::BenchmarkSummary),
}

/// A schema version supported by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Version {
    /// Schema version 6.
    V6,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct VersionProbe {
    version: String,
}

impl SummaryByVersion {
    /// Return the schema version used to deserialize this summary.
    pub fn version(&self) -> Version {
        match self {
            Self::V6(_) => Version::V6,
        }
    }
}

impl Version {
    /// Return the string representation for this schema version.
    pub const fn as_str(&self) -> &str {
        match self {
            Self::V6 => v6::SCHEMA_VERSION,
        }
    }
}

/// Parse a summary JSON file and return the matching [`SummaryByVersion`].
///
/// For parsing from an in-memory buffer instead of a [`Path`], see
/// [`parse_slice`].
///
/// # Errors
///
/// Returns [`Error::ParseError`] if the file cannot be read, if the JSON is invalid, or if the
/// `version` field cannot be deserialized. Returns [`Error::UnsupportedVersion`] if the summary has
/// a schema version this crate does not support.
pub fn parse(path: &Path) -> Result<SummaryByVersion> {
    fs::read(path)
        .map_err(|error| Error::ParseError(format!("'{}': {error}", path.display())))
        .and_then(|buffer| parse_slice(&buffer))
}

/// Parse a summary JSON buffer and return the matching [`SummaryByVersion`].
///
/// This method is similar to [`parse`] but takes a `&[u8]` instead of a
/// [`Path`].
///
/// # Errors
///
/// Returns [`Error::ParseError`] if the buffer is not valid JSON or if the `version` field cannot
/// be deserialized. Returns [`Error::UnsupportedVersion`] if the summary declares a schema version
/// this crate does not support.
pub fn parse_slice(buffer: &[u8]) -> Result<SummaryByVersion> {
    let probe: VersionProbe =
        serde_json::from_slice(buffer).map_err(|error| Error::ParseError(error.to_string()))?;

    match probe.version.as_str() {
        v6::SCHEMA_VERSION => v6::parse_slice(buffer).map(SummaryByVersion::V6),
        version => Err(Error::UnsupportedVersion(version.to_owned())),
    }
}
