//! Module containing the dhat trees

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Add;

use polonius_the_crab::{ForLt, PoloniusResult, polonius};

use super::model::{Accesses, DhatData, DhatMetadata, Frame, Mode, ProgramPoint};
use crate::api::DhatMetric;
use crate::metrics::model::Metrics;
use crate::summary::model::ToolMetrics;

/// The [`Data`] of each [`Node`]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Data {
    /// Per-byte access histogram data from DHAT program points.
    ///
    /// The data is stored expanded while building trees so child nodes can be aggregated with
    /// DHAT's access histogram semantics. It is compacted back to DHAT's run-length encoding
    /// when a tree node is serialized as a program point.
    pub accesses: Option<Accesses>,
    /// The blocks at t-end
    pub blocks_at_end: Option<u64>,
    /// The blocks at t-gmax
    pub blocks_at_max: Option<u64>,
    /// The reads of blocks
    pub blocks_read: Option<u64>,
    /// The writes of blocks
    pub blocks_write: Option<u64>,
    /// The bytes at t-end
    pub bytes_at_end: Option<u64>,
    /// The bytes at t-gmax
    pub bytes_at_max: Option<u64>,
    /// The maximum blocks
    pub maximum_blocks: Option<u64>,
    /// The maximum bytes
    pub maximum_bytes: Option<u64>,
    /// The total blocks
    pub total_blocks: u64,
    /// The total bytes
    pub total_bytes: u64,
    /// Total lifetimes of all blocks allocated
    pub total_lifetimes: Option<u64>,
}

/// A full-fledged dhat prefix tree
///
/// This tree aggregates the dhat data of child nodes into the current node.
///
/// # Developers
///
/// This tree is currently not used but it is fully functional. However, only `insert` is
/// implemented to be able to build the tree but it may be needed to add methods like `remove`,
/// `lookup`, etc.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DhatTree {
    metadata: DhatMetadata,
    root: Box<Node>,
    table: BTreeMap<usize, Frame>,
}

/// The [`Node`] in a [`Tree`]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Node {
    children: Vec<Self>,
    data: Data,
    orig_data: Option<Data>,
    prefix: Vec<usize>,
}

/// A [`Tree`] without any leafs. Useful if only the root data and metrics are of interest.
///
/// If you're just interested in the data of the root then it is more performant to use this tree
/// instead of building a complete [`DhatTree`]. The dhat metrics of the root are the summarized
/// metrics of all its children, so all this [`Tree`] does is summarizing the metrics without
/// actually building the tree.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RootTree {
    metadata: DhatMetadata,
    root: Box<Node>,
    table: BTreeMap<usize, Frame>,
}

/// The trait to be implemented for a dhat prefix tree
pub trait Tree: Into<DhatData> {
    /// Returns the frame table referenced by tree nodes.
    fn frame_table(&self) -> &BTreeMap<usize, Frame>;

    /// Creates a new `Tree` from the given parameters.
    fn from_json(dhat_data: DhatData) -> Self
    where
        Self: std::marker::Sized + Default,
    {
        let DhatData {
            metadata,
            program_points,
            frame_table,
        } = dhat_data;

        let mut tree = Self::with_metadata(metadata);
        if program_points.is_empty() {
            tree.set_root_data(Data::zero());
        } else {
            tree.insert_iter(program_points.into_iter(), &frame_table);
        }

        tree
    }

    /// Insert a prefix with the given [`Data`] into this [`Tree`]
    fn insert(&mut self, prefix: &[usize], data: &Data, table: &[Frame]);

    /// Insert all [`ProgramPoint`]s into this [`Tree`]
    fn insert_iter(&mut self, iter: impl Iterator<Item = ProgramPoint>, table: &[Frame]) {
        for elem in iter {
            let data = Data::from(&elem);
            self.insert(&elem.frames, &data, table);
        }
    }

