//! A hashmap parser for callgrind output files

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hash_map::Iter;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, anyhow};
use log::trace;
use serde::{Deserialize, Serialize};

use super::model::Metrics;
use super::parser::{CallgrindParser, CallgrindProperties, Sentinel, parse_header};
use crate::error::Error;

/// The possible paths found in the output file
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SourcePath {
    /// The unknown path found as `???` in the output file
    Unknown,
    /// A rust path, starting with `/rustc`
    Rust(PathBuf),
    /// A relative path, not starting with a `/`
    Relative(PathBuf),
    /// A absolute path, starting with a `/`
    Absolute(PathBuf),
}

/// The `CallgrindMap`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallgrindMap {
    /// The actual data containing the mapping between the [`Id`] and the [`Value`]
    pub map: HashMap<Id, Value>,
    /// The optional [`Sentinel`]
    pub sentinel: Option<Sentinel>,
    /// The key which contained the [`Sentinel`]
    pub sentinel_key: Option<Id>,
}

#[derive(Debug, Default)]
struct CfnRecord {
    calls: u64,
    file: Option<SourcePath>,
    id: Option<Id>,
    obj: Option<SourcePath>,
}

#[derive(Debug, Default, Clone)]
struct CurrentId {
    file: Option<SourcePath>,
    func: Option<String>,
    obj: Option<SourcePath>,
}

/// Parse a callgrind outfile into a `HashMap`
///
/// This parser is a based on `callgrind_annotate` and how it summarizes the inclusive costs.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashMapParser {
    /// The project root directory required to make paths relative
    pub project_root: PathBuf,
    /// Look for this [`Sentinel`] in the output files
    pub sentinel: Option<Sentinel>,
}

/// The unique `Id` identifying a function uniquely
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Id {
    /// the file the function is found in
    pub file: Option<SourcePath>,
    /// The function
    pub func: String,
    /// The object the function is found in
    pub obj: Option<SourcePath>,
}

/// The `Value` to be associated with an [`Id`]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Value {
    /// The callgrind `Metrics` of this `Value`
    pub metrics: Metrics,
}

impl CallgrindMap {
    /// Returns `true` if this map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Return an iterator over this map
    pub fn iter(&self) -> Iter<'_, Id, Value> {
        self.map.iter()
    }

    /// The the key, value pair with the given [`Id`]
    pub fn get_key_value(&self, k: &Id) -> Option<(&Id, &Value)> {
        self.map.get_key_value(k)
    }

    /// Sum this map up with another map
    pub fn add_mut(&mut self, other: &Self) {
        for (other_key, other_value) in &other.map {
            if let Some(value) = self.map.get_mut(other_key) {
                value.metrics.add(&other_value.metrics);
            } else {
                self.map.insert(other_key.clone(), other_value.clone());
            }
        }
    }
}

impl<'a> IntoIterator for &'a CallgrindMap {
    type Item = (&'a Id, &'a Value);

    type IntoIter = Iter<'a, Id, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl From<Id> for CurrentId {
    fn from(value: Id) -> Self {
        Self {
            obj: value.obj,
            file: value.file,
            func: Some(value.func),
        }
    }
}

impl CallgrindParser for HashMapParser {
    type Output = CallgrindMap;

