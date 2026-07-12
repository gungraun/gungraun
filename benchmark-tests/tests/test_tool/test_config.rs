use std::collections::HashMap;
use std::time::Duration;

use gungraun::{EntryPoint, SanitizeOutput};
use gungraun_runner::api::{PerfRunMode, RawToolArgs, Tool, ToolSpec, ToolSpecOptions, ToolSpecs};
use gungraun_runner::fixtures::{
    metadata_f, module_path_f, tool_config_builder_f, tool_config_f, tool_configs_f, tool_spec_f,
};
use gungraun_runner::runner::format::OutputFormat;
use gungraun_runner::runner::perf::args::DEFAULT_PERF_EVENTS;
use gungraun_runner::runner::tool::config::{ToolConfigOptions, ToolConfigs};
use gungraun_runner::runner::tool::regression::ToolRegressionConfig;
use gungraun_runner::units::Unit;

#[test]
fn test_tool_configs_apply_cli_valgrind_args_to_default_tool() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(&["--valgrind-args='--trace-children=no --num-callers=50'"])
        .fixture();

    let callgrind_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool() == Tool::Callgrind)
        .expect("callgrind config should be present");

    let args = callgrind_config.args.to_vec();
    assert!(args.iter().any(|a| a == "--trace-children=no"));
    assert!(args.iter().any(|a| a == "--num-callers=50"));
}

#[test]
fn test_tool_configs_apply_cli_valgrind_args_to_additional_tool() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(&["--valgrind-args=--trace-children=no"])
        .tool_specs(ToolSpecs(vec![ToolSpec::new(Tool::Memcheck)]))
        .fixture();

    let memcheck_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool() == Tool::Memcheck)
        .expect("memcheck config should be present");

    assert!(
        memcheck_config
            .args
            .to_vec()
            .iter()
            .any(|a| a == "--trace-children=no")
    );
}

#[test]
fn test_tool_configs_cli_tool_args_override_cli_valgrind_args() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(&[
            "--valgrind-args=--trace-children=no",
            "--callgrind-args=--trace-children=yes",
        ])
        .fixture();

    let callgrind_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool() == Tool::Callgrind)
        .expect("callgrind config should be present");

    let args = callgrind_config.args.to_vec();
    assert!(args.iter().any(|a| a == "--trace-children=yes"));
    assert!(args.iter().all(|a| a != "--trace-children=no"));
}

