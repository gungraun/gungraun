//! The module containing the [`Metadata`] and [`Cmd`]

// spell-checker: ignore beforemidafter startend

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use log::debug;

use super::args::CommandLineArgs;
use super::envs;
use crate::api::Tool;
use crate::runner::args::{self, RawArgs};
use crate::runner::tool::config::ToolConfig;
use crate::runner::tool::path::ToolOutputPath;
use crate::runner::tool::run::RunOptions;
use crate::util::{bool_to_yesno, resolve_binary_path};

#[derive(Debug, Clone, Copy)]
enum CoreTopologyTarget {
    LinuxAarch64,
    LinuxX8664,
    None,
}

/// Represents how perf is invoked for benchmark execution.
#[derive(Debug, Clone)]
pub enum PerfExecMode {
    /// Perf invocation with ASLR disabled via system utilities.
    DisabledASLR(Cmd),
    /// Direct perf invocation without ASLR control.
    Perf(Cmd),
    /// Custom runner executable for perf invocation.
    ///
    /// The first `PathBuf` is the resolved path to `--tool-runner`. The second `PathBuf` is the
    /// path to the perf entry binary (`perf` directly or `taskset` when CPU pinning is applied).
    /// The `Vec<OsString>` contains additional arguments that must be passed before benchmark-
    /// specific tool arguments.
    PerfRunner(PathBuf, PathBuf, Vec<OsString>),
}

/// A short-lived utility enum to help processing [`PerfExecMode`] and [`ValgrindExecMode`] in
/// [`Metadata::to_tool_command`]
enum ToolBaseCommand<'a> {
    Direct(&'a Cmd),
    Runner {
        runner_path: &'a PathBuf,
        tool_path: &'a PathBuf,
        tool_args: &'a [OsString],
    },
}

/// Represents how Valgrind is invoked for benchmark execution
///
/// This enum cannot use `std::process::Command` directly because it doesn't implement `Clone`,
/// which is needed for multiple benchmark runs. Instead, it stores the necessary components to
/// construct a `Command` when needed.
///
/// The run mode is determined by whether ASLR should be disabled and whether a custom runner is
/// specified:
///
/// - `DisabledASLR`: Valgrind is invoked through `setarch` (Linux) or `proccontrol` (FreeBSD) to
///   disable ASLR for more consistent benchmark results
/// - `Valgrind`: Valgrind is invoked directly without ASLR control
/// - `ValgrindRunner`: A custom runner executable is used to invoke Valgrind, useful for running
///   benchmarks in containers or specialized environments
#[derive(Debug, Clone)]
pub enum ValgrindExecMode {
    /// Valgrind invocation with ASLR disabled via system utilities
    ///
    /// On Linux, uses `setarch <arch> -R valgrind`. On FreeBSD, uses `proccontrol -m aslr -s
    /// disable valgrind`.
    DisabledASLR(Cmd),
    /// Direct Valgrind invocation without ASLR control
    Valgrind(Cmd),
    /// Custom runner executable for Valgrind invocation
    ///
    /// The first `PathBuf` is the path to the runner executable, resolved from
    /// `--tool-runner`. The second `PathBuf` is the path to Valgrind, either resolved from
    /// `--valgrind-bin` or the system default.
    ValgrindRunner(PathBuf, PathBuf),
}

/// A command to be executed, containing an executable and its arguments
///
/// This is a simplified version of `std::process::Command` that implements `Clone`, used by
/// [`ValgrindExecMode`] and [`PerfExecMode`] to prepare invocations before spawning the actual
/// process.
#[derive(Debug, Clone)]
pub struct Cmd {
    /// The arguments for the executable
    pub args: Vec<OsString>,
    /// The path to the executable
    pub bin: PathBuf,
}

/// `Metadata` contains all information that needs to be collected from cargo and the environment
///
/// More specifically, `Metadata` contains global constants, environment variables and command-line
/// arguments, the basic valgrind [`Cmd`], ...
#[derive(Debug, Clone)]
pub struct Metadata {
    /// A string describing the architecture of the CPU that is currently in use (e.g. "x86")
    pub arch: String,
    /// The command-line arguments parsed from the arguments to `cargo bench -- ARGS` as ARGS
    pub args: CommandLineArgs,
    /// The mode for running perf, determined by ASLR settings, runner configuration, and CPU
    /// topology.
    ///
    /// See [`PerfExecMode`] for details on whether perf will be invoked directly, through an
    /// ASLR-disabling wrapper, or through the custom tool runner. On supported hybrid CPU systems,
    /// the selected mode can already include a `taskset` wrapper for P-core pinning.
    pub perf_exec_mode: PerfExecMode,
    /// The path to the project top-level directory
    pub project_root: PathBuf,
    /// The absolute path of the `HOME` (per default `$WORKSPACE_ROOT/target/gungraun`). Plus, if
    /// configured, the target of the host like `x86_64-linux-unknown-gnu`. The final component is
    /// the `CARGO_PKG_NAME`.
    ///
    /// Examples:
    /// * `/home/my/workspace/my-project/target/gungraun/my-project` or
    /// * `/home/my/workspace/my-project/target/gungraun/x86_64-linux-unknown-gnu/my-project`
    pub target_dir: PathBuf,
    /// The mode for running Valgrind, determined by ASLR settings and runner configuration
    ///
    /// See [`ValgrindExecMode`] for details on how Valgrind will be invoked.
    pub valgrind_exec_mode: ValgrindExecMode,
}

