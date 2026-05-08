use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand, Stdio as StdStdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use bon::builder;
use gungraun::{EntryPoint, ExitWith, ValgrindTool};
use gungraun_runner::runner::common::{Assistant, AssistantKind, Config, ModulePath};
use gungraun_runner::runner::meta::Metadata;
use gungraun_runner::runner::summary::BaselineKind;
use gungraun_runner::runner::tasks::ProcessHandler;
use gungraun_runner::runner::tool::args::ToolArgs;
use gungraun_runner::runner::tool::config::{ToolConfig, ToolFlamegraphConfig};
use gungraun_runner::runner::tool::path::{ToolOutputPath, ToolOutputPathKind};
use gungraun_runner::runner::tool::regression::ToolRegressionConfig;
use gungraun_runner::runner::tool::run::{RunOptions, ToolCommand, ToolCommandChild};

use crate::util::common::DEFAULT_TOOL;

#[builder(finish_fn = "fixture")]
pub fn assistant_f(kind: AssistantKind) -> Assistant {
    Assistant::new_main_assistant(kind, vec![], false)
}

#[builder(finish_fn = "fixture")]
pub fn config_f(
    bench_bin: &Path,
    bench_file: Option<&Path>,
    metadata: Option<&Metadata>,
) -> Config {
    Config {
        bench_bin: bench_bin.to_path_buf(),
        bench_file: bench_file
            .map_or_else(|| PathBuf::from("does_not_exist.rs"), |f| f.to_path_buf()),
        meta: metadata.map_or_else(|| metadata_f().fixture(), Clone::clone),
        module_path: ModulePath::new("does_not_exist"),
        package_dir: PathBuf::from("test_package"),
    }
}

#[builder(finish_fn = "fixture")]
pub fn bench_child_f(
    exe: &Path,
    args: Option<&[&str]>,
    stdout: Option<StdStdio>,
) -> (PathBuf, Child) {
    let child = command_child_f()
        .exe(exe)
        .maybe_args(args)
        .maybe_stdout(stdout)
        .fixture();

    (exe.to_path_buf(), child)
}

#[builder(finish_fn = "fixture")]
pub fn command_child_f(exe: &Path, args: Option<&[&str]>, stdout: Option<StdStdio>) -> Child {
    let mut command = StdCommand::new(exe);
    if let Some(args) = args {
        command.args(args);
    }
    if let Some(stdout) = stdout {
        command.stdout(stdout);
    }

    command
        .spawn()
        .expect("Spawning the process should succeed.")
}

#[builder(finish_fn = "fixture")]
pub fn force_shutdown_f(yes: Option<bool>) -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(yes.unwrap_or(false)))
}

#[builder(finish_fn = "fixture")]
pub fn metadata_f(raw_command_line_args: Option<&[&str]>) -> Metadata {
    let args = raw_command_line_args
        .into_iter()
        .flatten()
        .map(|s| String::from(*s))
        .collect::<Vec<String>>();

    Metadata::new(
        &args,
        "benchmark-tests",
        &PathBuf::from("test_bench_template.rs"),
    )
    .expect("metadata should be valid")
}

#[builder(finish_fn = "fixture")]
pub fn module_path_f() -> ModulePath {
    ModulePath::new("test::path")
}

#[builder(finish_fn = "fixture")]
pub fn process_handler_f(
    set_force_shutdown: Option<Arc<AtomicBool>>,
    assistant: Option<(AssistantKind, Child)>,
    setup_is_parallel: Option<bool>,
    bench: Option<ToolCommandChild>,
) -> ProcessHandler {
    let mut handler = ProcessHandler::new(
        set_force_shutdown.unwrap_or_else(|| force_shutdown_f().fixture()),
        module_path_f().fixture(),
        false,
        Duration::from_millis(50),
        None,
    );

    if let Some(parallel) = setup_is_parallel {
        handler.setup_is_parallel = parallel;
    }

    if let Some((kind, child)) = assistant {
        match kind {
            AssistantKind::Setup => {
                handler.setup = Some((kind.id(), child));
            }
            AssistantKind::Teardown => {
                handler.teardown = Some((kind.id(), child));
            }
        }
    }

    if let Some(child) = bench {
        handler.bench = Some(child);
    }

    handler
}

