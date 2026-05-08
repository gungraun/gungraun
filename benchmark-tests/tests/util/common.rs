use std::ffi::OsString;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use anyhow::{Context, Result};
use gungraun::ValgrindTool;
use gungraun_runner::runner::summary::BaselineKind;
use gungraun_runner::runner::tasks::ProcessHandler;
use gungraun_runner::runner::tool::path::{ToolOutputPath, ToolOutputPathKind};
use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};

pub const DEFAULT_TOOL: ValgrindTool = ValgrindTool::Callgrind;
pub const FIXTURES_ROOT: &str = "tests/fixtures";

#[macro_export]
macro_rules! assert_not_elapsed {
    ($time:expr, $body:expr) => {{
        let start = std::time::Instant::now();
        let result = $body;
        let elapsed = start.elapsed();
        assert!(elapsed < $time);
        result
    }};
}

pub static BENCH_BIN_FAKE_EXE: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_BIN_EXE_bench-bin-fake")));
pub static ECHO_EXE: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_BIN_EXE_echo")));
pub static TIMEOUT_EXE: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_BIN_EXE_timeout")));
pub static DELAY_EXE: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_BIN_EXE_delay")));
pub static CAT_EXE: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from(env!("CARGO_BIN_EXE_cat")));
pub static EXIT_WITH_EXE: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_BIN_EXE_exit-with")));
pub static FILE_EXISTS_EXE: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_BIN_EXE_file-exists")));

#[derive(Debug)]
pub struct Fixtures;

#[derive(Debug)]
pub struct Runner {
    args: Vec<OsString>,
    path: OsString,
}

#[derive(Debug)]
pub struct RunnerOutput(Output);

#[derive(Debug, Clone)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Fixtures {
    pub fn get_path() -> PathBuf {
        let root = get_project_root();
        if root.ends_with("benchmark-tests") {
            root.join(FIXTURES_ROOT)
        } else {
            root.join("benchmark-tests").join(FIXTURES_ROOT)
        }
    }

    pub fn get_path_of<T>(name: T) -> PathBuf
    where
        T: AsRef<Path>,
    {
        let path = Self::get_path().join(name);
        assert!(
            path.exists(),
            "Fixtures path '{}' does not exist",
            path.display()
        );
        path
    }

    pub fn get_tool_output_path(
        dir: &str,
        tool: ValgrindTool,
        kind: ToolOutputPathKind,
        name: &str,
    ) -> ToolOutputPath {
        ToolOutputPath {
            kind,
            tool,
            baseline_kind: BaselineKind::Old,
            dir: Self::get_path().join(dir),
            name: name.to_owned(),
            modifiers: vec![],
            temp: None,
        }
    }

    pub fn load_serialized<T, N>(name: N) -> Result<T, serde_yaml::Error>
    where
        T: for<'de> Deserialize<'de>,
        N: AsRef<Path>,
    {
        let file = File::open(Self::get_path_of(name)).unwrap();
        serde_yaml::from_reader::<File, T>(file)
    }

    pub fn load_stacks<T>(path: T) -> Vec<String>
    where
        T: AsRef<Path>,
    {
        let path = Self::get_path_of(path);
        let reader = BufReader::new(File::open(path).unwrap());
        reader.lines().map(std::result::Result::unwrap).collect()
    }

    pub fn save_serialized<T, N>(name: N, value: &T) -> Result<(), serde_yaml::Error>
    where
        T: Serialize,
        N: AsRef<Path>,
    {
        let file = File::create(Self::get_path_of(name)).unwrap();
        serde_yaml::to_writer(file, value)
    }
}

impl Runner {
    pub fn new() -> Self {
        let root = get_project_root();
        let runner_path = root.join("target/release/gungraun-runner");
        Self {
            path: runner_path.into_os_string(),
            args: vec![],
        }
    }

    pub fn args(&mut self, args: &[&str]) -> &mut Self {
        for arg in args {
            self.args.push(OsString::from(arg));
        }

        self
    }

    pub fn run(&self) -> RunnerOutput {
        let build =
            Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
                .args(["build", "--package", "gungraun-runner", "--release"])
                .status()
                .expect("Running the build command to build gungraun-runner should succeed");
        assert!(build.success(), "Building gungraun-runner should succeed");

        Command::new(&self.path)
            .args(&self.args)
            .env("GUNGRAUN_COLOR", "never")
            .output()
            .map(RunnerOutput)
            .unwrap()
    }
}