impl Cmd {
    /// Create a new `Cmd`
    fn new<P>(bin: P) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            bin: bin.into(),
            args: Vec::default(),
        }
    }

    /// Create a new `Cmd`
    fn with_args<P, T>(bin: P, args: T) -> Self
    where
        P: Into<PathBuf>,
        T: IntoIterator,
        T::Item: Into<OsString>,
    {
        Self {
            bin: bin.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    /// Wrap this `Cmd` with another executable and leading arguments.
    ///
    /// The current command's binary is appended after `args`, followed by the current command's
    /// existing arguments. For example, wrapping `perf stat` with `taskset --cpu-list 0-3` produces
    /// `taskset --cpu-list 0-3 perf stat`.
    fn wrap_with(self, bin: PathBuf, args: Vec<OsString>) -> Self {
        let mut new_cmd = Self { args, bin };
        new_cmd.args.push(self.bin.into_os_string());
        new_cmd.args.extend(self.args);
        new_cmd
    }

    /// Wrap this `Cmd` with another command.
    ///
    /// This is a convenience wrapper for [`Cmd::wrap_by`] when the wrapper is already represented
    /// as a `Cmd`.
    fn wrap_with_other(self, other: Self) -> Self {
        self.wrap_with(other.bin, other.args)
    }

    /// Wrap this `Cmd` in `<path> --cpu-list <p_core_list>`.
    ///
    /// `path` should be the path to `taskset`. This is a convenience wrapper for [`Cmd::wrap_by`].
    fn wrap_with_taskset(self, path: PathBuf, p_core_list: OsString) -> Self {
        self.wrap_with(path, vec![OsString::from("--cpu-list"), p_core_list])
    }
}

impl CoreTopologyTarget {
    fn current() -> Self {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Self::LinuxX8664
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Self::LinuxAarch64
        } else {
            Self::None
        }
    }
}

impl Metadata {
    /// Create a `new` Metadata
    pub fn new(raw_command_line_args: &[String], target: &str) -> Result<Self> {
        let args = CommandLineArgs::parse_validated_from(raw_command_line_args);

        let arch = std::env::consts::ARCH.to_owned();
        debug!("Detected architecture: {arch}");

        // Execute `cargo` only if we really need to
        let (project_root, mut home) = match (
            args.workspace_root.as_ref(),
            args.home.clone().or_else(|| {
                std::env::var_os(envs::CARGO_TARGET_DIR).map(|p| PathBuf::from(p).join("gungraun"))
            }),
        ) {
            (None, None) => {
                let meta = cargo_metadata()?;
                (
                    meta.workspace_root.into_std_path_buf(),
                    meta.target_directory.into_std_path_buf().join("gungraun"),
                )
            }
            (None, Some(home)) => {
                let meta = cargo_metadata()?;
                (meta.workspace_root.into_std_path_buf(), home)
            }
            (Some(workspace_root), None) => {
                let meta = cargo_metadata()?;
                (
                    workspace_root.clone(),
                    meta.target_directory.into_std_path_buf().join("gungraun"),
                )
            }
            (Some(workspace_root), Some(home)) => (workspace_root.clone(), home),
        };

        debug!("Detected workspace root: '{}'", project_root.display());

        let target_dir = {
            if args.separate_targets {
                home = home.join(target);
            }
            home.join(
                std::env::var_os(envs::CARGO_PKG_NAME).map_or_else(PathBuf::new, PathBuf::from),
            )
        };

        debug!("Detected target directory: '{}'", target_dir.display());

        let aslr_wrapper = detect_aslr_wrapper(&args, &arch);
        let valgrind_exec_mode = detect_valgrind_exec_mode(&args, aslr_wrapper.as_ref())?;
        let perf_exec_mode = detect_perf_exec_mode_for(
            &args,
            aslr_wrapper.as_ref(),
            Path::new("/sys/devices"),
            CoreTopologyTarget::current(),
        )?;

        Ok(Self {
            arch,
            args,
            perf_exec_mode,
            project_root,
            target_dir,
            valgrind_exec_mode,
        })
    }

