//! The module responsible for the actual run of the benchmark

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};

use anyhow::Result;
use itertools::Itertools;
use log::{debug, error};
use os_str_bytes::OsStrBytesExt;

use super::config::ToolConfig;
use super::path::ToolOutputPath;
use crate::api::{self, BenchRunMode, ExitWith, PerfRunMode, Stream, Tool};
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::bin_bench::Delay;
use crate::runner::common::{Assistant, CapturedOutput, ModulePath};
use crate::runner::meta::Metadata;
use crate::runner::perf::run::{
    DEFAULT_PERF_CALIBRATION_TIME, PerfCalibration, PerfData, prepare_perf_command,
};
use crate::runner::tool::config::ToolConfigOptions;
use crate::util::resolve_binary_path;

/// The run options for the [`ToolCommand`]
#[derive(Debug, Default, Clone)]
pub struct RunOptions {
    /// Set the current directory of the [`ToolCommand`]
    pub current_dir: Option<PathBuf>,
    /// The optional [`Delay`] to apply to the command
    pub delay: Option<Delay>,
    /// If true, clear the environment variables
    pub env_clear: bool,
    /// The environment variables to pass into the [`ToolCommand`]
    pub envs: HashMap<OsString, OsString>,
    /// Configuration of the expected exit code/signal
    pub exit_with: Option<ExitWith>,
    /// If present, execute the [`ToolCommand`] in a [`api::Sandbox`]
    pub sandbox: Option<api::Sandbox>,
    /// The `setup` assistant to run if present
    pub setup: Option<Assistant>,
    /// The `stderr`
    pub stderr: Option<api::Stdio>,
    /// The `stdin`
    pub stdin: Option<api::Stdin>,
    /// The `stdout`
    pub stdout: Option<api::Stdio>,
    /// The `teardown` assistant to run if present
    pub teardown: Option<Assistant>,
}

/// A configured tool command ready to be executed.
///
/// This struct encapsulates a valgrind tool invocation with its command, output capture
/// configuration, and the specific tool being used.
#[derive(Debug)]
pub struct ToolCommand {
    /// The `std::process` command to be spawned
    pub command: Command,
    /// The resolved path to the benchmark executable.
    pub executable: PathBuf,
    /// Configuration for whether to capture or pass through the subprocess output
    pub nocapture: NoCapture,
    /// Optional path rebasing configuration for containerized runners
    ///
    /// When using `--tool-runner-root`, this contains the tuple `(original_workspace_root,
    /// replacement_path)` for rebasing paths to match the runner's perspective (e.g., inside a
    /// container).
    pub roots: Option<(PathBuf, PathBuf)>,
    /// The [`Tool`] to run
    pub tool: Tool,
}

/// A running tool process and its metadata.
///
/// This struct represents an actively spawned valgrind tool process and tracks information needed
/// to monitor its execution and validate its exit status.
#[derive(Debug)]
pub struct ToolCommandChild {
    /// The spawned child process, or `None` if the process has already been consumed
    pub child: Option<Child>,
    /// The path to the executable being profiled by the tool.
    pub executable: PathBuf,
    /// The expected exit behavior (exit code or signal), or `None` if any exit is acceptable
    pub exit_with: Option<ExitWith>,
    /// The path where the tool will write its normal output files.
    pub output_path: ToolOutputPath,
    /// Keeps the parent-side perf descriptors alive for the lifetime of the running tool process.
    pub perf_data: Option<PerfData>,
    /// The tool running this process (e.g., Memcheck, Callgrind, Massif)
    pub tool: Tool,
}

impl ToolCommand {
    /// Creates new `ToolCommand`.
    pub fn new(
        tool_config: &ToolConfig,
        meta: &Metadata,
        output_path: &ToolOutputPath,
        run_options: &RunOptions,
        executable: &Path,
        sandbox_dir: Option<&Path>,
    ) -> Result<Self> {
        let nocapture = if tool_config.is_default {
            meta.args.nocapture
        } else {
            NoCapture::False
        };

        let command = meta.to_tool_command(tool_config, output_path, run_options)?;
        let mut tool_command = Self {
            command,
            executable: executable.to_path_buf(),
            nocapture,
            tool: tool_config.tool(),
            roots: meta
                .args
                .tool_runner_root
                .clone()
                .map(|r| (meta.project_root.clone(), r)),
        };
        tool_command.executable = tool_command.resolve_executable(executable, sandbox_dir);

        Ok(tool_command)
    }

