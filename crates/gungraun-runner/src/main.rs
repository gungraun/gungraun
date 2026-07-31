//! The `gungraun-runner` binary
use std::io::Write;

use colored::{Colorize, control};
use env_logger::Env;
use gungraun_runner::error::Error;
use gungraun_runner::runner::envs;
use log::{error, warn};

/// The main function of the `gungraun-runner` binary
///
/// We initialize the logging interface and configure the usage of colors as early as possible here.
/// Then we're printing warnings with [`print_warnings`] and finally call the main
/// [`gungraun_runner::runner::run`] library function catching and printing
/// [`gungraun_runner::error::Error`]s.
fn main() {
    // Configure the colored crate to respect GUNGRAUN_COLOR and CARGO_TERM_COLOR
    let gungraun_color = std::env::var(envs::GUNGRAUN_COLOR).ok();
    if let Some(var) = gungraun_color
        .clone()
        .or_else(|| std::env::var(envs::CARGO_TERM_COLOR).ok())
    {
        if var == "never" {
            control::set_override(false);
        } else if var == "always" {
            control::set_override(true);
        } else {
            // do nothing
        }
    }

    // Configure the env_logger crate to respect GUNGRAUN_COLOR and CARGO_TERM_COLOR
    env_logger::Builder::from_env(
        Env::default()
            .filter_or(envs::GUNGRAUN_LOG, "warn")
            .write_style(
                gungraun_color.map_or_else(|| envs::CARGO_TERM_COLOR, |_| envs::GUNGRAUN_COLOR),
            ),
    )
    .format(|buf, record| {
        writeln!(
            buf,
            "{}: {:<5}: {}",
            record
                .module_path()
                .unwrap_or_else(|| record.module_path_static().unwrap_or("???")),
            match record.level() {
                log::Level::Error => "Error".red().bold(),
                log::Level::Warn => "Warn".yellow().bold(),
                log::Level::Info => "Info".green().bold(),
                log::Level::Debug => "Debug".blue().bold(),
                log::Level::Trace => "Trace".cyan().bold(),
            },
            record.args()
        )
    })
    .init();

    print_warnings();
    match gungraun_runner::runner::run::run() {
        Ok(()) => {}
        Err(error) => {
            if let Some(Error::RegressionError(is_fatal)) = error.downcast_ref::<Error>() {
                if *is_fatal {
                    error!("{error}");
                }
                std::process::exit(3)
            } else {
                error!("{error}");
                std::process::exit(1)
            }
        }
    }
}

/// Print warnings for deprecated usages of environment variables
fn print_warnings() {
    if std::env::var("IAI_ALLOW_ASLR").is_ok() {
        warn!("The IAI_ALLOW_ASLR environment variable changed to GUNGRAUN_ALLOW_ASLR");
    }

    if std::env::var("RUST_LOG").is_ok() {
        warn!("The RUST_LOG environment variable to set the log level changed to GUNGRAUN_LOG");
    }

    if std::env::var("IAI_CALLGRIND_REGRESSION").is_ok() {
        warn!(
            "With version 0.17.0, the name of the environment variable `IAI_CALLGRIND_REGRESSION` \
             has changed to `GUNGRAUN_CALLGRIND_LIMITS`."
        );
    }

    for var in [
        "ALLOW_ASLR",
        "BASELINE",
        "BBV_ARGS",
        "CACHEGRIND_ARGS",
        "CACHEGRIND_LIMITS",
        "CACHEGRIND_METRICS",
        "CALLGRIND_ARGS",
        "CALLGRIND_LIMITS",
        "CALLGRIND_METRICS",
        "COLOR",
        "DEFAULT_TOOL",
        "DHAT_ARGS",
        "DHAT_LIMITS",
        "DHAT_METRICS",
        "DRD_ARGS",
        "DRD_METRICS",
        "FILTER",
        "HELGRIND_ARGS",
        "HELGRIND_METRICS",
        "HOME",
        "LIST",
        "LOAD_BASELINE",
        "LOG",
        "MASSIF_ARGS",
        "MEMCHECK_ARGS",
        "MEMCHECK_METRICS",
        "NOCAPTURE",
        "NOSUMMARY",
        "OUTPUT_FORMAT",
        "REGRESSION_FAIL_FAST",
        "SAVE_BASELINE",
        "SAVE_SUMMARY",
        "SEPARATE_TARGETS",
        "SHOW_GRID",
        "SHOW_INTERMEDIATE",
        "SHOW_ONLY_COMPARISON",
        "TOLERANCE",
        "TOOLS",
        "TRUNCATE_DESCRIPTION",
        "VALGRIND_ARGS",
    ] {
        let old = format!("IAI_CALLGRIND_{var}");
        if std::env::var(&old).is_ok() {
            let new = format!("GUNGRAUN_{var}");
            if std::env::var(&new).is_err() {
                warn!(
                    "With version 0.17.0, the name of the environment variable `{old}` has \
                     changed to `{new}`."
                );
            }
        }
    }
}
