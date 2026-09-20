# gungraun-summary Knowledge Guide

## Overview

Typed Rust data model and parsing helpers for Gungraun summary JSON files. The
crate supports frozen v6 and current v7 summary schemas. Future releases may
retain older version modules for backwards compatibility.

## Structure

```text
crates/gungraun-summary/
|- src/lib.rs          # Crate root: re-exports either_or_both, indexmap; declares version modules
|- src/v6/             # Frozen gungraun-summary-v6.0.0 model and v6-specific parse helpers
|- src/v7/             # Current gungraun-runner model re-exports and v7-specific parse helpers
|- src/util.rs         # Version-aware parsing: probes `version` field, dispatches to matching parser
|- src/error.rs        # Error enum: ParseError, UnsupportedVersion, CliArgument
|- src/main.rs         # `gungraun-summary-schemagen` binary (schema feature): emits JSON schema
|- schemas/            # Versioned JSON schema files (summary.v1.schema.json ... summary.v7.schema.json)
|- tests/main.rs       # Smoke test: deserializes a fixture summary.json
`- Cargo.toml          # Features: `schema` gates schemars + gungraun-runner/schema
```

## Where To Look

| Task                | Location             | Notes                                                                        |
| ------------------- | -------------------- | ---------------------------------------------------------------------------- |
| Versioned types     | `src/v6/`, `src/v7/` | Frozen v6 snapshot; v7 re-exports the current runner model                   |
| Version-aware parse | `src/util.rs`        | `SummaryByVersion`, `Version`, `parse` / `parse_slice`                       |
| Schema generation   | `src/main.rs`        | Uses `schemars::SchemaSettings::draft07()`; writes `summary.schema.json`     |
| Generated schemas   | `schemas/`           | One file per schema version; v7 is current                                   |
| Schema recipes      | `../../Justfile`     | `schema-gen`, `schema-gen-diff`, `schema-gen-move`                           |
| Compatibility tests | `tests/main.rs`      | Smoke deserializes fixture; gungraun-tests also assert on `BenchmarkSummary` |

## Conventions

- Each version module (for example, `v6` or `v7`) is self-contained; all types
  needed to decode that schema live there or are re-exported.
- `util` is the entrypoint when the schema version is unknown; use `v6::parse`
  or `v7::parse` directly when it is known.
- The `schema` feature gates `JsonSchema` derives and the
  `gungraun-summary-schemagen` binary.
- Schema files are generated, not hand-edited; use `just schema-gen-move` to
  update after model changes.
- `v6::SCHEMA_VERSION` belongs to the frozen snapshot. `v7::SCHEMA_VERSION`
  describes the current runner-backed summary model.

## Anti-Patterns

- Do not hand-edit files in `schemas/`; regenerate via the schemagen binary and
  `just schema-gen-move`.
- Do not add new summary versions without updating `util::SummaryByVersion` and
  `util::Version`.
- Do not remove old schema files from `schemas/` even if the corresponding
  version module is gone; they are part of the published schema history.
- Do not enable the `schema` feature in downstream crates unless you need schema
  generation; it pulls in `schemars`.
