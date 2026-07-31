//! The main test module

use std::fs::File;
use std::path::PathBuf;

use gungraun_summary::v6::BenchmarkSummary;

#[test]
fn test_smoke() {
    let current = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/summary.json");

    let summary: BenchmarkSummary = serde_json::from_reader(File::open(&current).unwrap()).unwrap();
    assert_eq!(summary.version, "6");
}