    /// Resolve an executable path, applying path rebasing if configured
    ///
    /// When `--tool-runner-root` is specified, this method attempts to rebase the executable
    /// path from the original workspace root to the runner's perspective. If rebasing is not
    /// possible or not configured, falls back to resolving the binary path normally.
    pub fn resolve_executable(&self, executable: &Path, current_dir: Option<&Path>) -> PathBuf {
        if let Some(rebased) = self.try_rebase_arg(executable.as_os_str()) {
            PathBuf::from(rebased)
        } else {
            resolve_binary_path(executable, current_dir)
                .unwrap_or_else(|_| executable.to_path_buf())
        }
    }

    /// Add an argument to the command
    ///
    /// This is a convenience wrapper around `self.command.arg()`.
    pub fn arg<T>(&mut self, arg: T) -> &mut Self
    where
        T: AsRef<OsStr>,
    {
        self.command.arg(arg.as_ref());
        self
    }

    /// Add an argument to the command, applying path rebasing if configured
    ///
    /// When `--tool-runner-root` is specified and the argument appears to be a path that needs
    /// rebasing, this method will rebase it. Otherwise, it behaves like [`Self::arg`].
    pub fn arg_rebase<T>(&mut self, arg: T) -> &mut Self
    where
        T: AsRef<OsStr>,
    {
        let arg = arg.as_ref();

        if let Some(rebased) = self.try_rebase_arg(arg) {
            self.command.arg(rebased);
        } else {
            self.command.arg(arg);
        }

        self
    }

    /// Add multiple arguments to the command
    ///
    /// This is a convenience wrapper that calls [`Self::arg`] for each argument.
    pub fn args<I, T>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    /// Add multiple arguments to the command, applying path rebasing if configured
    ///
    /// This is a convenience wrapper that calls [`Self::arg_rebase`] for each argument.
    pub fn args_rebase<I, T>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<OsStr>,
    {
        for arg in args {
            self.arg_rebase(arg);
        }
        self
    }

    /// Clone this command, preserving path rebasing state.
    pub fn clone_command(&self) -> Self {
        Self {
            command: clone_command(&self.command),
            executable: self.executable.clone(),
            nocapture: self.nocapture,
            roots: self.roots.clone(),
            tool: self.tool,
        }
    }

    /// Append tool args, the stored executable, and executable args to the command.
    pub fn append_tool_invocation<I, T>(
        &mut self,
        tool_args: I,
        executable_args: &[OsString],
    ) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<OsStr>,
    {
        self.args_rebase(tool_args);
        self.command.arg(&self.executable);
        self.args_rebase(executable_args)
    }

