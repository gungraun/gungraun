//! This module contains test fixtures for gungraun-runner types
//!
//! The fixtures are usable via the `__fixtures` feature gate in other packages than gungraun-runner
//! and in the gungraun-runner crate itself within cfg(test) modules

#![allow(missing_docs)]

pub mod api;
pub mod perf;

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

use crate::api::{
    CachegrindMetric, DhatMetric, DhatSpec, EntryPoint, ExitWith, PerfMetric, PerfRunMode,
    PerfSpec, RawToolArgs, SanitizeOutput, Tool, ToolSpec, ToolSpecOptions, ToolSpecs,
};
use crate::metrics::model::{AnnotatedMetric, Metric, PerfQualities};
use crate::runner::cachegrind::args::CachegrindArgs;
use crate::runner::cachegrind::regression::CachegrindRegressionConfig;
use crate::runner::callgrind::args::CallgrindArgs;
use crate::runner::common::{Assistant, AssistantKind, Config, ModulePath};
use crate::runner::dhat::regression::DhatRegressionConfig;
use crate::runner::format::OutputFormat;
use crate::runner::meta::Metadata;
use crate::runner::perf::args::{DEFAULT_PERF_EVENTS, PerfStatArgs};
use crate::runner::perf::regression::PerfRegressionConfig;
use crate::runner::tasks::ProcessHandler;
use crate::runner::tool::args::{ToolArgs, ToolArgsLike, ValgrindArgs};
use crate::runner::tool::config::{
    DEFAULT_PERF_ALPHA, DEFAULT_PERF_NON_ZERO_METRICS, DhatConfig, PerfConfig, ToolConfig,
    ToolConfigBuilder, ToolConfigOptions, ToolConfigs, ToolFlamegraphConfig,
};
use crate::runner::tool::path::{ToolOutputPath, ToolOutputPathKind};
use crate::runner::tool::regression::ToolRegressionConfig;
use crate::runner::tool::run::{RunOptions, ToolCommand, ToolCommandChild};
use crate::summary::model::BaselineKind;
use crate::units::Unit;

pub const DEFAULT_TOOL: Tool = Tool::Callgrind;

#[builder(finish_fn = "fixture", on(Metric, into))]
pub fn annotated_metric_perf_f(
    metric: Metric,
    event_runtime: Option<u64>,
    pcnt_running: Option<f64>,
    rse: Option<f64>,
    n: Option<u64>,
    mean: Option<f64>,
    unit: Option<Unit>,
) -> AnnotatedMetric<PerfQualities> {
    AnnotatedMetric::new(
        metric,
        PerfQualities::new(event_runtime, pcnt_running, rse, n, mean),
        unit,
    )
}

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
            .map_or_else(|| PathBuf::from("does_not_exist.rs"), Path::to_path_buf),
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
pub fn cachegrind_regression_config_f(
    soft_limits: Option<Vec<(CachegrindMetric, f64)>>,
    hard_limits: Option<Vec<(CachegrindMetric, Metric)>>,
    fail_fast: Option<bool>,
) -> CachegrindRegressionConfig {
    CachegrindRegressionConfig {
        soft_limits: soft_limits.unwrap_or_default(),
        hard_limits: hard_limits.unwrap_or_default(),
        fail_fast: fail_fast.unwrap_or(false),
    }
}

#[builder(finish_fn = "fixture")]
pub fn dhat_regression_config_f(
    soft_limits: Option<Vec<(DhatMetric, f64)>>,
    hard_limits: Option<Vec<(DhatMetric, Metric)>>,
    fail_fast: Option<bool>,
) -> DhatRegressionConfig {
    DhatRegressionConfig {
        soft_limits: soft_limits.unwrap_or_default(),
        hard_limits: hard_limits.unwrap_or_default(),
        fail_fast: fail_fast.unwrap_or(false),
    }
}

#[builder(finish_fn = "fixture")]
pub fn force_shutdown_f(yes: Option<bool>) -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(yes.unwrap_or(false)))
}

