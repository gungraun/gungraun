//! Rust types for decoding [Gungraun][gungraun-github] summary JSON files.
//!
//! This crate provides the Rust data model for Gungraun summary JSON files: versioned structs,
//! enums, and related types that can be deserialized from the summaries emitted by Gungraun.
//!
//! # Goals
//!
//! Its main purpose is to let consumers work with strongly typed Rust values directly, without
//! having to traverse `serde_json::Value` by hand or go through an external schema-to-code
//! generation step. Each version module like [`v6`] and any future version modules `v7`, ... will
//! be completely self-contained, so all structures required to decode and work with version 6 can
//! be found in [`v6`].
//!
//! In addition to the types themselves, this crate also provides convenience parsers for loading
//! summaries from files or byte slices.
//!
//! # Project organization
//!
//! gungraun-summary's major version number is based on the latest summary version it supports. In
//! future versions of this crate, the crate is going to contain the older summary versions for
//! backwards-compatibility. For example gungraun-summary v8.x.x will contain the v6, v7 and v8
//! modules to be able to deserialize the summary versions 6, 7 and 8.
//!
//! Any external types which are needed to work with the data model (like
//! [`either_or_both::EitherOrBoth`]) are re-exported from this crate's root.
//!
//! The minor and patch versions are used to fix and extend the functionality of the crate itself
//! but not to change the underlying data model.
//!
//! The Gungraun summary version number is increased if the data model changes in an incompatible
//! way.
//!
//! The json schema for a specific summary file can be found in the `schemas` directory of this
//! crate in the github repository.
//!
//! # Structural details
//!
//! This crate contains the summary version 6 structures in the [`v6`] module. Earlier versions are
//! currently not supported, hence no `v5` module. The summary version v6 is used since the
//! Iai-callgrind/Gungraun version `v0.16.0` which should be old enough to reach most users. If you
//! need support for an older version, feel free to open an [issue][gungraun-issue] in the
//! [Gungraun][gungraun-github] repository. I usually would recommend updating to a recent Gungraun
//! version which supports [`v6`].
//!
//! There are two convenience entrypoints, depending on whether the summary schema version is known
//! ahead of time:
//!
//! - Use [`util`] for version-aware parsing. It probes the summary's `version` field and dispatches
//!   to the matching parser.
//! - Use a versioned module such as [`v6`] when the input is already known to match a specific
//!   schema version.
//!
//! # Examples
//!
//! Parse a summary when the schema version is not known ahead of time:
//!
//! ```no_run
//! use std::path::Path;
//!
//! use gungraun_summary::util::{SummaryByVersion, parse};
//!
//! match parse(Path::new("target/summary.json")).unwrap() {
//!     SummaryByVersion::V6(summary) => {
//!         println!("{}", summary.function_name);
//!     }
//!     _ => unreachable!("no other summary versions are currently supported"),
//! }
//! ```
//!
//! Decode a summary when the schema version is already known:
//!
//! ```no_run
//! use std::path::Path;
//!
//! let summary = gungraun_summary::v6::parse(Path::new("target/summary.json")).unwrap();
//! println!("{}", summary.function_name);
//! ```
//!
//! [gungraun-issue]: https://github.com/gungraun/gungraun/issues
//! [gungraun-github]: https://github.com/gungraun/gungraun

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(test(attr(warn(unused))))]
#![doc(test(attr(allow(unused_extern_crates))))]

pub mod error;
pub mod util;
pub mod v6;

pub use either_or_both;
pub use indexmap;