    /// Run the `ToolCommand`
    pub fn run<'args, F>(
        mut self,
        config: &ToolConfig,
        executable_args_fn: &F,
        run_options: &RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        child: Option<&mut Child>,
        captured_output: Option<&CapturedOutput>,
        sandbox_dir: Option<&Path>,
        tool_runner_dest: Option<&Path>,
    ) -> Result<ToolCommandChild>
    where
        F: Fn(&ToolConfig, Option<BenchRunMode>) -> Cow<'args, [OsString]>,
    {
        let RunOptions {
            current_dir,
            exit_with,
            stdin,
            stdout,
            stderr,
            ..
        } = run_options;

        // If set, the timeout is expected to happen and the program/perf exits with a signal code
        // since we interrupt perf with SIGTERM or SIGKILL. We do not override a user-specified exit
        // status expectation, but in all other cases we set the expected exit signals.
        let exit_with = match exit_with {
            None if config.timeout.is_some() => Some(ExitWith::Signals(vec![9, 15])),
            _ => exit_with.clone(),
        };

        match (sandbox_dir, current_dir.as_ref()) {
            (None, None) => {}
            (None, Some(current_dir)) => {
                self.command.current_dir(current_dir);
            }
            (Some(sandbox_dir), None) => {
                self.command.current_dir(sandbox_dir);
            }
            (Some(sandbox_dir), Some(current_dir)) => {
                // If run_dir is absolute uses run_dir otherwise joins the paths
                let path = sandbox_dir.join(current_dir);
                self.command.current_dir(path);
            }
        }

        let executable_args = executable_args_fn(config, None);

        let (mut perf_data, args) = if let ToolConfigOptions::Perf(options) = &config.options {
            if let Some(time) = match options.run_mode {
                PerfRunMode::DefaultCalibrate => Some(DEFAULT_PERF_CALIBRATION_TIME),
                PerfRunMode::Calibrate(time) => Some(time),
                _ => None,
            } {
                let calibration_args =
                    executable_args_fn(config, Some(BenchRunMode::PerfCalibrate));
                PerfCalibration::new(
                    &self,
                    config,
                    &calibration_args,
                    output_path,
                    time,
                    tool_runner_dest,
                )
                .run()?;
            }

            prepare_perf_command(
                &mut self.command,
                config,
                output_path,
                options.use_sampling,
                tool_runner_dest,
            )
            .map(|(perf_data, tool_args, _)| (Some(perf_data), tool_args))?
        } else {
            let mut tool_args = config.args.clone();
            tool_args.set_output_arg(output_path, tool_runner_dest);
            tool_args.set_log_arg(output_path, tool_runner_dest);
            tool_args.set_xtree_arg(output_path, tool_runner_dest);
            tool_args.set_xleak_arg(output_path, tool_runner_dest);

            (None, tool_args.to_vec())
        };

        debug!(
            "{}: Tool arguments: {}",
            self.tool.id(),
            args.iter()
                .map(|s| s.to_string_lossy().to_string())
                .join(" ")
        );

        debug!(
            "{}: Executable: {}",
            self.tool.id(),
            self.executable.display()
        );
        debug!(
            "{}: Executable arguments: {}",
            self.tool.id(),
            executable_args
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .join(" ")
        );

        if let Some(p) = perf_data.as_mut() {
            p.log_file.write_header_command(
                &self.command,
                &args,
                &self.executable,
                executable_args.as_ref(),
            )?;
        }

        self.append_tool_invocation(args, executable_args.as_ref());

        self.nocapture.apply(&mut self.command, captured_output)?;

        if let Some(stdin) = stdin {
            stdin
                .apply(&mut self.command, Stream::Stdin, child, sandbox_dir)
                .map_err(|error| Error::BenchmarkError(self.tool, module_path.clone(), error))?;
        }

        if let Some(stdout) = stdout {
            stdout
                .apply(&mut self.command, Stream::Stdout, sandbox_dir)
                .map_err(|error| Error::BenchmarkError(self.tool, module_path.clone(), error))?;
        }

        if let Some(stderr) = stderr {
            stderr
                .apply(&mut self.command, Stream::Stderr, sandbox_dir)
                .map_err(|error| Error::BenchmarkError(self.tool, module_path.clone(), error))?;
        }

        self.command
            .spawn()
            .and_then(|c| {
                if let Some(p) = perf_data.as_mut() {
                    p.log_file.finalize_header(c.id(), config.part)?;
                }

                Ok(ToolCommandChild::new(
                    self.tool,
                    c,
                    self.executable,
                    exit_with,
                    output_path.clone(),
                    perf_data,
                ))
            })
            .map_err(|error| {
                Error::LaunchError(PathBuf::from(config.tool().id()), error.to_string()).into()
            })
    }

    /// Attempts to rebase a path argument if it starts with the workspace root.
    ///
    /// Returns `Some(rebased_arg)` if rebasing was successful, `None` if the argument
    /// should be passed through unchanged.
    fn try_rebase_arg(&self, arg: &OsStr) -> Option<OsString> {
        let (workspace_root, new_root) = self.roots.as_ref()?;

        if arg.starts_with("-") {
            if let Some((key, value)) = arg.split_once("=") {
                Self::try_rebase_path_arg(key, value, workspace_root, new_root, "=")
            } else if let Some((key, value)) = arg.split_once(" ") {
                Self::try_rebase_path_arg(key, value, workspace_root, new_root, " ")
            } else {
                None
            }
        } else {
            Path::new(arg)
                .strip_prefix(workspace_root)
                .ok()
                .map(|suffix| new_root.join(suffix).into_os_string())
        }
    }

    /// Attempts to rebase a key-value argument where the value is a path.
    ///
    /// Returns `Some(rebased_arg)` if the value path was successfully rebased,
    /// `None` if the value is not under the workspace root.
    fn try_rebase_path_arg(
        key: &OsStr,
        value: &OsStr,
        workspace_root: &Path,
        new_root: &Path,
        separator: &str,
    ) -> Option<OsString> {
        let suffix = Path::new(value).strip_prefix(workspace_root).ok()?;

        let new_path = new_root.join(suffix);
        let mut new_arg = key.to_os_string();
        new_arg.push(separator);
        new_arg.push(new_path.into_os_string());

        Some(new_arg)
    }
}