#[builder(finish_fn = "fixture")]
pub fn metadata_f(raw_command_line_args: Option<&[&str]>, target: Option<&str>) -> Metadata {
    let args = raw_command_line_args
        .into_iter()
        .flatten()
        .map(|s| String::from(*s))
        .collect::<Vec<String>>();
    let target = target.unwrap_or("x86_64-unknown-linux-gnu");

    Metadata::new(&args, target).expect("metadata should be valid")
}

#[builder(finish_fn = "fixture")]
pub fn module_path_f() -> ModulePath {
    ModulePath::new("test::path")
}

#[builder(finish_fn = "fixture")]
pub fn perf_regression_config_f(
    soft_limits: Option<Vec<(PerfMetric, f64)>>,
    hard_limits: Option<Vec<(PerfMetric, Option<Unit>, Metric)>>,
    fail_fast: Option<bool>,
    alpha: Option<f64>,
) -> PerfRegressionConfig {
    PerfRegressionConfig {
        alpha: alpha.unwrap_or(DEFAULT_PERF_ALPHA),
        soft_limits: soft_limits.unwrap_or_default(),
        hard_limits: hard_limits.unwrap_or_default(),
        fail_fast: fail_fast.unwrap_or(false),
    }
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
    tool_config: Option<&ToolConfig>,
) -> ToolCommand {
    let meta = metadata.unwrap_or_else(|| metadata_f().fixture());

    let run_options =
        run_options.map_or_else(|| Cow::Owned(run_options_f().fixture()), Cow::Borrowed);

    let tool_config = if let Some(tool_config) = tool_config {
        tool_config.clone()
    } else {
        tool_config_f().fixture()
    };

    ToolCommand::new(&tool_config, &meta, output_path, &run_options).unwrap()
}

#[builder(finish_fn = "fixture")]
pub fn tool_command_child_f(
    exe: &Path,
    args: Option<&[&str]>,
    log_path: ToolOutputPath,
    tool: Option<Tool>,
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
        None,
    )
}

#[builder(finish_fn = "fixture")]
pub fn tool_config_builder_f(
    tool: Option<Tool>,
    is_default: Option<bool>,
    tool_spec: Option<ToolSpec>,
) -> ToolConfigBuilder {
    ToolConfigBuilder::new(
        tool.unwrap_or(DEFAULT_TOOL),
        tool_spec,
        is_default.unwrap_or(true),
        &HashMap::default(),
        &module_path_f().fixture(),
        Option::default(),
        &metadata_f().fixture(),
        &RawToolArgs::default(),
        &EntryPoint::Default,
    )
    .expect("ToolConfigBuilder should be valid")
}