    /// Returns a mapping from original frame indices to compacted frame-table indices.
    fn mapping_table(&self) -> Vec<usize> {
        self.frame_table()
            .last_key_value()
            .map_or_else(Vec::new, |(max, _)| {
                self.frame_table().iter().enumerate().fold(
                    vec![0; max + 1],
                    |mut mapping_table, (index, (p, _))| {
                        mapping_table[*p] = index;
                        mapping_table
                    },
                )
            })
    }

    /// Returns the dhat metadata.
    fn metadata(&self) -> &DhatMetadata;

    /// Returns the metrics of the root node.
    fn metrics(&self) -> ToolMetrics {
        self.root_data().metrics(self.metadata().mode)
    }

    /// Returns the [`Data`] of the root.
    fn root_data(&self) -> &Data;

    /// Returns the [`Data`] of the root.
    fn root_node(self) -> Box<Node>;

    /// Set the [`Data`] of the root
    fn set_root_data(&mut self, data: Data);

    /// Creates an empty tree initialized with the given DHAT metadata.
    fn with_metadata(metadata: DhatMetadata) -> Self
    where
        Self: Sized + Default;
}

impl Data {
    fn to_program_point(&self, frames: Vec<usize>) -> ProgramPoint {
        ProgramPoint {
            total_bytes: self.total_bytes,
            total_blocks: self.total_blocks,
            total_lifetimes: self.total_lifetimes,
            maximum_bytes: self.maximum_bytes,
            maximum_blocks: self.maximum_blocks,
            bytes_at_max: self.bytes_at_max,
            blocks_at_max: self.blocks_at_max,
            bytes_at_end: self.bytes_at_end,
            blocks_at_end: self.blocks_at_end,
            blocks_read: self.blocks_read,
            blocks_write: self.blocks_write,
            accesses: self
                .accesses
                .as_ref()
                .and_then(|accesses| (!accesses.is_empty()).then(|| accesses.compact())),
            frames,
        }
    }

    fn zero() -> Self {
        Self {
            total_bytes: 0,
            total_blocks: 0,
            total_lifetimes: Some(0),
            maximum_bytes: Some(0),
            maximum_blocks: Some(0),
            bytes_at_max: Some(0),
            blocks_at_max: Some(0),
            bytes_at_end: Some(0),
            blocks_at_end: Some(0),
            blocks_read: Some(0),
            blocks_write: Some(0),
            accesses: None,
        }
    }

    fn add(&mut self, other: &Self) {
        self.total_bytes += other.total_bytes;
        self.total_blocks += other.total_blocks;
        self.total_lifetimes = sum_options(self.total_lifetimes, other.total_lifetimes);
        self.maximum_bytes = sum_options(self.maximum_bytes, other.maximum_bytes);
        self.maximum_blocks = sum_options(self.maximum_blocks, other.maximum_blocks);
        self.bytes_at_max = sum_options(self.bytes_at_max, other.bytes_at_max);
        self.blocks_at_max = sum_options(self.blocks_at_max, other.blocks_at_max);
        self.bytes_at_end = sum_options(self.bytes_at_end, other.bytes_at_end);
        self.blocks_at_end = sum_options(self.blocks_at_end, other.blocks_at_end);
        self.blocks_read = sum_options(self.blocks_read, other.blocks_read);
        self.blocks_write = sum_options(self.blocks_write, other.blocks_write);
        match (&mut self.accesses, other.accesses.as_ref()) {
            (Some(lhs), rhs) => lhs.add(rhs),
            (None, Some(rhs)) => self.accesses = Some(rhs.clone()),
            (None, None) => self.accesses = Some(Accesses::default()),
        }
    }

