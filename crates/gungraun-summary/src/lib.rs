//! Rust types for decoding [Gungraun][gungraun-github] summary JSON files.
//!
//! This crate provides the Rust data model for Gungraun summary JSON files: versioned structs,
//! enums, and related types that can be deserialized from the summaries emitted by Gungraun.
//!
//! # Goals
//!
//! Its main purpose is to let consumers work with strongly typed Rust values directly, without
//! having to traverse `serde_json::Value` by hand or go through an external schema-to-code
//! generation step. Each version module, [`v6`] and [`v7`], is self-contained: all structures
//! required to decode and work with a supported summary version are available from its module.
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
//! This crate contains a frozen version 6 data model in [`v6`] and the current version 7 data model
//! in [`v7`]. Earlier versions are currently not supported, hence no `v5` module. Version 6 has
//! been emitted since Iai-callgrind/Gungraun `v0.16.0`, so it should cover most existing summaries.
//! If you need support for an older version, please open an [issue][gungraun-issue] in the
//! [Gungraun][gungraun-github] repository. Otherwise, update to a recent Gungraun version that
//! supports [`v7`].
//!
//! There are two convenience entrypoints, depending on whether the summary schema version is known
//! ahead of time:
//!
//! - Use [`util`] for version-aware parsing. It probes the summary's `version` field and dispatches
//!   to the matching parser.
//! - Use [`v6`] or [`v7`] when the input is already known to match that schema version.
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
//!     SummaryByVersion::V7(summary) => {
//!         println!("{}", summary.function_name);
//!     }
//!     other => eprintln!("unsupported summary: {other:?}"),
//! }
//! ```
//!
//! Decode a summary when the schema version is already known:
//!
//! ```no_run
//! use std::path::Path;
//!
//! let summary = gungraun_summary::v7::parse(Path::new("target/summary.json")).unwrap();
//! println!("{}", summary.function_name);
//! ```
//!
//! [gungraun-issue]: https://github.com/gungraun/gungraun/issues
//! [gungraun-github]: https://github.com/gungraun/gungraun

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(test(attr(warn(unused))))]
#![doc(test(attr(allow(unused_extern_crates))))]
#![warn(missing_docs)]

pub mod error;
pub mod util;
pub mod v6;
pub mod v7;

pub use either_or_both;
pub use indexmap;
