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

To be able to run the benchmarks you'll also need the `gungraun-runner` binary
installed somewhere in your `$PATH`. Otherwise, there is no need to interact
with `gungraun-runner` as it is just an implementation detail. However,
`gungraun-runner` understands the commands `--help`, `-h` and `--version`, `-V`.

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
