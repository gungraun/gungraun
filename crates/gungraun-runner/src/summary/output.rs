//! TODO: DOCS

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Identifies the format of a summary file written by Gungraun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SummaryFormat {
    /// The format in a space optimal json representation without newlines
    Json,
    /// The format in pretty printed json
    PrettyJson,
}

/// Describes where Gungraun wrote the summary file and in which format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SummaryOutput {
    /// The [`SummaryFormat`]
    pub format: SummaryFormat,
    /// The path to the destination file of this summary
    pub path: PathBuf,
}

impl SummaryOutput {
    /// Creates a new `SummaryOutput` with `dir` as base dir and an extension fitting the.
    /// [`SummaryFormat`]
    pub(crate) fn new(format: SummaryFormat, dir: &Path) -> Self {
        Self {
            format,
            path: Self::path(dir),
        }
    }

    /// Try to create an empty summary file returning the [`File`] object
    pub fn create(&self) -> Result<File> {
        File::create(&self.path).with_context(|| "Failed to create json summary file")
    }

    /// Returns the path to this summary file.
    pub(crate) fn path(dir: &Path) -> PathBuf {
        dir.join("summary.json")
    }
}
