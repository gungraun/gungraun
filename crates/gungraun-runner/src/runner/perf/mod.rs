//! Integration with the Linux `perf` tool for statistical profiling and recording.
//!
//! This module handles
//!
//! * `perf stat` and `perf record` execution in the [`run`] module, including the calibration run
//! * `perf stat/record` argument parsing in [`args`]
//! * `perf stat` JSON/log output parsing in [`json_parser`], [`records`] and [`logfile_parser`]
//!   using the JSON data [`model`]
//! * regression analysis in [`regression`]

pub mod args;
pub mod json_parser;
pub mod logfile_parser;
pub mod model;
pub mod pattern;
pub mod records;
pub mod regression;
pub mod run;
