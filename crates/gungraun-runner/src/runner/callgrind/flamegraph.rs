//! Module containing the Callgrind flamegraph elements

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufWriter, Cursor, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use inferno::flamegraph::{Direction, Options};
use tempfile::TempDir;

use super::flamegraph_parser::{FlamegraphMap, FlamegraphParser};
use super::parser::{CallgrindParser, CallgrindProperties, Sentinel};
use crate::api::{self, EventKind, FlamegraphKind};
use crate::runner::tool::path::{ToolOutputPath, ToolOutputPathKind};
use crate::summary::model::{BaselineKind, BaselineName, FlamegraphSummaries, FlamegraphSummary};

type ParserOutput = Vec<(PathBuf, CallgrindProperties, FlamegraphMap)>;

const DEFAULT_DIRECTION: Direction = Direction::Inverted;
const DEFAULT_EVENT_KIND: EventKind = EventKind::Ir;
const DEFAULT_FLAMEGRAPH_KIND: FlamegraphKind = FlamegraphKind::All;
const DEFAULT_MIN_WIDTH: f64 = 0.1;
const DEFAULT_NEGATE_DIFFERENTIAL: bool = false;
const DEFAULT_NORMALIZE_DIFFERENTIAL: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputPathKind {
    Regular,
    Old,
    Base(String),
    DiffOld,
    DiffBase(String),
    DiffBases(String, String),
}

/// The generator for flamegraphs when not run with --load-baseline or --save-baseline
#[derive(Debug)]
pub struct BaselineFlamegraphGenerator {
    /// The [`BaselineKind`]
    pub baseline_kind: BaselineKind,
}

/// The generator for flamegraphs when comparing one baseline and saving as another baseline.
#[derive(Debug)]
pub struct BaselineAndSaveFlamegraphGenerator {
    /// The baseline to compare with.
    pub baseline: BaselineName,
    /// The baseline used to save the current run.
    pub save_baseline: BaselineName,
}

/// The main configuration for a flamegraph
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// The direction of the flamegraph. Top to bottom or vice versa
    pub direction: Direction,
    /// The event kinds for which a flamegraph should be generated
    pub event_kinds: Vec<EventKind>,
    /// The [`FlamegraphKind`]
    pub kind: FlamegraphKind,
    /// The minimum width which should be displayed
    pub min_width: f64,
    /// If true, negate a differential flamegraph
    pub negate_differential: bool,
    /// If true, normalize a differential flamegraph
    pub normalize_differential: bool,
    /// The subtitle to use for the flamegraphs
    pub subtitle: Option<String>,
    /// The title to use for the flamegraphs
    pub title: Option<String>,
}

/// The generated callgrind `Flamegraph`
#[derive(Debug, Clone)]
pub struct Flamegraph {
    /// The [`Config`]
    pub config: Config,
}

/// The generator for flamegraphs when run with --load-baseline
#[derive(Debug)]
pub struct LoadBaselineFlamegraphGenerator {
    /// The baseline to compare with
    pub baseline: BaselineName,
    /// The loaded baseline with [`BaselineName`]
    pub loaded_baseline: BaselineName,
}

#[derive(Debug, Clone)]
struct OutputPath {
    pub baseline_kind: BaselineKind,
    pub dir: PathBuf,
    pub event_kind: EventKind,
    pub kind: OutputPathKind,
    pub modifiers: Vec<String>,
    pub name: String,
    pub temp: Option<Arc<TempDir>>,
}

/// The generator for flamegraphs when run with --save-baseline
#[derive(Debug)]
pub struct SaveBaselineFlamegraphGenerator {
    /// The baseline with [`BaselineName`]
    pub baseline: BaselineName,
}

/// The trait a flamegraph generator needs to implement
pub trait FlamegraphGenerator {
    /// Creates a new flamegraph generator.
    fn create(
        &self,
        flamegraph: &Flamegraph,
        tool_output_path: &ToolOutputPath,
        sentinel: Option<&Sentinel>,
        project_root: &Path,
    ) -> Result<Vec<FlamegraphSummary>>;
}

