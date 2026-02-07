//! Module containing the parser for callgrind flamegraphs
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use log::debug;

use super::hashmap_parser::{CallgrindMap, HashMapParser, Id, SourcePath};
use super::model::Metrics;
use super::parser::{CallgrindParser, CallgrindProperties, Sentinel};
use crate::api::EventKind;
use crate::runner::metrics::Metric;

/// The `FlamegraphMap` based on a [`CallgrindMap`]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlamegraphMap {
    callees: HashSet<Id>,
    costs: CallgrindMap,
    edges: HashMap<Id, HashMap<Id, Metrics>>,
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
        self.costs.is_empty()
    }

    /// Calculate the cache summary for each entry in the map in-place
    pub fn make_summary(&mut self) -> Result<()> {
        let mut iter = self.costs.map.values_mut().peekable();
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
        for (other_id, other_value) in &other.costs {
            // The performance of HashMap::entry is worse than the following method because we have
            // a heavy id which needs to be cloned, although it is already present in the map.
            if let Some(value) = self.costs.map.get_mut(other_id) {
                value.metrics.add(&other_value.metrics);
            } else {
                self.costs.map.insert(other_id.clone(), other_value.clone());
            }
        }

        for (caller, callee_map) in &other.edges {
            if let Some(entry) = self.edges.get_mut(caller) {
                for (callee, metrics) in callee_map {
                    if let Some(m) = entry.get_mut(callee) {
                        m.add(metrics);
                    } else {
                        entry.insert(callee.clone(), metrics.clone());
                    }
                }
            } else {
                self.edges.insert(caller.clone(), callee_map.clone());
            }
        }
        self.callees.extend(other.callees.iter().cloned());
    }

    /// Convert to stacks string format for this `EventType`
    ///
    /// # Errors
    ///
    /// If the event type was not present in the stacks
    pub fn to_stack_format(&self, event_kind: &EventKind) -> Result<Vec<String>> {
        if self.costs.map.is_empty() {
            return Ok(vec![]);
        }

        for (_id, value) in &self.costs {
            value.metrics.metric_by_kind(event_kind).ok_or_else(|| {
                anyhow!("Failed creating flamegraph stack: Missing event type '{event_kind}'")
            })?;
        }

        let roots: Vec<&Id> = if let Some(sentinel_key) = &self.costs.sentinel_key {
            vec![sentinel_key]
        } else {
            let mut roots: Vec<&Id> = self
                .costs
                .map
                .keys()
                .filter(|id| !self.callees.contains(id))
                .collect();
            roots.sort();
            roots
        };

        let mut stacks: Vec<String> = vec![];
        let mut visited = HashSet::new();
        for root in roots {
            self.dfs_emit(root, "", event_kind, None, &mut stacks, &mut visited);
        }

        Ok(stacks)
    }

    fn dfs_emit(
        &self,
        id: &Id,
        parent_stack: &str,
        event_kind: &EventKind,
        edge_cost: Option<Metric>,
        stacks: &mut Vec<String>,
        visited: &mut HashSet<Id>,
    ) {
        if !visited.insert(id.clone()) {
            return;
        }

        let inclusive = edge_cost.unwrap_or_else(|| {
            self.costs
                .map
                .get(id)
                .and_then(|v| v.metrics.metric_by_kind(event_kind))
                .unwrap_or(Metric::Int(0))
        });

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

        let current_stack = if parent_stack.is_empty() {
            source
        } else {
            format!("{parent_stack};{source}")
        };

        let mut children: Vec<(&Id, Metric)> = self
            .edges
            .get(id)
            .map(|m| {
                m.iter()
                    .filter_map(|(callee, edge_metrics)| {
                        edge_metrics.metric_by_kind(event_kind).map(|c| (callee, c))
                    })
                    .collect()
            })
            .unwrap_or_default();
        children.sort_by(|(id_a, cost_a), (id_b, cost_b)| {
            cost_b.cmp(cost_a).then_with(|| id_a.cmp(id_b))
        });

        let children_cost: Metric = children
            .iter()
            .map(|(_, c)| *c)
            .fold(Metric::Int(0), |acc, c| acc + c);

        stacks.push(format!("{} {}", current_stack, inclusive - children_cost));

        for (child_id, child_edge_cost) in &children {
            self.dfs_emit(
                child_id,
                &current_stack,
                event_kind,
                Some(*child_edge_cost),
                stacks,
                visited,
            );
        }

        visited.remove(id);
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

        let parser = HashMapParser {
            project_root: self.project_root.clone(),
            sentinel: self.sentinel.clone(),
        };

        let mut callees = HashSet::new();
        let mut edges: HashMap<Id, HashMap<Id, Metrics>> = HashMap::new();

        let (props, costs) = parser.parse_with_edges(path, |caller_id, callee_id, metrics| {
            if let Some(callee_map) = edges.get_mut(caller_id) {
                if let Some(m) = callee_map.get_mut(callee_id) {
                    m.add(metrics);
                } else {
                    callee_map.insert(callee_id.clone(), metrics.clone());
                }
            } else {
                let mut callee_map = HashMap::new();
                callee_map.insert(callee_id.clone(), metrics.clone());
                edges.insert(caller_id.clone(), callee_map);
            }
            if !callees.contains(callee_id) {
                callees.insert(callee_id.clone());
            }
        })?;

        Ok((
            props,
            FlamegraphMap {
                callees,
                costs,
                edges,
            },
        ))
    }
}
