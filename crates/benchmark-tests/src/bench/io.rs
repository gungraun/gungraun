use std::fs::File;
use std::path::Path;

use anyhow::Context;
use colored::Colorize;
use rustc_version::VersionMeta;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(super) fn deserialize_json<T>(path: &Path) -> T
where
    T: DeserializeOwned,
{
    serde_json::from_reader(
        File::open(path)
            .with_context(|| format!("File should exist: '{}'", path.display()))
            .unwrap(),
    )
    .map_err(|error| format!("Failed to deserialize '{}': {error}", path.display()))
    .expect("File should be deserializable")
}

pub(super) fn deserialize_yaml<T>(path: &Path) -> T
where
    T: DeserializeOwned,
{
    serde_yaml::from_reader(
        File::open(path)
            .with_context(|| format!("File should exist: '{}'", path.display()))
            .unwrap(),
    )
    .map_err(|error| format!("Failed to deserialize '{}': {error}", path.display()))
    .expect("File should be deserializable")
}

pub(super) fn deserialize_yaml_str<T>(string: &str, path: &Path) -> T
where
    T: DeserializeOwned,
{
    serde_yaml::from_str(string)
        .map_err(|error| format!("Failed to deserialize '{}': {error}", path.display()))
        .expect("File should be deserializable")
}

pub(super) fn get_rust_version() -> Option<VersionMeta> {
    rustc_version::version_meta().ok()
}

pub(super) fn print_error<T>(message: T)
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

pub(super) fn print_info<T>(message: T)
where
    T: AsRef<str>,
{
    eprintln!("{}: {}", "bench".purple().bold(), message.as_ref());
}

pub(super) fn serialize_yaml<T>(path: &Path, data: &T)
where
    T: Serialize,
{
    serde_yaml::to_writer(
        File::create(path)
            .with_context(|| {
                format!(
                    "Opening '{}' for files overwrite should succeed",
                    path.display()
                )
            })
            .unwrap(),
        &data,
    )
    .map_err(|error| format!("Failed to serialize '{}': {error}", path.display()))
    .expect("File should be serializable");
}
