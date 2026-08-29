use std::path::PathBuf;

use gungraun_runner::api::{ErrorMetric, Tool};
use gungraun_runner::metrics::model::Metrics;
use gungraun_runner::runner::tool::error_metric_parser::ErrorMetricLogfileParser;
use gungraun_runner::runner::tool::parser::Parser;
use gungraun_runner::runner::tool::path::ToolOutputPathKind;
use gungraun_runner::summary::model::ToolMetrics;
use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::util::common::Fixtures;

/// The tests for the drd error metrics parser can be seen as exemplary for error metrics of other
/// tools like helgrind. Memcheck is tested separately.
#[rstest]
#[case::zero_errors("errors_all_zero", [0, 0, 0, 0])]
#[case::with_errors("with_errors", [12, 34, 56, 78])]
#[case::with_multiple_error_lines("with_two_error_lines", [12, 34, 56, 78])]
fn test_drd_error_metric_parser(#[case] fixture: &str, #[case] expected: [u64; 4]) {
    let metrics = Metrics::with_metric_kinds([
        (ErrorMetric::Errors, expected[0]),
        (ErrorMetric::Contexts, expected[1]),
        (ErrorMetric::SuppressedErrors, expected[2]),
        (ErrorMetric::SuppressedContexts, expected[3]),
    ]);
    let expected_metrics = ToolMetrics::DRD(metrics);

    let drd_output_path =
        Fixtures::get_tool_output_path("drd", Tool::DRD, ToolOutputPathKind::Log, fixture);

    let parser = ErrorMetricLogfileParser {
        output_path: drd_output_path,
        root_dir: PathBuf::from("/does/not/matter"),
        tool: Tool::DRD,
    };

    let logfiles = parser.parse().unwrap();
    assert_eq!(logfiles.len(), 1);
    assert_eq!(logfiles[0].metrics, expected_metrics);
}

#[test]
fn test_drd_error_metric_parser_when_multiple_pids() {
    let first_metrics = Metrics::with_metric_kinds([
        (ErrorMetric::Errors, 0),
        (ErrorMetric::Contexts, 0),
        (ErrorMetric::SuppressedErrors, 0),
        (ErrorMetric::SuppressedContexts, 0),
    ]);
    let expected_first_metrics = ToolMetrics::DRD(first_metrics);
    let second_metrics = Metrics::with_metric_kinds([
        (ErrorMetric::Errors, 1),
        (ErrorMetric::Contexts, 23),
        (ErrorMetric::SuppressedErrors, 345),
        (ErrorMetric::SuppressedContexts, 4567),
    ]);
    let expected_second_metrics = ToolMetrics::DRD(second_metrics);

    let drd_output_path =
        Fixtures::get_tool_output_path("drd", Tool::DRD, ToolOutputPathKind::Log, "multiple_pids");

    let parser = ErrorMetricLogfileParser {
        output_path: drd_output_path.to_log_output(),
        root_dir: PathBuf::from("/does/not/matter"),
        tool: Tool::DRD,
    };

    let logfiles = parser.parse().unwrap();
    assert_eq!(logfiles.len(), 2);
    assert_eq!(logfiles[0].metrics, expected_first_metrics);
    assert_eq!(logfiles[1].metrics, expected_second_metrics);
}

/// Memcheck is tested separately because the content of the log files can differ greatly from drd
/// log files, although the `ERROR SUMMARY` line is the same.
#[rstest]
#[case::zero_errors("without_errors", [0, 0, 0, 0])]
#[case::bad_memory("bad_memory", [2, 2, 0, 0])]
#[case::with_errors("with_many_errors", [12, 34, 56, 78])]
#[case::with_multiple_error_lines("with_multiple_error_lines", [44, 555, 6666, 77777])]
fn test_memcheck_error_metric_parser(#[case] fixture: &str, #[case] expected: [u64; 4]) {
    let metrics = Metrics::with_metric_kinds([
        (ErrorMetric::Errors, expected[0]),
        (ErrorMetric::Contexts, expected[1]),
        (ErrorMetric::SuppressedErrors, expected[2]),
        (ErrorMetric::SuppressedContexts, expected[3]),
    ]);
    let expected_metrics = ToolMetrics::Memcheck(metrics);

    let memcheck_output_path = Fixtures::get_tool_output_path(
        "memcheck",
        Tool::Memcheck,
        ToolOutputPathKind::Log,
        fixture,
    );

    let parser = ErrorMetricLogfileParser {
        output_path: memcheck_output_path,
        root_dir: PathBuf::from("/does/not/matter"),
        tool: Tool::Memcheck,
    };

    let logfiles = parser.parse().unwrap();
    assert_eq!(logfiles.len(), 1);
    assert_eq!(logfiles[0].metrics, expected_metrics);
}

#[test]
fn test_memcheck_error_metric_parser_when_multiple_pids() {
    let first_metrics = Metrics::with_metric_kinds([
        (ErrorMetric::Errors, 0),
        (ErrorMetric::Contexts, 0),
        (ErrorMetric::SuppressedErrors, 0),
        (ErrorMetric::SuppressedContexts, 0),
    ]);
    let expected_first_metrics = ToolMetrics::Memcheck(first_metrics);
    let second_metrics = Metrics::with_metric_kinds([
        (ErrorMetric::Errors, 11),
        (ErrorMetric::Contexts, 222),
        (ErrorMetric::SuppressedErrors, 3333),
        (ErrorMetric::SuppressedContexts, 44444),
    ]);
    let expected_second_metrics = ToolMetrics::Memcheck(second_metrics);

    let memcheck_output_path = Fixtures::get_tool_output_path(
        "memcheck",
        Tool::Memcheck,
        ToolOutputPathKind::Log,
        "multiple_pids",
    );

    let parser = ErrorMetricLogfileParser {
        output_path: memcheck_output_path,
        root_dir: PathBuf::from("/does/not/matter"),
        tool: Tool::Memcheck,
    };

    let logfiles = parser.parse().unwrap();
    assert_eq!(logfiles.len(), 2);
    assert_eq!(logfiles[0].metrics, expected_first_metrics);
    assert_eq!(logfiles[1].metrics, expected_second_metrics);
}
