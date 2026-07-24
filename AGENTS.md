# Gungraun Agent Guidelines

This document provides instructions for AI agents working on the Gungraun
repository. If present load the user-local AI agents file .opencode/AGENTS.md in
addition to this file.

## 1. Build, Lint, and Test Commands

Gungraun uses `just` as a task runner. Always prefer `just` commands over direct
`cargo` invocations when available to ensure consistency with CI/CD.

### formatting & Linting

- **Format Code (Rust):** `just fmt` (Requires nightly toolchain)
- **Format TOML:** `just fmt-toml`
- **Format Prettier (JSON/YAML/MD):** `just fmt-prettier`
    - **Important:** Always run `just fmt-prettier` after making changes to
      `AGENTS.md`
- **Lint (Clippy):** `just lint` (Uses stable toolchain)
- **Check All Formatting:** `just check-fmt-all`

### Testing

- **Run All Tests:** `just test-all` (Excludes client-request-tests and
  benchmarks)
- **Run Package Tests:** `just test <package_name>`
    - Example: `just test gungraun`
- **Run UI Tests:** `just test-ui` (Fixed to MSRV compiler)
    - **Overwrite UI Test Fixtures:** `just test-ui-overwrite`
- **Run Doc Tests:** `just test-doc`

### Benchmarks (System Tests)

- **Run Single Benchmark System Test:** `just full-bench-test <bench_name>`
    - Example: `just full-bench-test test_lib_bench_tools`
- **Run All Benchmark System Tests:** `just full-bench-test-all`

## 2. Code Style & Conventions

### General

- **Rust Edition:** 2024
- **Line Length:** 100 characters for comments (enforced by rustfmt).
- **Newlines:** Unix style (`\n`).

### Public API and Semantic Versioning

Although much of `gungraun-runner` uses `pub`, most of its API is internal to
the Gungraun project and changes to it are not considered semver-breaking. The
exception is all items in modules exported through public feature flags, such as
`api`, `summary`, and `schema`. Treat changes to these items as changes to the
public API and preserve semantic versioning guarantees.

### Formatting & Imports

- **Rustfmt:** Strictly adhere to `rustfmt.toml`.
    - `imports_granularity = "Module"`
    - `group_imports = "StdExternalCrate"`
- **Import Order:** std -> external crates -> crate modules.
- **Sorting:** Imports and modules should be sorted alphabetically.

### Naming Conventions

- **Types/Traits:** `UpperCamelCase`
- **Functions/Methods/Modules/Variables:** `snake_case`
- **Constants/Statics:** `SCREAMING_SNAKE_CASE`
- **Files:** `snake_case.rs`

### Error Handling

- **Library (`gungraun`):** Use specific, typed errors where possible.
- **Runner (`gungraun-runner`):**
    - Uses a central `Error` enum in `src/error.rs`.
    - Variants include `BenchmarkError`, `ConfigurationError`, `JobError`
      (wrapping `anyhow`), etc.
    - `JobError` wraps `anyhow::Error` for internal task failures.
    - Implement `Display` for user-facing error messages.
    - Use `thiserror` (if available) or manual `std::error::Error`
      implementation.

### Code Structure

- **Workspace:** Multi-crate workspace.
    - `gungraun`: Main library crate.
    - `gungraun-runner`: Binary runner.
    - `gungraun-macros`: Proc-macros.
    - `benchmark-tests`: System tests.
    - `valgrind-requests`: Valgrind client requests support crate.

### Testing Guidelines

- **Unit Tests:** Co-located in the same file or `mod tests` within the file.
- **Integration Tests:** Located in `tests/` directory of the package.
- **gungraun-runner Integration Tests:** Integration-style tests for
  gungraun-runner live in `benchmark-tests/tests/`.
- **Benchmarks:** defined using `#[library_benchmark]` and `#[binary_benchmark]`
  attributes.

## 3. Workflow specific

- **Dependencies:** Check `Cargo.toml` before adding new dependencies. Use
  `cargo add` only if necessary and approved.
- **Lockfile:** Do not manually edit `Cargo.lock`.
- **Pre-commit:** Ensure `just fmt` and `just lint` pass before committing.

### `valgrind-requests` Feature Semantics

- **`stubs`** is the minimum supported feature for `valgrind-requests`. It
  enables the full public API surface and build-time generated request
  definitions, but implementations must compile to no-ops or default return
  values. This supports production dependencies using
  `default-features = false, features = ["stubs"]` while tests/benchmarks use
  active requests through dev-dependencies.
- **`act`** enables real Valgrind client request execution and implies `stubs`.
  Do not treat `act` as a separate API surface from `stubs`.
- **`alloc`** only enables allocation-backed convenience APIs, such as
  formatting macros and owned C string helpers. Core client requests and
  `core::ffi::CStr`-based APIs must work without `alloc`.
- Do not make `act` imply `alloc`; active `no_std` without allocation is a
  supported use case.
- When changing `valgrind-requests`, verify with
  `just build-hack-valgrind-requests`
