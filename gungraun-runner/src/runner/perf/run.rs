//! Execution support for `perf stat` and `perf record` runs, including calibration and overhead
//! measurement.
//!
//! This module implements most of the runner-side half of the actual perf benchmark run. The
//! benchmark-side control macros (`perf_enable!`, `perf_disable!`) live in the `gungraun::perf`
//! module of the main library crate.
//!
//! # Run flow
//!
//! 1. **Orchestration** — [`ToolCommand::run`] decides whether to execute a normal run or a perf
//!    run. For perf, it may first run [`PerfCalibration`] if configured via
//!    [`PerfRunMode::DefaultCalibrate`] or [`PerfRunMode::Calibrate`].
//! 2. **Command preparation** — [`prepare_perf_command`] configures the `perf` subprocess:
//!    * Creates control pipes ([`PERF_CTL_FD_READ`]/[`PERF_CTL_FD_WRITE`] and
//!      [`PERF_ACK_FD_READ`]/[`PERF_ACK_FD_WRITE`]) for enabling/disabling data collection via
//!      `perf`'s --control argument.
//!    * Creates a log file and passes its file descriptor ([`PERF_LOG_FD`]) to the child along with
//!      the other pipes
//!    * Uses the [`Command::pre_exec`] hook to install and remap the file descriptors of the
//!      control pipes inside the child process.
//!    * See [`PerfData`] for more details about the control pipes.
//! 3. **Benchmark execution** — The benchmark binary runs under `perf`. By default perf collection
//!    starts automatically at the entry point. The benchmark harness calls `perf_enable!` /
//!    `perf_disable!` (from `gungraun::perf`) to toggle collection around the code region of
//!    interest (by default the benchmark function)
//! 4. **Result collection** — After the child exits, the runner reads the log file and parses the
//!    raw `perf` output. [`parse_perf_log`] converts the log into structured [`PerfLogData`]
//!    records that feed into the final [`BenchmarkSummary`].
//!
//! [`BenchmarkSummary`]: crate::summary::model::BenchmarkSummary
//! [`PERF_ACK_FD_READ`]: crate::api::PERF_ACK_FD_READ
//! [`PERF_ACK_FD_WRITE`]: crate::api::PERF_ACK_FD_WRITE
//! [`PERF_CTL_FD_READ`]: crate::api::PERF_CTL_FD_READ
//! [`PERF_CTL_FD_WRITE`]: crate::api::PERF_CTL_FD_WRITE
//! [`PERF_LOG_FD`]: crate::api::PERF_LOG_FD
//! [`PerfRunMode::DefaultCalibrate`]: crate::api::PerfRunMode
//! [`PerfRunMode::Calibrate`]: crate::api::PerfRunMode
//! [`ToolCommand::run`]: crate::runner::tool::run::ToolCommand::run
//! [`parse_perf_log`]: crate::runner::perf::logfile_parser::parse_perf_log
//! [`PerfLogData`]: super::logfile_parser::PerfLogData

use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use derive_more::{AsRef, Deref, DerefMut};
use itertools::Itertools;
use log::debug;
use nix::unistd::pipe;

use crate::api::{
    BenchRunMode, ExitWith, PERF_ACK_FD_READ, PERF_ACK_FD_WRITE, PERF_CTL_FD_READ,
    PERF_CTL_FD_WRITE, PERF_LOG_FD,
};
use crate::error::Error;
use crate::runner::meta::Metadata;
use crate::runner::perf::logfile_parser::parse_perf_log;
use crate::runner::tasks::ProcessChild;
use crate::runner::tool::config::ToolConfig;
use crate::runner::tool::path::ToolOutputPath;
use crate::runner::tool::run::{RunOptions, ToolCommand, check_exit};
use crate::util::{close_if_different, dup_to_inheritable_fd};

/// The default perf calibration sample time
pub const DEFAULT_PERF_CALIBRATION_TIME: Duration = Duration::new(1, 0);
/// File name modifier used for calibration [`ToolOutputPath`]s.
pub const PERF_CALIBRATION_FILE_MODIFIER: &str = "cal";
/// File name modifier used for overhead measurement [`ToolOutputPath`]s.
pub const PERF_OVERHEAD_FILE_MODIFIER: &str = "overhead";

/// Runs perf calibration candidates and retains the minimum JSON/log pair.
pub struct PerfCalibration<'a> {
    base_command: &'a ToolCommand,
    config: &'a ToolConfig,
    executable: &'a Path,
    executable_args: &'a [OsString],
    output_path: &'a ToolOutputPath,
    time: Duration,
    tool_runner_dest: Option<&'a Path>,
}

