//! Summary types and runner-side summary logic.
//!
//! This module separates the summary data model in [`model`] from the runner-specific
//! implementations in [`logic`]. Keeping the model independent minimizes dependencies for
//! `gungraun-summary`, which only needs the summary types.

#[cfg(feature = "runner")]
pub mod logic;
#[cfg(any(feature = "runner", feature = "summary", feature = "schema"))]
pub mod model;
#[cfg(feature = "runner")]
pub mod output;
