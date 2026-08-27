use std::fs;
use std::path::Path;

use gungraun::Tool;
use gungraun_runner::api::Unit;
use gungraun_runner::fixtures::perf::{json_parser_f, metric_perf_f, tool_metrics_perf_f};
use gungraun_runner::fixtures::{header_f, parser_output_f, tool_output_path_f};
use gungraun_runner::metrics::model::PerfQualities;
use gungraun_runner::runner::tool::parser::Parser;
use gungraun_runner::runner::tool::path::ToolOutputPath;
use gungraun_runner::summary::model::ToolMetrics;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

use crate::util::common::Fixtures;

fn copy_perf_fixtures(case: &str) -> (TempDir, ToolOutputPath) {
    let temp_dir = tempdir().unwrap();
    let prefix = format!("perf.{case}.");
    let output_path = tool_output_path_f()
        .init(true)
        .tool(Tool::Perf)
        .target_dir(temp_dir.path())
        .name(case)
        .fx();

    for entry in fs::read_dir(Fixtures::get_path().join("perf")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let file_name = entry.file_name();
        let is_matching_fixture = file_name.to_string_lossy().starts_with(&prefix)
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("out" | "log")
            );

        if is_matching_fixture {
            fs::copy(path, output_path.dest_dir().join(file_name)).unwrap();
        }
    }

    (temp_dir, output_path)
}