impl Default for Runner {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerOutput {
    #[track_caller]
    pub fn assert_stderr(&self, expected: &str) -> &Self {
        assert_eq!(std::str::from_utf8(&self.0.stderr).unwrap(), expected);
        self
    }

    #[track_caller]
    pub fn assert_stdout(&self, expected: &str) -> &Self {
        assert_eq!(std::str::from_utf8(&self.0.stdout).unwrap(), expected);
        self
    }

    #[track_caller]
    pub fn assert_stderr_bytes(&self, expected: &[u8]) -> &Self {
        assert_eq!(&self.0.stderr, expected);
        self
    }

    #[track_caller]
    pub fn assert_stdout_bytes(&self, expected: &[u8]) -> &Self {
        assert_eq!(&self.0.stdout, expected);
        self
    }

    #[track_caller]
    pub fn assert_stdout_is_empty(&self) -> &Self {
        assert!(
            self.0.stdout.is_empty(),
            "Expected stdout to be empty but was: {}",
            std::str::from_utf8(&self.0.stdout).unwrap()
        );
        self
    }
}

impl Version {
    pub fn new(version: &str) -> Self {
        let [major, minor, patch] = version
            .split('.')
            .map(|s| s.parse::<u64>().unwrap())
            .collect::<Vec<u64>>()[..]
        else {
            panic!("Invalid version: '{version}'");
        };

        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn increment(&mut self, part: &str) {
        match part {
            "major" => {
                self.major += 1;
            }
            "minor" => {
                self.minor += 1;
            }
            "patch" => {
                self.patch += 1;
            }
            _ => {
                panic!("Invalid part: {part}");
            }
        }
    }

    pub fn decrement(&mut self, part: &str) {
        match part {
            "major" => {
                self.major = self.major.saturating_sub(1);
            }
            "minor" => {
                self.minor = self.minor.saturating_sub(1);
            }
            "patch" => {
                self.patch = self.patch.saturating_sub(1);
            }
            _ => {
                panic!("Invalid part: {part}");
            }
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[track_caller]
pub fn assert_parse_error<T>(file: &Path, result: Result<T>, message: &str)
where
    T: std::cmp::PartialEq + std::fmt::Debug,
{
    assert_eq!(
        result.unwrap_err().to_string(),
        format!("Error parsing file '{}': {message}", file.display())
    );
}

/// Cleanup the process handler child processes to avoid zombie processes if possible
pub fn cleanup_test_process_handler(process_handler: ProcessHandler) {
    if let Some((_, mut child)) = process_handler.setup {
        child
            .kill()
            .expect("Killing the setup process should succeed");
        child
            .wait()
            .expect("Waiting for the setup process should succeed");
    }
    if let Some(tool_command_child) = process_handler.bench {
        if let Some(mut child) = tool_command_child.child {
            child
                .kill()
                .expect("Killing the benchmark process should succeed");
            child
                .wait()
                .expect("Waiting for the benchmark process should succeed");
        }
    }
    if let Some((_, mut child)) = process_handler.teardown {
        child
            .kill()
            .expect("Killing the teardown process should succeed");
        child
            .wait()
            .expect("Waiting for the teardown process should succeed");
    }
}

/// If the [`log::Level`] matches dump the content of all output files into the `writer`
pub fn tool_output_path_dump<W>(tool_output_path: &ToolOutputPath, writer: &mut W) -> Result<()>
where
    W: Write,
{
    for path in tool_output_path.real_paths()? {
        let file = File::open(&path).with_context(|| {
            format!(
                "Error opening {} output file '{}'",
                tool_output_path.tool.id(),
                path.display()
            )
        })?;

        let mut reader = BufReader::new(file);
        std::io::copy(&mut reader, writer)?;
    }
    Ok(())
}

pub fn get_project_root() -> PathBuf {
    let meta = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("Querying metadata of cargo workspace succeeds");

    meta.workspace_root.into_std_path_buf()
}

pub fn get_runner_version() -> Version {
    let meta = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("Querying metadata of cargo workspace succeeds");

    meta.packages
        .iter()
        .find_map(|p| {
            p.name.as_str().eq("gungraun-runner").then_some(Version {
                major: p.version.major,
                minor: p.version.minor,
                patch: p.version.patch,
            })
        })
        .expect("The version information for gungraun-runner should exists")
}