/// File descriptors and log file that must stay alive for a spawned perf run.
///
/// `prepare_perf_command` captures raw fd numbers from these handles and installs a `pre_exec` hook
/// that remaps them onto the fixed perf protocol file descriptors in the child process. Keeping
/// this struct alive ensures the underlying [`OwnedFd`]s and [`File`] remain open until after the
/// benchmark process has been spawned.
///
/// Once the run is finished, dropping this struct closes the original descriptors in the parent
/// process.
#[derive(Debug)]
#[expect(
    dead_code,
    reason = "This struct keeps the original perf file descriptors alive until spawn completes \
              and closes them on drop afterwards"
)]
pub struct PerfData {
    ack_fd_read: OwnedFd,
    ack_fd_write: OwnedFd,
    ctl_fd_read: OwnedFd,
    ctl_fd_write: OwnedFd,
    pub(crate) log_file: PerfLogFile,
}

/// Wrapper around the perf log file used while constructing a perf run.
#[derive(Debug, AsRef, Deref, DerefMut)]
pub struct PerfLogFile(File);

impl<'a> PerfCalibration<'a> {
    /// Creates a perf calibration runner for one benchmark invocation.
    pub fn new(
        base_command: &'a ToolCommand,
        config: &'a ToolConfig,
        executable: &'a Path,
        executable_args: &'a [OsString],
        output_path: &'a ToolOutputPath,
        time: Duration,
        tool_runner_dest: Option<&'a Path>,
    ) -> Self {
        Self {
            base_command,
            config,
            executable,
            executable_args,
            output_path,
            time,
            tool_runner_dest,
        }
    }

    /// Runs perf calibration in sampling mode for a fixed duration, leaving raw
    /// records in `.cal.out`. The mean is computed later by `parse_adjustment`.
    pub fn run(&self) -> Result<()> {
        debug!("Running perf calibration");

        let cal_output_path = self
            .output_path
            .with_added_modifiers([PERF_CALIBRATION_FILE_MODIFIER]);

        let mut tool_command = self.base_command.clone_command();
        let (mut perf_data, args, _) = prepare_perf_command(
            &mut tool_command.command,
            self.config,
            &cal_output_path,
            true,
            self.tool_runner_dest,
        )?;

        perf_data.log_file.write_header_command(
            &tool_command.command,
            &args,
            self.executable,
            self.executable_args,
        )?;

        tool_command.append_tool_invocation(args, self.executable, self.executable_args);

        tool_command.command.stdout(Stdio::null());
        tool_command.command.stderr(perf_data.log_file.try_clone()?);

        let child = tool_command
            .command
            .spawn()
            .map_err(|error| {
                Error::LaunchError(PathBuf::from(self.config.tool().id()), error.to_string())
            })
            .map(ProcessChild)?;

        perf_data
            .log_file
            .finalize_header(child.id(), self.config.part)?;

        thread::sleep(self.time);

        let output = child.wait(
            &Arc::new(AtomicBool::new(false)),
            Duration::from_millis(10),
            Some(Duration::ZERO),
        )?;

        check_exit(
            self.config.tool(),
            self.executable,
            output,
            &cal_output_path.to_log_output(),
            Some(&ExitWith::Signals(vec![9, 15])),
        )
        .with_context(|| "Failed calibration run")
        .map(|_| ())
    }
}

impl PerfLogFile {
    /// Write the command line header entry for a perf log.
    pub fn write_header_command(
        &mut self,
        command: &Command,
        args: &[OsString],
        executable: &Path,
        executable_args: &[OsString],
    ) -> std::io::Result<()> {
        writeln!(
            self.0,
            "Command: {} {} {} {} {}",
            Path::new(command.get_program()).display(),
            command
                .get_args()
                .map(|s| s.to_string_lossy().to_string())
                .join(" "),
            args.iter()
                .map(|s| s.to_string_lossy().to_string())
                .join(" "),
            executable.display(),
            executable_args
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .join(" ")
        )
    }

    /// Write the spawned process metadata and terminate the perf log header with a newline
    pub fn finalize_header(&mut self, pid: u32, part: Option<usize>) -> std::io::Result<()> {
        writeln!(self.0, "Pid: {pid}")
            .and_then(|()| writeln!(self.0, "Part: {}", part.unwrap_or(1)))
            // The empty line separates the header from the user log entries
            .and_then(|()| writeln!(self.0))
    }
}

impl From<File> for PerfLogFile {
    fn from(value: File) -> Self {
        Self(value)
    }
}