fn read_dir_sorted(path: &Path) -> Vec<String> {
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[test]
fn test_perf_one() {
    let (temp_dir, output_path) = copy_perf_fixtures("one");

    let file_path = output_path.dest_dir().join("perf.one.out");
    let original = fs::read(&file_path).unwrap();
    let parser = json_parser_f().output_path(output_path.clone()).fx();

    let expected_header = header_f().part(1).command("bench").pid(12345).fx();
    let expected_metrics = tool_metrics_perf_f()
        .metrics([metric_perf_f()
            .event("event_1")
            .value(42.0)
            .qualities(PerfQualities::new(None, 100.0, None, None, None))
            .unit(Unit::Unknown("count".to_owned()))
            .fx()])
        .fx();

    let expected_parser_output = parser_output_f()
        .path(file_path.clone())
        .header(expected_header)
        .tool_metrics(expected_metrics)
        .fx();

    let outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(outputs.len(), 1);
    let output = &outputs[0];
    assert_eq!(output, &expected_parser_output);
    assert_eq!(fs::read(&file_path).unwrap(), original);
    assert_eq!(
        read_dir_sorted(output_path.dest_dir()),
        ["perf.one.log".to_owned(), "perf.one.out".to_owned()]
    );

    temp_dir.close().unwrap();
}

#[test]
fn test_perf_mixed() {
    let (temp_dir, output_path) = copy_perf_fixtures("mixed");
    let file_path = output_path.dest_dir().join("perf.mixed.out");
    let original = fs::read(file_path.clone()).unwrap();
    let parser = json_parser_f().output_path(output_path.clone()).fx();

    let expected_header = header_f()
        .part(2)
        .command("full-schema-benchmark")
        .pid(23456)
        .fx();
    let expected_metrics = tool_metrics_perf_f()
        .metrics(
            [
                metric_perf_f().event("event_001").value(1).fx(),
                metric_perf_f()
                    .event("event_002")
                    .value(2.0)
                    .unit(Unit::Unknown("foo".to_owned()))
                    .fx(),
                metric_perf_f().event("event_003_empty_unit").value(3).fx(),
                metric_perf_f()
                    .event("event_004")
                    .value(4.0)
                    .unit(Unit::Nanoseconds)
                    .qualities(PerfQualities::new(None, None, 0.025, None, None))
                    .fx(),
                // event_005 is filtered out due to low pcnt_running
                metric_perf_f()
                    .event("event_006_all")
                    .value(6)
                    .unit(Unit::Milliseconds)
                    .qualities(PerfQualities::new(
                        Some(1100),
                        Some(100.0),
                        Some(0.05),
                        Some(1),
                        Some(6.0),
                    ))
                    .fx(),
                metric_perf_f()
                    .event("event_007_fract_unit")
                    .value(7.12345)
                    .unit(Unit::Joules)
                    .fx(),
                // event_008 is filtered due to low pcnt_running
            ]
            .into_iter()
            .chain((9..=100).map(|n| metric_perf_f().event(format!("event_{n:03}")).value(n).fx())),
        )
        .fx();

    let expected_parser_output = parser_output_f()
        .path(file_path.clone())
        .header(expected_header)
        .tool_metrics(expected_metrics)
        .fx();

    let parser_outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(parser_outputs.len(), 1);
    let parser_output = &parser_outputs[0];
    assert_eq!(parser_output, &expected_parser_output);
    assert_eq!(fs::read(&file_path).unwrap(), original);
    assert_eq!(
        read_dir_sorted(output_path.dest_dir()),
        ["perf.mixed.log".to_owned(), "perf.mixed.out".to_owned()]
    );

    temp_dir.close().unwrap();
}

#[test]
fn test_perf_parts_ordering() {
    let (temp_dir, output_path) = copy_perf_fixtures("parts");
    let p1_out_path = output_path.dest_dir().join("perf.parts.p1.out");
    let p2_out_path = output_path.dest_dir().join("perf.parts.p2.out");
    let parser = json_parser_f().output_path(output_path.clone()).fx();

    let expected_header_part_2 = header_f()
        .part(2)
        .command("parts-benchmark")
        .pid(11111)
        .fx();
    let expected_metrics_part_2 = tool_metrics_perf_f()
        .metrics([metric_perf_f()
            .event("event_parts_p2_01")
            .value(200.0)
            .unit(Unit::Unknown("count".to_owned()))
            .fx()])
        .fx();
    let expected_part_2 = parser_output_f()
        .path(p2_out_path)
        .header(expected_header_part_2)
        .tool_metrics(expected_metrics_part_2)
        .fx();

    let expected_header_part_1 = header_f()
        .part(1)
        .command("parts-benchmark")
        .pid(99999)
        .fx();
    let expected_metrics_part_1 = tool_metrics_perf_f()
        .metrics([metric_perf_f()
            .event("event_parts_p1_01")
            .value(100.0)
            .unit(Unit::Unknown("count".to_owned()))
            .fx()])
        .fx();
    let expected_part_1 = parser_output_f()
        .path(p1_out_path)
        .header(expected_header_part_1)
        .tool_metrics(expected_metrics_part_1)
        .fx();

    let parser_outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(parser_outputs, vec![expected_part_2, expected_part_1]);

    temp_dir.close().unwrap();
}

#[test]
fn test_perf_repeated_event_records() {
    let (temp_dir, output_path) = copy_perf_fixtures("repeated");
    let file_path = output_path.dest_dir().join("perf.repeated.out");
    let original = fs::read(file_path.clone()).unwrap();
    let parser = json_parser_f()
        .min_pcnt_running(90.0)
        .output_path(output_path.clone())
        .fx();

    let expected_header = header_f()
        .part(3)
        .command("repeated-benchmark")
        .pid(34567)
        .fx();

    #[expect(clippy::cast_precision_loss)]
    let expected_metrics = tool_metrics_perf_f()
        .metrics((1..=10).map(|x| {
            metric_perf_f()
                .event(format!("event_repeat_{x:02}"))
                .value(100 + x)
                .qualities(PerfQualities::new(
                    1000 + x,
                    90.0 + x as f64,
                    x as f64 / 100.0,
                    None,
                    None,
                ))
                .fx()
        }))
        .fx();

    let expected_parser_output = parser_output_f()
        .path(file_path.clone())
        .header(expected_header)
        .tool_metrics(expected_metrics)
        .fx();

    let parser_outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(parser_outputs, vec![expected_parser_output]);
    assert_eq!(fs::read(&file_path).unwrap(), original);
    assert_eq!(
        read_dir_sorted(output_path.dest_dir()),
        [
            "perf.repeated.log".to_owned(),
            "perf.repeated.out".to_owned()
        ]
    );

    temp_dir.close().unwrap();
}

#[test]
fn test_perf_duplicates_write_back() {
    let (_temp_dir, output_path) = copy_perf_fixtures("duplicates");
    let out_path = output_path.dest_dir().join("perf.duplicates.out");
    let original = {
        let path: &Path = &out_path;
        fs::read(path).unwrap()
    };
    let parser = json_parser_f().output_path(output_path.clone()).fx();

    let expected_header = header_f()
        .part(4)
        .command("duplicates-benchmark")
        .pid(45678)
        .fx();
    let expected_metrics = tool_metrics_perf_f()
        .metrics([
            metric_perf_f()
                .event("event_dup_01")
                .value(66.666_666_666_666_67)
                .unit(Unit::Unknown("count".to_owned()))
                .qualities(PerfQualities::new(
                    None,
                    None,
                    0.5,
                    2,
                    66.666_666_666_666_67,
                ))
                .fx(),
            metric_perf_f()
                .event("event_control_01")
                .value(300.0)
                .unit(Unit::Milliseconds)
                .qualities(PerfQualities::new(None, None, 0.04, None, None))
                .fx(),
        ])
        .fx();
    let expected_parser_output = parser_output_f()
        .path(out_path.clone())
        .header(expected_header)
        .tool_metrics(expected_metrics)
        .fx();

    let expected_records = [
        json!({
            "counter-value": "66.666667",
            "event": "event_dup_01",
            "gungraun-mean": 66.666_666_666_666_67,
            "gungraun-n": 2,
            "unit": "count",
            "variance": 50.0,
        }),
        json!({
            "counter-value": "300.000000",
            "event": "event_control_01",
            "unit": "msec",
            "variance": 4.0,
        }),
    ];

    let outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(outputs, vec![expected_parser_output]);
    assert_ne!(fs::read(&out_path).unwrap(), original);

    let rewritten = fs::read_to_string(&out_path).unwrap();
    let records = rewritten
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records, expected_records);
    assert_eq!(
        read_dir_sorted(output_path.dest_dir()),
        [
            "perf.duplicates.log".to_owned(),
            "perf.duplicates.out".to_owned()
        ]
    );
}

