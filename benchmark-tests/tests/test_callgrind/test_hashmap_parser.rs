use gungraun_runner::api::Tool;
use gungraun_runner::runner::callgrind::hashmap_parser::{CallgrindMap, HashMapParser};
use gungraun_runner::runner::callgrind::parser::CallgrindParser;
use gungraun_runner::runner::tool::path::ToolOutputPathKind;
use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::util::common::{Fixtures, assert_parse_error};

#[test]
fn test_valid_just_main() {
    let parser = HashMapParser::default();
    let output = Fixtures::get_tool_output_path(
        "callgrind.out",
        Tool::Callgrind,
        ToolOutputPathKind::Out,
        "valid.minimal_main",
    );
    let expected_map =
        Fixtures::load_serialized("callgrind.out/callgrind.valid.minimal_main.exp_map").unwrap();

    let actual_map = parser.parse(&output).unwrap();

    assert_eq!(actual_map.len(), 1);
    assert_eq!(actual_map[0].2, expected_map);
}

#[test]
fn test_when_empty_file_then_should_return_error() {
    let parser = HashMapParser::default();
    let output = Fixtures::get_tool_output_path(
        "callgrind.out",
        Tool::Callgrind,
        ToolOutputPathKind::Out,
        "empty",
    );
    assert_parse_error(&output.to_path(), parser.parse(&output), "Empty file");
}

#[test]
fn test_when_version_mismatch_then_should_return_error() {
    let parser = HashMapParser::default();
    let output = Fixtures::get_tool_output_path(
        "callgrind.out",
        Tool::Callgrind,
        ToolOutputPathKind::Out,
        "invalid.version_too_high",
    );
    assert_parse_error(
        &output.to_path(),
        parser.parse(&output),
        "Version mismatch: Requires callgrind format version '1' but was '2'",
    );
}

#[rstest]
#[case::no_summary_and_totals("no_records.no_summary_and_totals")]
#[case::summary_and_totals("no_records.with_summary_and_totals")]
fn test_when_no_records(#[case] name: &str) {
    let parser = HashMapParser::default();
    let output = Fixtures::get_tool_output_path(
        "callgrind.out",
        Tool::Callgrind,
        ToolOutputPathKind::Out,
        name,
    );
    let expected_map = CallgrindMap::default();

    let actual_map = parser.parse(&output).unwrap();

    assert_eq!(actual_map.len(), 1);
    assert_eq!(actual_map[0].2, expected_map);
}