impl FlamegraphGenerator for BaselineFlamegraphGenerator {
    fn create(
        &self,
        flamegraph: &Flamegraph,
        tool_output_path: &ToolOutputPath,
        sentinel: Option<&Sentinel>,
        project_root: &Path,
    ) -> Result<Vec<FlamegraphSummary>> {
        // We need the dummy path just to clean up and organize the output files independently of
        // the EventKind of the OutputPath
        let mut output_path = OutputPath::new(tool_output_path, EventKind::Ir);
        output_path.init()?;
        output_path.to_diff_path().clear(true)?;
        output_path.shift(true)?;
        output_path.set_modifiers(["total"]);

        if flamegraph.config.kind == FlamegraphKind::None
            || flamegraph.config.event_kinds.is_empty()
        {
            return Ok(vec![]);
        }

        let (maps, base_maps) =
            flamegraph.parse(tool_output_path, sentinel, project_root, false)?;

        let total = total_flamegraph_map_from_parsed(&maps).unwrap();

        let mut flamegraph_summaries = FlamegraphSummaries::default();
        for event_kind in &flamegraph.config.event_kinds {
            let mut flamegraph_summary = FlamegraphSummary::new(*event_kind);
            output_path.set_event_kind(*event_kind);

            let stacks_lines = total.to_stack_format(event_kind)?;
            if flamegraph.is_regular() {
                Flamegraph::write(
                    &output_path,
                    &mut flamegraph.options(*event_kind, output_path.file_name()),
                    stacks_lines.iter().map(std::string::String::as_str),
                )?;
                flamegraph_summary.regular_path = Some(output_path.to_path());
            }

            if let Some(base_maps) = &base_maps {
                let total_base = total_flamegraph_map_from_parsed(base_maps).unwrap();
                // Is Some if FlamegraphKind::Differential or FlamegraphKind::All
                Flamegraph::create_differential(
                    &output_path,
                    &mut flamegraph.options(*event_kind, output_path.to_diff_path().file_name()),
                    &total_base,
                    // This unwrap is safe since we always have differential options if the
                    // flamegraph kind is differential
                    flamegraph.differential_options().unwrap(),
                    *event_kind,
                    &stacks_lines,
                )?;

                flamegraph_summary.base_path = Some(output_path.to_base_path().to_path());
                flamegraph_summary.diff_path = Some(output_path.to_diff_path().to_path());
            }

            flamegraph_summaries.totals.push(flamegraph_summary);
        }

        Ok(flamegraph_summaries.totals)
    }
}

impl FlamegraphGenerator for BaselineAndSaveFlamegraphGenerator {
    fn create(
        &self,
        flamegraph: &Flamegraph,
        tool_output_path: &ToolOutputPath,
        sentinel: Option<&Sentinel>,
        project_root: &Path,
    ) -> Result<Vec<FlamegraphSummary>> {
        debug_assert_eq!(tool_output_path.baseline_name(), Some(&self.baseline));
        debug_assert_eq!(
            tool_output_path.loaded_baseline_name(),
            Some(self.save_baseline.clone())
        );

        let mut output_path = OutputPath::new(tool_output_path, EventKind::Ir);
        output_path.init()?;
        output_path.clear(true)?;
        output_path.to_diff_path().clear_diffs()?;
        output_path.set_modifiers(["total"]);

        if flamegraph.config.kind == FlamegraphKind::None
            || flamegraph.config.event_kinds.is_empty()
        {
            return Ok(vec![]);
        }

        let (maps, base_maps) =
            flamegraph.parse(tool_output_path, sentinel, project_root, false)?;
        let Some(total) = total_flamegraph_map_from_parsed(&maps) else {
            return Ok(vec![]);
        };
        let base_total = base_maps
            .as_ref()
            .and_then(total_flamegraph_map_from_parsed);

        let mut flamegraph_summaries = FlamegraphSummaries::default();
        for event_kind in &flamegraph.config.event_kinds {
            let mut flamegraph_summary = FlamegraphSummary::new(*event_kind);
            output_path.set_event_kind(*event_kind);

            let stacks_lines = total.to_stack_format(event_kind)?;
            if flamegraph.is_regular() {
                Flamegraph::write(
                    &output_path,
                    &mut flamegraph.options(*event_kind, output_path.file_name()),
                    stacks_lines.iter().map(String::as_str),
                )?;
                flamegraph_summary.regular_path = Some(output_path.to_path());
            }

            if let Some(base_total) = &base_total {
                Flamegraph::create_differential(
                    &output_path,
                    &mut flamegraph.options(*event_kind, output_path.to_diff_path().file_name()),
                    base_total,
                    flamegraph.differential_options().expect(
                        "differential options should be present when creating a differential \
                         flamegraph",
                    ),
                    *event_kind,
                    &stacks_lines,
                )?;

                flamegraph_summary.base_path = Some(output_path.to_base_path().to_path());
                flamegraph_summary.diff_path = Some(output_path.to_diff_path().to_path());
            }

            flamegraph_summaries.totals.push(flamegraph_summary);
        }

        Ok(flamegraph_summaries.totals)
    }
}

