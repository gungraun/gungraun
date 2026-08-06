use std::collections::HashMap;
use std::time::Duration;

use gungraun::{EntryPoint, SanitizeOutput};
use gungraun_runner::api::{PerfRunMode, RawToolArgs, Tool, ToolSpec, ToolSpecOptions, ToolSpecs};
use gungraun_runner::fixtures::perf::{perf_config_f, perf_spec_f};
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
        .raw_command_line_args(["--valgrind-args='--trace-children=no --num-callers=50'"])
        .fx();

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
        .raw_command_line_args(["--valgrind-args=--trace-children=no"])
        .tool_specs(ToolSpecs(vec![ToolSpec::new(Tool::Memcheck)]))
        .fx();

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
        .raw_command_line_args([
            "--valgrind-args=--trace-children=no",
            "--callgrind-args=--trace-children=yes",
        ])
        .fx();

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
            .fx(),
    ];
    let builder = tool_config_builder_f().tool(Tool::Perf).fx();
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
            .fx(),
        tool_config_f()
            .tool(Tool::Perf)
            .is_default(false)
            .events("second".to_owned())
            .entry_point(EntryPoint::Default)
            .part(2)
            .sanitize_output(SanitizeOutput::No)
            .has_analyzer(false)
            .fx(),
    ];

    let builder = tool_config_builder_f()
        .tool(Tool::Perf)
        .tool_spec(
            tool_spec_f()
                .tool(Tool::Perf)
                .entry_point(EntryPoint::Default)
                .options(ToolSpecOptions::Perf(
                    perf_spec_f().events(vec!["first", "second"]).fx(),
                ))
                .fx(),
        )
        .fx();
    let actual = builder.build().unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn test_tool_configs_apply_cli_perf_options() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args([
            "--tools=perf",
            "--perf-args=--all-user",
            "--perf-events=instructions,cycles",
            "--perf-events=task-clock",
            "--perf-run-mode=calibrate=250ms",
            "--perf-limits=*instructions*=1.5%|1000,task-clock*=10%|2.5ms",
        ])
        .fx();

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
            ToolConfigOptions::Perf(
                perf_config_f()
                    .alpha(0.05)
                    .events(expected_events)
                    .min_pcnt_running(100.0)
                    .non_zero_metrics(["task-clock*", "cpu-clock*", "*instructions*"])
                    .run_mode(PerfRunMode::Calibrate(Duration::from_millis(250)))
                    .use_sampling(false)
                    .fx()
            )
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
fn test_tool_configs_cli_perf_sampling_enables_sampling() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(["--tools=perf", "--perf-sampling=250ms"])
        .fx();

    let perf_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool() == Tool::Perf)
        .expect("perf config should be present");
    assert_eq!(perf_config.timeout, Some(Duration::from_millis(250)));
    let ToolConfigOptions::Perf(options) = &perf_config.options else {
        unreachable!("expected perf options")
    };
    assert!(options.use_sampling);
}

#[test]
fn test_tool_configs_cli_perf_sampling_no_overrides_benchmark_sample_duration() {
    let perf_tool_spec = tool_spec_f()
        .tool(Tool::Perf)
        .options(ToolSpecOptions::Perf(
            perf_spec_f().sample_duration(Duration::from_secs(5)).fx(),
        ))
        .fx();

    let tool_configs = tool_configs_f()
        .raw_command_line_args(["--tools=perf", "--perf-sampling=no"])
        .tool_specs(ToolSpecs(vec![perf_tool_spec]))
        .fx();

    let perf_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool() == Tool::Perf)
        .expect("perf config should be present");
    assert_eq!(perf_config.timeout, None);
    let ToolConfigOptions::Perf(options) = &perf_config.options else {
        unreachable!("expected perf options")
    };
    assert!(!options.use_sampling);
}

#[test]
fn test_tool_configs_absent_cli_perf_sampling_preserves_benchmark_sample_duration() {
    let perf_tool_spec = tool_spec_f()
        .tool(Tool::Perf)
        .options(ToolSpecOptions::Perf(
            perf_spec_f().sample_duration(Duration::from_secs(5)).fx(),
        ))
        .fx();

    let tool_configs = tool_configs_f()
        .raw_command_line_args(["--tools=perf"])
        .tool_specs(ToolSpecs(vec![perf_tool_spec]))
        .fx();

    let perf_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool() == Tool::Perf)
        .expect("perf config should be present");
    assert_eq!(perf_config.timeout, Some(Duration::from_secs(5)));
    let ToolConfigOptions::Perf(options) = &perf_config.options else {
        unreachable!("expected perf options")
    };
    assert!(options.use_sampling);
}

#[test]
fn test_tool_configs_perf_record_clears_cli_sampling_timeout() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(["--tools=perf", "--perf-record", "--perf-sampling=250ms"])
        .fx();

    let stat_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool() == Tool::Perf && !config.is_perf_record())
        .expect("perf stat config should be present");
    let record_config = tool_configs
        .0
        .iter()
        .find(|config| config.is_perf_record())
        .expect("perf record config should be present");

    assert_eq!(stat_config.timeout, Some(Duration::from_millis(250)));
    assert_eq!(record_config.timeout, None);
    let ToolConfigOptions::Perf(record_options) = &record_config.options else {
        unreachable!("expected perf options")
    };
    assert!(!record_options.use_sampling);
}

#[test]
fn test_tool_configs_cli_perf_record_options_override_benchmark_options() {
    let perf_tool_spec = tool_spec_f()
        .tool(Tool::Perf)
        .options(ToolSpecOptions::Perf(
            perf_spec_f()
                .record(false)
                .record_args(RawToolArgs::from_iter(["--old-record-arg"]))
                .fx(),
        ))
        .fx();

    let tool_configs = tool_configs_f()
        .raw_command_line_args([
            "--tools=perf",
            "--perf-record",
            "--perf-record-args=--metric-only",
        ])
        .tool_specs(ToolSpecs(vec![perf_tool_spec]))
        .fx();

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
    let perf_tool_spec = tool_spec_f()
        .tool(Tool::Perf)
        .options(ToolSpecOptions::Perf(
            perf_spec_f().min_pcnt_running(f64::NAN).fx(),
        ))
        .fx();

    let module_path = module_path_f().fx();
    let mut output_format = OutputFormat::default();

    let result = ToolConfigs::new(
        &mut output_format,
        ToolSpecs(vec![perf_tool_spec]),
        &module_path,
        None,
        &metadata_f().fx(),
        Tool::Callgrind,
        &EntryPoint::None,
        &RawToolArgs::default(),
        &HashMap::default(),
        None,
    );

    let err = result.expect_err("should fail for invalid min_pcnt_running");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("Invalid min_pcnt_running value 'NaN'"),
        "expected validation error message, got: {msg}"
    );
}