    fn metrics(&self, mode: Mode) -> ToolMetrics {
        // This is the same order as order of metrics in the log file output
        let metrics = match mode {
            Mode::Heap | Mode::Copy => [
                (DhatMetric::TotalBytes, Some(self.total_bytes)),
                (DhatMetric::TotalBlocks, Some(self.total_blocks)),
                // These should all be None in copy mode
                (DhatMetric::AtTGmaxBytes, self.bytes_at_max),
                (DhatMetric::AtTGmaxBlocks, self.blocks_at_max),
                (DhatMetric::AtTEndBytes, self.bytes_at_end),
                (DhatMetric::AtTEndBlocks, self.blocks_at_end),
                (DhatMetric::ReadsBytes, self.blocks_read),
                (DhatMetric::WritesBytes, self.blocks_write),
                (DhatMetric::TotalLifetimes, self.total_lifetimes),
                (DhatMetric::MaximumBytes, self.maximum_bytes),
                (DhatMetric::MaximumBlocks, self.maximum_blocks),
            ],
            Mode::AdHoc => [
                (DhatMetric::TotalUnits, Some(self.total_bytes)),
                (DhatMetric::TotalEvents, Some(self.total_blocks)),
                // These should all be None in ad-hoc mode
                (DhatMetric::AtTGmaxBytes, self.bytes_at_max),
                (DhatMetric::AtTGmaxBlocks, self.blocks_at_max),
                (DhatMetric::AtTEndBytes, self.bytes_at_end),
                (DhatMetric::AtTEndBlocks, self.blocks_at_end),
                (DhatMetric::ReadsBytes, self.blocks_read),
                (DhatMetric::WritesBytes, self.blocks_write),
                (DhatMetric::TotalLifetimes, self.total_lifetimes),
                (DhatMetric::MaximumBytes, self.maximum_bytes),
                (DhatMetric::MaximumBlocks, self.maximum_blocks),
            ],
        };

        let mut tool_metrics = Metrics::empty();
        for (key, value) in metrics
            .iter()
            .filter_map(|(metric, value)| value.map(|value| (metric, value)))
        {
            tool_metrics.insert(*key, value.into());
        }
        ToolMetrics::Dhat(tool_metrics)
    }
}

impl From<&ProgramPoint> for Data {
    fn from(value: &ProgramPoint) -> Self {
        Self {
            total_bytes: value.total_bytes,
            total_blocks: value.total_blocks,
            total_lifetimes: value.total_lifetimes,
            maximum_bytes: value.maximum_bytes,
            maximum_blocks: value.maximum_blocks,
            bytes_at_max: value.bytes_at_max,
            blocks_at_max: value.blocks_at_max,
            bytes_at_end: value.bytes_at_end,
            blocks_at_end: value.blocks_at_end,
            blocks_read: value.blocks_read,
            blocks_write: value.blocks_write,
            accesses: Some(
                value
                    .accesses
                    .as_ref()
                    .map_or_else(Accesses::default, Accesses::expand),
            ),
        }
    }
}

impl Tree for DhatTree {
    /// Insert a prefix with the given [`Data`] into this tree
    ///
    /// The rust borrow checker without the polonius crate below would give a false positive.
    fn insert(&mut self, prefix: &[usize], data: &Data, table: &[Frame]) {
        let mut current = &mut *self.root;
        let mut index = 0;

        for p in prefix {
            self.table.entry(*p).or_insert_with(|| table[*p].clone());
        }

        // root aggregates all data
        current.add_data(data);

        while index < prefix.len() {
            let key = prefix[index];
            let current_prefix = &prefix[index..];

            match polonius::<_, _, ForLt!(&mut Node)>(current, |current| {
                if let Some(child) = current.find_child(key) {
                    PoloniusResult::Borrowing(child)
                } else {
                    PoloniusResult::Owned(())
                }
            }) {
                PoloniusResult::Borrowing(child) => {
                    if let Some(split_index) = child.split_index(current_prefix) {
                        child.split(split_index, data, None);
                        index += split_index;
                    } else {
                        match current_prefix.len().cmp(&child.prefix.len()) {
                            Ordering::Less => {
                                child.split(current_prefix.len(), data, Some(data.clone()));
                                return;
                            }
                            Ordering::Greater => {
                                child.add_data(data);
                                index += child.prefix.len();
                            }
                            Ordering::Equal => {
                                child.add_data(data);
                                child.add_orig_data(data);
                                return;
                            }
                        }
                    }

                    current = child;
                }
                PoloniusResult::Owned {
                    input_borrow: current,
                    ..
                } => {
                    current.add_child(current_prefix, data);
                    return;
                }
            }
        }
    }