    /// Construct a `Command` for running the Valgrind or perf tool
    ///
    /// Creates the appropriate `Command` based on the selected execution mode, clearing and
    /// configuring environment variables, and arguments according to the tool configuration and
    /// run options.
    ///
    /// For custom runner invocation (`*Runner` variants):
    /// - Sets `GUNGRAUN_TR_DEST_DIR`, `GUNGRAUN_TR_HOME`, `GUNGRAUN_TR_WORKSPACE_ROOT`, and
    ///   `GUNGRAUN_ALLOW_ASLR` environment variables
    /// - Interpolates environment variables in `--tool-runner-args` arguments
    /// - Passes the tool path after runner arguments
    pub fn to_tool_command(
        &self,
        tool_config: &ToolConfig,
        output_path: &ToolOutputPath,
        run_options: &RunOptions,
    ) -> Result<Command> {
        let base_command = if tool_config.tool() == Tool::Perf {
            let exec_mode: &PerfExecMode = &self.perf_exec_mode;
            match exec_mode {
                PerfExecMode::DisabledASLR(cmd) | PerfExecMode::Perf(cmd) => {
                    ToolBaseCommand::Direct(cmd)
                }
                PerfExecMode::PerfRunner(runner_path, tool_path, tool_args) => {
                    ToolBaseCommand::Runner {
                        runner_path,
                        tool_path,
                        tool_args,
                    }
                }
            }
        } else {
            let exec_mode: &ValgrindExecMode = &self.valgrind_exec_mode;
            match exec_mode {
                ValgrindExecMode::DisabledASLR(cmd) | ValgrindExecMode::Valgrind(cmd) => {
                    ToolBaseCommand::Direct(cmd)
                }
                ValgrindExecMode::ValgrindRunner(runner_path, tool_path) => {
                    ToolBaseCommand::Runner {
                        runner_path,
                        tool_path,
                        tool_args: &[],
                    }
                }
            }
        };

        match base_command {
            ToolBaseCommand::Direct(cmd) => {
                let mut command = Command::new(&cmd.bin);

                if run_options.env_clear {
                    debug!("Clearing environment variables");
                    env_clear(tool_config.tool(), &mut command);
                }

                command.args(&cmd.args);
                command.envs(&run_options.envs);
                Ok(command)
            }
            ToolBaseCommand::Runner {
                runner_path,
                tool_path,
                tool_args,
            } => {
                let mut command = Command::new(runner_path);
                let additional_envs = {
                    let mut additional_envs = HashMap::new();
                    additional_envs.insert(
                        OsString::from("GUNGRAUN_TR_DEST_DIR"),
                        OsString::from(output_path.dest_dir()),
                    );
                    additional_envs.insert(
                        OsString::from("GUNGRAUN_TR_HOME"),
                        self.target_dir.clone().into_os_string(),
                    );
                    additional_envs.insert(
                        OsString::from("GUNGRAUN_TR_WORKSPACE_ROOT"),
                        self.project_root.clone().into_os_string(),
                    );
                    additional_envs.insert(
                        OsString::from("GUNGRAUN_ALLOW_ASLR"),
                        self.args
                            .allow_aslr
                            .map_or_else(
                                || bool_to_yesno(args::defaults::ALLOW_ASLR),
                                bool_to_yesno,
                            )
                            .into(),
                    );
                    additional_envs
                };

                for args in self
                    .args
                    .tool_runner_args
                    .iter()
                    .filter(|&r| !r.is_empty())
                    .map(RawArgs::as_slice)
                {
                    let interpolated =
                        interpolate_arguments(args, &run_options.envs, &additional_envs)?;
                    command.args(interpolated);
                }

                command.arg(tool_path);
                command.args(tool_args);

                if run_options.env_clear {
                    debug!("Clearing environment variables");
                    env_clear(tool_config.tool(), &mut command);
                }

                // `additional_envs` are added before the run options envs so the user can
                // overwrite them if required.
                command.envs(additional_envs);
                command.envs(&run_options.envs);

                Ok(command)
            }
        }
    }
}

fn cargo_metadata() -> Result<cargo_metadata::Metadata> {
    cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .context(
            "failed to query Cargo metadata; either provide `cargo` or set \
             GUNGRAUN_WORKSPACE_ROOT and GUNGRAUN_HOME (or alternatively CARGO_TARGET_DIR) \
             manually to run without `cargo`",
        )
}

