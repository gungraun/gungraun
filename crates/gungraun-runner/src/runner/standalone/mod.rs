//! Public standalone commands, translated into the existing binary benchmark runner.
//!
//! Only this front end requires complete absolute limits. Rust harness invocations keep their
//! existing configuration and execution semantics.

mod config;

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use self::config::{GROUP_ID, Manifest};
use super::args::CommandLineArgs;
use super::bin_bench;
use super::common::{BaselineKind, BenchmarkSummaries, Config, ModulePath};
use super::format::{OutputFormatKind, print_standalone_status};
use super::meta::Metadata;
use super::tool::path::{ToolOutputPath, ToolOutputPathKind};
use crate::api::{EventKind, Tool};
use crate::error::Error;
use crate::metrics::model::Metric;
use crate::summary::model::{BenchmarkKind, Profile, ToolMetricResults};
use crate::summary::output::{SummaryFormat, SummaryOutput};

const MODULE: &str = "standalone";

#[derive(Debug, Subcommand)]
enum Command {
    /// Capture current measurements; missing instruction caps are identified explicitly.
    Run(Options),
    /// Enforce positive absolute Ir limits for every declared case using current measurements.
    Check(Options),
}

/// The intentionally small standalone interface; the private harness CLI is not flattened here.
#[derive(Debug, Parser)]
#[command(
    name = "gungraun-runner",
    version,
    about = "Run external binary benchmarks from a versioned TOML configuration",
    after_help = "Exit codes: 0 success, 1 execution/configuration/measurement error, 2 invalid \
                  CLI, 3 exceeded absolute limit. Only 'check' reports an enforced guard pass."
)]
pub(super) struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct Options {
    /// Allow ASLR, for example when a container forbids disabling it.
    #[arg(long)]
    allow_aslr: bool,
    /// Versioned TOML manifest. Its directory anchors program, cwd and output paths.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
    /// Override the manifest output directory (relative to the manifest's directory).
    #[arg(long, value_name = "PATH")]
    output_dir: Option<PathBuf>,
    /// Native terminal report format. A native summary.json is always retained per measured case.
    #[arg(long, value_enum, default_value = "default")]
    output_format: OutputFormatKind,
    /// Valgrind executable path (relative to the invocation's current directory).
    #[arg(long, value_name = "PATH")]
    valgrind_bin: Option<PathBuf>,
}

impl Cli {
    pub(super) fn run(self) -> Result<()> {
        let (options, strict) = match self.command {
            Command::Run(options) => (options, false),
            Command::Check(options) => (options, true),
        };
        let manifest = config::load(&options.config, strict)?;
        if options
            .output_dir
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(Error::ManifestError(
                manifest.source,
                "--output-dir must not be empty".into(),
            )
            .into());
        }
        let output_dir = options.output_dir.map_or_else(
            || manifest.output_dir.clone(),
            |path| manifest.root.join(path),
        );

        let mut args = CommandLineArgs::standalone_defaults()?;
        args.allow_aslr = Some(options.allow_aslr);
        args.output_format = options.output_format;
        args.save_summary = Some(SummaryFormat::PrettyJson);
        args.valgrind_bin = options
            .valgrind_bin
            .map(std::path::absolute)
            .transpose()
            .context("Cannot resolve --valgrind-bin")?;

        let module_path = ModulePath::new(MODULE);
        let config = Config {
            // No setup/teardown callbacks exist in this schema, so the harness path is never
            // executed. Native summaries obtain benchmark_exe from each actual Command.
            bench_bin: manifest.source.clone(),
            bench_file: manifest.source.clone(),
            meta: Metadata::standalone(args, manifest.root.clone(), output_dir),
            module_path,
            package_dir: manifest.root.clone(),
        };

        // An interrupted/failed run must not leave a previous summary advertised as current.
        // Keep native profiles as historical evidence; no historical result is used by this gate.
        clear_summaries(&manifest, &config)?;
        print_standalone_status(strict, false, manifest.case_ids.len(), &manifest.uncapped);

