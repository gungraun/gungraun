//! Module containing the parser for callgrind flamegraphs
use std::collections::BinaryHeap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use log::debug;

use super::hashmap_parser::{CallgrindMap, HashMapParser, SourcePath};
use super::parser::{CallgrindParser, CallgrindProperties, Sentinel};
use crate::api::EventKind;
use crate::metrics::model::Metric;

/// The `FlamegraphMap` based on a [`CallgrindMap`]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlamegraphMap(CallgrindMap);

/// The parser for flamegraphs
#[derive(Debug)]
pub struct FlamegraphParser {
    project_root: PathBuf,
    sentinel: Option<Sentinel>,
}

#[derive(Debug, PartialEq, Eq)]
struct HeapElem {
    cost: Metric,
    source: String,
}

impl FlamegraphMap {
    /// Returns `true` if this map is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Calculate the cache summary for each entry in the map in-place
    pub fn make_summary(&mut self) -> Result<()> {
        let mut iter = self.0.map.values_mut().peekable();
        if let Some(value) = iter.peek() {
            // If one cost can be summarized then all costs can be summarized.
            if value.metrics.can_summarize() {
                for value in iter {
                    value
                        .metrics
                        .make_summary()
                        .map_err(|error| anyhow!("Failed calculating summary events: {error}"))?;
                }
            }
        }

        Ok(())
    }

    /// Sum this map with another map
    pub fn add(&mut self, other: &Self) {
        for (other_id, other_value) in &other.0 {
            // The performance of HashMap::entry is worse than the following method because we have
            // a heavy id which needs to be cloned, although it is already present in the map.
            if let Some(value) = self.0.map.get_mut(other_id) {
                value.metrics.add(&other_value.metrics);
            } else {
                self.0.map.insert(other_id.clone(), other_value.clone());
            }
        }
    }

    /// Convert to stacks string format for this `EventType`
    ///
    /// # Errors
    ///
    /// If the event type was not present in the stacks
    pub fn to_stack_format(&self, event_kind: &EventKind) -> Result<Vec<String>> {
        if self.0.map.is_empty() {
            return Ok(vec![]);
        }

        let mut heap = BinaryHeap::new();
        let sentinel_value = self
            .0
            .sentinel_key
            .as_ref()
            .map(|key| {
                self.0
                    .map
                    .get(key)
                    .expect("Resolved sentinel must be present in map")
                    .metrics
                    .metric_by_kind(event_kind)
                    .ok_or_else(|| {
                        anyhow!(
                            "Failed creating flamegraph stack: Missing event type '{event_kind}'"
                        )
                    })
            })
            .transpose()?;

        for (id, value) in &self.0.map {
            let cost = value.metrics.metric_by_kind(event_kind).ok_or_else(|| {
                anyhow!("Failed creating flamegraph stack: Missing event type '{event_kind}'")
            })?;

            if let Some(reference_cost) = sentinel_value {
                if cost > reference_cost {
                    continue;
                }
            }

            let mut source = String::new();
            if let Some(file) = &id.file {
                match file {
                    SourcePath::Unknown => write!(source, "{}", id.func).unwrap(),
                    SourcePath::Rust(path)
                    | SourcePath::Relative(path)
                    | SourcePath::Absolute(path) => {
                        write!(source, "{}:{}", path.display(), id.func).unwrap();
                    }
                }
            } else {
                write!(source, "{}", id.func).unwrap();
            }

            if let Some(path) = &id.obj {
                match path {
                    SourcePath::Unknown => {}
                    SourcePath::Rust(path)
                    | SourcePath::Relative(path)
                    | SourcePath::Absolute(path) => {
                        write!(source, " [{}]", path.display()).unwrap();
                    }
                }
            }

            heap.push(HeapElem { cost, source });
        }

        let mut stacks: Vec<String> = vec![];
        let len = heap.len();
        if len > 1 {
            for window in heap.into_sorted_vec().windows(2) {
                // There is only the slice size of 2 possible due to the window size of 2
                if let [h1, h2] = window {
                    let stack = if let Some(last) = stacks.last() {
                        // This unwrap is safe since the space must be present due to stack format
                        let (split, _) = last.rsplit_once(' ').unwrap();
                        format!("{split};{} {}", h1.source, h1.cost - h2.cost)
                    } else {
                        format!("{} {}", h1.source, h1.cost - h2.cost)
                    };

                    stacks.push(stack);

                    // The last window needs to push the last element too
                    if stacks.len() == len - 1 {
                        // This unwrap is safe since we have a last element the moment we enter the
                        // `if` statement
                        let last = stacks.last().unwrap();
                        let (split, _) = last.rsplit_once(' ').unwrap();

                        let stack = format!("{split};{} {}", h2.source, h2.cost);
                        stacks.push(stack);
                    }
                }
            }
        } else {
            // This unwrap is safe since heap.len() == 1 here
            let elem = heap.pop().unwrap();
            stacks.push(format!("{} {}", elem.source, elem.cost));
        }
        Ok(stacks)
    }
}

impl FlamegraphParser {
    /// Creates a new `FlamegraphParser`.
    pub fn new<P>(sentinel: Option<&Sentinel>, project_root: P) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            sentinel: sentinel.cloned(),
            project_root: project_root.into(),
        }
    }
}

impl CallgrindParser for FlamegraphParser {
    type Output = FlamegraphMap;

    fn parse_single(&self, path: &Path) -> Result<(CallgrindProperties, Self::Output)> {
        debug!("Parsing flamegraph from file '{}'", path.display());

        let parser = HashMapParser {
            project_root: self.project_root.clone(),
            sentinel: self.sentinel.clone(),
        };

        parser
            .parse_single(path)
            .map(|(props, map)| (props, FlamegraphMap(map)))
    }
}

impl Ord for HeapElem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cost
            .cmp(&other.cost)
            .reverse()
            .then_with(|| self.source.cmp(&other.source))
    }
}

impl PartialOrd for HeapElem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