    fn set_root_data(&mut self, data: Data) {
        self.root.data = data;
    }

    fn root_data(&self) -> &Data {
        &self.root.data
    }

    fn frame_table(&self) -> &BTreeMap<usize, Frame> {
        &self.table
    }

    fn metadata(&self) -> &DhatMetadata {
        &self.metadata
    }

    fn with_metadata(metadata: DhatMetadata) -> Self
    where
        Self: Sized + Default,
    {
        Self {
            metadata,
            table: BTreeMap::from([(0, Frame::Root)]),
            ..Default::default()
        }
    }

    fn root_node(self) -> Box<Node> {
        self.root
    }
}

impl From<DhatTree> for DhatData {
    fn from(tree: DhatTree) -> Self {
        let mapping_table = tree.mapping_table();
        let mut program_points = Vec::default();
        tree.root
            .collect_program_points(Vec::default(), &mut program_points, &mapping_table);

        Self {
            metadata: tree.metadata,
            program_points,
            frame_table: tree.table.into_values().collect(),
        }
    }
}

impl Node {
    fn collect_program_points(
        &self,
        mut frames: Vec<usize>,
        program_points: &mut Vec<ProgramPoint>,
        mapping_table: &[usize],
    ) {
        frames.extend(self.prefix.iter());

        for child in &self.children {
            child.collect_program_points(frames.clone(), program_points, mapping_table);
        }

        if let Some(data) = &self.orig_data {
            program_points.push(
                data.to_program_point(frames.iter().map(|frame| mapping_table[*frame]).collect()),
            );
        }
    }

    /// Creates a new `Node`.
    pub fn new(
        prefix: Vec<usize>,
        children: Vec<Self>,
        data: Data,
        orig_data: Option<Data>,
    ) -> Self {
        Self {
            children,
            data,
            orig_data,
            prefix,
        }
    }

    /// Creates a new `Node` without `orig_data` (test helper)
    #[cfg(test)]
    fn without_orig(prefix: Vec<usize>, children: Vec<Self>, data: Data) -> Self {
        Self {
            children,
            data,
            orig_data: None,
            prefix,
        }
    }

    fn with_cloned_orig(prefix: Vec<usize>, children: Vec<Self>, data: Data) -> Self {
        Self {
            children,
            orig_data: Some(data.clone()),
            data,
            prefix,
        }
    }

    fn add_child(&mut self, prefix: &[usize], data: &Data) {
        self.children.push(Self::with_cloned_orig(
            prefix.to_vec(),
            vec![],
            data.clone(),
        ));
    }

    fn find_child(&mut self, num: usize) -> Option<&mut Self> {
        self.children
            .iter_mut()
            .find(|node| node.prefix.first().is_some_and(|a| *a == num))
    }

    fn split(&mut self, index: usize, data: &Data, orig_data: Option<Data>) {
        let node = Self::new(
            self.prefix.split_off(index),
            std::mem::take(&mut self.children),
            self.data.clone(),
            self.orig_data.take(),
        );

        self.add_data(data);
        self.orig_data = orig_data;

        self.children.push(node);
    }

    fn split_index(&self, other: &[usize]) -> Option<usize> {
        let length = self.prefix.len().min(other.len());
        (0..length).find(|&index| self.prefix[index] != other[index])
    }

    fn add_data(&mut self, data: &Data) {
        self.data.add(data);
    }

