//! Shared IO and serialization helpers for the bench subcrate.
//!
//! Centralizes the small, everywhere-used operations - reading and parsing YAML and JSON
//! config/manifests, writing YAML back out, and the [`print_info`] / [`print_error`] informational
//! logging the other modules use to narrate progress - so every module reads and writes case data
//! through the same context-bearing wrappers instead of reopening files and rederiving error
//! messages.

use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use rustc_version::VersionMeta;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Reads and deserializes the JSON file at `path` into `T`.
///
/// Errors are annotated with the file path for both open and parse failures.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or its contents cannot be deserialized into `T`.
pub fn deserialize_json<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    let file = File::open(path).with_context(|| format!("Failed to open '{}'", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("Failed to deserialize '{}'", path.display()))
}

/// Reads and deserializes the YAML file at `path` into `T`.
///
/// Errors are annotated with the file path for both open and parse failures.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or its contents cannot be deserialized into `T`.
pub fn deserialize_yaml<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    let file = File::open(path).with_context(|| format!("Failed to open '{}'", path.display()))?;
    serde_yaml::from_reader(file)
        .with_context(|| format!("Failed to deserialize '{}'", path.display()))
}

/// Deserializes `T` from a YAML `string`.
///
/// `path` is not read from disk - it is used only to annotate any parse error with a meaningful
/// source location. Use [`deserialize_yaml`] when the YAML source already lives in a file.
///
/// # Errors
///
/// Returns an error if `string` cannot be deserialized into `T`.
pub fn deserialize_yaml_str<T>(string: &str, path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_yaml::from_str(string)
        .with_context(|| format!("Failed to deserialize '{}'", path.display()))
}

/// Returns the metadata for the active Rust toolchain, or `None` if `rustc` cannot be invoked or
/// its output parsed.
pub fn get_rust_version() -> Result<VersionMeta> {
    rustc_version::version_meta().map_err(anyhow::Error::msg)
}

/// Prints an error `message` to stderr, prefixed with a colored `bench` tag and an `Error` label.
pub fn print_error<T>(message: T)
where
    T: AsRef<str>,
{
    eprintln!(
        "{}: {}: {}",
        "bench".purple().bold(),
        "Error".red().bold(),
        message.as_ref()
    );
}

/// Prints an informational `message` to stderr, prefixed with a colored `bench` tag.
pub fn print_info<T>(message: T)
where
    T: AsRef<str>,
{
    eprintln!("{}: {}", "bench".purple().bold(), message.as_ref());
}

/// Serializes `data` as YAML and writes it to `path`, creating or truncating the file as needed.
///
/// # Errors
///
/// Returns an error if the file cannot be created or `data` cannot be serialized.
pub fn serialize_yaml<T>(path: &Path, data: &T) -> Result<()>
where
    T: Serialize,
{
    let file =
        File::create(path).with_context(|| format!("Failed to create '{}'", path.display()))?;
    serde_yaml::to_writer(file, data)
        .with_context(|| format!("Failed to serialize '{}'", path.display()))
}
