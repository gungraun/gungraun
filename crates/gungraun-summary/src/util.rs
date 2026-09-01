//! Utilities and version-aware parsing helpers for Gungraun summary JSON.
//!
//! This module is the entrypoint for callers that do not know the summary schema version ahead of
//! time. The parse functions first inspect the summary's `version` field and then dispatch to the
//! matching versioned parser, such as [`crate::v6`] or [`crate::v7`].
//!
//! Use [`parse`] when reading from a file path and [`parse_slice`] when the summary JSON is already
//! available in memory.
//!
//! If you already know that the input is version 6 or 7, prefer the convenience parsers from the
//! corresponding version module directly.
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
//!     SummaryByVersion::V7(summary) => {
//!         assert_eq!(summary.version, "7");
//!     }
//!     other => eprintln!("unsupported summary: {other:?}"),
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
//!     SummaryByVersion::V7(summary) => {
//!         assert_eq!(summary.version, "7");
//!     }
//!     other => eprintln!("unsupported summary: {other:?}"),
//! }
//! # Ok::<(), gungraun_summary::error::Error>(())
//! ```

/// To prevent serializing f64 values inf, -inf, NaN into a null value, serialize f64 as string.
/// That way the reverse operation retains the original value.
pub(crate) mod float_64 {
    use std::str::FromStr;

    use serde::de::Visitor;
    use serde::{Deserializer, Serializer};

    struct FieldVisitor;

    impl Visitor<'_> for FieldVisitor {
        type Value = f64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string with a f64 value")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            f64::from_str(v).map_err(|error| serde::de::Error::custom(error.to_string()))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&v)
        }
    }

    /// Deserializes a `String` into a `f64`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FieldVisitor)
    }

    /// Serializes `f64` into a `String`.
    #[expect(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(input: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&input.to_string())
    }
}

use std::fs;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::{v6, v7};

/// A parsed summary tagged with the schema version used to deserialize it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SummaryByVersion {
    /// A summary parsed according to schema version 6.
    V6(v6::BenchmarkSummary),
    /// A summary parsed according to schema version 7.
    V7(v7::BenchmarkSummary),
}

/// A schema version supported by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Version {
    /// Schema version 6.
    V6,
    /// Schema version 7.
    V7,
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
            Self::V7(_) => Version::V7,
        }
    }
}

impl Version {
    /// Return the string representation for this schema version.
    pub const fn as_str(&self) -> &str {
        match self {
            Self::V6 => v6::SCHEMA_VERSION,
            Self::V7 => v7::SCHEMA_VERSION,
        }
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "v6" => Ok(Self::V6),
            "v7" => Ok(Self::V7),
            _ => Err(Error::CliArgument(
                "schema version".to_owned(),
                format!("invalid value '{s}'"),
            )),
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
        v7::SCHEMA_VERSION => v7::parse_slice(buffer).map(SummaryByVersion::V7),
        version => Err(Error::UnsupportedVersion(version.to_owned())),
    }
}
