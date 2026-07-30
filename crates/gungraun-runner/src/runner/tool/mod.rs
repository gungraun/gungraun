//! The generic tool module. Contains all the basic modules to be expanded and used in the more
//! specific `Callgrind`, `Cachegrind`, ... modules

pub mod args;
pub mod config;
pub mod error_metric_parser;
pub mod generic_parser;
pub mod logfile_parser;
pub mod parser;
pub mod path;
pub mod regression;
pub mod run;