impl ToolCommandChild {
    /// Creates a new `ToolCommandChild` instance to manage a spawned tool process.
    ///
    /// This constructor wraps a spawned child process along with metadata needed to track and
    /// manage its execution. The `tool` parameter specifies which [`Tool`] is being run,
    /// `child` is the actual spawned process, `executable` is the path to the binary being
    /// instrumented, `exit_with` defines the expected exit behavior, `output_path` specifies where
    /// the tool writes its output.
    pub fn new(
        tool: Tool,
        child: Child,
        executable: PathBuf,
        exit_with: Option<ExitWith>,
        output_path: ToolOutputPath,
        perf_data: Option<PerfData>,
    ) -> Self {
        Self {
            child: Some(child),
            executable,
            exit_with,
            output_path,
            tool,
            perf_data,
        }
    }
}

/// Check the exit code of the [`ToolCommand`] and verify the expected [`ExitWith`] if present
pub fn check_exit(
    tool: Tool,
    executable: &Path,
    output: Output,
    output_path: &ToolOutputPath,
    exit_with: Option<&ExitWith>,
) -> Result<Output> {
    let Some(status_code) = output.status.code() else {
        match (output.status.signal(), exit_with) {
            (None, _) => {
                error!(
                    "{}: Expected '{}' to exit in a reproducible way but neither signal nor exit \
                     code were set",
                    tool.id(),
                    executable.display()
                );
                return Err(
                    Error::new_process_error(tool.id(), output, Some(output_path.clone())).into(),
                );
            }
            (Some(signal), None | Some(ExitWith::Success)) => {
                error!(
                    "{}: Expected '{}' to exit with success but exited with signal '{signal}'",
                    tool.id(),
                    executable.display()
                );
                return Err(
                    Error::new_process_error(tool.id(), output, Some(output_path.clone())).into(),
                );
            }
            (Some(_), Some(ExitWith::Failure)) => {
                return Ok(output);
            }
            (Some(signal), Some(ExitWith::Signal(expected_signal)))
                if signal == *expected_signal =>
            {
                return Ok(output);
            }
            (Some(signal), Some(ExitWith::Signals(expected_signals)))
                if expected_signals.contains(&signal) =>
            {
                return Ok(output);
            }
            (Some(signal), Some(ExitWith::Signal(expected_signal))) => {
                error!(
                    "{}: Expected '{}' to exit with signal '{expected_signal}' but exited with \
                     signal '{signal}'",
                    tool.id(),
                    executable.display()
                );
                return Err(
                    Error::new_process_error(tool.id(), output, Some(output_path.clone())).into(),
                );
            }
            (Some(signal), Some(ExitWith::Signals(expected_signals))) => {
                error!(
                    "{}: Expected '{}' to exit with one of these signals '{}' but exited with \
                     signal '{signal}'",
                    tool.id(),
                    executable.display(),
                    expected_signals.iter().map(ToString::to_string).join(", ")
                );
                return Err(
                    Error::new_process_error(tool.id(), output, Some(output_path.clone())).into(),
                );
            }
            (Some(signal), Some(ExitWith::Code(code))) => {
                error!(
                    "{}: Expected '{}' to exit with code '{code}' but exited with signal \
                     '{signal}'",
                    tool.id(),
                    executable.display()
                );
                return Err(
                    Error::new_process_error(tool.id(), output, Some(output_path.clone())).into(),
                );
            }
        }
    };

    match (status_code, exit_with) {
        (0i32, None | Some(ExitWith::Code(0i32) | ExitWith::Success)) => Ok(output),
        (0i32, Some(ExitWith::Code(code))) => {
            error!(
                "{}: Expected '{}' to exit with '{}' but it succeeded",
                tool.id(),
                executable.display(),
                code
            );
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
        }
        (0i32, Some(ExitWith::Failure)) => {
            error!(
                "{}: Expected '{}' to fail but it succeeded",
                tool.id(),
                executable.display(),
            );
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
        }
        (_, Some(ExitWith::Failure)) => Ok(output),
        (code, Some(ExitWith::Success)) => {
            error!(
                "{}: Expected '{}' to succeed but it terminated with '{}'",
                tool.id(),
                executable.display(),
                code
            );
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
        }
        (actual_code, Some(ExitWith::Code(expected_code))) if actual_code == *expected_code => {
            Ok(output)
        }
        (actual_code, Some(ExitWith::Code(expected_code))) => {
            error!(
                "{}: Expected '{}' to exit with '{}' but it terminated with '{}'",
                tool.id(),
                executable.display(),
                expected_code,
                actual_code
            );
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
        }
        _ => Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into()),
    }
}