#[builder(finish_fn = "fixture")]
pub fn setup_child_f(
    exe: &Path,
    args: Option<&[&str]>,
    stdout: Option<StdStdio>,
) -> (AssistantKind, Child) {
    let child = command_child_f()
        .exe(exe)
        .maybe_args(args)
        .maybe_stdout(stdout)
        .fixture();
    (AssistantKind::Setup, child)
}

#[builder(finish_fn = "fixture")]
pub fn teardown_child_f(
    exe: &Path,
    args: Option<&[&str]>,
    stdout: Option<StdStdio>,
) -> (AssistantKind, Child) {
    let child = command_child_f()
        .exe(exe)
        .maybe_args(args)
        .maybe_stdout(stdout)
        .fixture();
    (AssistantKind::Teardown, child)
}

#[builder(finish_fn = "fixture")]
pub fn test_file_f(dir: Option<&Path>) -> (PathBuf, File) {
    let path = if let Some(dir) = dir {
        dir.join("test-file")
    } else {
        PathBuf::from("test-file")
    };
    let file = File::create(&path).expect("Creating the test file should succeed");

    (path, file)
}

#[builder(finish_fn = "fixture")]
pub fn run_options_f(env_clear: Option<bool>) -> RunOptions {
    // Sometimes necessary to be able to run the tests with valgrind
    let valgrind_lib = OsString::from("VALGRIND_LIB");

    let mut envs = HashMap::new();
    if let Some(value) = std::env::var_os(&valgrind_lib) {
        envs.insert(valgrind_lib, value);
    }

    RunOptions {
        current_dir: None,
        delay: None,
        env_clear: env_clear.unwrap_or(true),
        envs,
        exit_with: None,
        sandbox: None,
        setup: None,
        stderr: None,
        stdin: None,
        stdout: None,
        teardown: None,
    }
}

#[builder(finish_fn = "fixture")]
pub fn tool_command_f(
    output_path: &ToolOutputPath,
    metadata: Option<Metadata>,
    run_options: Option<&RunOptions>,
) -> ToolCommand {
    let meta = metadata.unwrap_or_else(|| metadata_f().fixture());

    let run_options =
        run_options.map_or_else(|| Cow::Owned(run_options_f().fixture()), Cow::Borrowed);

    ToolCommand::new(&tool_config_f().fixture(), &meta, output_path, &run_options).unwrap()
}

#[builder(finish_fn = "fixture")]
pub fn tool_command_child_f(
    exe: &Path,
    args: Option<&[&str]>,
    log_path: ToolOutputPath,
    tool: Option<ValgrindTool>,
    exit_with: Option<ExitWith>,
    stdout: Option<StdStdio>,
) -> ToolCommandChild {
    let (path, child) = bench_child_f()
        .exe(exe)
        .maybe_args(args)
        .maybe_stdout(stdout)
        .fixture();

    ToolCommandChild::new(
        tool.unwrap_or(DEFAULT_TOOL),
        child,
        path,
        exit_with,
        log_path,
    )
}

#[builder(finish_fn = "fixture")]
pub fn tool_config_f(tool: Option<ValgrindTool>, is_default: Option<bool>) -> ToolConfig {
    let tool = tool.unwrap_or(DEFAULT_TOOL);
    ToolConfig::new(
        tool,
        true,
        ToolArgs::try_from_raw_tool_args(tool, &[]).unwrap(),
        ToolRegressionConfig::None,
        ToolFlamegraphConfig::None,
        EntryPoint::None,
        is_default.unwrap_or(true),
        vec![],
    )
}

#[builder(finish_fn = "fixture")]
pub fn tool_output_path_f(
    target_dir: &Path,
    tool: Option<ValgrindTool>,
    name: Option<&str>,
    module_path_string: Option<&str>,
    init: Option<bool>,
    #[builder(default = vec![], with = FromIterator::from_iter)] files: Vec<(&str, &str)>,
) -> ToolOutputPath {
    let path = ToolOutputPath::new(
        ToolOutputPathKind::Out,
        tool.unwrap_or(ValgrindTool::Callgrind),
        &BaselineKind::Old,
        target_dir,
        &module_path_string.map_or_else(|| module_path_f().fixture(), ModulePath::new),
        name.unwrap_or("foo"),
        false,
    )
    .unwrap();

    if init.unwrap_or(false) {
        path.init()
            .expect("Initializing the output path should succeed");
    }

    if !files.is_empty() {
        let dir = path.dest_dir();
        for (path, content) in files {
            std::fs::write(dir.join(path), content).unwrap()
        }
    }

    path
}
