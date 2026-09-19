//! A utility binary to create the json schema for the summary.json file
use std::io::stdout;

use gungraun_summary::util::Version;
use gungraun_summary::{v6, v7};
use schemars::generate::SchemaSettings;

fn main() {
    let mut args = std::env::args().skip(1);

    let generator = SchemaSettings::draft07().into_generator();
    let version = args.next().map_or(Ok(Version::V7), |s| s.parse()).unwrap();

    let schema = match version {
        Version::V6 => &generator.into_root_schema_for::<v6::BenchmarkSummary>(),
        Version::V7 => &generator.into_root_schema_for::<v7::BenchmarkSummary>(),
    };

    serde_json::to_writer_pretty(stdout(), schema).expect("Schema creation should be successful");
}