    fn add_orig_data(&mut self, data: &Data) {
        if let Some(orig_data) = &mut self.orig_data {
            orig_data.add(data);
        } else {
            self.orig_data = Some(data.clone());
        }
    }
}

impl Tree for RootTree {
    fn insert(&mut self, prefix: &[usize], data: &Data, table: &[Frame]) {
        for p in prefix {
            self.table.entry(*p).or_insert_with(|| table[*p].clone());
        }

        self.root.data.add(data);
    }

    fn set_root_data(&mut self, data: Data) {
        self.root.data = data;
    }

    fn root_data(&self) -> &Data {
        &self.root.data
    }

    fn frame_table(&self) -> &BTreeMap<usize, Frame> {
        &self.table
    }

    fn metadata(&self) -> &DhatMetadata {
        &self.metadata
    }

    fn with_metadata(metadata: DhatMetadata) -> Self
    where
        Self: Sized + Default,
    {
        Self {
            metadata,
            table: BTreeMap::from([(0, Frame::Root)]),
            ..Default::default()
        }
    }

    fn root_node(self) -> Box<Node> {
        self.root
    }
}

impl From<RootTree> for DhatData {
    fn from(tree: RootTree) -> Self {
        Self {
            metadata: tree.metadata,
            program_points: Vec::default(),
            frame_table: tree.table.into_values().collect(),
        }
    }
}

