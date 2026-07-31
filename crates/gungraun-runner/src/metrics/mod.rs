//! Metric value, comparison, and summary support.
//!
//! This module separates the serializable metric data model in [`model`] from runner-side metric
//! behavior in [`logic`]. Keeping the model independent lets summary (`gungraun-summary`) and
//! schema builds use metric types without pulling in runner-only logic.

#[cfg(feature = "runner")]
pub mod logic;
#[cfg(any(feature = "runner", feature = "summary", feature = "schema"))]
pub mod model;
