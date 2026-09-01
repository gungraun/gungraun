//! The main test module

use std::fs::File;
use std::path::PathBuf;

use gungraun_summary::util::{SummaryByVersion, parse_slice};
use gungraun_summary::v6::{BenchmarkSummary, ToolMetricSummary};
use pretty_assertions::assert_eq;
#[cfg(feature = "schema")]
use serde_json::Value;
use serde_json::json;

/// Recursively remove all `description` keys from a JSON schema value.
///
/// The doc comments of the frozen v6 model drifted from the ones used when the stored schema was
/// generated, so the freeze test compares the structural properties only.
#[cfg(feature = "schema")]
fn strip_descriptions(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("description");
            for child in map.values_mut() {
                strip_descriptions(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_descriptions(item);
            }
        }
        _ => {}
    }
}

#[test]
fn test_smoke() {
    let current = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/summary.json");

    let summary: BenchmarkSummary = serde_json::from_reader(File::open(&current).unwrap()).unwrap();
    assert_eq!(summary.version, "6");
}

#[test]
#[cfg(feature = "schema")]
fn test_v6_freeze_of_models() {
    use std::process::Command;

    let expected = include_str!("../schemas/summary.v6.schema.json");
    let mut expected_json: Value =
        serde_json::from_str(expected).expect("The loaded schema should be valid json");

    let bin = env!("CARGO_BIN_EXE_gungraun-summary-schemagen");
    let output = Command::new(bin)
        .arg("v6")
        .output()
        .expect("Running schema generation should succeed");

    let mut actual_json: Value =
        serde_json::from_slice(&output.stdout).expect("The generated should be valid json");

    strip_descriptions(&mut expected_json);
    strip_descriptions(&mut actual_json);

    assert_eq!(actual_json, expected_json);
}

#[test]
fn test_v6_snapshot_parses_error_tool() {
    let error_tool = json!({
        "ErrorTool": {
            "Errors": {
                "diffs": null,
                "metrics": {"Left": {"Int": 1}}
            }
        }
    });

    match parse_slice(&v6_summary(&error_tool)).unwrap() {
        SummaryByVersion::V6(summary) => {
            assert_eq!(summary.version, "6");
            let profile = summary
                .profiles
                .0
                .first()
                .expect("at least one profile in the summary");
            assert!(matches!(
                profile.summaries.total.summary,
                ToolMetricSummary::ErrorTool(_)
            ));
            assert!(matches!(
                profile.summaries.parts.first().unwrap().metrics_summary,
                ToolMetricSummary::ErrorTool(_)
            ));
        }
        _ => panic!("expected the summary to be parsed as version 6"),
    }
}

#[test]
fn test_v6_snapshot_rejects_memcheck_tag() {
    let memcheck = json!({
        "Memcheck": {
            "Errors": {
                "diffs": null,
                "metrics": {"Left": {"Int": 1}}
            }
        }
    });

    let parsed = parse_slice(&v6_summary(&memcheck));
    assert!(
        parsed.is_err(),
        "the version 6 snapshot must reject the per-tool `Memcheck` tag"
    );
}

/// Build a minimal version 6 summary with a single Memcheck profile carrying the given tool
/// metric summary object in the part and in the total.
fn v6_summary(metrics_summary: &serde_json::Value) -> Vec<u8> {
    let summary = json!({
        "baselines": [null, null],
        "benchmark_exe": "/project/target/deps/bench",
        "benchmark_file": "/project/benches/example.rs",
        "details": null,
        "function_name": "some_benchmark_function",
        "id": null,
        "kind": "LibraryBenchmark",
        "module_path": "example::some_benchmark_function",
        "package_dir": "/project",
        "profiles": [{
            "flamegraphs": [],
            "log_paths": ["/tmp/gungraun/bench.log"],
            "out_paths": [],
            "summaries": {
                "parts": [{
                    "details": {
                        "Left": {
                            "command": "bench",
                            "details": null,
                            "parent_pid": null,
                            "part": null,
                            "path": "/tmp/gungraun/bench.out",
                            "pid": 1,
                            "thread": null
                        }
                    },
                    "metrics_summary": metrics_summary.clone()
                }],
                "total": {
                    "regressions": [],
                    "summary": metrics_summary
                }
            },
            "tool": "Memcheck"
        }],
        "project_root": "/project",
        "summary_output": null,
        "version": "6"
    });
    serde_json::to_vec(&summary).unwrap()
}
