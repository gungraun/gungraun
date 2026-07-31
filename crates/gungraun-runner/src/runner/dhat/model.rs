//! This module contains the structs to model the dhat output file content

// spell-checker: ignore bklt bkacc bksu tuth ftbl tgmax
use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use simplematch::DoWild;

use crate::api::EntryPoint;
use crate::runner::DEFAULT_TOGGLE;

static FRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?<root>\[root\])|(?<addr>0x[0-9a-fA-F]+):\s*(?<func>.*)\s\((?<in>.*)\)$")
        .expect("Regex should compile")
});

/// A [`Frame`] in the [`DhatData::frame_table`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// The root frame
    Root,
    /// All other frames than the root are leafs
    Leaf(String, String, String),
}

/// The dhat invocation mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// --mode=heap
    #[default]
    Heap,
    /// --mode=ad-hoc
    AdHoc,
    /// --mode=copy
    Copy,
}

/// Exact per-block access counts for a [`ProgramPoint`].
///
/// DHAT stores this as an optional array of signed integers. Non-negative values are access
/// counts. Negative values are run-length encoding markers for the following non-negative value;
/// for example, `[-3, 4]` expands to `[4, 4, 4]`.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Accesses(pub Vec<i64>);

/// The top-level data extracted from dhat json output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[expect(clippy::arbitrary_source_item_ordering)]
pub struct DhatData {
    /// Top-level metadata
    #[serde(flatten)]
    pub metadata: DhatMetadata,

    /// The [`ProgramPoint`]s
    #[serde(rename = "pps")]
    pub program_points: Vec<ProgramPoint>,

    /// [`Frame`] table
    #[serde(rename = "ftbl")]
    pub frame_table: Vec<Frame>,
}

/// Top-level metadata extracted from dhat json output.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[expect(clippy::arbitrary_source_item_ordering)]
pub struct DhatMetadata {
    /// Version number of the format
    #[serde(rename = "dhatFileVersion")]
    pub dhat_file_version: usize,

    /// The invocation mode
    pub mode: Mode,

    /// The verb used before above stack frames
    pub verb: String,

    /// Are block lifetimes recorded? Affects whether some other fields are present.
    #[serde(rename = "bklt")]
    pub has_block_lifetimes: bool,

    /// Are block accesses recorded? Affects whether some other fields are present
    #[serde(rename = "bkacc")]
    pub has_block_accesses: bool,

    /// Byte units. "byte" is the values used if these fields are omitted.
    #[serde(rename = "bu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_unit: Option<String>,

    /// Bytes units. "bytes" is the values used if these fields are omitted.
    #[serde(rename = "bsu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_unit: Option<String>,

    /// Blocks units. "blocks" is the values used if these fields are omitted.
    #[serde(rename = "bksu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_unit: Option<String>,

    /// Time units (individual)
    #[serde(rename = "tu")]
    pub time_unit: String,

    /// Time units (1,000,000x)
    #[serde(rename = "Mtu")]
    pub time_unit_m: String,

    /// The "short-lived" time threshold, measures in "tu"s (`time_unit`).
    /// - bklt=true: a mandatory integer.
    /// - bklt=false: omitted.
    #[serde(rename = "tuth")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_threshold: Option<usize>,

    /// The executed command
    #[serde(rename = "cmd")]
    pub command: String,

    /// The process ID
    pub pid: i32,

    /// The time at the end of execution (t-end)
    #[serde(rename = "te")]
    pub time_end: u64,

    /// The time of the global max (t-gmax)
    /// - bklt=true: a mandatory integer.
    /// - bklt=false: omitted.
    #[serde(rename = "tg")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_global_max: Option<u64>,
}

/// A `ProgramPoint`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[expect(clippy::arbitrary_source_item_ordering)]
pub struct ProgramPoint {
    /// Total bytes
    #[serde(rename = "tb")]
    pub total_bytes: u64,

    /// Total blocks
    #[serde(rename = "tbk")]
    pub total_blocks: u64,

    /// Total lifetimes of all blocks allocated at this `ProgramPoint`.
    /// - bklt=true: a mandatory integer.
    /// - bklt=false: omitted.
    #[serde(rename = "tl")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_lifetimes: Option<u64>,

    /// The maximum bytes for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "mb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_bytes: Option<u64>,

