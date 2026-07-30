# Client Request Tests

Tests for the `valgrind-requests` crate's architecture-specific client request
implementation. Not published; excluded from `just test-all`.

## Overview

These tests verify that `valgrind-requests` emits correct magic sequences and
client requests for each supported architecture. We do not test Valgrind itself,
only that our assembly and request encoding match what Valgrind expects.

Native tests run the test binaries directly. Cross-target tests run inside QEMU
system images via `cross`, using custom Docker images with Valgrind
pre-installed.

## Structure

| Path                          | Role                                                                            |
| ----------------------------- | ------------------------------------------------------------------------------- |
| `src/bin/`                    | Test binaries (`*-reqs-test.rs`) plus `valgrind-wrapper.rs`                     |
| `tests/common/mod.rs`         | Test harness: runner detection, fixture loading, matchers                       |
| `tests/test_client_requests/` | Per-tool test modules (valgrind, memcheck, callgrind, cachegrind, print_macros) |
| `tests/fixtures/`             | Target-specific stderr snapshots for cross-architecture comparison              |
| `build.rs`                    | Sets fixture path (`CLIENT_REQUEST_TESTS_FIXTURES`) for native vs `qemu-system` |
| `../Cross.toml`               | Cross target definitions with custom `ghcr.io/gungraun/...` images              |

## Conventions

- **Native path**: `cargo test -p client-request-tests --test tests` runs
  binaries directly.
- **Cross path**: `just reqs-test <target>` (e.g. `x86_64-unknown-linux-gnu`)
  runs via `cross` + QEMU.
- **Exit codes**: Test binaries exit `0` when not under Valgrind, `1` when under
  Valgrind. Higher codes are not tested.
- **Wrapper**: `valgrind-wrapper` invokes Valgrind, filters
  architecture-specific output (addresses, backtraces, numbers), and emits
  normalized stderr for fixture comparison.
- **Fixtures**: Named `<test>.<target>.stderr` or
  `<test>.since_<rust-version>.<target>.stderr` for per-target or per-version
  variance.
- **Features**: `_stubs` (default) and `_act` map to `valgrind-requests/stubs`
  and `valgrind-requests/act`.
- **Architecture guards**: Tests use `cfg!(target_arch)` and `cfg!(target_os)`
  to select the correct fixture.

## Anti-Patterns

- Do not expect `just test-all` to cover this crate; cross-target runs require
  `cross` and Docker.
- Do not conflate these with `benchmark-tests`; they test different layers.
- Do not add new test binaries without updating `tests/test_client_requests/`
  and fixtures.
- Do not assume all targets share the same stderr output; always provide a
  target-specific fixture when output differs.
