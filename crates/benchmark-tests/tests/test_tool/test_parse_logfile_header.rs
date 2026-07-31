use std::fs::File;
use std::io::{BufRead, BufReader};

use gungraun_runner::api::Tool;
use gungraun_runner::runner::tool::logfile_parser::parse_header;
use gungraun_runner::runner::tool::parser::Header;
use rstest::rstest;

use crate::util::common::Fixtures;

fn expected_header(command: &str, pid: i32, parent_pid: Option<i32>, desc: Vec<String>) -> Header {
    Header {
        command: command.to_owned(),
        pid,
        parent_pid,
        thread: None,
        part: None,
        desc,
    }
}

/// The basic structure of the logfile header is the same for all tools, so only drd is tested
/// exemplary here
#[rstest]
#[case::when_no_errors(
    "errors_all_zero",
    expected_header(
        "/home/some/workspace/target/release/deps/test_lib_bench_some-4c5214398e2f5bd1",
        1_915_454,
        Some(1_915_177_i32),
        vec![],
    )
)]
// What comes after the header and if there are errors or not should not influence the resulting
// header
#[case::when_errors(
    "with_errors",
    expected_header(
        "/home/some/workspace/target/release/deps/test_lib_bench_some-4c5214398e2f5bd1",
        1_915_455,
        Some(1_915_178_i32),
        vec![],
    )
)]
fn test_parse_logfile_header(#[case] name: &str, #[case] expected_header: Header) {
    use gungraun_runner::runner::tool::path::ToolOutputPathKind;

    let tool_output_path =
        Fixtures::get_tool_output_path("drd", Tool::DRD, ToolOutputPathKind::Log, name);
    let mut logfile_headers = vec![];
    for path in tool_output_path.sanitized_paths().unwrap() {
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);

        let header = parse_header(&path, &mut reader.lines()).unwrap();
        logfile_headers.push(header);
    }

    assert_eq!(logfile_headers.len(), 1);
    assert_eq!(logfile_headers[0], expected_header);
}
