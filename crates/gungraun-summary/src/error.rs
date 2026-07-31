//! Error types returned while parsing Gungraun summary JSON.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned by the version-aware and version-specific summary parsers.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// Parsing failed because the input could not be read or deserialized into the expected summary
    /// shape.
    #[error("error parsing summary: {0}")]
    ParseError(String),
    /// Parsing failed because the summary declares a schema version this crate does not support.
    #[error("failed parsing summary: unsupported version '{0}'")]
    UnsupportedVersion(String),
}

/// Convenience alias for results returned by this crate.
pub type Result<T> = std::result::Result<T, Error>;