// ARM64/Linux does not expose a definitive P-core/E-core split, so this uses the same heuristic as
// rustc-perf: read each CPU's capacity value, treat cores with at least 90% of the maximum capacity
// as P-cores, and ignore lower-capacity cores. This keeps near-equivalent high-capacity cores
// together even when their reported capacities differ slightly. Reference:
// https://github.com/rust-lang/rustc-perf/blob/master/collector/src/compile/execute/mod.rs
fn detect_arm_p_core_list(system_cpu: &Path) -> Option<OsString> {
    let mut capacities = fs::read_dir(system_cpu)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let cpu = entry
                .file_name()
                .to_str()?
                .strip_prefix("cpu")?
                .parse::<usize>()
                .ok()?;
            let capacity = fs::read_to_string(entry.path().join("cpu_capacity"))
                .ok()?
                .trim()
                .parse::<u64>()
                .ok()?;

            Some((cpu, capacity))
        })
        .collect::<Vec<_>>();

    // Not strictly necessary, but keeps the test expectations and logging output stable
    capacities.sort_unstable_by_key(|(cpu, _)| *cpu);

    let max_capacity = capacities.iter().map(|(_, capacity)| *capacity).max()?;
    let p_cores = capacities
        .iter()
        .filter_map(|(cpu, capacity)| {
            (capacity.saturating_mul(10) >= max_capacity * 9).then_some(*cpu)
        })
        .collect::<Vec<_>>();

    if p_cores.is_empty() || p_cores.len() == capacities.len() {
        return None;
    }

    Some(OsString::from(
        p_cores
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
    ))
}

/// Detect the platform command used to disable ASLR for benchmark tool invocations.
///
/// Returns `None` when ASLR is allowed by the command-line arguments, when the current platform has
/// no supported ASLR wrapper, or when the wrapper binary cannot be resolved. On Linux this resolves
/// `setarch <arch> -R`; on FreeBSD this resolves `proccontrol -m aslr -s disable`.
pub fn detect_aslr_wrapper(args: &CommandLineArgs, arch: &str) -> Option<Cmd> {
    if args.allow_aslr.unwrap_or(args::defaults::ALLOW_ASLR) {
        debug!("Running with ASLR enabled");
        None
    } else if cfg!(target_os = "linux") {
        debug!("Trying to run with ASLR disabled: Using 'setarch'");

        if let Ok(set_arch) = resolve_binary_path("setarch", None) {
            Some(Cmd::with_args(set_arch, [arch, "-R"]))
        } else {
            debug!("Failed to switch ASLR off: 'setarch' not found. Running with ASLR enabled");
            None
        }
    } else if cfg!(target_os = "freebsd") {
        debug!("Trying to run with ASLR disabled: Using 'proccontrol'");

        if let Ok(proc_control) = resolve_binary_path("proccontrol", None) {
            Some(Cmd::with_args(
                proc_control,
                ["-m", "aslr", "-s", "disable"],
            ))
        } else {
            debug!("Failed to switch ASLR off: 'proccontrol' not found. Running with ASLR enabled");
            None
        }
    } else {
        debug!("Failed to switch ASLR off. No utility available. Running with ASLR enabled");
        None
    }
}

/// Build a direct or ASLR-disabled execution mode for a resolved tool binary.
///
/// When an ASLR wrapper is available, the tool binary is appended to the wrapper arguments and the
/// `disabled_aslr` constructor is used. Otherwise the tool binary is used directly with no initial
/// arguments and the `direct` constructor is used.
fn detect_direct_exec_mode<T, FDisabled, FDirect>(
    aslr_wrapper: Option<&Cmd>,
    tool_path: PathBuf,
    disabled_aslr: FDisabled,
    direct: FDirect,
) -> T
where
    FDisabled: FnOnce(Cmd) -> T,
    FDirect: FnOnce(Cmd) -> T,
{
    let tool_cmd = Cmd::new(tool_path);
    match aslr_wrapper {
        Some(cmd) => disabled_aslr(tool_cmd.wrap_with_other(cmd.clone())),
        None => direct(tool_cmd),
    }
}

/// Detect the Intel Linux P-core CPU list from sysfs.
///
/// Hybrid Intel systems expose `/sys/devices/cpu_core/cpus`; non-hybrid systems expose only the
/// broader CPU topology and do not require perf pinning.
fn detect_intel_p_core_list(sys_devices: &Path) -> Option<OsString> {
    let cpu_core_path = sys_devices.join("cpu_core/cpus");
    if cpu_core_path.exists() {
        let cpu_list = fs::read_to_string(cpu_core_path).ok()?;
        let cpu_list = cpu_list.trim();
        return (!cpu_list.is_empty()).then(|| OsString::from(cpu_list));
    }

    if sys_devices.join("cpu/cpus").exists() {
        debug!("Detected non-hybrid CPU topology");
    }

    None
}