        let summaries = bin_bench::run(manifest.groups, config)?;
        validate_results(&manifest.source, &manifest.case_ids, &summaries)?;
        summaries.print(false, options.output_format);
        if summaries.is_regressed() {
            return Err(Error::RegressionError(false).into());
        }
        print_standalone_status(strict, true, manifest.case_ids.len(), &manifest.uncapped);
        Ok(())
    }
}

fn clear_summaries(manifest: &Manifest, config: &Config) -> Result<()> {
    for case in &manifest.case_ids {
        let output = ToolOutputPath::new(
            ToolOutputPathKind::Out,
            Tool::Callgrind,
            &BaselineKind::Old,
            &config.meta.target_dir,
            &config.module_path.join(GROUP_ID),
            case,
            false,
        )?;
        match std::fs::remove_file(SummaryOutput::path(&output.dir)) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Cannot remove previous benchmark summary"),
        }
    }
    Ok(())
}

fn validate_profile(profile: &Profile) -> Result<(), &'static str> {
    if profile.tool != Tool::Callgrind {
        return Err("expected a Callgrind measurement");
    }
    let ToolMetricResults::Callgrind(metrics) = &profile.data.total.metrics else {
        return Err("missing current Callgrind metrics");
    };
    match metrics
        .0
        .get(&EventKind::Ir)
        .and_then(|result| result.values.as_ref().left())
    {
        Some(Metric::Int(count)) if *count > 0 => {}
        _ => return Err("missing or zero current Ir; check that the entry point was executed"),
    }
    let mut current_parts = HashSet::new();
    for part in &profile.data.parts {
        if let Some(run) = part.tool_run.as_ref().left()
            && !current_parts.insert((run.pid, run.part, run.thread))
        {
            return Err("duplicate current process/part/thread result");
        }
    }
    if current_parts.is_empty() {
        return Err("missing current profile parts; historical results cannot satisfy a run");
    }
    Ok(())
}