/// Measure the overhead of [`crate::api::PerfRunMode::FixedBatch`] or
/// [`crate::api::PerfRunMode::DynamicBatch`]
///
/// Batched perf runs measure more than the benchmark body itself. Although not much, each batch
/// also pays for the surrounding control machinery: toggling measurement windows through the
/// control protocol, waiting for acknowledgements, and collecting the resulting benchmark output
/// into a vector.
///
/// That cost is especially visible for short-running benchmarks, where the control overhead can be
/// large compared to the code being measured. Gungraun therefore performs a matching overhead run
/// and subtracts it later in the [`crate::runner::perf::json_parser`] from the real measurement
/// instead of assuming that perf's coordination cost is negligible.
///
/// The overhead run must use the same repetition count and the same tool-runner setup as the real
/// run. Otherwise the measured adjustment would describe a different execution shape than the one
/// that produced the benchmark data.
pub fn measure_perf_overhead<'args, F>(
    meta: &Metadata,
    config: &ToolConfig,
    executable: &Path,
    executable_args_fn: &F,
    run_options: &RunOptions,
    output_path: &ToolOutputPath,
    sandbox_dir: Option<&Path>,
    tool_runner_dest: Option<&Path>,
) -> Result<()>
where
    F: Fn(&ToolConfig, Option<BenchRunMode>) -> Cow<'args, [OsString]>,
{
    let log_path = output_path.to_log_output().to_path();

    let file = File::open(&log_path)?;
    let data = parse_perf_log(&log_path, BufReader::new(file).lines())?;

    let output_path = output_path.with_added_modifiers([PERF_OVERHEAD_FILE_MODIFIER]);
    let RunOptions { current_dir, .. } = run_options;

    let mut tool_command = ToolCommand::new(config, meta, &output_path, run_options)?;
    match (sandbox_dir, current_dir.as_ref()) {
        (None, None) => {}
        (None, Some(current_dir)) => {
            tool_command.command.current_dir(current_dir);
        }
        (Some(sandbox_dir), None) => {
            tool_command.command.current_dir(sandbox_dir);
        }
        (Some(sandbox_dir), Some(current_dir)) => {
            // If run_dir is absolute uses run_dir otherwise joins the paths
            let path = sandbox_dir.join(current_dir);
            tool_command.command.current_dir(path);
        }
    }

    let executable = tool_command.resolve_executable(executable, sandbox_dir);

    let executable_args =
        executable_args_fn(config, Some(BenchRunMode::PerfOverhead(data.repetitions)));

    let (mut perf_data, args, _) = prepare_perf_command(
        &mut tool_command.command,
        config,
        &output_path,
        false,
        tool_runner_dest,
    )?;

    perf_data.log_file.write_header_command(
        &tool_command.command,
        &args,
        &executable,
        executable_args.as_ref(),
    )?;

    tool_command.append_tool_invocation(args, &executable, executable_args.as_ref());

    tool_command
        .command
        .stderr(perf_data.log_file.try_clone()?)
        .stdout(Stdio::null());

    let child = tool_command
        .command
        .spawn()
        .and_then(|child| {
            perf_data
                .log_file
                .finalize_header(child.id(), config.part)
                .map(|()| child)
        })
        .map_err(|error| {
            Error::LaunchError(PathBuf::from(config.tool().id()), error.to_string())
        })?;

    child
        .wait_with_output()
        .map_err(Into::into)
        .and_then(|output| {
            check_exit(config.tool(), &executable, output, &output_path, None).map(|_| ())
        })
}