/// Detect the P-core CPU list for the current target topology.
///
/// Returns `None` for unsupported targets, non-hybrid systems, or systems where the relevant sysfs
/// files cannot be read.
fn detect_p_core_list_for(sys_devices: &Path, target: CoreTopologyTarget) -> Option<OsString> {
    match target {
        CoreTopologyTarget::LinuxAarch64 => detect_arm_p_core_list(&sys_devices.join("system/cpu")),
        CoreTopologyTarget::LinuxX8664 => detect_intel_p_core_list(sys_devices),
        CoreTopologyTarget::None => None,
    }
}

/// Detect how perf should be invoked for the given topology target.
///
/// This resolves the perf binary, applies `--tool-runner` when configured, otherwise chooses a
/// direct or ASLR-disabled command, and then wraps that mode with `taskset` when P-core topology is
/// detected and `taskset` is available.
///
/// `sys_devices` is the root of the sysfs devices tree, usually `/sys/devices`, and is injected by
/// tests to exercise topology detection without reading the host system.
fn detect_perf_exec_mode_for(
    args: &CommandLineArgs,
    aslr_wrapper: Option<&Cmd>,
    sys_devices: &Path,
    target: CoreTopologyTarget,
) -> Result<PerfExecMode> {
    let perf_path = resolve_tool_bin("perf", args.perf_bin.as_ref());

    debug!("Detected perf path: '{}'", perf_path.display());

    let perf_exec_mode = if let Some(runner) = args.tool_runner.as_ref() {
        let resolved = resolve_binary_path(runner, None)?;
        debug!("Using tool runner for perf: '{}'", resolved.display());

        PerfExecMode::PerfRunner(resolved, perf_path, Vec::default())
    } else {
        detect_direct_exec_mode(
            aslr_wrapper,
            perf_path,
            PerfExecMode::DisabledASLR,
            PerfExecMode::Perf,
        )
    };

    let Some(p_core_list) = detect_p_core_list_for(sys_devices, target) else {
        return Ok(perf_exec_mode);
    };

    let Ok(taskset_path) = resolve_binary_path("taskset", None) else {
        debug!("Failed to detect taskset. Running perf without CPU affinity");
        return Ok(perf_exec_mode);
    };

    debug!(
        "Detected P-core CPU list '{}'. Running perf with taskset",
        p_core_list.to_string_lossy()
    );

    let wrapped = match perf_exec_mode {
        PerfExecMode::DisabledASLR(cmd) => {
            PerfExecMode::DisabledASLR(cmd.wrap_with_taskset(taskset_path, p_core_list))
        }
        PerfExecMode::Perf(cmd) => {
            PerfExecMode::Perf(cmd.wrap_with_taskset(taskset_path, p_core_list))
        }
        PerfExecMode::PerfRunner(runner_path, tool_path, tool_args) => {
            let wrapped =
                Cmd::with_args(tool_path, tool_args).wrap_with_taskset(taskset_path, p_core_list);
            PerfExecMode::PerfRunner(runner_path, wrapped.bin, wrapped.args)
        }
    };

    Ok(wrapped)
}

/// Detect how Valgrind should be invoked for benchmark execution.
///
/// This resolves `--valgrind-bin` or the default `valgrind` binary, applies `--tool-runner` when
/// configured, and otherwise selects a direct or ASLR-disabled Valgrind command.
pub fn detect_valgrind_exec_mode(
    args: &CommandLineArgs,
    aslr_wrapper: Option<&Cmd>,
) -> Result<ValgrindExecMode> {
    let valgrind_path = resolve_tool_bin("valgrind", args.valgrind_bin.as_ref());

    debug!("Detected valgrind path: '{}'", valgrind_path.display());

    let valgrind_exec_mode = if let Some(runner) = args.tool_runner.as_ref() {
        let resolved = resolve_binary_path(runner, None)?;
        debug!("Using tool runner for valgrind: '{}'", resolved.display());

        ValgrindExecMode::ValgrindRunner(resolved, valgrind_path)
    } else {
        detect_direct_exec_mode(
            aslr_wrapper,
            valgrind_path,
            ValgrindExecMode::DisabledASLR,
            ValgrindExecMode::Valgrind,
        )
    };

    Ok(valgrind_exec_mode)
}

// TODO: does perf needs specific env vars?
/// Clear the environment variables
///
/// The `LD_PRELOAD` and `LD_LIBRARY_PATH` variables are skipped. If they are set there's
/// usually a good reason for it.
///
/// If the tool is `Memcheck`: In order to be able run `Memcheck` without errors, the `PATH`,
/// `HOME` and `DEBUGINFOD_URLS` variables are skipped.
pub fn env_clear(tool: Tool, command: &mut Command) {
    debug!("{}: Clearing environment variables", tool.id());
    for (key, _) in std::env::vars() {
        match (key.as_str(), tool) {
            (key @ ("DEBUGINFOD_URLS" | "PATH" | "HOME"), Tool::Memcheck)
            | (key @ ("LD_PRELOAD" | "LD_LIBRARY_PATH"), _) => {
                debug!(
                    "{}: Clearing environment variables: Skipping {key}",
                    tool.id()
                );
            }
            _ => {
                command.env_remove(key);
            }
        }
    }
}