/// Validate identity, completeness and current measurements without evaluating limits a second
/// time. Limit comparison remains exclusively in the native runner's regression machinery.
fn validate_results(
    source: &Path,
    expected: &[String],
    summaries: &BenchmarkSummaries,
) -> Result<()> {
    let invalid = |message: String| Error::ManifestError(source.to_owned(), message);
    if expected.is_empty() || summaries.num_filtered != 0 {
        return Err(invalid(
            "all declared cases must be measured; filtering is unsupported".into(),
        )
        .into());
    }
    let required: HashSet<&str> = expected.iter().map(String::as_str).collect();
    if required.len() != expected.len() {
        return Err(invalid("duplicate required case IDs".into()).into());
    }
    let mut measured = HashSet::new();
    for summary in &summaries.summaries {
        let id = summary.function_name.as_str();
        if !required.contains(id)
            || summary.kind != BenchmarkKind::BinaryBenchmark
            || summary.group != GROUP_ID
            || summary.id.is_some()
            || summary.module_path != format!("{MODULE}::{GROUP_ID}::{id}")
        {
            return Err(invalid(format!("unexpected result identity for case '{id}'")).into());
        }
        if !measured.insert(id) {
            return Err(invalid(format!("duplicate result for case '{id}'")).into());
        }
        let [profile] = summary.profiles.0.as_slice() else {
            return Err(invalid(format!(
                "case '{id}' requires exactly one current Callgrind profile"
            ))
            .into());
        };
        validate_profile(profile).map_err(|message| invalid(format!("case '{id}': {message}")))?;
    }
    let missing: Vec<&str> = expected
        .iter()
        .map(String::as_str)
        .filter(|id| !measured.contains(id))
        .collect();
    if !missing.is_empty() {
        return Err(invalid(format!(
            "missing current results for cases: {}",
            missing.join(", ")
        ))
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use either_or_both::EitherOrBoth;
    use indexmap::IndexMap;
    use rstest::rstest;

    use super::*;
    use crate::metrics::model::{MetricResult, MetricResults};
    use crate::summary::model::{
        BenchmarkSummary, ProfileData, ProfilePart, ProfileTotal, ToolRun,
    };

    #[test]
    fn complete_current_results_pass() {
        let summaries = BenchmarkSummaries {
            summaries: vec![summary("Quote.Single.Owner"), summary("Pending.Quiet.Tick")],
            ..BenchmarkSummaries::default()
        };
        validate_results(
            Path::new("perf.toml"),
            &["Quote.Single.Owner".into(), "Pending.Quiet.Tick".into()],
            &summaries,
        )
        .unwrap();
    }

    #[test]
    fn duplicate_current_parts_fail_but_distinct_threads_pass() {
        let mut summary = summary("case");
        let profile = &mut summary.profiles.0[0];
        profile.data.parts.push(profile.data.parts[0].clone());
        assert!(validate_profile(profile).is_err());
        profile.data.parts[1]
            .tool_run
            .as_mut()
            .left()
            .unwrap()
            .thread = Some(2);
        validate_profile(profile).unwrap();
    }

    #[rstest]
    #[case::missing(vec![], 0)]
    #[case::duplicate(vec!["case", "case"], 0)]
    #[case::unexpected(vec!["other"], 0)]
    #[case::filtered(vec!["case"], 1)]
    fn incomplete_results_fail(#[case] ids: Vec<&str>, #[case] num_filtered: usize) {
        let summaries = BenchmarkSummaries {
            summaries: ids.into_iter().map(summary).collect(),
            num_filtered,
            ..BenchmarkSummaries::default()
        };
        assert!(validate_results(Path::new("perf.toml"), &["case".into()], &summaries).is_err());
    }

    #[rstest]
    #[case::zero(Some(EitherOrBoth::Left(Metric::Int(0))))]
    #[case::old_only(Some(EitherOrBoth::Right(Metric::Int(42))))]
    #[case::float(Some(EitherOrBoth::Left(Metric::Float(42.0))))]
    #[case::missing(None)]
    fn invalid_current_ir_fails(#[case] values: Option<EitherOrBoth<Metric>>) {
        let mut summary = summary("case");
        let profile = &mut summary.profiles.0[0];
        let ToolMetricResults::Callgrind(metrics) = &mut profile.data.total.metrics else {
            unreachable!()
        };
        metrics.0.clear();
        if let Some(values) = values {
            metrics.0.insert(
                EventKind::Ir,
                MetricResult {
                    change: None,
                    values,
                },
            );
        }
        assert!(validate_profile(profile).is_err());
    }

    #[test]
    fn only_old_parts_cannot_supply_current_measurement() {
        let mut summary = summary("case");
        let profile = &mut summary.profiles.0[0];
        let run = profile.data.parts[0]
            .tool_run
            .as_ref()
            .left()
            .unwrap()
            .clone();
        profile.data.parts[0].tool_run = EitherOrBoth::Right(run);
        assert!(validate_profile(profile).is_err());
    }

    fn summary(id: &str) -> BenchmarkSummary {
        let mut summary = BenchmarkSummary::new(
            BenchmarkKind::BinaryBenchmark,
            "/project".into(),
            "/project".into(),
            "perf.toml".into(),
            "workload".into(),
            &ModulePath::new(&format!("{MODULE}::{GROUP_ID}::{id}")),
            id,
            GROUP_ID,
            None,
            None,
            "target/gungraun".into(),
            (None, None),
        );
        let metrics = ToolMetricResults::Callgrind(MetricResults(IndexMap::from([(
            EventKind::Ir,
            MetricResult {
                change: None,
                values: EitherOrBoth::Left(Metric::Int(42)),
            },
        )])));
        summary.profiles.0.push(Profile::new(
            Tool::Callgrind,
            ProfileData {
                parts: vec![ProfilePart {
                    metrics: metrics.clone(),
                    tool_run: EitherOrBoth::Left(ToolRun {
                        command: "workload".into(),
                        output: None,
                        parent_pid: None,
                        part: Some(1),
                        pid: 1,
                        thread: Some(1),
                    }),
                }],
                total: ProfileTotal {
                    metrics,
                    regressions: Vec::new(),
                },
            },
        ));
        summary
    }
}
