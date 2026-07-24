# Prerequisites

Gungraun requires at least one supported profiling tool for the benchmark
target. Valgrind is required for the default `Callgrind` configuration and the
other Valgrind tools. On Linux, Gungraun also supports `perf`, so a benchmark
configured exclusively for perf does not require Valgrind.

Target support and runtime availability are separate. A configured tool must be
installed and executable in the environment where the benchmark runs. Perf may
also require suitable kernel permissions for the requested events.

The default benchmarking tool is `Callgrind` and is in most cases perfectly
suited to do the job but if you want or need to use
[`Cachegrind`](../cachegrind.md) instead of `Callgrind` you require Valgrind
version `>= 3.22` and client requests (see below).

## Debug Symbols

It's required to run the Gungraun benchmarks with debugging symbols switched on.
For example in your `~/.cargo/config` or your project's `Cargo.toml`:

```toml
[profile.bench]
debug = true
```

Now, all benchmarks which are run with `cargo bench` include the debug symbols.
(See also [Cargo Profiles][cargo-profiles] and [Cargo Config][cargo-config]).

It's required that settings like `strip = true` or other configuration options
stripping the debug symbols need to be disabled explicitly in the `bench`
profile if you have changed this option for the `release` profile. For example:

```toml
[profile.release]
strip = true

[profile.bench]
debug = true
strip = false
```

## Valgrind Client Requests

If you want to make use of the [Valgrind Client Requests][valgrind-client-req]
which are a re-export of the [valgrind-requests] package, you also need
`libclang` (clang >= 5.0) installed. See also the requirements of [bindgen] and
of [cc].

More details on the usage and requirements of `Valgrind Client Requests` in
[this](../client_requests.md) chapter of the guide.

## Installation of Valgrind

Gungraun is intentionally independent of a specific version of Valgrind.
However, Gungraun was only tested with versions of Valgrind >= `3.20.0`. It is
therefore highly recommended to use a recent version of Valgrind. Also, if you
want or need to, [building valgrind from source][valgrind-source] is usually a
straightforward process. Just make sure the `valgrind` binary is in your `$PATH`
so that Gungraun can find it. See [installation in the CI](./ci.md) for tips to
install Valgrind in the CI.

### Installation of Valgrind with Your Package Manager

#### Alpine Linux

```bash
apk add valgrind
```

#### Arch Linux

```bash
pacman -Sy valgrind
```

#### Debian/Ubuntu

```bash
apt-get install valgrind
```

#### Fedora Linux

```bash
dnf install valgrind
```

#### FreeBSD

```bash
pkg install valgrind
```

### Running Valgrind or perf in Containers

If Valgrind or perf cannot be installed directly on your host system, or you
want to customize tool execution in a wrapper, you can use the `--tool-runner`
argument to run the selected tool through a container runtime like Docker or
Podman.

For detailed instructions and more examples, see
[Running Tools with a Custom Runner](../cli_and_env/tool_runner.md).

### Valgrind is Available for the Following Distributions

[![Packaging status](https://repology.org/badge/vertical-allrepos/valgrind.svg)](https://repology.org/project/valgrind/versions)

[bindgen]: https://rust-lang.github.io/rust-bindgen/requirements.html
[cargo-config]: https://doc.rust-lang.org/cargo/reference/config.html
[cargo-profiles]: https://doc.rust-lang.org/cargo/reference/profiles.html
[cc]: https://github.com/rust-lang/cc-rs
[Valgrind]: https://www.valgrind.org
[valgrind-client-req]:
    https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq
[valgrind-requests]: https://docs.rs/valgrind-requests/latest/valgrind_requests/
[valgrind-source]:
    https://sourceware.org/git/?p=valgrind.git;a=blob;f=README;h=eabcc6ad88c8cab6dfe73cfaaaf5543023c2e941;hb=HEAD
