use core::panic;
use std::fmt::Write;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_cmd::assert::{Assert, AssertError};
use tempfile::{TempDir, tempdir};
use version_compare::Cmp;

pub const VALGRIND_WRAPPER: &str = env!("CARGO_BIN_EXE_valgrind-wrapper");
pub const FIXTURES_DIR: &str = env!("CLIENT_REQUEST_TESTS_FIXTURES");
pub const RUST_VERSION: &str = env!("CLIENT_REQUEST_TESTS_RUST_VERSION");

#[derive(Debug, Clone)]
pub enum Matcher {
    Exact(String),
    Contains(Vec<(String, usize)>),
}

impl Matcher {
    pub fn try_assert_output(self, assert: Assert) -> Result<Assert, Box<AssertError>> {
        match self {
            Matcher::Exact(fixture) => assert.try_stdout("").map_err(Box::new).and_then(|assert| {
                assert
                    .try_stderr(predicates::str::diff(fixture))
                    .map_err(Box::new)
            }),
            Matcher::Contains(items) => {
                let assert = assert.try_stdout("").map_err(Box::new)?;
                let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
                let mut failures = String::new();

                for (to_match, expected) in items {
                    let actual = stderr.matches(&to_match).count();
                    if actual != expected {
                        writeln!(
                            failures,
                            "expected {expected} occurrences, found {actual}: {to_match:?}"
                        )
                        .unwrap();
                    }
                }

                if failures.is_empty() {
                    Ok(assert)
                } else {
                    panic!("stderr occurrence count assertion failed:\n{failures}");
                }
            }
        }
    }
}

fn find_runner() -> Option<String> {
    for (key, value) in std::env::vars() {
        if key.starts_with("CARGO_TARGET_") && key.ends_with("_RUNNER") && !value.is_empty() {
            return Some(value);
        }
    }
    None
}

pub fn get_rust_version() -> String {
    RUST_VERSION.to_string()
}

pub fn compare_rust_version(cmp: Cmp, expected: &str) -> bool {
    version_compare::compare_to(get_rust_version(), expected, cmp)
        .expect("Rust version comparison should succeed")
}

pub fn get_test_bin_path(name: &str) -> PathBuf {
    PathBuf::from(VALGRIND_WRAPPER).parent().unwrap().join(name)
}

pub fn get_command<T>(path: T) -> Command
where
    T: AsRef<Path>,
{
    if let Some(runner) = find_runner() {
        let mut runner = runner.split_whitespace();
        let mut cmd = Command::new(runner.next().unwrap());
        for arg in runner {
            cmd.arg(arg);
        }
        cmd.arg(path.as_ref());
        cmd
    } else {
        Command::new(path.as_ref())
    }
}

pub fn get_test_bin_command<T>(name: T) -> Command
where
    T: AsRef<str>,
{
    let path = PathBuf::from(VALGRIND_WRAPPER)
        .parent()
        .unwrap()
        .join(name.as_ref());
    get_command(path)
}

pub fn get_valgrind_wrapper_command() -> Command {
    Command::new(VALGRIND_WRAPPER)
}

pub fn get_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(FIXTURES_DIR).join(name)
}

pub fn get_fixture_as_string(name: &str) -> String {
    let mut file = File::open(get_fixture_path(name))
        .unwrap_or_else(|_| panic!("Opening fixture '{name}' should succeed"));

    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .unwrap_or_else(|_| panic!("Reading content of fixture '{name}' should succeed"));

    buf
}

pub fn get_fixture(name: &str, target: Option<&str>, since: Option<&str>, suffix: &str) -> String {
    let mut file_name = String::from(name);
    if let Some(since) = since {
        write!(file_name, ".since_{since}").unwrap();
    }
    if let Some(target) = target {
        write!(file_name, ".{target}").unwrap();
    }
    write!(file_name, ".{suffix}").unwrap();
    get_fixture_as_string(&file_name)
}

pub fn get_sandbox() -> TempDir {
    tempdir().expect("Creating sandbox directory failed")
}
