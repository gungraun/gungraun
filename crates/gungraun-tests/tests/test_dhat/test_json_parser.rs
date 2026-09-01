use std::fs;
use std::path::PathBuf;

use gungraun::Tool;
use gungraun_runner::api::{EntryPoint, SanitizeOutput};
use gungraun_runner::fixtures::tool_output_path_f;
use gungraun_runner::runner::dhat::json_parser::{JsonParser, parse};
use gungraun_runner::runner::dhat::model::DhatData;
use gungraun_runner::runner::dhat::tree::{RootTree, Tree};
use gungraun_runner::runner::tool::parser::Parser;
use gungraun_runner::summary::model::ToolMetrics;
use pretty_assertions::assert_eq;
use tempfile::{TempDir, tempdir};

use crate::util::common::Fixtures;

const DHAT_FIXTURE: &str = "dhat/dhat.with_entry_point.out";

fn assert_frame_indices_are_valid(data: &DhatData) {
    for program_point in &data.program_points {
        for frame in &program_point.frames {
            assert!(
                *frame < data.frame_table.len(),
                "frame index {frame} should reference a frame table entry"
            );
        }
    }
}

fn expected_metrics() -> ToolMetrics {
    let mut data = parse(&Fixtures::get_path_of(DHAT_FIXTURE)).unwrap();
    data.filter_program_points(&EntryPoint::Default, &[]);
    RootTree::from_json(data).metrics()
}

fn test_file() -> (TempDir, PathBuf, Vec<u8>) {
    let temp_dir = tempdir().unwrap();
    let source = Fixtures::get_path_of(DHAT_FIXTURE);
    let bytes = fs::read(&source).unwrap();
    let path = temp_dir.path().join("dhat.out");
    fs::write(&path, &bytes).unwrap();

    (temp_dir, path, bytes)
}

#[test]
fn test_json_parser_when_sanitize_keep_orig() {
    let (temp_dir, path, original) = test_file();
    let orig_path = path.with_extension("out.orig");

    let output = JsonParser::new(
        tool_output_path_f()
            .target_dir(temp_dir.path())
            .tool(Tool::DHAT)
            .name("dhat")
            .fx(),
        EntryPoint::Default,
        vec![],
        SanitizeOutput::KeepOrig,
    )
    .parse_single(path.clone())
    .unwrap();

    assert_eq!(output.metrics, expected_metrics());
    assert_ne!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read(orig_path).unwrap(), original);

    let data = parse(&path).unwrap();
    assert_eq!(data.program_points.len(), 1);
    assert_frame_indices_are_valid(&data);
}

#[test]
fn test_json_parser_when_sanitize_no() {
    let (temp_dir, path, original) = test_file();

    let output = JsonParser::new(
        tool_output_path_f()
            .target_dir(temp_dir.path())
            .tool(Tool::DHAT)
            .name("dhat")
            .fx(),
        EntryPoint::Default,
        vec![],
        SanitizeOutput::No,
    )
    .parse_single(path.clone())
    .unwrap();

    assert_eq!(output.metrics, expected_metrics());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!path.with_extension("out.orig").exists());
}

#[test]
fn test_json_parser_when_sanitize_yes() {
    let (temp_dir, path, original) = test_file();

    let output = JsonParser::new(
        tool_output_path_f()
            .target_dir(temp_dir.path())
            .tool(Tool::DHAT)
            .name("dhat")
            .fx(),
        EntryPoint::Default,
        vec![],
        SanitizeOutput::Yes,
    )
    .parse_single(path.clone())
    .unwrap();

    assert_eq!(output.path, path);
    let original_data = parse(&Fixtures::get_path_of(DHAT_FIXTURE)).unwrap();
    assert_eq!(output.header.pid, original_data.metadata.pid);
    assert_eq!(output.header.parent_pid, None);
    assert!(output.details.is_empty());
    assert_eq!(output.metrics, expected_metrics());

    let sanitized = fs::read(&path).unwrap();
    assert_ne!(sanitized, original);
    assert!(!path.with_extension("out.orig").exists());

    let data = parse(&path).unwrap();
    assert_eq!(data.program_points.len(), 1);
    assert!(data.frame_table.len() < original_data.frame_table.len());
    assert_frame_indices_are_valid(&data);
}