fn interpolate_argument(
    arg: &str,
    envs: &HashMap<OsString, OsString>,
    additional_envs: &HashMap<OsString, OsString>,
) -> Result<OsString> {
    let mut result = Vec::with_capacity(arg.len());
    let chars = arg.as_bytes();
    let mut index = 0;

    while index < chars.len() {
        let char = chars[index];

        let next_index = index + 1;
        if next_index < chars.len() {
            let next = chars[next_index];
            match (char, next) {
                (b'$', b'{') if next_index + 1 < chars.len() => {
                    let dollar_pos = index;
                    let mut is_valid = false;
                    let start = next_index + 1;
                    let mut end = 0;
                    index = next_index + 1;
                    for c in &chars[start..] {
                        end = index;
                        index += 1;

                        if *c == b'}' {
                            is_valid = end > start;
                            break;
                        }
                    }

                    if is_valid {
                        // SAFETY: The input arg is a `&str` and valid UTF-8 so everything within
                        // `${...}` must be valid UTF-8, too.
                        let var = unsafe {
                            OsStr::from_encoded_bytes_unchecked(&arg.as_bytes()[start..end])
                        };
                        let value = additional_envs
                            .get(var)
                            .cloned()
                            .or_else(|| envs.get(var).cloned())
                            .or_else(|| std::env::var_os(var))
                            .ok_or_else(|| {
                                anyhow!(
                                    "Failed to interpolate the variable '{}' at column \
                                     '{dollar_pos}': Variable not found in the environment",
                                    var.to_string_lossy()
                                )
                            })?;

                        result.append(&mut value.into_encoded_bytes());
                    } else {
                        return Err(anyhow!(
                            "Failed to interpolate the variable at column '{dollar_pos}': Invalid \
                             syntax"
                        ));
                    }
                }
                (b'$', b'{') => {
                    return Err(anyhow!(
                        "Failed to interpolate the variable at column '{index}': Premature end of \
                         variable declaration"
                    ));
                }
                (char, b'$') if next_index + 1 < chars.len() => {
                    result.push(char);

                    index += 1;
                }
                (a, b) => {
                    result.push(a);
                    result.push(b);

                    index += 2;
                }
            }
        } else {
            result.push(char);
            index += 1;
        }
    }

    // SAFETY: The result bytes vector is a mixture of valid `UTF-8` from &arg which is a `&str` and
    // the values of environment variables which are valid `OsStrings` producing together a valid
    // encoding.
    Ok(unsafe { OsString::from_encoded_bytes_unchecked(result) })
}

fn interpolate_arguments(
    args: &[String],
    envs: &HashMap<OsString, OsString>,
    additional_envs: &HashMap<OsString, OsString>,
) -> Result<Vec<OsString>> {
    args.iter()
        .map(|arg| interpolate_argument(arg, envs, additional_envs))
        .collect::<Result<_>>()
}

