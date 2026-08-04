# gungraun-summary Knowledge Guide

## Overview

Typed Rust data model and parsing helpers for Gungraun summary JSON files. The
crate major version tracks the latest summary schema version it supports.
Currently v6 only; future releases may retain older version modules for
backwards compatibility.

## Structure

```text
crates/gungraun-summary/
|- src/lib.rs          # Crate root: re-exports either_or_both, indexmap; declares version modules
|- src/v6.rs           # Re-exports gungraun-runner summary/metrics/api types; v6-specific parse helpers
|- src/util.rs         # Version-aware parsing: probes `version` field, dispatches to matching parser
|- src/error.rs        # Error enum: ParseError, UnsupportedVersion
|- src/main.rs         # `gungraun-summary-schemagen` binary (schema feature): emits JSON schema
|- schemas/            # Versioned JSON schema files (summary.v1.schema.json ... summary.v6.schema.json)
|- tests/main.rs       # Smoke test: deserializes a fixture summary.json
`- Cargo.toml          # Features: `schema` gates schemars + gungraun-runner/schema
```

## Where To Look

| Task                | Location         | Notes                                                                        |
| ------------------- | ---------------- | ---------------------------------------------------------------------------- |
| Versioned types     | `src/v6.rs`      | Re-exports from `gungraun-runner::summary::model`, `metrics::model`, `api`   |
| Version-aware parse | `src/util.rs`    | `SummaryByVersion`, `Version`, `parse` / `parse_slice`                       |
| Schema generation   | `src/main.rs`    | Uses `schemars::SchemaSettings::draft07()`; writes `summary.schema.json`     |
| Generated schemas   | `schemas/`       | One file per schema version; v6 is current                                   |
| Schema recipes      | `../../Justfile` | `schema-gen`, `schema-gen-diff`, `schema-gen-move`                           |
| Compatibility tests | `tests/main.rs`  | Smoke deserializes fixture; gungraun-tests also assert on `BenchmarkSummary` |

## Conventions

- Each version module (e.g. `v6`) is self-contained; all types needed to decode
  that schema live there or are re-exported.
- `util` is the entrypoint when the schema version is unknown; use `v6::parse`
  directly when it is known.
- The `schema` feature gates `JsonSchema` derives and the
  `gungraun-summary-schemagen` binary.
- Schema files are generated, not hand-edited; use `just schema-gen-move` to
  update after model changes.
- `SCHEMA_VERSION` in `crates/gungraun-runner/src/summary/model.rs` is the
  single source of truth for the version string.

## Anti-Patterns

- Do not hand-edit files in `schemas/`; regenerate via the schemagen binary and
  `just schema-gen-move`.
- Do not add new summary versions without updating `util::SummaryByVersion` and
  `util::Version`.
- Do not remove old schema files from `schemas/` even if the corresponding
  version module is gone; they are part of the published schema history.
- Do not enable the `schema` feature in downstream crates unless you need schema
  generation; it pulls in `schemars`.