/// Clone the stable parts of a [`Command`]
///
/// The stable parts are: exe path, args, current dir and env vars
pub fn clone_command(command: &Command) -> Command {
    let mut clone = Command::new(command.get_program());
    clone.args(command.get_args());

    if let Some(current_dir) = command.get_current_dir() {
        clone.current_dir(current_dir);
    }

    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            clone.env(key, value);
        } else {
            clone.env_remove(key);
        }
    }

    clone
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{tool_config_f, tool_output_path_f};

    #[test]
    fn test_append_tool_invocation_rebases_tool_and_benchmark_args() {
        let mut tool_command = ToolCommand {
            command: Command::new("runner"),
            executable: PathBuf::from("/container/workspace/target/release/deps/bench"),
            nocapture: NoCapture::False,
            roots: Some((
                PathBuf::from("/host/workspace"),
                PathBuf::from("/container/workspace"),
            )),
            tool: Tool::Perf,
        };
        let tool_args = [
            OsString::from("--output=/host/workspace/target/perf.json"),
            OsString::from("--plain"),
        ];
        let executable_args = [OsString::from(
            "--fixture=/host/workspace/fixtures/input.txt",
        )];

        tool_command.append_tool_invocation(&tool_args, &executable_args);

        let args = tool_command
            .command
            .get_args()
            .map(OsStr::to_os_string)
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                OsString::from("--output=/container/workspace/target/perf.json"),
                OsString::from("--plain"),
                OsString::from("/container/workspace/target/release/deps/bench"),
                OsString::from("--fixture=/container/workspace/fixtures/input.txt"),
            ]
        );
    }

    #[test]
    fn test_cloned_tool_command_preserves_rebasing_roots() {
        let tool_command = ToolCommand {
            command: Command::new("runner"),
            executable: PathBuf::from("/container/workspace/target/release/deps/bench"),
            nocapture: NoCapture::False,
            roots: Some((
                PathBuf::from("/host/workspace"),
                PathBuf::from("/container/workspace"),
            )),
            tool: Tool::Perf,
        };

        let mut cloned = tool_command.clone_command();
        cloned.arg_rebase("--input=/host/workspace/data.txt");

        assert_eq!(
            cloned.command.get_args().collect::<Vec<_>>(),
            vec![OsStr::new("--input=/container/workspace/data.txt")]
        );
        assert_eq!(cloned.executable, tool_command.executable);
    }

    #[test]
    fn test_prepare_perf_command_uses_tool_runner_dest_for_output_arg() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = tool_output_path_f()
            .init(true)
            .target_dir(temp_dir.path())
            .tool(Tool::Perf)
            .fx();
        let config = tool_config_f().tool(Tool::Perf).fx();
        let tool_runner_dest = Path::new("/runner/dest");
        let mut command = Command::new("perf");

        let (_perf_data, args, _log_path) = prepare_perf_command(
            &mut command,
            &config,
            &output_path,
            false,
            Some(tool_runner_dest),
        )
        .unwrap();

        let mut expected = OsString::from("--output=");
        expected.push(tool_runner_dest.join(output_path.file_name()));

        assert!(
            args.contains(&expected),
            "perf args did not contain rebased output path {expected:?}: {args:?}"
        );
    }
}