    /// The maximum blocks for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "mbk")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_blocks: Option<u64>,

    /// The bytes at t-gmax for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "gb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_at_max: Option<u64>,

    /// The blocks at t-gmax for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "gbk")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_at_max: Option<u64>,

    /// The bytes at t-end for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "eb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_at_end: Option<u64>,

    /// The blocks at t-end for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "ebk")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_at_end: Option<u64>,

    /// The reads of blocks for this `ProgramPoint`
    /// - bkacc=true: mandatory integers.
    /// - bkacc=false: omitted.
    #[serde(rename = "rb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_read: Option<u64>,

    /// The writes of blocks for this `ProgramPoint`
    /// - bkacc=true: mandatory integers.
    /// - bkacc=false: omitted.
    #[serde(rename = "wb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_write: Option<u64>,

    /// The exact accesses of blocks for this `ProgramPoint`. Only used when all allocations are
    /// the same size and sufficiently small. A negative element indicates run-length encoding
    /// of the following integer. E.g. `-3, 4` means "three 4s in a row".
    /// - bkacc=true: an optional array of integers.
    /// - bkacc=false: omitted.
    #[serde(rename = "acc")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accesses: Option<Accesses>,

    /// Frames. Each element is an index into the [`DhatData::frame_table`]
    #[serde(rename = "fs")]
    pub frames: Vec<usize>,
}

impl Accesses {
    /// Aggregates access counts using the same semantics as DHAT's `dh_view.js`
    ///
    /// This method expects `Accesses` data expanded with [`Accesses::expand`].
    pub fn add(&mut self, other: Option<&Self>) {
        match other {
            Some(rhs) if self.0.len() == rhs.0.len() => {
                for (lhs, rhs) in self.0.iter_mut().zip(&rhs.0) {
                    *lhs += rhs;
                }
            }
            _ => self.0 = Vec::default(),
        }
    }

    /// Compacts expanded access counts into DHAT's run-length encoding.
    ///
    /// # Panics
    ///
    /// Panics if an access count is negative. Negative values are reserved for run-length encoding
    /// markers and must not appear in expanded access data.
    pub fn compact(&self) -> Self {
        fn push_access(compacted: &mut Vec<i64>, value: i64, len: usize) {
            let len = i64::try_from(len).expect("access run length should fit into i64");
            if len != 1 {
                compacted.push(-len);
            }
            compacted.push(value);
        }

        let mut compacted = Vec::new();
        let Some(mut run_value) = self.0.first().copied() else {
            return Self(compacted);
        };
        let mut run_len = 0_usize;

        for access in &self.0 {
            assert!(*access >= 0, "access count should be >= 0");
            if *access == run_value {
                run_len += 1;
            } else {
                push_access(&mut compacted, run_value, run_len);
                run_value = *access;
                run_len = 1;
            }
        }

        push_access(&mut compacted, run_value, run_len);
        Self(compacted)
    }

    /// Expands DHAT's run-length encoded access counts.
    ///
    /// # Panics
    ///
    /// Panics if a run-length encoding marker is missing its value, if the repeated value is
    /// negative, or if the repeat count cannot fit into [`usize`].
    pub fn expand(&self) -> Self {
        let mut expanded = Vec::new();
        let mut iter = self.0.iter();

        while let Some(access) = iter.next() {
            if *access < 0 {
                let value = iter
                    .next()
                    .expect("run-length encoded accesses should have a value");
                assert!(*value >= 0, "run-length encoded values should be >= 0");

                let reps = access
                    .checked_neg()
                    .and_then(|value| usize::try_from(value).ok())
                    .expect("run-length encoded access repeat count should fit into usize");
                expanded.extend(std::iter::repeat_n(*value, reps));
            } else {
                expanded.push(*access);
            }
        }

        Self(expanded)
    }