impl From<api::FlamegraphConfig> for Config {
    fn from(value: api::FlamegraphConfig) -> Self {
        Self {
            kind: value.kind.unwrap_or(DEFAULT_FLAMEGRAPH_KIND),
            negate_differential: value
                .negate_differential
                .unwrap_or(DEFAULT_NEGATE_DIFFERENTIAL),
            normalize_differential: value
                .normalize_differential
                .unwrap_or(DEFAULT_NORMALIZE_DIFFERENTIAL),
            event_kinds: value
                .event_kinds
                .unwrap_or_else(|| vec![DEFAULT_EVENT_KIND]),
            direction: value
                .direction
                .map_or_else(|| DEFAULT_DIRECTION, std::convert::Into::into),
            title: value.title.clone(),
            subtitle: value.subtitle.clone(),
            min_width: value.min_width.unwrap_or(DEFAULT_MIN_WIDTH),
        }
    }
}

impl From<api::Direction> for Direction {
    fn from(value: api::Direction) -> Self {
        match value {
            api::Direction::TopToBottom => Self::Inverted,
            api::Direction::BottomToTop => Self::Straight,
        }
    }
}

impl Flamegraph {
    /// Creates a new `Flamegraph`.
    pub fn new(heading: String, mut config: Config) -> Self {
        if config.title.is_none() {
            config.title = Some(heading);
        }

        Self { config }
    }

    /// Returns `true` if this flamegraph is a differential flamegraph.
    pub fn is_differential(&self) -> bool {
        matches!(
            self.config.kind,
            FlamegraphKind::Differential | FlamegraphKind::All
        )
    }

    /// Returns `true` if this flamegraph is a regular flamegraph.
    pub fn is_regular(&self) -> bool {
        matches!(
            self.config.kind,
            FlamegraphKind::Regular | FlamegraphKind::All
        )
    }

    /// Returns the [`Options`] of this flamegraph.
    pub fn options(&self, event_kind: EventKind, subtitle: String) -> Options<'_> {
        let mut options = Options::default();
        options.negate_differentials = self.config.negate_differential;
        options.direction = self.config.direction;
        options.title.clone_from(
            self.config
                .title
                .as_ref()
                .expect("A title must be present at this point"),
        );

        options.subtitle = if let Some(subtitle) = &self.config.subtitle {
            Some(subtitle.clone())
        } else {
            Some(subtitle)
        };

        options.min_width = self.config.min_width;
        options.count_name = event_kind.to_string();
        options
    }

    /// Returns the [`inferno::differential::Options`] for a differential flamegraph.
    pub fn differential_options(&self) -> Option<inferno::differential::Options> {
        self.is_differential()
            .then(|| inferno::differential::Options {
                normalize: self.config.normalize_differential,
                ..Default::default()
            })
    }

    /// Parse the flamegraph
    pub fn parse<P>(
        &self,
        tool_output_path: &ToolOutputPath,
        sentinel: Option<&Sentinel>,
        project_root: P,
        no_differential: bool,
    ) -> Result<(ParserOutput, Option<ParserOutput>)>
    where
        P: Into<PathBuf>,
    {
        let parser = FlamegraphParser::new(sentinel, project_root);
        // We need this map in all remaining cases of `FlamegraphKinds`
        let mut maps = parser.parse(tool_output_path)?;

        let base_path = tool_output_path.to_base_path();
        let mut base_maps = (!no_differential && self.is_differential() && base_path.exists())
            .then(|| parser.parse(&base_path))
            .transpose()?;

        if self.config.event_kinds.iter().any(EventKind::is_derived) {
            for map in &mut maps {
                map.2.make_summary()?;
            }
            if let Some(maps) = base_maps.as_mut() {
                for map in maps {
                    map.2.make_summary()?;
                }
            }
        }

        Ok((maps, base_maps))
    }

    fn create_differential(
        output_path: &OutputPath,
        options: &mut inferno::flamegraph::Options,
        base_map: &FlamegraphMap,
        differential_options: inferno::differential::Options,
        event_kind: EventKind,
        stacks_lines: &[String],
    ) -> Result<()> {
        let base_stacks_lines = base_map.to_stack_format(&event_kind)?;

        let cursor = Cursor::new(stacks_lines.join("\n"));
        let base_cursor = Cursor::new(base_stacks_lines.join("\n"));
        let mut result = Cursor::new(vec![]);

        inferno::differential::from_readers(differential_options, base_cursor, cursor, &mut result)
            .context("Failed creating a differential flamegraph")?;

        let diff_output_path = output_path.to_diff_path();
        Self::write(
            &diff_output_path,
            options,
            String::from_utf8_lossy(result.get_ref()).lines(),
        )
    }

    fn write<'stacks>(
        output_path: &OutputPath,
        options: &mut Options<'_>,
        stacks: impl Iterator<Item = &'stacks str>,
    ) -> Result<()> {
        let path = output_path.to_path();
        let mut writer = BufWriter::new(output_path.create()?);
        inferno::flamegraph::from_lines(options, stacks, &mut writer)
            .with_context(|| format!("Failed creating a flamegraph at '{}'", path.display()))?;

        writer
            .flush()
            .with_context(|| format!("Failed flushing content to '{}'", path.display()))
    }
}