fn sum_options<T: Add<Output = T>>(lhs: Option<T>, rhs: Option<T>) -> Option<T> {
    match (lhs, rhs) {
        (None, None) => None,
        (None, Some(b)) => Some(b),
        (Some(a), None) => Some(a),
        (Some(a), Some(b)) => Some(a + b),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn data_fixture(num: u64) -> Data {
        Data {
            total_bytes: num,
            accesses: Some(Accesses::default()),
            ..Default::default()
        }
    }

    fn data_with_accesses(accesses: Option<Vec<i64>>) -> Data {
        Data {
            accesses: accesses.map(Accesses),
            ..Default::default()
        }
    }

    fn metadata_fixture() -> DhatMetadata {
        DhatMetadata {
            mode: Mode::Heap,
            ..Default::default()
        }
    }

    fn frame_table_fixture() -> Vec<Frame> {
        vec![Frame::Root; 7]
    }

    fn program_point_fixture(frames: Vec<usize>, total_bytes: u64) -> ProgramPoint {
        ProgramPoint {
            total_bytes,
            frames,
            ..Data::default().to_program_point(Vec::default())
        }
    }

    fn program_point_summary(data: &DhatData) -> Vec<(Vec<usize>, u64)> {
        let mut summary = data
            .program_points
            .iter()
            .map(|program_point| (program_point.frames.clone(), program_point.total_bytes))
            .collect::<Vec<_>>();
        summary.sort();
        summary
    }

    fn table_fixture(indices: &[usize]) -> BTreeMap<usize, Frame> {
        indices.iter().map(|index| (*index, Frame::Root)).collect()
    }

    fn dhat_tree_fixture() -> DhatTree {
        DhatTree {
            metadata: metadata_fixture(),
            table: BTreeMap::default(),
            root: Box::new(Node::without_orig(
                vec![],
                vec![Node::without_orig(
                    vec![1, 2, 3],
                    vec![Node::with_cloned_orig(vec![4, 5], vec![], data_fixture(1))],
                    data_fixture(2),
                )],
                data_fixture(2),
            )),
        }
    }

    #[test]
    fn test_data_from_program_point_expands_accesses() {
        let program_point = ProgramPoint {
            accesses: Some(Accesses(vec![1, -2, 3])),
            ..program_point_fixture(vec![1], 1)
        };

        let data = Data::from(&program_point);

        assert_eq!(data.accesses, Some(Accesses(vec![1, 3, 3])));
    }

    #[test]
    fn test_data_to_program_point_compacts_accesses() {
        let program_point = Data {
            accesses: Some(Accesses(vec![1, 3, 3])),
            ..Default::default()
        }
        .to_program_point(vec![1]);

        assert_eq!(program_point.accesses, Some(Accesses(vec![1, -2, 3])));
    }

    #[rstest]
    #[case::unset_accesses_plus_accesses(None, Some(vec![1, 2]), Some(vec![1, 2]))]
    #[case::same_length_accesses(Some(vec![1, 2]), Some(vec![3, 4]), Some(vec![4, 6]))]
    #[case::different_length_accesses(Some(vec![1]), Some(vec![2, 3]), Some(vec![]))]
    #[case::accesses_plus_no_accesses(Some(vec![1]), Some(vec![]), Some(vec![]))]
    #[case::no_accesses_plus_accesses(Some(vec![]), Some(vec![1]), Some(vec![]))]
    fn test_data_add_accesses(
        #[case] lhs: Option<Vec<i64>>,
        #[case] rhs: Option<Vec<i64>>,
        #[case] expected: Option<Vec<i64>>,
    ) {
        let mut lhs = data_with_accesses(lhs);
        let rhs = data_with_accesses(rhs);

        lhs.add(&rhs);

        assert_eq!(lhs.accesses, expected.map(Accesses));
    }

    #[test]
    fn test_dhat_tree_insert_empty() {
        let mut expected = DhatTree::default();
        expected.root.data = data_fixture(1);

        let mut tree = DhatTree::default();
        tree.insert(&[], &data_fixture(1), &[]);

        assert_eq!(tree, expected);
    }

    #[test]
    fn test_dhat_tree_insert_equal() {
        let expected = DhatTree {
            metadata: metadata_fixture(),
            table: table_fixture(&[1, 2, 3]),
            root: Box::new(Node::without_orig(
                vec![],
                vec![Node::new(
                    vec![1, 2, 3],
                    vec![Node::with_cloned_orig(vec![4, 5], vec![], data_fixture(1))],
                    data_fixture(3),
                    Some(data_fixture(1)),
                )],
                data_fixture(3),
            )),
        };

        let mut tree = dhat_tree_fixture();
        tree.insert(&[1, 2, 3], &data_fixture(1), &frame_table_fixture());

        assert_eq!(tree, expected);
    }

    #[test]
    fn test_dhat_tree_insert_full_longer() {
        let expected = DhatTree {
            metadata: metadata_fixture(),
            table: table_fixture(&[1, 2, 3, 6]),
            root: Box::new(Node::without_orig(
                vec![],
                vec![Node::without_orig(
                    vec![1, 2, 3],
                    vec![
                        Node::with_cloned_orig(vec![4, 5], vec![], data_fixture(1)),
                        Node::with_cloned_orig(vec![6], vec![], data_fixture(1)),
                    ],
                    data_fixture(3),
                )],
                data_fixture(3),
            )),
        };

        let mut tree = dhat_tree_fixture();
        tree.insert(&[1, 2, 3, 6], &data_fixture(1), &frame_table_fixture());

        assert_eq!(tree, expected);
    }

    #[test]
    fn test_dhat_tree_insert_full_shorter() {
        let expected = DhatTree {
            metadata: metadata_fixture(),
            table: table_fixture(&[1]),
            root: Box::new(Node::without_orig(
                vec![],
                vec![Node::new(
                    vec![1],
                    vec![Node::without_orig(
                        vec![2, 3],
                        vec![Node::with_cloned_orig(vec![4, 5], vec![], data_fixture(1))],
                        data_fixture(2),
                    )],
                    data_fixture(3),
                    Some(data_fixture(1)),
                )],
                data_fixture(3),
            )),
        };

        let mut tree = dhat_tree_fixture();
        tree.insert(&[1], &data_fixture(1), &frame_table_fixture());

        assert_eq!(tree, expected);
    }

    #[test]
    fn test_dhat_tree_insert_mixed() {
        let expected = DhatTree {
            metadata: metadata_fixture(),
            table: table_fixture(&[1, 6]),
            root: Box::new(Node::without_orig(
                vec![],
                vec![Node::without_orig(
                    vec![1],
                    vec![
                        Node::without_orig(
                            vec![2, 3],
                            vec![Node::with_cloned_orig(vec![4, 5], vec![], data_fixture(1))],
                            data_fixture(2),
                        ),
                        Node::with_cloned_orig(vec![6], vec![], data_fixture(1)),
                    ],
                    data_fixture(3),
                )],
                data_fixture(3),
            )),
        };

        let mut tree = dhat_tree_fixture();
        tree.insert(&[1, 6], &data_fixture(1), &frame_table_fixture());

        assert_eq!(tree, expected);
    }

    #[test]
    fn test_dhat_tree_insert_no_match() {
        let expected = DhatTree {
            metadata: metadata_fixture(),
            table: table_fixture(&[6]),
            root: Box::new(Node::without_orig(
                vec![],
                vec![
                    Node::without_orig(
                        vec![1, 2, 3],
                        vec![Node::with_cloned_orig(vec![4, 5], vec![], data_fixture(1))],
                        data_fixture(2),
                    ),
                    Node::with_cloned_orig(vec![6], vec![], data_fixture(1)),
                ],
                data_fixture(3),
            )),
        };

        let mut tree = dhat_tree_fixture();
        tree.insert(&[6], &data_fixture(1), &frame_table_fixture());

        assert_eq!(tree, expected);
    }

    #[test]
    fn test_root_tree_insert() {
        let expected = RootTree {
            metadata: metadata_fixture(),
            root: Box::new(Node::without_orig(vec![], vec![], data_fixture(1))),
            table: table_fixture(&[1, 2, 3]),
        };

        let mut tree = RootTree::default();
        tree.insert(&[1, 2, 3], &data_fixture(1), &frame_table_fixture());

        assert_eq!(tree, expected);
    }

    #[rstest]
    #[case::root_skipped(
        Node::without_orig(vec![], vec![], data_fixture(3)),
        vec![0],
        vec![],
    )]
    #[case::leaf_emitted(
        Node::with_cloned_orig(vec![1, 2], vec![], data_fixture(3)),
        vec![0, 1, 2],
        vec![(vec![1, 2], 3)],
    )]
    #[case::parent_orig_data_emitted_after_child(
        Node::new(
            vec![1],
            vec![Node::with_cloned_orig(vec![2], vec![], data_fixture(2))],
            data_fixture(5),
            Some(data_fixture(3)),
        ),
        vec![0, 1, 2],
        vec![(vec![1, 2], 2), (vec![1], 3)],
    )]
    #[case::synthetic_zero_parent_skipped(
        Node::without_orig(
            vec![1],
            vec![Node::with_cloned_orig(vec![2], vec![], data_fixture(5))],
            data_fixture(5),
        ),
        vec![0, 1, 2],
        vec![(vec![1, 2], 5)],
    )]
    #[case::sparse_frames_rebased(
        Node::with_cloned_orig(vec![5], vec![], data_fixture(1)),
        vec![0, 0, 0, 0, 0, 1],
        vec![(vec![1], 1)],
    )]
    fn test_node_collect_program_points(
        #[case] node: Node,
        #[case] mapping_table: Vec<usize>,
        #[case] expected: Vec<(Vec<usize>, u64)>,
    ) {
        let mut program_points = Vec::default();

        node.collect_program_points(Vec::default(), &mut program_points, &mapping_table);

        let actual = program_points
            .into_iter()
            .map(|program_point| (program_point.frames, program_point.total_bytes))
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_root_tree_insert_two() {
        let expected = RootTree {
            metadata: metadata_fixture(),
            root: Box::new(Node::without_orig(vec![], vec![], data_fixture(3))),
            table: table_fixture(&[1, 2, 3]),
        };

        let mut tree = RootTree::default();
        tree.insert(&[1, 2, 3], &data_fixture(1), &frame_table_fixture());
        tree.insert(&[1], &data_fixture(2), &frame_table_fixture());

        assert_eq!(tree, expected);
    }

    #[test]
    fn test_dhat_tree_into_data_reconstructs_prefix_program_point() {
        let mut tree = DhatTree::with_metadata(metadata_fixture());
        let table = frame_table_fixture();

        tree.insert(&[1, 2, 3], &data_fixture(2), &table);
        tree.insert(&[1], &data_fixture(1), &table);

        let data = DhatData::from(tree);

        assert_eq!(data.frame_table, vec![Frame::Root; 4]);
        assert_eq!(data.program_points.len(), 2);
        assert!(
            data.program_points
                .iter()
                .any(|pp| pp.frames == [1, 2, 3] && pp.total_bytes == 2)
        );
        assert!(
            data.program_points
                .iter()
                .any(|pp| pp.frames == [1] && pp.total_bytes == 1)
        );
    }

    #[test]
    fn test_dhat_tree_into_data_skips_synthetic_split_program_point() {
        let mut tree = DhatTree::with_metadata(metadata_fixture());
        let table = frame_table_fixture();

        tree.insert(&[1, 2], &data_fixture(2), &table);
        tree.insert(&[1, 3], &data_fixture(3), &table);

        let data = DhatData::from(tree);

        assert_eq!(data.frame_table, vec![Frame::Root; 4]);
        assert_eq!(data.program_points.len(), 2);
        assert!(
            data.program_points
                .iter()
                .any(|pp| pp.frames == [1, 2] && pp.total_bytes == 2)
        );
        assert!(
            data.program_points
                .iter()
                .any(|pp| pp.frames == [1, 3] && pp.total_bytes == 3)
        );
        assert!(!data.program_points.iter().any(|pp| pp.frames == [1]));
    }

    #[test]
    fn test_dhat_tree_into_data_rebases_sparse_frame_ids() {
        let mut tree = DhatTree::with_metadata(metadata_fixture());
        let table = frame_table_fixture();

        tree.insert(&[5], &data_fixture(1), &table);

        let data = DhatData::from(tree);

        assert_eq!(data.frame_table, vec![Frame::Root; 2]);
        assert_eq!(data.program_points.len(), 1);
        assert_eq!(data.program_points[0].frames, [1]);
        assert_eq!(data.program_points[0].total_bytes, 1);
    }

    #[test]
    fn test_dhat_data_dhat_tree_round_trip_reconstructs_program_points() {
        let input = DhatData {
            metadata: metadata_fixture(),
            program_points: vec![
                program_point_fixture(vec![1], 1),
                program_point_fixture(vec![1, 2, 3], 2),
            ],
            frame_table: vec![Frame::Root; 4],
        };

        let tree = DhatTree::from_json(input.clone());
        let actual = DhatData::from(tree);

        assert_eq!(actual.metadata, input.metadata);
        assert_eq!(actual.frame_table, input.frame_table);
        assert_eq!(
            program_point_summary(&actual),
            vec![(vec![1], 1), (vec![1, 2, 3], 2)]
        );
    }

    #[test]
    fn test_dhat_data_dhat_tree_round_trip_rebases_sparse_frame_ids() {
        let frame = Frame::from(("0x5", "sparse", "lib.rs:5"));
        let mut frame_table = vec![Frame::Root; 6];
        frame_table[5] = frame.clone();
        let input = DhatData {
            metadata: metadata_fixture(),
            program_points: vec![program_point_fixture(vec![5], 1)],
            frame_table,
        };

        let tree = DhatTree::from_json(input);
        let actual = DhatData::from(tree);

        assert_eq!(actual.frame_table, vec![Frame::Root, frame]);
        assert_eq!(program_point_summary(&actual), vec![(vec![1], 1)]);
    }
}
