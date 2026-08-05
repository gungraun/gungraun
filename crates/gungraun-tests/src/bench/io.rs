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

pub fn deserialize_json<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    let file = File::open(path).with_context(|| format!("Failed to open '{}'", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("Failed to deserialize '{}'", path.display()))
}

pub fn deserialize_yaml<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    let file = File::open(path).with_context(|| format!("Failed to open '{}'", path.display()))?;
    serde_yaml::from_reader(file)
        .with_context(|| format!("Failed to deserialize '{}'", path.display()))
}

pub fn deserialize_yaml_str<T>(string: &str, path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_yaml::from_str(string)
        .with_context(|| format!("Failed to deserialize '{}'", path.display()))
}

pub fn get_rust_version() -> Option<VersionMeta> {
    rustc_version::version_meta().ok()
}

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

pub fn print_info<T>(message: T)
where
    T: AsRef<str>,
{
    eprintln!("{}: {}", "bench".purple().bold(), message.as_ref());
}

pub fn serialize_yaml<T>(path: &Path, data: &T) -> Result<()>
where
    T: Serialize,
{
    let file =
        File::create(path).with_context(|| format!("Failed to create '{}'", path.display()))?;
    serde_yaml::to_writer(file, data)
        .with_context(|| format!("Failed to serialize '{}'", path.display()))
}
