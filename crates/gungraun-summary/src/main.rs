//! A utility binary to create the json schema for the summary.json file
use std::fs::File;

use gungraun_summary::v6::BenchmarkSummary;
use schemars::generate::SchemaSettings;

fn main() {
    let generator = SchemaSettings::draft07().into_generator();
    serde_json::to_writer_pretty(
        File::create("summary.schema.json").unwrap(),
        &generator.into_root_schema_for::<BenchmarkSummary>(),
    )
    .expect("Schema creation should be successful");
}