    /// Returns `true` if there are no accesses
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl DhatData {
    /// Filters program points by entry point and additional frame glob patterns.
    ///
    /// Returns `true` if the program point list changed. If no entry point and no frames are
    /// configured, the data is left unchanged. If filters are configured but no matching frames are
    /// found, all program points are removed.
    pub fn filter_program_points(&mut self, entry_point: &EntryPoint, frames: &[String]) -> bool {
        let mut globs = frames.iter().collect::<Vec<_>>();

        let glob = match entry_point {
            EntryPoint::None => None,
            EntryPoint::Default => Some(DEFAULT_TOGGLE.into()),
            EntryPoint::Custom(custom) => Some(custom.into()),
        };

        if let Some(glob) = &glob {
            globs.push(glob);
        }

        let mut indices = vec![];
        if !globs.is_empty() {
            for (index, frame) in self.frame_table.iter().enumerate() {
                if let Frame::Leaf(_, func_name, _) = frame {
                    for glob in &globs {
                        if glob.as_str().dowild(func_name) {
                            indices.push(index);
                        }
                    }
                }
            }
        }

        if *entry_point == EntryPoint::None && frames.is_empty() {
            false
        } else if !indices.is_empty() {
            let old_len = self.program_points.len();
            self.program_points
                .retain(|p| p.frames.iter().any(|f| indices.contains(f)));

            old_len != self.program_points.len()
        } else {
            let old_len = self.program_points.len();
            self.program_points = Vec::default();

            old_len != self.program_points.len()
        }
    }

    /// Remaps program point frame indices after the frame table has been compacted.
    ///
    /// The `mapping_table` maps original frame indices to the new indices in the compact frame
    /// table. If the frame table is empty, a synthetic root frame is inserted so the DHAT output
    /// remains valid.
    pub fn sanitize(&mut self, mapping_table: &[usize]) {
        if self.frame_table.is_empty() {
            self.frame_table.insert(0, Frame::Root);
        } else {
            for program_point in &mut self.program_points {
                for frame in &mut program_point.frames {
                    *frame = mapping_table[*frame];
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let frame = String::deserialize(deserializer)?;
        Self::from_str(&frame).map_err(Error::custom)
    }
}

impl From<(&str, &str, &str)> for Frame {
    fn from((addr, func, loc): (&str, &str, &str)) -> Self {
        Self::Leaf(addr.to_owned(), func.to_owned(), loc.to_owned())
    }
}

impl FromStr for Frame {
    type Err = String;

    fn from_str(haystack: &str) -> Result<Self, Self::Err> {
        let caps = FRAME_RE
            .captures(haystack)
            .ok_or_else(|| "invalid frame format".to_owned())?;

        if caps.name("root").is_some() {
            Ok(Self::Root)
        } else {
            Ok(Self::Leaf(
                caps.name("addr")
                    .expect("An address should be present")
                    .as_str()
                    .to_owned(),
                caps.name("func")
                    .expect("A function should be present")
                    .as_str()
                    .to_owned(),
                caps.name("in")
                    .expect("A location should be present")
                    .as_str()
                    .to_owned(),
            ))
        }
    }
}

impl ProgramPoint {
    /// Returns `true` if all present metric fields are zero.
    ///
    /// Missing optional metric fields are treated as zero.
    pub fn is_zero(&self) -> bool {
        self.total_bytes == 0
            && self.total_blocks == 0
            && self.total_lifetimes.is_none_or(|value| value == 0)
            && self.maximum_bytes.is_none_or(|value| value == 0)
            && self.maximum_blocks.is_none_or(|value| value == 0)
            && self.bytes_at_max.is_none_or(|value| value == 0)
            && self.blocks_at_max.is_none_or(|value| value == 0)
            && self.bytes_at_end.is_none_or(|value| value == 0)
            && self.blocks_at_end.is_none_or(|value| value == 0)
            && self.blocks_read.is_none_or(|value| value == 0)
            && self.blocks_write.is_none_or(|value| value == 0)
    }
}

impl Serialize for Frame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string = match self {
            Self::Root => "[root]".to_owned(),
            Self::Leaf(addr, func, loc) => format!("{addr}: {func} ({loc})"),
        };

        serializer.serialize_str(&string)
    }
}

impl<'de> Deserialize<'de> for Mode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let frame = String::deserialize(deserializer)?;
        let mode = match frame.to_lowercase().as_str() {
            "ad-hoc" => Self::AdHoc,
            "heap" => Self::Heap,
            "copy" => Self::Copy,
            _ => return Err(Error::custom("Invalid mode")),
        };

        Ok(mode)
    }
}