impl FlamegraphGenerator for LoadBaselineFlamegraphGenerator {
    fn create(
        &self,
        flamegraph: &Flamegraph,
        tool_output_path: &ToolOutputPath,
        sentinel: Option<&Sentinel>,
        project_root: &Path,
    ) -> Result<Vec<FlamegraphSummary>> {
        // We need the dummy path just to clean up and organize the output files independently of
        // the EventKind of the OutputPath
        let mut output_path = OutputPath::new(tool_output_path, EventKind::Ir);

        if flamegraph.config.kind == FlamegraphKind::None
            || flamegraph.config.event_kinds.is_empty()
            || !flamegraph.is_differential()
        {
            return Ok(vec![]);
        }

        output_path.to_diff_path().clear(true)?;
        output_path.set_modifiers(["total"]);

        let (maps, base_maps) = flamegraph
            .parse(tool_output_path, sentinel, project_root, false)
            .map(|(a, b)| (a, b.unwrap()))?;

        let mut flamegraph_summaries = FlamegraphSummaries::default();
        if let Some(total) = total_flamegraph_map_from_parsed(&maps) {
            let base_total = total_flamegraph_map_from_parsed(&base_maps);

            if let Some(base_total) = base_total {
                for event_kind in &flamegraph.config.event_kinds {
                    let mut flamegraph_summary = FlamegraphSummary::new(*event_kind);
                    output_path.set_event_kind(*event_kind);

                    Flamegraph::create_differential(
                        &output_path,
                        &mut flamegraph
                            .options(*event_kind, output_path.to_diff_path().file_name()),
                        &base_total,
                        // This unwrap is safe since we always produce a differential flamegraph
                        flamegraph.differential_options().unwrap(),
                        *event_kind,
                        &total.to_stack_format(event_kind)?,
                    )?;

                    flamegraph_summary.regular_path = Some(output_path.to_path());
                    flamegraph_summary.base_path = Some(output_path.to_base_path().to_path());
                    flamegraph_summary.diff_path = Some(output_path.to_diff_path().to_path());

                    flamegraph_summaries.totals.push(flamegraph_summary);
                }
            }
        }

        Ok(flamegraph_summaries.totals)
    }
}