fn resolve_tool_bin(default_bin: &str, configured_bin: Option<&PathBuf>) -> PathBuf {
    configured_bin
        .cloned()
        .or_else(|| resolve_binary_path(default_bin, None).ok())
        .unwrap_or_else(|| PathBuf::from(default_bin))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::fs;

    use rstest::rstest;

    use super::*;

    fn make_envs(pairs: &[(&str, &str)]) -> HashMap<OsString, OsString> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    #[test]
    fn test_detect_p_core_list_in_when_arm_hybrid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let system_cpu = temp_dir.path().join("system/cpu");

        for (cpu, capacity) in [(0, 1024), (1, 1024), (2, 512), (3, 512)] {
            let cpu_dir = system_cpu.join(format!("cpu{cpu}"));
            fs::create_dir_all(&cpu_dir).unwrap();
            fs::write(cpu_dir.join("cpu_capacity"), capacity.to_string()).unwrap();
        }

        assert_eq!(
            detect_p_core_list_for(temp_dir.path(), CoreTopologyTarget::LinuxAarch64),
            Some(OsString::from("0,1"))
        );
    }

    #[test]
    fn test_detect_p_core_list_in_when_arm_non_hybrid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let system_cpu = temp_dir.path().join("system/cpu");

        for cpu in 0..4 {
            let cpu_dir = system_cpu.join(format!("cpu{cpu}"));
            fs::create_dir_all(&cpu_dir).unwrap();
            fs::write(cpu_dir.join("cpu_capacity"), "1024").unwrap();
        }

        assert_eq!(
            detect_p_core_list_for(temp_dir.path(), CoreTopologyTarget::LinuxAarch64),
            None
        );
    }

    #[test]
    fn test_detect_p_core_list_in_when_intel_hybrid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sys_devices = temp_dir.path();

        fs::create_dir_all(sys_devices.join("cpu_core")).unwrap();
        fs::write(sys_devices.join("cpu_core/cpus"), "0-7\n").unwrap();
        fs::create_dir_all(sys_devices.join("cpu_atom")).unwrap();
        fs::write(sys_devices.join("cpu_atom/cpus"), "8-15\n").unwrap();

        assert_eq!(
            detect_p_core_list_for(sys_devices, CoreTopologyTarget::LinuxX8664),
            Some(OsString::from("0-7"))
        );
    }

    #[test]
    fn test_detect_p_core_list_in_when_intel_non_hybrid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sys_devices = temp_dir.path();

        fs::create_dir_all(sys_devices.join("cpu")).unwrap();
        fs::write(sys_devices.join("cpu/cpus"), "0-15\n").unwrap();

        assert_eq!(
            detect_p_core_list_for(sys_devices, CoreTopologyTarget::LinuxX8664),
            None
        );
    }

    #[test]
    fn test_detect_p_core_list_in_when_target_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sys_devices = temp_dir.path();

        fs::create_dir_all(sys_devices.join("cpu_core")).unwrap();
        fs::write(sys_devices.join("cpu_core/cpus"), "0-7\n").unwrap();

        assert_eq!(
            detect_p_core_list_for(sys_devices, CoreTopologyTarget::None),
            None
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_detect_perf_exec_mode_in_with_taskset_and_aslr() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sys_devices = temp_dir.path().join("sys/devices");

        fs::create_dir_all(sys_devices.join("cpu_core")).unwrap();
        fs::write(sys_devices.join("cpu_core/cpus"), "0-7\n").unwrap();

        let aslr_wrapper = Cmd::with_args("setarch", ["x86_64", "-R"]);

        let cmd = detect_perf_exec_mode_for(
            &CommandLineArgs::parse_validated_from(["gungraun-runner"]),
            Some(&aslr_wrapper),
            &sys_devices,
            CoreTopologyTarget::LinuxX8664,
        )
        .unwrap();
        let perf_path = resolve_binary_path("perf", None).unwrap_or_else(|_| PathBuf::from("perf"));

        match cmd {
            PerfExecMode::DisabledASLR(cmd) => {
                if let Ok(taskset_path) = resolve_binary_path("taskset", None) {
                    assert_eq!(cmd.bin, taskset_path);
                    assert_eq!(
                        cmd.args,
                        vec![
                            OsString::from("--cpu-list"),
                            OsString::from("0-7"),
                            OsString::from("setarch"),
                            OsString::from("x86_64"),
                            OsString::from("-R"),
                            perf_path.into_os_string(),
                        ]
                    );
                } else {
                    assert_eq!(cmd.bin, PathBuf::from("setarch"));
                    assert_eq!(
                        cmd.args,
                        vec![
                            OsString::from("x86_64"),
                            OsString::from("-R"),
                            perf_path.into_os_string(),
                        ]
                    );
                }
            }
            other => panic!("expected DisabledASLR perf exec mode, got {other:?}"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_detect_perf_exec_mode_uses_configured_perf_bin() {
        let temp_dir = tempfile::tempdir().unwrap();
        let perf_bin = temp_dir.path().join("perf");
        let sys_devices = temp_dir.path().join("sys/devices");

        let cmd = detect_perf_exec_mode_for(
            &CommandLineArgs::parse_validated_from([
                "gungraun-runner",
                &format!("--perf-bin={}", perf_bin.display()),
            ]),
            None,
            &sys_devices,
            CoreTopologyTarget::None,
        )
        .unwrap();

        assert!(matches!(cmd, PerfExecMode::Perf(cmd) if cmd.bin == perf_bin));
    }

    #[rstest]
    #[case::single_var("${VAR}", make_envs(&[("VAR", "value")]), make_envs(&[]), "value")]
    #[case::multiple_vars("${A}${B}", make_envs(&[("A", "1"), ("B", "2")]), make_envs(&[]), "12")]
    #[case::var_with_text(
            "prefix_${VAR}_suffix",
            make_envs(&[("VAR", "value")]),
            make_envs(&[]),
            "prefix_value_suffix"
        )]
    #[case::var_middle("before${MID}after", make_envs(&[("MID", "mid")]), make_envs(&[]), "beforemidafter")]
    #[case::var_at_start("${START}end", make_envs(&[("START", "start")]), make_envs(&[]), "startend")]
    #[case::var_at_end("start${END}", make_envs(&[("END", "end")]), make_envs(&[]), "startend")]
    #[case::empty_string("", make_envs(&[]), make_envs(&[]), "")]
    #[case::no_vars("plain text", make_envs(&[]), make_envs(&[]), "plain text")]
    #[case::additional_envs_priority(
            "${VAR}",
            make_envs(&[("VAR", "from_envs")]),
            make_envs(&[("VAR", "from_additional")]),
            "from_additional"
        )]
    #[case::envs_over_real_env("${VAR}", make_envs(&[("VAR", "from_envs")]), make_envs(&[]), "from_envs")]
    #[serial_test::serial]
    fn test_interpolate_argument_basic(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[rstest]
    #[case::dollar_only("$", make_envs(&[]), make_envs(&[]), "$")]
    #[case::double_dollar("$$", make_envs(&[]), make_envs(&[]), "$$")]
    #[case::dollar_before_text("$abc", make_envs(&[]), make_envs(&[]), "$abc")]
    #[case::dollar_before_brace("$} text", make_envs(&[]), make_envs(&[]), "$} text")]
    #[serial_test::serial]
    fn test_interpolate_argument_literal_dollar(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[rstest]
    #[case::same_var_thrice("${A}${A}${A}", make_envs(&[("A", "x")]), make_envs(&[]), "xxx")]
    #[case::same_var_with_text("${A}_${A}", make_envs(&[("A", "val")]), make_envs(&[]), "val_val")]
    #[serial_test::serial]
    fn test_interpolate_argument_same_var(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[rstest]
    #[case::utf8_in_value("${VAR}", make_envs(&[("VAR", "日本語")]), make_envs(&[]), "日本語")]
    #[case::utf8_in_text("日本${VAR}語", make_envs(&[("VAR", "-")]), make_envs(&[]), "日本-語")]
    #[case::space_in_value("${VAR}", make_envs(&[("VAR", "hello world")]), make_envs(&[]), "hello world")]
    #[case::special_chars("${VAR}", make_envs(&[("VAR", "--flag=value")]), make_envs(&[]), "--flag=value")]
    #[case::path_separators(
            "${VAR}",
            make_envs(&[("VAR", "/usr/local/bin")]),
            make_envs(&[]),
            "/usr/local/bin"
        )]
    #[serial_test::serial]
    fn test_interpolate_argument_special_chars(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_interpolate_argument_std_env_is_used() {
        const VAR_NAME: &str = "GUNGRAUN_TEST_INTERPOLATE_VAR";
        // SAFETY: This test is run serially
        unsafe {
            std::env::set_var(VAR_NAME, "from_real_env");
        }
        let envs = HashMap::new();
        let additional_envs = HashMap::new();
        let result = interpolate_argument(&format!("${{{VAR_NAME}}}"), &envs, &additional_envs);
        // SAFETY: This test is run serially
        unsafe {
            std::env::remove_var(VAR_NAME);
        }
        assert_eq!(result.unwrap(), OsString::from("from_real_env"));
    }

    #[rstest]
    #[case::empty_var_name(
        "${}",
        "Failed to interpolate the variable at column '0': Invalid syntax"
    )]
    #[case::unclosed_var(
        "${VAR",
        "Failed to interpolate the variable at column '0': Invalid syntax"
    )]
    #[case::var_not_found(
        "${NOTFOUND}",
        "Failed to interpolate the variable 'NOTFOUND' at column '0': Variable not found in the \
         environment"
    )]
    #[serial_test::serial]
    fn test_interpolate_argument_when_error(#[case] arg: &str, #[case] expected_error: &str) {
        let envs = HashMap::new();
        let additional_envs = HashMap::new();
        let err = interpolate_argument(arg, &envs, &additional_envs).unwrap_err();
        assert_eq!(err.to_string(), expected_error);
    }

    #[rstest]
    #[case::empty_slice(&[], make_envs(&[]), vec![])]
    #[case::single_arg(&["${VAR}"], make_envs(&[("VAR", "val")]), vec!["val"])]
    #[case::multiple_args(&["${A}", "${B}"], make_envs(&[("A", "1"), ("B", "2")]), vec!["1", "2"])]
    #[case::mixed(&["plain", "${VAR}", "text"], make_envs(&[("VAR", "x")]), vec!["plain", "x", "text"])]
    #[serial_test::serial]
    fn test_interpolate_arguments(
        #[case] args: &[&str],
        #[case] envs: HashMap<OsString, OsString>,
        #[case] expected: Vec<&str>,
    ) {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        let result = interpolate_arguments(&args, &envs, &HashMap::new()).unwrap();
        assert_eq!(
            result,
            expected.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
    }
}