#[test]
fn test_test_configs_when_perf_default() {
    let expected = vec![
        tool_config_f()
            .tool(Tool::Perf)
            .events(DEFAULT_PERF_EVENTS.to_owned())
            .entry_point(gungraun::EntryPoint::Default)
            .maybe_part(None)
            .sanitize_output(gungraun::SanitizeOutput::No)
            .fixture(),
    ];
    let builder = tool_config_builder_f().tool(Tool::Perf).fixture();
    let actual = builder.build().unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn test_test_configs_when_perf_multiple_events_expands_to_multiple_configs() {
    let expected = vec![
        tool_config_f()
            .tool(Tool::Perf)
            .is_default(true)
            .events("first".to_owned())
            .entry_point(EntryPoint::Default)
            .part(1)
            .sanitize_output(SanitizeOutput::No)
            .has_analyzer(true)
            .fixture(),
        tool_config_f()
            .tool(Tool::Perf)
            .is_default(false)
            .events("second".to_owned())
            .entry_point(EntryPoint::Default)
            .part(2)
            .sanitize_output(SanitizeOutput::No)
            .has_analyzer(false)
            .fixture(),
    ];

    let builder = tool_config_builder_f()
        .tool(Tool::Perf)
        .tool_spec(
            tool_spec_f()
                .tool(Tool::Perf)
                .entry_point(EntryPoint::Default)
                .events(vec!["first".to_owned(), "second".to_owned()])
                .fixture(),
        )
        .fixture();
    let actual = builder.build().unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn test_tool_configs_apply_cli_perf_options() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(&[
            "--tools=perf",
            "--perf-args=--all-user",
            "--perf-events=instructions,cycles",
            "--perf-events=task-clock",
            "--perf-run-mode=calibrate=250ms",
            "--perf-limits=*instructions*=1.5%|1000,task-clock*=10%|2.5ms",
        ])
        .fixture();

    let perf_configs = tool_configs
        .0
        .iter()
        .filter(|config| config.tool() == Tool::Perf)
        .collect::<Vec<_>>();
    assert_eq!(perf_configs.len(), 2);

    for (config, expected_events) in perf_configs
        .iter()
        .zip(["instructions,cycles", "task-clock"])
    {
        assert!(config.args.to_vec().iter().any(|arg| arg == "--all-user"));
        assert_eq!(
            config.options,
            ToolConfigOptions::Perf(gungraun_runner::runner::tool::config::PerfConfig {
                alpha: 0.05,
                events: expected_events.to_owned(),
                non_zero_metrics: vec![
                    "task-clock*".to_owned(),
                    "cpu-clock*".to_owned(),
                    "*instructions*".to_owned(),
                ],
                min_pcnt_running: 100.0,
                run_mode: PerfRunMode::Calibrate(Duration::from_millis(250)),
                use_sampling: false,
            })
        );
        assert_eq!(
            config.regression_config,
            ToolRegressionConfig::Perf(
                gungraun_runner::runner::perf::regression::PerfRegressionConfig {
                    alpha: 0.05,
                    fail_fast: false,
                    hard_limits: vec![
                        (
                            gungraun_runner::api::PerfMetric("*instructions*".to_owned()),
                            None,
                            1000.into(),
                        ),
                        (
                            gungraun_runner::api::PerfMetric("task-clock*".to_owned()),
                            Some(Unit::Milliseconds),
                            2.5.into(),
                        ),
                    ],
                    soft_limits: vec![
                        (
                            gungraun_runner::api::PerfMetric("*instructions*".to_owned()),
                            1.5,
                        ),
                        (
                            gungraun_runner::api::PerfMetric("task-clock*".to_owned()),
                            10.0,
                        ),
                    ],
                }
            )
        );
    }
}

#[test]
fn test_tool_configs_cli_perf_record_options_override_benchmark_options() {
    let mut perf_tool_spec = ToolSpec::new(Tool::Perf);
    let ToolSpecOptions::Perf(perf_spec) = &mut perf_tool_spec.options else {
        unreachable!("perf tool specs must have perf options");
    };
    perf_spec.record = Some(false);
    perf_spec.record_args = RawToolArgs::from_iter(["--old-record-arg"]);

    let tool_configs = tool_configs_f()
        .raw_command_line_args(&[
            "--tools=perf",
            "--perf-record",
            "--perf-record-args=--metric-only",
        ])
        .tool_specs(ToolSpecs(vec![perf_tool_spec]))
        .fixture();

    let perf_configs = tool_configs
        .0
        .iter()
        .filter(|config| config.tool() == Tool::Perf)
        .collect::<Vec<_>>();
    assert_eq!(
        perf_configs.len(),
        2,
        "expected perf stat and perf record configs"
    );

    let record_config = perf_configs
        .iter()
        .find(|config| config.is_perf_record())
        .expect("perf record config should exist");
    let record_args = record_config.args.to_vec();
    assert!(record_args.iter().any(|arg| arg == "--metric-only"));
    assert!(record_args.iter().all(|arg| arg != "--old-record-arg"));
}

#[test]
fn test_tool_configs_reject_invalid_perf_min_pcnt_running() {
    let mut perf_tool_spec = ToolSpec::new(Tool::Perf);
    let ToolSpecOptions::Perf(perf_spec) = &mut perf_tool_spec.options else {
        unreachable!("perf tool specs must have perf options");
    };
    perf_spec.min_pcnt_running = Some(f64::NAN);

    let module_path = module_path_f().fixture();
    let mut output_format = OutputFormat::default();

    let result = ToolConfigs::new(
        &mut output_format,
        ToolSpecs(vec![perf_tool_spec]),
        &module_path,
        None,
        &metadata_f().fixture(),
        Tool::Callgrind,
        &EntryPoint::None,
        &RawToolArgs::default(),
        &HashMap::default(),
    );

    let err = result.expect_err("should fail for invalid min_pcnt_running");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("Invalid min_pcnt_running value 'NaN'"),
        "expected validation error message, got: {msg}"
    );
}
