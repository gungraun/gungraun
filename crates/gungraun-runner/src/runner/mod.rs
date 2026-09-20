//! The main runner module

pub mod args;
pub mod bin_bench;
pub mod cachegrind;
pub mod callgrind;
pub mod common;
pub mod dhat;
/// Names of environment variables which are used in different places
///
/// The variables here are not part of the parsed environment variables of `clap` in
/// [`crate::runner::args::CommandLineArgs`]
pub mod envs {
    /// The name of the package
    pub const CARGO_PKG_NAME: &str = "CARGO_PKG_NAME";
    /// Location of where to place all generated artifacts
    pub const CARGO_TARGET_DIR: &str = "CARGO_TARGET_DIR";
    /// The default color mode
    pub const CARGO_TERM_COLOR: &str = "CARGO_TERM_COLOR";

    /// The environment variable to set the color (same syntax as `CARGO_TERM_COLOR`)
    pub const GUNGRAUN_COLOR: &str = "GUNGRAUN_COLOR";
    /// Set the logging output of Gungraun
    pub const GUNGRAUN_LOG: &str = "GUNGRAUN_LOG";

    /// Internally used to set the terminal width for the --help print usable in just recipes
    pub const GUNGRAUN_TERM_WIDTH: &str = "__GUNGRAUN_TERM_WIDTH";
}
pub mod format;
pub mod lib_bench;
pub mod meta;
pub mod perf;
pub mod run;
pub mod tasks;
pub mod tool;

/// The default toggle/frame used by the [`crate::api::EntryPoint::Default`]
pub const DEFAULT_TOGGLE: &str = "*::__gungraun_wrapper_mod::*";