impl OutputPath {
    pub fn new(tool_output_path: &ToolOutputPath, event_kind: EventKind) -> Self {
        Self {
            kind: match &tool_output_path.kind {
                ToolOutputPathKind::Out
                | ToolOutputPathKind::Data
                | ToolOutputPathKind::Log
                | ToolOutputPathKind::Xtree
                | ToolOutputPathKind::Xleak => OutputPathKind::Regular,
                ToolOutputPathKind::OldOut
                | ToolOutputPathKind::OldData
                | ToolOutputPathKind::OldLog
                | ToolOutputPathKind::OldXtree
                | ToolOutputPathKind::OldXleak => OutputPathKind::Old,
                ToolOutputPathKind::BaseLog(name)
                | ToolOutputPathKind::BaseData(name)
                | ToolOutputPathKind::BaseOut(name)
                | ToolOutputPathKind::BaseXtree(name)
                | ToolOutputPathKind::BaseXleak(name) => OutputPathKind::Base(name.clone()),
            },
            event_kind,
            baseline_kind: tool_output_path.baseline_kind.clone(),
            dir: tool_output_path.dir.clone(),
            name: tool_output_path.name.clone(),
            modifiers: Vec::default(),
            temp: tool_output_path.temp.clone(),
        }
    }

    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir).with_context(|| {
            format!(
                "Failed creating flamegraph directory '{}'",
                self.dir.display()
            )
        })
    }

    pub fn create(&self) -> Result<File> {
        let path = self.to_path();
        File::create(&path)
            .with_context(|| format!("Failed creating flamegraph file '{}'", path.display()))
    }

    pub fn clear(&self, ignore_event_kind: bool) -> Result<()> {
        for path in self.sanitized_paths_in(&self.dir, ignore_event_kind)? {
            std::fs::remove_file(path)?;
        }

        Ok(())
    }

    /// Removes differential flamegraph files for this output path's baseline.
    ///
    /// # Errors
    ///
    /// Propagates filesystem errors encountered while reading the output directory or removing
    /// matching flamegraph files.
    pub fn clear_diffs(&self) -> Result<()> {
        let extension = match &self.kind {
            OutputPathKind::DiffOld => (None, "diff.old.svg".to_owned()),
            OutputPathKind::DiffBase(name) => {
                let basename = format!("base@{name}");
                let extension = format!("diff.{basename}.svg");
                (Some(basename), extension)
            }
            OutputPathKind::DiffBases(first, _) => {
                let first_basename = format!("base@{first}");
                let first_extension = format!("diff.{first_basename}.svg");
                (Some(first_basename), first_extension)
            }
            _ => return Ok(()),
        };

        for entry in std::fs::read_dir(&self.dir)
            .with_context(|| format!("Failed reading directory '{}'", self.dir.display()))?
        {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if let Some(suffix) =
                file_name.strip_prefix(format!("callgrind.{}", self.name).as_str())
            {
                let path = entry.path();

                let (basename, extension) = &extension;
                if suffix.ends_with(extension.as_str()) {
                    std::fs::remove_file(&path).with_context(|| {
                        format!("Failed removing flamegraph file: '{}'", path.display())
                    })?;
                    continue;
                }

                if let Some(name) = &basename {
                    if suffix
                        .split('.')
                        .skip_while(|p| *p != "flamegraph")
                        .take(3)
                        .eq(["flamegraph".to_owned(), name.to_owned(), "diff".to_owned()])
                    {
                        std::fs::remove_file(&path).with_context(|| {
                            format!(
                                "Failed removing other flamegraph file: '{}'",
                                path.display()
                            )
                        })?;
                    }
                } else {
                    // do nothing
                }
            }
        }

        Ok(())
    }

    pub fn shift(&self, ignore_event_kind: bool) -> Result<()> {
        match &self.baseline_kind {
            BaselineKind::Old => {
                self.to_base_path().clear(ignore_event_kind)?;
                for path in self.sanitized_paths_in(&self.dir, ignore_event_kind)? {
                    let new_path = path.with_extension("old.svg");
                    std::fs::rename(&path, &new_path).with_context(|| {
                        format!(
                            "Failed moving flamegraph file from '{}' to '{}'",
                            path.display(),
                            new_path.display()
                        )
                    })?;
                }
                Ok(())
            }
            BaselineKind::Name(_) => self.clear(ignore_event_kind),
        }
    }

    pub fn to_diff_path(&self) -> Self {
        Self {
            kind: match (&self.kind, &self.baseline_kind) {
                (OutputPathKind::Regular, BaselineKind::Old) => OutputPathKind::DiffOld,
                (OutputPathKind::Regular, BaselineKind::Name(name)) => {
                    OutputPathKind::DiffBase(name.to_string())
                }
                (OutputPathKind::Base(name), BaselineKind::Name(other)) => {
                    OutputPathKind::DiffBases(name.clone(), other.to_string())
                }
                (OutputPathKind::Old | OutputPathKind::Base(_), _) => unreachable!(),
                (value, _) => value.clone(),
            },
            ..self.clone()
        }
    }

    pub fn to_base_path(&self) -> Self {
        Self {
            kind: match &self.baseline_kind {
                BaselineKind::Old => OutputPathKind::Old,
                BaselineKind::Name(name) => OutputPathKind::Base(name.to_string()),
            },
            ..self.clone()
        }
    }

    pub fn extension(&self) -> String {
        match &self.kind {
            OutputPathKind::Regular => format!("{}.flamegraph.svg", self.event_kind.to_name()),
            OutputPathKind::Old => format!("{}.flamegraph.old.svg", self.event_kind.to_name()),
            OutputPathKind::Base(name) => {
                format!("{}.flamegraph.base@{name}.svg", self.event_kind.to_name())
            }
            OutputPathKind::DiffOld => {
                format!("{}.flamegraph.diff.old.svg", self.event_kind.to_name())
            }
            OutputPathKind::DiffBase(name) => {
                format!(
                    "{}.flamegraph.diff.base@{name}.svg",
                    self.event_kind.to_name()
                )
            }
            OutputPathKind::DiffBases(name, base) => {
                format!(
                    "{}.flamegraph.base@{name}.diff.base@{base}.svg",
                    self.event_kind.to_name()
                )
            }
        }
    }

    pub fn set_modifiers<I, T>(&mut self, modifiers: T)
    where
        T: IntoIterator<Item = I>,
        I: Into<String>,
    {
        self.modifiers = modifiers.into_iter().map(Into::into).collect();
    }

    pub fn set_event_kind(&mut self, event_kind: EventKind) {
        self.event_kind = event_kind;
    }

    pub fn dest_dir(&self) -> &Path {
        if let Some(temp) = &self.temp {
            temp.path()
        } else {
            &self.dir
        }
    }

    pub fn sanitized_paths_in(&self, dir: &Path, ignore_event_kind: bool) -> Result<Vec<PathBuf>> {
        let extension = self.extension();
        let to_match = if ignore_event_kind {
            extension
                .split_once('.')
                .expect("The '.' delimiter should be present at least once")
                .1
        } else {
            &extension
        };

        let mut paths = vec![];
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("Failed reading directory '{}'", dir.display()))?
        {
            let path = entry?;
            let file_name = path.file_name().to_string_lossy().to_string();
            if let Some(suffix) =
                file_name.strip_prefix(format!("callgrind.{}.", self.name).as_str())
                && suffix.ends_with(to_match)
            {
                paths.push(path.path());
            }
        }

        Ok(paths)
    }

    pub fn file_name(&self) -> String {
        if self.modifiers.is_empty() {
            format!("callgrind.{}.{}", self.name, self.extension())
        } else {
            format!(
                "callgrind.{}.{}.{}",
                self.name,
                self.modifiers.join("."),
                self.extension()
            )
        }
    }

    pub fn to_path(&self) -> PathBuf {
        self.dest_dir().join(self.file_name())
    }
}