impl Serialize for Mode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string = match self {
            Self::Heap => "heap",
            Self::AdHoc => "ad-hoc",
            Self::Copy => "copy",
        };

        serializer.serialize_str(string)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_test::{Token, assert_tokens};

    use super::*;

    fn program_point_fixture(frames: Vec<usize>) -> ProgramPoint {
        ProgramPoint {
            total_bytes: 1,
            total_blocks: 1,
            total_lifetimes: Some(1),
            maximum_bytes: Some(1),
            maximum_blocks: Some(1),
            bytes_at_max: Some(1),
            blocks_at_max: Some(1),
            bytes_at_end: Some(0),
            blocks_at_end: Some(0),
            blocks_read: Some(0),
            blocks_write: Some(0),
            accesses: None,
            frames,
        }
    }

    fn program_point_with_options(value: Option<u64>) -> ProgramPoint {
        ProgramPoint {
            total_bytes: value.unwrap_or(0),
            total_blocks: value.unwrap_or(0),
            total_lifetimes: value,
            maximum_bytes: value,
            maximum_blocks: value,
            bytes_at_max: value,
            blocks_at_max: value,
            bytes_at_end: value,
            blocks_at_end: value,
            blocks_read: value,
            blocks_write: value,
            accesses: None,
            frames: vec![1],
        }
    }

    fn dhat_data_filter_fixture() -> DhatData {
        DhatData {
            metadata: DhatMetadata::default(),
            program_points: vec![
                program_point_fixture(vec![1, 2]),
                program_point_fixture(vec![3]),
                program_point_fixture(vec![4]),
            ],
            frame_table: vec![
                Frame::Root,
                Frame::from(("0x1", DEFAULT_TOGGLE, "bench.rs:1")),
                Frame::from(("0x2", "malloc", "alloc.c:1")),
                Frame::from(("0x3", "custom_entry", "lib.rs:1")),
                Frame::from(("0x4", "other", "lib.rs:2")),
            ],
        }
    }

    #[rstest]
    #[case::short_addr(
        "0x1234: malloc (in /usr/lib/some.so)", ("0x1234", "malloc", "in /usr/lib/some.so")
    )]
    #[case::no_in(
        "0x12345678: malloc (/usr/lib/some.so)", ("0x12345678", "malloc", "/usr/lib/some.so")
    )]
    #[case::some(
        "0x12345678: malloc (in /usr/lib/some.so)", ("0x12345678", "malloc", "in /usr/lib/some.so")
    )]
    #[case::long_with_multiple_parentheses(
    "0x40440E3: call_once<(), (dyn core::ops::function::Fn<(), Output=i32> + core::marker::Sync + \
    core::panic::unwind_safe::RefUnwindSafe)> (function.rs:284)",
    (
        "0x40440E3",
        "call_once<(), (dyn core::ops::function::Fn<(), Output=i32> + \
        core::marker::Sync + core::panic::unwind_safe::RefUnwindSafe)>",
        "function.rs:284"
    )
)]
    fn test_frame_from_str(#[case] haystack: &str, #[case] frame: (&str, &str, &str)) {
        let expected = Frame::from(frame);
        let actual = haystack.parse::<Frame>().unwrap();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::no_filter(
        EntryPoint::None,
        vec![],
        false,
        vec![vec![1, 2], vec![3], vec![4]],
    )]
    #[case::default_entry_point(EntryPoint::Default, vec![], true, vec![vec![1, 2]])]
    #[case::frames_only(
        EntryPoint::None,
        vec!["malloc".to_owned()],
        true,
        vec![vec![1, 2]],
    )]
    #[case::custom_entry_and_frames(
        EntryPoint::Custom("custom_*".to_owned()),
        vec!["malloc".to_owned()],
        true,
        vec![vec![1, 2], vec![3]],
    )]
    #[case::no_match(EntryPoint::None, vec!["missing".to_owned()], true, vec![])]
    #[case::all_match(
        EntryPoint::None,
        vec!["*".to_owned()],
        false,
        vec![vec![1, 2], vec![3], vec![4]],
    )]
    fn test_dhat_data_filter_program_points(
        #[case] entry_point: EntryPoint,
        #[case] frames: Vec<String>,
        #[case] expected_is_filtered: bool,
        #[case] expected_frames: Vec<Vec<usize>>,
    ) {
        let mut data = dhat_data_filter_fixture();

        let is_filtered = data.filter_program_points(&entry_point, &frames);

        assert_eq!(is_filtered, expected_is_filtered);
        assert_eq!(
            data.program_points
                .iter()
                .map(|program_point| program_point.frames.clone())
                .collect::<Vec<_>>(),
            expected_frames
        );
    }

    #[test]
    fn test_dhat_data_sanitize_when_frame_table_empty_then_creates_root_frame() {
        let mut data = DhatData {
            metadata: DhatMetadata::default(),
            program_points: Vec::default(),
            frame_table: Vec::default(),
        };

        data.sanitize(&[]);

        assert_eq!(data.frame_table, vec![Frame::Root]);
    }

    #[rstest]
    #[case::identity(
        vec![program_point_fixture(vec![1, 2]), program_point_fixture(vec![3])],
        vec![0, 1, 2, 3],
        vec![vec![1, 2], vec![3]],
    )]
    #[case::sparse(
        vec![program_point_fixture(vec![2, 4]), program_point_fixture(vec![4])],
        vec![0, 0, 1, 0, 2],
        vec![vec![1, 2], vec![2]],
    )]
    fn test_dhat_data_sanitize(
        #[case] program_points: Vec<ProgramPoint>,
        #[case] mapping_table: Vec<usize>,
        #[case] expected_frames: Vec<Vec<usize>>,
    ) {
        let mut data = DhatData {
            metadata: DhatMetadata::default(),
            program_points,
            frame_table: vec![Frame::Root],
        };

        data.sanitize(&mapping_table);

        assert_eq!(
            data.program_points
                .iter()
                .map(|program_point| program_point.frames.clone())
                .collect::<Vec<_>>(),
            expected_frames
        );
    }

    #[rstest]
    #[case::all_options_none(None, true)]
    #[case::all_options_some_zero(Some(0), true)]
    #[case::all_options_some_one(Some(1), false)]
    fn test_program_point_is_zero(#[case] value: Option<u64>, #[case] expected: bool) {
        let program_point = program_point_with_options(value);

        assert_eq!(program_point.is_zero(), expected);
    }

    #[test]
    fn test_frame_de_and_serialize_frame() {
        let frame = Frame::from(("0x1234", "malloc", "in /usr/lib/some.so"));
        assert_tokens(
            &frame,
            &[Token::Str("0x1234: malloc (in /usr/lib/some.so)")],
        );
    }

    #[test]
    fn test_frame_de_and_serialize_root() {
        let frame = Frame::Root;
        assert_tokens(&frame, &[Token::Str("[root]")]);
    }

    #[test]
    fn test_frame_from_str_when_root() {
        let expected = Frame::Root;
        let actual = "[root]".parse::<Frame>().unwrap();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::plain(vec![1, 2], vec![1, 2])]
    #[case::run_length_encoded(vec![-3, 4], vec![4, 4, 4])]
    #[case::mixed(vec![1, -2, 3, 4], vec![1, 3, 3, 4])]
    fn test_accesses_expand(#[case] accesses: Vec<i64>, #[case] expected: Vec<i64>) {
        assert_eq!(Accesses(accesses).expand(), Accesses(expected));
    }

    #[test]
    #[should_panic(expected = "run-length encoded accesses should have a value")]
    fn test_accesses_expand_when_missing_repeated_value_then_panics() {
        Accesses(vec![-3]).expand();
    }

    #[test]
    #[should_panic(expected = "run-length encoded values should be >= 0")]
    fn test_accesses_expand_when_repeated_value_is_negative_then_panics() {
        Accesses(vec![-3, -4]).expand();
    }

    #[rstest]
    #[case::empty(vec![], vec![])]
    #[case::plain(vec![1, 2], vec![1, 2])]
    #[case::repeated(vec![4, 4, 4], vec![-3, 4])]
    #[case::mixed(vec![1, 3, 3, 4], vec![1, -2, 3, 4])]
    fn test_accesses_compact(#[case] accesses: Vec<i64>, #[case] expected: Vec<i64>) {
        assert_eq!(Accesses(accesses).compact(), Accesses(expected));
    }

    #[test]
    #[should_panic(expected = "access count should be >= 0")]
    fn test_accesses_compact_when_access_count_is_negative_then_panics() {
        Accesses(vec![-4]).compact();
    }
}
