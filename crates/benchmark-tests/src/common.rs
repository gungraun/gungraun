use std::fs::File;
use std::path::Path;

use anyhow::{Result, anyhow};
use either_or_both::EitherOrBoth;
use gungraun_runner::metrics::model::Metric;
use gungraun_runner::summary::model::{BenchmarkSummary, ToolMetricSummary};

#[derive(Debug)]
pub struct Summary(pub BenchmarkSummary);

impl Summary {
    pub fn new(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        serde_json::from_reader(file)
            .map(Self)
            .map_err(|error| anyhow!(error))
    }

    pub fn get_name(&self) -> String {
        if let Some(id) = self.0.id.as_ref() {
            format!("{} {id}", self.0.module_path)
        } else {
            self.0.module_path.to_string()
        }
    }

    #[track_caller]
    pub fn assert_costs_not_all_zero(&self) {
        for profile in self.0.profiles.iter() {
            for summary in profile
                .summaries
                .parts
                .iter()
                .map(|s| &s.metrics_summary)
                .chain(std::iter::once(&profile.summaries.total.summary))
            {
                match summary {
                    ToolMetricSummary::Dhat(metrics_summary) => {
                        match metrics_summary.extract_costs() {
                            EitherOrBoth::Left(new_costs) => {
                                assert!(
                                    !new_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *new* costs for dhat were zero for '{}'",
                                    self.get_name()
                                );
                            }
                            EitherOrBoth::Right(old_costs) => {
                                assert!(
                                    !old_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *old* costs for dhat were zero for '{}'",
                                    self.get_name()
                                );
                            }
                            EitherOrBoth::Both(new_costs, old_costs) => {
                                assert!(
                                    !new_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *new* costs for dhat were zero for '{}'",
                                    self.get_name()
                                );
                                assert!(
                                    !old_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *old* costs for dhat were zero for '{}'",
                                    self.get_name()
                                );
                            }
                        }
                    }
                    ToolMetricSummary::Callgrind(metrics_summary) => {
                        match metrics_summary.extract_costs() {
                            EitherOrBoth::Left(new_costs) => {
                                assert!(
                                    !new_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *new* costs for callgrind were zero for '{}'",
                                    self.get_name()
                                );
                            }
                            EitherOrBoth::Right(old_costs) => {
                                assert!(
                                    !old_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *old* costs for callgrind were zero for '{}'",
                                    self.get_name()
                                );
                            }
                            EitherOrBoth::Both(new_costs, old_costs) => {
                                assert!(
                                    !new_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *new* costs for callgrind were zero for '{}'",
                                    self.get_name()
                                );
                                assert!(
                                    !old_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *old* costs for callgrind were zero for '{}'",
                                    self.get_name()
                                );
                            }
                        }
                    }
                    ToolMetricSummary::Cachegrind(metrics_summary) => {
                        match metrics_summary.extract_costs() {
                            EitherOrBoth::Left(new_costs) => {
                                assert!(
                                    !new_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *new* costs for cachegrind were zero for '{}'",
                                    self.get_name()
                                );
                            }
                            EitherOrBoth::Right(old_costs) => {
                                assert!(
                                    !old_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *old* costs for cachegrind were zero for '{}'",
                                    self.get_name()
                                );
                            }
                            EitherOrBoth::Both(new_costs, old_costs) => {
                                assert!(
                                    !new_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *new* costs for cachegrind were zero for '{}'",
                                    self.get_name()
                                );
                                assert!(
                                    !old_costs.0.iter().all(|(_, c)| *c == Metric::Int(0)),
                                    "All *old* costs for cachegrind were zero for '{}'",
                                    self.get_name()
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
