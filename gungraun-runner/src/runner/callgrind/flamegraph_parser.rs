//! Module containing the parser for callgrind flamegraphs
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use log::debug;

use super::hashmap_parser::{parse_callgrind_output, CallgrindMap, Id, SourcePath};
use super::model::Metrics;
use super::parser::{CallgrindParser, CallgrindProperties, Sentinel};
use crate::api::EventKind;
use crate::runner::metrics::Metric;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CallGraph {
    /// caller → {callee → edge_metrics (inclusive cost of callee from this caller)}
    edges: HashMap<Id, HashMap<Id, Metrics>>,
    /// All Ids that appear as callees
    callees: HashSet<Id>,
}

/// The `FlamegraphMap` based on a [`CallgrindMap`]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlamegraphMap {
    map: CallgrindMap,
    call_graph: CallGraph,
}

/// The parser for flamegraphs
#[derive(Debug)]
pub struct FlamegraphParser {
    project_root: PathBuf,
    sentinel: Option<Sentinel>,
}

impl FlamegraphMap {
    /// Return true if this map is empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Calculate the cache summary for each entry in the map in-place
    pub fn make_summary(&mut self) -> Result<()> {
        let mut iter = self.map.map.values_mut().peekable();
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
        for (other_id, other_value) in &other.map {
            // The performance of HashMap::entry is worse than the following method because we have
            // a heavy id which needs to be cloned, although it is already present in the map.
            if let Some(value) = self.map.map.get_mut(other_id) {
                value.metrics.add(&other_value.metrics);
            } else {
                self.map.map.insert(other_id.clone(), other_value.clone());
            }
        }

        for (caller, callee_map) in &other.call_graph.edges {
            let entry = self.call_graph.edges.entry(caller.clone()).or_default();
            for (callee, metrics) in callee_map {
                entry
                    .entry(callee.clone())
                    .and_modify(|m| m.add(metrics))
                    .or_insert_with(|| metrics.clone());
            }
        }
        self.call_graph
            .callees
            .extend(other.call_graph.callees.iter().cloned());
    }

    /// Convert to stacks string format for this `EventType`
    ///
    /// # Errors
    ///
    /// If the event type was not present in the stacks
    pub fn to_stack_format(&self, event_kind: &EventKind) -> Result<Vec<String>> {
        if self.map.map.is_empty() {
            return Ok(vec![]);
        }

        let mut stacks: Vec<String> = vec![];

        let roots: Vec<&Id> = if let Some(sentinel_key) = &self.map.sentinel_key {
            vec![sentinel_key]
        } else {
            let mut roots: Vec<&Id> = self
                .map
                .map
                .keys()
                .filter(|id| !self.call_graph.callees.contains(id))
                .collect();
            roots.sort_by_cached_key(|id| format_source(id));
            roots
        };

        let mut visited = HashSet::new();
        for root in roots {
            self.dfs_emit(root, "", event_kind, None, &mut stacks, &mut visited)?;
        }

        Ok(stacks)
    }

    fn dfs_emit(
        &self,
        node: &Id,
        parent_stack: &str,
        event_kind: &EventKind,
        edge_cost: Option<Metric>,
        stacks: &mut Vec<String>,
        visited: &mut HashSet<Id>,
    ) -> Result<()> {
        if !visited.insert(node.clone()) {
            return Ok(());
        }

        let source = format_source(node);
        let current_stack = if parent_stack.is_empty() {
            source
        } else {
            format!("{parent_stack};{source}")
        };

        let inclusive = edge_cost.unwrap_or_else(|| {
            self.map
                .map
                .get(node)
                .and_then(|v| v.metrics.metric_by_kind(event_kind))
                .unwrap_or(Metric::Int(0))
        });

        let mut children: Vec<(&Id, Metric)> = self
            .call_graph
            .edges
            .get(node)
            .map(|m| {
                m.iter()
                    .filter_map(|(callee, edge_metrics)| {
                        edge_metrics
                            .metric_by_kind(event_kind)
                            .map(|c| (callee, c))
                    })
                    .collect()
            })
            .unwrap_or_default();
        children.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| format_source(a.0).cmp(&format_source(b.0)))
        });

        let children_cost: Metric = children
            .iter()
            .map(|(_, c)| *c)
            .fold(Metric::Int(0), |acc, c| acc + c);
        let self_cost = inclusive - children_cost;

        stacks.push(format!("{current_stack} {self_cost}"));

        for (child_id, child_edge_cost) in &children {
            self.dfs_emit(
                child_id,
                &current_stack,
                event_kind,
                Some(*child_edge_cost),
                stacks,
                visited,
            )?;
        }

        visited.remove(node);
        Ok(())
    }
}

impl FlamegraphParser {
    /// Create a new `FlamegraphParser`
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

        let mut call_graph = CallGraph::default();

        let (props, map) = parse_callgrind_output(
            path,
            &self.project_root,
            self.sentinel.as_ref(),
            |caller_id, callee_id, metrics| {
                call_graph
                    .edges
                    .entry(caller_id.clone())
                    .or_default()
                    .entry(callee_id.clone())
                    .and_modify(|m| m.add(metrics))
                    .or_insert_with(|| metrics.clone());
                call_graph.callees.insert(callee_id.clone());
            },
        )?;

        Ok((props, FlamegraphMap { map, call_graph }))
    }
}

fn format_source(id: &Id) -> String {
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

    source
}
