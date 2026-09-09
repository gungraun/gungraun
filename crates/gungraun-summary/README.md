<!-- markdownlint-disable MD041 MD033 -->

<h1 align="center">gungraun-summary</h1>

<div align="center">Typed Rust data model and parsing helpers for Gungraun summary JSON files.</div>

<br>
<div align="center">
    <a href="https://gungraun.github.io/gungraun">Guide</a>
    |
    <a href="https://docs.rs/crate/gungraun-summary/">API Docs</a>
    |
    <a href="https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/CHANGELOG.md">Changelog</a>
</div>
<div align="center">
    <a href="https://github.com/gungraun/gungraun/actions/workflows/cicd.yml" style="text-decoration:none">
        <img src="https://github.com/gungraun/gungraun/actions/workflows/cicd.yml/badge.svg" alt="GitHub branch checks state"/>
    </a>
    |
    <a href="https://crates.io/crates/gungraun-summary" style="text-decoration:none">
        <img src="https://shields.io/crates/v/gungraun-summary.svg" alt="Crates.io"/>
    </a>
    |
    <a href="https://docs.rs/gungraun-summary/" style="text-decoration:none">
        <img src="https://img.shields.io/docsrs/gungraun-summary/latest" alt="docs.rs"/>
    </a>
</div>
<br>

`gungraun-summary` is a companion crate to [Gungraun][gungraun-github]. It
provides the typed Rust data model for Gungraun summary JSON files, so consumers
can work with strongly typed values instead of traversing `serde_json::Value` by
hand or generating Rust types from a schema as a separate step.

## What this crate provides

- Versioned summary types for supported Gungraun summary schemas
- Version-aware parsing helpers in [`util`][util]
- Version-specific parsing helpers in [`v6`] and [`v7`]
- Re-exports of helper crates used by the public data model

## Quickstart

If the schema version is not known ahead of time, parse the summary with the
version-aware helpers and convenience functions in [`util`][util]:

```rust
use std::path::Path;

use gungraun_summary::util::{SummaryByVersion, parse};

match parse(Path::new("target/summary.json")).unwrap() {
    SummaryByVersion::V6(summary) => {
        println!("{}", summary.function_name);
    }
    SummaryByVersion::V7(summary) => {
        println!("{}", summary.function_name);
    }
    other => eprintln!("unsupported summary: {other:?}"),
}
```

If you already know the summary schema version, use the corresponding versioned
module, such as [`v6`] or [`v7`], and call its `parse` function directly.

## Versioning

The major version of `gungraun-summary` tracks the latest Gungraun summary
schema version supported by this crate.

This crate supports summary schema versions `v6` and `v7`. The `v6` module is a
frozen snapshot of the version 6 data model, while `v7` follows the current
runner data model. Future major versions may retain older version modules for
backwards compatibility while adding support for newer summary formats.

## More information

- The generated JSON schemas live in the [`schemas/`](./schemas/) directory of
  this crate in the repository.
- Full API documentation is available on [docs.rs][api-docs].
- If you need support for an older summary version, please [open an
  issue][gungraun-issue].

[api-docs]: https://docs.rs/gungraun-summary/latest/gungraun_summary/
[gungraun-github]: https://github.com/gungraun/gungraun
[gungraun-issue]: https://github.com/gungraun/gungraun/issues
[util]: https://docs.rs/gungraun-summary/latest/gungraun_summary/util/
[v6]: https://docs.rs/gungraun-summary/latest/gungraun_summary/v6/
[v7]: https://docs.rs/gungraun-summary/latest/gungraun_summary/v7/