#[test]
fn test_perf_calibration_adjustment() {
    let (temp_dir, output_path) = copy_perf_fixtures("calibration");
    let file_path = output_path.dest_dir().join("perf.calibration.out");
    let original = fs::read(file_path.clone()).unwrap();
    let parser = json_parser_f().output_path(output_path.clone()).fx();

    let expected_header = header_f()
        .part(5)
        .command("calibration-benchmark")
        .pid(56789)
        .fx();
    let expected_metrics = tool_metrics_perf_f()
        .metrics([
            metric_perf_f()
                .event("event_calibration_01")
                .value(100)
                .fx(),
            metric_perf_f()
                .event("event_calibration_02")
                .value(200)
                .fx(),
        ])
        .fx();
    let expected_parser_output = parser_output_f()
        .path(file_path.clone())
        .header(expected_header)
        .tool_metrics(expected_metrics)
        .fx();

    let outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(outputs, vec![expected_parser_output]);
    assert_ne!(fs::read(&file_path).unwrap(), original);
    assert_eq!(
        read_dir_sorted(output_path.dest_dir()),
        [
            "perf.calibration.log".to_owned(),
            "perf.calibration.out".to_owned()
        ]
    );

    temp_dir.close().unwrap();
}

#[test]
fn test_perf_adjustment_priority() {
    // This copies the cal, overhead and regular benchmark files
    let (temp_dir, output_path) = copy_perf_fixtures("adjustment_priority");
    let file_path = output_path.dest_dir().join("perf.adjustment_priority.out");
    let original = fs::read(file_path.clone()).unwrap();
    let parser = json_parser_f().output_path(output_path.clone()).fx();

    let expected_header = header_f()
        .part(6)
        .command("adjustment-priority-benchmark")
        .pid(67890)
        .fx();
    let expected_metrics = tool_metrics_perf_f()
        .metrics([
            metric_perf_f().event("event_priority_01").value(100).fx(),
            metric_perf_f().event("event_priority_02").value(200).fx(),
        ])
        .fx();
    let expected_parser_output = parser_output_f()
        .path(file_path.clone())
        .header(expected_header)
        .tool_metrics(expected_metrics)
        .fx();

    let outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(outputs, vec![expected_parser_output]);
    assert_ne!(fs::read(&file_path).unwrap(), original);
    assert_eq!(
        read_dir_sorted(output_path.dest_dir()),
        [
            "perf.adjustment_priority.log".to_owned(),
            "perf.adjustment_priority.out".to_owned()
        ]
    );

    temp_dir.close().unwrap();
}

#[test]
fn test_perf_filtered_empty_min_running() {
    let fixture_path = Fixtures::get_path().join("perf/perf.filtered.out");
    let original = fs::read(fixture_path.clone()).unwrap();
    let (temp_dir, output_path) = copy_perf_fixtures("filtered");
    let parser = json_parser_f()
        .min_pcnt_running(50.0)
        .output_path(output_path.clone())
        .fx();

    let outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(outputs.len(), 1);
    let output = &outputs[0];
    assert!(matches!(&output.metrics, ToolMetrics::Perf(m) if m.is_empty()));
    assert_eq!(fs::read(&fixture_path).unwrap(), original);

    temp_dir.close().unwrap();
}

#[test]
fn test_perf_filtered_empty_non_zero() {
    let fixture_path = Fixtures::get_path().join("perf/perf.filtered.out");
    let original = fs::read(fixture_path.clone()).unwrap();
    let (temp_dir, output_path) = copy_perf_fixtures("filtered");
    let parser = json_parser_f()
        .min_pcnt_running(0.0)
        .non_zero_metrics(["event_filtered/01"])
        .output_path(output_path.clone())
        .fx();

    let outputs = parser.parse_with(&output_path).unwrap();

    assert_eq!(outputs.len(), 1);
    let output = &outputs[0];
    assert!(matches!(&output.metrics, ToolMetrics::Perf(m) if m.is_empty()));
    assert_eq!(fs::read(&fixture_path).unwrap(), original);

    temp_dir.close().unwrap();
}