impl FlamegraphGenerator for SaveBaselineFlamegraphGenerator {
    fn create(
        &self,
        flamegraph: &Flamegraph,
        tool_output_path: &ToolOutputPath,
        sentinel: Option<&Sentinel>,
        project_root: &Path,
    ) -> Result<Vec<FlamegraphSummary>> {
        // We need the dummy path just to clean up and organize the output files independently of
        // the EventKind of the OutputPath
        let mut output_path = OutputPath::new(tool_output_path, EventKind::Ir);
        output_path.init()?;
        output_path.clear(true)?;
        output_path.to_diff_path().clear_diffs()?;
        output_path.set_modifiers(["total"]);

        if flamegraph.config.kind == FlamegraphKind::None
            || flamegraph.config.event_kinds.is_empty()
            || !flamegraph.is_regular()
        {
            return Ok(vec![]);
        }

        let (maps, _) = flamegraph.parse(tool_output_path, sentinel, project_root, true)?;
        let total_map = total_flamegraph_map_from_parsed(&maps).unwrap();

        let mut flamegraph_summaries = FlamegraphSummaries::default();
        for event_kind in &flamegraph.config.event_kinds {
            let mut flamegraph_summary = FlamegraphSummary::new(*event_kind);
            output_path.set_event_kind(*event_kind);

            Flamegraph::write(
                &output_path,
                &mut flamegraph.options(*event_kind, output_path.file_name()),
                total_map
                    .to_stack_format(event_kind)?
                    .iter()
                    .map(String::as_str),
            )?;

            flamegraph_summary.regular_path = Some(output_path.to_path());
            flamegraph_summaries.summaries.push(flamegraph_summary);
        }

        Ok(flamegraph_summaries.totals)
    }
}

fn total_flamegraph_map_from_parsed(maps: &ParserOutput) -> Option<Cow<'_, FlamegraphMap>> {
    match maps.len().cmp(&1) {
        Ordering::Less => None,
        Ordering::Equal => Some(Cow::Borrowed(&maps[0].2)),
        Ordering::Greater => {
            let mut total = maps[0].2.clone();
            for (_, _, map) in maps.iter().skip(1) {
                total.add(map);
            }
            Some(Cow::Owned(total))
        }
    }
}