    #[expect(clippy::too_many_lines)]
    fn parse_single(&self, path: &Path) -> Result<(CallgrindProperties, Self::Output)> {
        let mut iter = BufReader::new(File::open(path)?).lines();
        let config = parse_header(&mut iter)
            .map_err(|error| Error::ParseError(path.to_owned(), error.to_string()))?;

        let mut current_id = CurrentId::default();
        let mut cfn_record = None;

        let mut cfn_totals = HashMap::<Id, Value>::new();
        let mut fn_totals = HashMap::<Id, Value>::new();

        // FIXME: This should be a vec. The sentinel can match many functions. This is only ok,
        // since we currently use the sentinel for the benchmark function exclusively. The benchmark
        // function is very special in that it is called exactly once, is not recursive etc.
        let mut sentinel_key = None;

        // We start within the header
        let mut is_header = true;
        for line in iter {
            let line = line?;
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // The first line which can be split around '=' is a non header line
            let split = if is_header {
                if let Some(split) = line.split_once('=') {
                    is_header = false;
                    Some(split)
                } else {
                    continue;
                }
            } else {
                line.split_once('=')
            };

            match split {
                Some(("ob", obj)) => {
                    current_id.obj = Some(make_path(&self.project_root, obj));
                }
                Some(("fl", file)) => {
                    current_id.file = Some(make_path(&self.project_root, file));
                }
                Some(("fn", func)) => {
                    current_id.func = Some(func.to_owned());

                    if self
                        .sentinel
                        .as_ref()
                        .is_some_and(|sentinel| sentinel.matches(func))
                    {
                        trace!("Found sentinel: {func}");
                        sentinel_key = Some(
                            current_id
                                .clone()
                                .try_into()
                                .map_err(|error| anyhow!("Malformed file: {error}"))?,
                        );
                    }
                }
                Some(("fi" | "fe", inline)) => {
                    current_id.file = Some(make_path(&self.project_root, inline));
                }
                Some(("cob", cob)) => {
                    let record = cfn_record.get_or_insert_with(CfnRecord::default);
                    record.obj = Some(make_path(&self.project_root, cob));
                }
                Some(("cfi" | "cfl", inline)) => {
                    let record = cfn_record.get_or_insert_with(CfnRecord::default);
                    record.file = Some(make_path(&self.project_root, inline));
                }
                Some(("cfn", cfn)) => {
                    let record = cfn_record.get_or_insert_with(CfnRecord::default);
                    record.id = Some(Id {
                        obj: record.obj.take().or_else(|| current_id.obj.clone()),
                        func: cfn.to_owned(),
                        file: record.file.take().or_else(|| current_id.file.clone()),
                    });
                }
                Some(("calls", calls)) => {
                    let record = cfn_record
                        .as_mut()
                        .ok_or_else(|| anyhow!("Malformed file: A cfn record should be present"))?;
                    record.calls = calls
                        .split_ascii_whitespace()
                        .next()
                        .ok_or_else(|| anyhow!("Missing calls count"))?
                        .parse::<u64>()?;
                }
                None if line.starts_with(|c: char| c.is_ascii_digit()) => {
                    let mut metrics = config.metrics_prototype.clone();
                    metrics.add_iter_str(
                        line.split_whitespace()
                            .skip(config.positions_prototype.len()),
                    )?;

                    if let Some(cfn_record) = cfn_record.take() {
                        let id = cfn_record.id.ok_or_else(|| {
                            anyhow!("Malformed file: A cfn record id should be present")
                        })?;
                        cfn_totals
                            .entry(id)
                            .and_modify(|value| value.metrics.add(&metrics))
                            .or_insert(Value {
                                metrics: metrics.clone(),
                            });
                    }

                    let id = current_id
                        .try_into()
                        .map_err(|error| anyhow!("Malformed file: {error}"))?;
                    match fn_totals.get_mut(&id) {
                        Some(value) => value.metrics.add(&metrics),
                        None => {
                            fn_totals.insert(id.clone(), Value { metrics });
                        }
                    }
                    current_id = id.into();
                }
                Some(("jump" | "jcnd" | "jfi" | "jfn", _)) => {
                    // we ignore these
                }
                None if line.starts_with("totals:") || line.starts_with("summary:") => {
                    // we ignore these
                }
                Some(_) | None => return Err(anyhow!("Malformed line: '{line}'")),
            }
        }

        // Correct inclusive totals
        for (key, value) in cfn_totals {
            fn_totals.insert(key, value);
        }

        Ok((
            config,
            CallgrindMap {
                map: fn_totals,
                sentinel: self.sentinel.clone(),
                sentinel_key,
            },
        ))
    }
}

impl TryFrom<CurrentId> for Id {
    type Error = String;

    fn try_from(value: CurrentId) -> std::result::Result<Self, Self::Error> {
        match value.func {
            Some(func) => Ok(Self {
                obj: value.obj,
                file: value.file,
                func,
            }),
            None => Err("Missing function".to_owned()),
        }
    }
}
impl Ord for SourcePath {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Unknown, Self::Unknown) => Ordering::Equal,
            (Self::Unknown, _) => Ordering::Less,
            (_, Self::Unknown) => Ordering::Greater,
            (
                Self::Rust(path) | Self::Relative(path) | Self::Absolute(path),
                Self::Rust(other_path) | Self::Relative(other_path) | Self::Absolute(other_path),
            ) => path.cmp(other_path),
        }
    }
}

impl PartialOrd for SourcePath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn make_path(root: &Path, source: &str) -> SourcePath {
    if source == "???" {
        SourcePath::Unknown
    } else {
        let path = PathBuf::from(source);
        match path.strip_prefix(root).ok() {
            Some(suffix) => SourcePath::Relative(suffix.to_owned()),
            None if path.is_absolute() => {
                let mut components = path.components().skip(1);
                if components.next() == Some(Component::Normal(OsStr::new("rustc"))) {
                    let mut new_path = PathBuf::from("/rustc");
                    if let Some(Component::Normal(string)) = components.next() {
                        new_path.push(string.to_string_lossy().chars().take(8).collect::<String>());
                    }
                    SourcePath::Rust(new_path.join(components.collect::<PathBuf>()))
                } else {
                    SourcePath::Absolute(path)
                }
            }
            None => SourcePath::Relative(path),
        }
    }
}
