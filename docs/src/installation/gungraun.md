# Gungraun

Gungraun is divided into the library `gungraun` and the benchmark runner
`gungraun-runner`.

## Installation of the Library

To start with Gungraun, add the following to your `Cargo.toml` file:

```toml
[dev-dependencies]
gungraun = "0.20.0"
```

or run

```bash
cargo add --dev gungraun@0.20.0
```

See [Benchmarking Best Practices](../best_practices.md) for a more comprehensive
installation guide.

## Installation of the Benchmark Runner

To run Rust/Cargo benchmarks you'll also need the `gungraun-runner` binary
installed somewhere in your `$PATH`. The Rust harness invokes it automatically;
`--help`, `-h`, `--version`, and `-V` are available directly.

Development builds containing the [standalone binary benchmark frontend] also
provide `run --config PATH` and `check --config PATH` for external executables
without a consumer Rust project. Published `0.20.0` binaries do not contain that
frontend. See its installation section for building from source or using a
future pinned prebuilt release containing the feature.

### From Source

```shell
cargo install --version 0.20.0 gungraun-runner
```

There's also the possibility to install the binary somewhere else and point the
`GUNGRAUN_RUNNER` environment variable to the absolute path of the
`gungraun-runner` binary like so:

```shell
cargo install --version 0.20.0 --root /tmp gungraun-runner
GUNGRAUN_RUNNER=/tmp/bin/gungraun-runner cargo bench --bench my-bench
```

### Binstall

The `gungraun-runner` binary is [pre-built] for most platforms supported by
Valgrind and easily installable with [binstall]

```shell
cargo binstall gungraun-runner@0.20.0
```

## Updating

When updating the `gungraun` library, you'll also need to update
`gungraun-runner` and vice-versa or else the benchmark runner will exit with an
error.

### In the GitHub CI

For CI installation, see [CI Installation](./ci.md).

[binstall]: https://github.com/cargo-bins/cargo-binstall
[pre-built]: https://github.com/gungraun/gungraun/releases/tag/v0.20.0
[standalone binary benchmark frontend]:
    ../benchmarks/binary_benchmarks/standalone.md
