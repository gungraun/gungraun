//! Parse `perf stat -j` JSON output into benchmark [`Metrics`].

use std::cmp;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use log::debug;

use crate::api::PerfMetric;
use crate::metrics::model::{AnnotatedMetric, Metrics, PerfQualities};
use crate::runner::perf::logfile_parser::parse_perf_log;
use crate::runner::perf::records::PerfStatRecords;
use crate::runner::perf::run::{PERF_CALIBRATION_FILE_MODIFIER, PERF_OVERHEAD_FILE_MODIFIER};
use crate::runner::tool::parser::{Parser, ParserOutput};
use crate::runner::tool::path::ToolOutputPath;
use crate::summary::model::ToolMetrics;

/// Parser for `perf stat -j` JSON output.
#[derive(Debug)]
pub struct JsonParser {
    /// Minimum percentage of time a PMU counter must be running.
    ///
    /// Records below this threshold are discarded.
    pub min_pcnt_running: f64,
    /// Patterns for perf metrics that must not be zero.
    ///
    /// If a metric matching any of these patterns has a zero value, the entire measurement batch
    /// is discarded.
    pub non_zero_metrics: Vec<String>,
    /// Path to the JSON file produced by `perf stat -j`.
    pub output_path: ToolOutputPath,
}

impl JsonParser {
    fn parse_single_with_repetitions(
        &self,
        path: PathBuf,
        adjustment: Option<&Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>>,
    ) -> Result<(ParserOutput, usize, PerfStatRecords, bool)> {
        debug!("Parsing file: {}", path.display());

        let log_data = self
            .output_path
            .log_path_of(&path)
            .ok_or_else(|| anyhow!("A perf log file should exist"))
            .and_then(|log_path| {
                let file = File::open(&log_path)?;
                parse_perf_log(&log_path, BufReader::new(file).lines())
            })?;

        let ((metrics, has_duplicates), records) = PerfStatRecords::parse(&path).map(|r| {
            (
                r.to_metrics(self.min_pcnt_running, adjustment, &self.non_zero_metrics),
                r,
            )
        })?;

        // This error can happen if the measured workload is small and with for example `-r 100`. A
        // single perf run with trash data can contaminate the whole benchmark run with the 100
        // samples (As of perf version 7.0.10).
        if metrics.is_empty() {
            return Err(anyhow!(
                "No usable perf metrics found in '{}'",
                path.display()
            ));
        }

        Ok((
            ParserOutput {
                details: vec![],
                header: log_data.header,
                path,
                metrics: ToolMetrics::Perf(metrics),
            },
            log_data.repetitions,
            records,
            has_duplicates,
        ))
    }

    fn parse_adjustment(
        &self,
        (path, modifiers): (&PathBuf, &Option<String>),
    ) -> Result<Option<Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>>> {
        if modifiers.as_deref() == Some(PERF_OVERHEAD_FILE_MODIFIER)
            || modifiers.as_deref() == Some(PERF_CALIBRATION_FILE_MODIFIER)
        {
            debug!("Parsing perf adjustment '{}'", path.display());

            let (metrics, _) = PerfStatRecords::parse(path)
                .map(|r| r.to_metrics(self.min_pcnt_running, None, &self.non_zero_metrics))?;

            debug!("Adjustment metrics: {metrics:#?}");

            Ok(Some(metrics))
        } else {
            Ok(None)
        }
    }

    fn parse_part_path(
        &self,
        path: &Path,
        adjustment: Option<&Metrics<PerfMetric, AnnotatedMetric<PerfQualities>>>,
    ) -> Result<ParserOutput> {
        let (mut parsed, repetitions, mut records, has_duplicates) =
            self.parse_single_with_repetitions(path.to_path_buf(), adjustment)?;

        match (has_duplicates, adjustment) {
            (false, None) => {}
            (..) => match &mut parsed.metrics {
                ToolMetrics::Perf(metrics) => {
                    metrics.normalize_by_repetitions(repetitions);
                    records.update(metrics);
                    records.write(path)?;
                }
                _ => {
                    unreachable!("The metrics of this parser should always be perf metrics")
                }
            },
        }

        Ok(parsed)
    }
}

// TODO: Add tests
impl Parser for JsonParser {
    fn get_output_path(&self) -> &ToolOutputPath {
        &self.output_path
    }

    fn parse_single(&self, path: PathBuf) -> Result<ParserOutput> {
        self.parse_single_with_repetitions(path, None)
            .map(|(output, _, _, _)| output)
    }

    fn parse_with(&self, output_path: &ToolOutputPath) -> Result<Vec<ParserOutput>> {
        debug!(
            "{}: Parsing output path with name '{}'",
            output_path.tool.id(),
            output_path.name
        );

        let Ok(parts) = output_path.sanitized_paths_by_part() else {
            return Ok(vec![]);
        };

        let mut parser_results = Vec::with_capacity(parts.len());

        for (part, mut paths) in parts {
            paths.sort_by_key(|(_, modifier)| cmp::Reverse(modifier_rank(modifier.as_deref())));

            // We can only use one adjustment. That's the one with the highest modifier rank
            // ('overhead').
            let adjustment = paths
                .iter()
                .find_map(|(path, modifiers)| self.parse_adjustment((path, modifiers)).transpose())
                .transpose()?;

            output_path.clear_part_with_modifiers(
                part,
                &[PERF_CALIBRATION_FILE_MODIFIER, PERF_OVERHEAD_FILE_MODIFIER],
            )?;
            output_path.to_log_output().clear_part_with_modifiers(
                part,
                &[PERF_CALIBRATION_FILE_MODIFIER, PERF_OVERHEAD_FILE_MODIFIER],
            )?;

            // The .out files without a modifier are the ones we expect to contain the main
            // benchmark metrics
            for path in paths
                .into_iter()
                .filter_map(|(p, m)| m.is_none().then_some(p))
            {
                let parsed = self.parse_part_path(&path, adjustment.as_ref())?;
                let position = parser_results
                    .binary_search_by(|probe: &ParserOutput| probe.compare_target_ids(&parsed))
                    .unwrap_or_else(|e| e);

                parser_results.insert(position, parsed);
            }
        }

        Ok(parser_results)
    }
}

fn modifier_rank(modifier: Option<&str>) -> u8 {
    match modifier {
        Some(PERF_OVERHEAD_FILE_MODIFIER) => 3,
        Some(PERF_CALIBRATION_FILE_MODIFIER) => 2,
        Some(_) => 1,
        None => 0,
    }
}