/// Configure a [`Command`] for a controlled `perf` run.
///
/// Perf is controlled through fixed file descriptors rather than inherited stdin/stdout pipes. This
/// function creates the parent-side pipes used by the perf control protocol, creates the perf log
/// file, records all of their raw file descriptor numbers, and installs a `pre_exec` hook on the
/// command. The returned [`PerfData`] must stay alive until after the command is spawned so those
/// descriptors remain open in the parent.
///
/// The `pre_exec` hook runs in the child process after `fork` and immediately before `exec`. At
/// that point it duplicates the freshly-created pipe and log descriptors onto the fixed descriptor
/// numbers expected by Gungraun's perf protocol:
///
/// - [`PERF_CTL_FD_READ`] receives the control pipe read end.
/// - [`PERF_CTL_FD_WRITE`] receives the control pipe write end.
/// - [`PERF_ACK_FD_READ`] receives the acknowledgement pipe read end.
/// - [`PERF_ACK_FD_WRITE`] receives the acknowledgement pipe write end.
/// - [`PERF_LOG_FD`] receives the perf log file.
///
/// This setup has to happen in [`Command::pre_exec`] because the fixed descriptor numbers must
/// exist in the process that will become `perf`. Duplicating them in the parent would mutate the
/// runner process' descriptor table globally and could collide with other concurrent benchmark
/// runs. Doing it in the child keeps the remapping isolated to the command being spawned.
///
/// After duplicating each descriptor, the hook closes the original descriptor number when it
/// differs from the fixed target descriptor. This avoids leaking duplicate descriptors into the
/// child while keeping the fixed protocol descriptors open for `perf`.
///
/// The function also builds the managed perf arguments: it sets the [`ToolOutputPath`], adds the
/// configured event set, and enables sampling for `perf stat` when `use_sampling` is true. If
/// `tool_runner_dest` is set, managed output paths are written from the tool runner's filesystem
/// perspective.
///
/// # Errors
///
/// Returns an error if the log file cannot be created or if the control or acknowledgement pipes
/// cannot be created.
///
/// # Panics
///
/// Panics if no event sets are configured in the [`ToolConfig`] and [`ToolConfig::events`] returns
/// `None`.
///
/// [`PERF_ACK_FD_READ`]: crate::api::PERF_ACK_FD_READ
/// [`PERF_ACK_FD_WRITE`]: crate::api::PERF_ACK_FD_WRITE
/// [`PERF_CTL_FD_READ`]: crate::api::PERF_CTL_FD_READ
/// [`PERF_CTL_FD_WRITE`]: crate::api::PERF_CTL_FD_WRITE
/// [`PERF_LOG_FD`]: crate::api::PERF_LOG_FD
pub fn prepare_perf_command(
    command: &mut Command,
    config: &ToolConfig,
    output_path: &ToolOutputPath,
    use_sampling: bool,
    tool_runner_dest: Option<&Path>,
) -> Result<(PerfData, Vec<OsString>, ToolOutputPath)> {
    let log_output_path = if config.is_perf_record() {
        output_path.with_added_modifiers(["record"]).to_log_output()
    } else {
        output_path.to_log_output()
    };

    let log_file = File::create(log_output_path.to_path())?;
    let log_file_fd = log_file.as_raw_fd();

    let (ctl_fd_read, ctl_fd_write) = pipe()?;
    let (ack_fd_read, ack_fd_write) = pipe()?;

    let ctl_fd_read_raw = ctl_fd_read.as_raw_fd();
    let ctl_fd_write_raw = ctl_fd_write.as_raw_fd();
    let ack_fd_read_raw = ack_fd_read.as_raw_fd();
    let ack_fd_write_raw = ack_fd_write.as_raw_fd();

    let perf_data = PerfData {
        ack_fd_read,
        ack_fd_write,
        ctl_fd_read,
        ctl_fd_write,
        log_file: log_file.into(),
    };

    #[expect(
        clippy::multiple_unsafe_ops_per_block,
        reason = "pre_exec requires a single unsafe setup block"
    )]
    // SAFETY: `pre_exec` runs in the forked child immediately before `exec`, where only
    // async-signal-safe operations are allowed. This unsafe block exists solely to register a
    // closure that remaps already-open file descriptors onto the fixed perf protocol fd numbers and
    // closes the no-longer-needed raw descriptors. The captured raw fds come from live
    // `OwnedFd`/`File` values stored in `perf_data`, so they remain valid until after spawn
    // completes.
    unsafe {
        command.pre_exec(move || {
            dup_to_inheritable_fd(ctl_fd_read_raw, PERF_CTL_FD_READ)
                .and_then(|()| dup_to_inheritable_fd(ack_fd_write_raw, PERF_ACK_FD_WRITE))
                .and_then(|()| dup_to_inheritable_fd(ctl_fd_write_raw, PERF_CTL_FD_WRITE))
                .and_then(|()| dup_to_inheritable_fd(ack_fd_read_raw, PERF_ACK_FD_READ))
                .and_then(|()| dup_to_inheritable_fd(log_file_fd, PERF_LOG_FD))
                .and_then(|()| close_if_different(ctl_fd_read_raw, PERF_CTL_FD_READ))
                .and_then(|()| close_if_different(ctl_fd_write_raw, PERF_CTL_FD_WRITE))
                .and_then(|()| close_if_different(ack_fd_read_raw, PERF_ACK_FD_READ))
                .and_then(|()| close_if_different(ack_fd_write_raw, PERF_ACK_FD_WRITE))
                .and_then(|()| close_if_different(log_file_fd, PERF_LOG_FD))
        });
    }

    let mut tool_args = config.args.clone();
    tool_args.set_output_arg(output_path, tool_runner_dest);
    tool_args.add_events(config.events().expect("An event set should be present"));
    tool_args.use_sampling(use_sampling);

    Ok((perf_data, tool_args.to_vec(), log_output_path))
}
