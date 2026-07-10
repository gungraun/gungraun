use gungraun::{EntryPoint, SanitizeOutput};
use gungraun_runner::api::{Tool, ToolSpec, ToolSpecs};
use gungraun_runner::fixtures::{
    tool_config_builder_f, tool_config_f, tool_configs_f, tool_spec_f,
};
use gungraun_runner::runner::perf::args::DEFAULT_PERF_EVENTS;

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
