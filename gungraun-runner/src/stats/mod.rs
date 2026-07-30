//! Statistical helpers used by benchmark execution.
//!
//! This module groups the shared statistical utilities in [`common`] and, when the `runner` feature
//! is enabled, the runner-specific statistics in [`runner`]. Keeping the `common` logic independent
//! from the `runner` logic minimizes dependencies for gungraun crates (like `gungraun-macros`).

pub mod common;
#[cfg(feature = "runner")]
pub mod runner;
