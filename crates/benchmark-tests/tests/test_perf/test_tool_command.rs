use core::str;
use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::File;
use std::path::Path;

use gungraun::Tool;
use gungraun_runner::fixtures::{
    metadata_f, module_path_f, run_options_f, tool_command_f, tool_config_f, tool_output_path_f,
};
use gungraun_runner::runner::perf::args::DEFAULT_PERF_EVENTS;
use serde_json::Value;
use tempfile::tempdir;

#[cfg(target_os = "linux")]
#[test]
fn test_tool_command_perf_basic() {
    let temp_dir = tempdir().unwrap();
    let config = tool_config_f()
        .tool(Tool::Perf)
        .events(DEFAULT_PERF_EVENTS.to_owned())
        .fx();
    let output_path = &tool_output_path_f()
        .init(true)
        .tool(Tool::Perf)
        .target_dir(temp_dir.path())
        .fx();
    let tool_command = tool_command_f()
        .tool_config(&config)
        .metadata(metadata_f().fx())
        .output_path(output_path)
        .fx();

    let perf_bench = env!("CARGO_BIN_EXE_perf-bench");
    let executable_args: Vec<OsString> = vec![];

    let child = tool_command
        .run(
            &config,
            Path::new(perf_bench),
            &|_, _| Cow::Borrowed(executable_args.as_slice()),
            &run_options_f().fx(),
            output_path,
            &module_path_f().fx(),
            None,
            None,
            None,
            None,
        )
        .unwrap();

    let output = child.child.unwrap().wait_with_output().unwrap();
    let file = File::open(output_path.to_path()).unwrap();
    let json = serde_json::Deserializer::from_reader(file)
        .into_iter::<Value>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();

    assert!(output.status.success(), "perf command failed: {stderr}",);

    assert!(stderr.contains("Events enabled\n"));

    assert!(
        json.iter()
            .any(|entry| entry.get("counter-value").is_some()),
        "perf output did not contain any counter entries: {json:#?}"
    );

    assert!(
        json.iter().any(|entry| {
            entry
                .get("event")
                .and_then(Value::as_str)
                .is_some_and(|event| event.contains("instructions"))
        }),
        "perf output did not contain an instructions event: {json:#?}"
    );
}