#[builder(finish_fn = "fixture")]
pub fn tool_config_f(
    tool: Option<Tool>,
    is_default: Option<bool>,
    events: Option<String>,
    entry_point: Option<EntryPoint>,
    part: Option<usize>,
    sanitize_output: Option<SanitizeOutput>,
    has_analyzer: Option<bool>,
) -> ToolConfig {
    let tool = tool.unwrap_or(DEFAULT_TOOL);
    let args = match tool {
        Tool::Perf => ToolArgs::Perf(PerfStatArgs::default().into()),
        Tool::Callgrind => ToolArgs::Valgrind(
            CallgrindArgs::try_from_raw_tool_args(tool, &[])
                .unwrap()
                .into(),
        ),
        Tool::Cachegrind => ToolArgs::Valgrind(
            CachegrindArgs::try_from_raw_tool_args(tool, &[])
                .unwrap()
                .into(),
        ),
        _ => ToolArgs::Valgrind(ValgrindArgs::try_from_raw_tool_args(tool, &[]).unwrap()),
    };

    let options = match tool {
        Tool::Perf => ToolConfigOptions::Perf(PerfConfig {
            alpha: DEFAULT_PERF_ALPHA,
            events: events.unwrap_or_else(|| DEFAULT_PERF_EVENTS.to_owned()),
            non_zero_metrics: DEFAULT_PERF_NON_ZERO_METRICS
                .iter()
                .map(ToString::to_string)
                .collect(),
            run_mode: PerfRunMode::default(),
            use_sampling: false,
            percent_running: 100.0,
        }),
        Tool::DHAT => ToolConfigOptions::DHAT(DhatConfig {
            frames: Vec::default(),
        }),
        Tool::Callgrind => ToolConfigOptions::Callgrind,
        Tool::Cachegrind => ToolConfigOptions::Cachegrind,
        Tool::Memcheck => ToolConfigOptions::Memcheck,
        Tool::Helgrind => ToolConfigOptions::Helgrind,
        Tool::DRD => ToolConfigOptions::DRD,
        Tool::Massif => ToolConfigOptions::Massif,
        Tool::BBV => ToolConfigOptions::BBV,
    };

    ToolConfig::new(
        true,
        args,
        ToolRegressionConfig::None,
        ToolFlamegraphConfig::None,
        entry_point.unwrap_or(EntryPoint::None),
        is_default.unwrap_or(true),
        sanitize_output.unwrap_or(SanitizeOutput::Yes),
        part,
        options,
        has_analyzer.unwrap_or(true),
        None,
    )
}

#[builder(finish_fn = "fixture")]
pub fn tool_configs_f(
    raw_command_line_args: Option<&[&str]>,
    tool_specs: Option<ToolSpecs>,
    default_tool: Option<Tool>,
    valgrind_args: Option<RawToolArgs>,
    default_entry_point: Option<EntryPoint>,
) -> ToolConfigs {
    let meta = metadata_f()
        .maybe_raw_command_line_args(raw_command_line_args)
        .fixture();
    let module_path = module_path_f().fixture();
    let mut output_format = OutputFormat::default();

    ToolConfigs::new(
        &mut output_format,
        tool_specs.unwrap_or_default(),
        &module_path,
        None,
        &meta,
        default_tool.unwrap_or(DEFAULT_TOOL),
        &default_entry_point.unwrap_or(EntryPoint::None),
        &valgrind_args.unwrap_or_default(),
        &HashMap::default(),
    )
    .expect("tool configs should be valid")
}

#[builder(finish_fn = "fixture")]
pub fn tool_output_path_f(
    target_dir: &Path,
    tool: Option<Tool>,
    name: Option<&str>,
    module_path_string: Option<&str>,
    init: Option<bool>,
    #[builder(default = vec![], with = FromIterator::from_iter)] files: Vec<(&str, &str)>,
) -> ToolOutputPath {
    let path = ToolOutputPath::new(
        ToolOutputPathKind::Out,
        tool.unwrap_or(Tool::Callgrind),
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
            std::fs::write(dir.join(path), content).unwrap();
        }
    }

    path
}

#[builder(finish_fn = "fixture")]
pub fn tool_spec_f(
    tool: Option<Tool>,
    events: Option<Vec<String>>,
    entry_point: Option<EntryPoint>,
) -> ToolSpec {
    let tool = tool.unwrap_or(DEFAULT_TOOL);
    let options = match tool {
        Tool::Perf => ToolSpecOptions::Perf(PerfSpec {
            alpha: None,
            events,
            record: None,
            non_zero_metrics: None,
            record_args: RawToolArgs::default(),
            run_mode: None,
            samples: None,
            percent_running: None,
        }),
        Tool::DHAT => ToolSpecOptions::Dhat(DhatSpec { frames: None }),
        _ => ToolSpecOptions::None,
    };
    ToolSpec {
        enable: Option::default(),
        entry_point,
        flamegraph_config: Option::default(),
        tool,
        output_format: Option::default(),
        raw_tool_args: RawToolArgs::default(),
        regression_config: Option::default(),
        sanitize_output: Option::default(),
        show_log: Option::default(),
        options,
    }
}
