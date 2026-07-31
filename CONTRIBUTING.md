<!-- spell-checker: ignore readlink -->

# Contributing to Gungraun

Thank you for your interest in contributing to Gungraun!

## Feature Requests and Bug reports

Feature requests and bug reports should be reported in the [Issue
Tracker][gungraun-issues]. Please have a look at existing issues with the
[enhancement][gungraun-issues-enhancement] or [bug][gungraun-issues-bug] labels.

## Patches / Pull Requests

All patches have to be sent on GitHub as [pull requests][gungraun-pull]. Before
starting a pull request, it is best to open an issue first so no efforts are
wasted.

If you are looking for a place to start contributing to Gungraun, take a look at
the [help wanted][gungraun-help-wanted] or [good first
issue][gungraun-issues-first] issues.

The minimum supported version (MSRV) of Gungraun is Rust `1.85.1` and all
patches are expected to work with the minimum supported version.

All notable changes need to be added to the [CHANGELOG][changelog].

## Use of AI tools

We adhere to the [rust-analyzer] policy:

AI tool use is not discouraged on the gungraun codebase, as long as it meets our
quality standards. We kindly ask you to disclose usage of AI tools in your
contributions. If you used them without disclosing it, we may reject your
contribution on that basis alone due to the assumption that you have, most
likely, not reviewed your own submission (so why should we?).

We may still reject AI-assisted contributions if we deem the quality of the
contribution to be unsatisfactory.

## How to get started

Clone this repo

```shell
git clone https://github.com/gungraun/gungraun.git
cd gungraun
```

Working on this project is a piece of cake with [just] and the `just` shell
completions installed. Using `just` also ensures that you use the same commands,
arguments, options as they are used in the ci.

Before running any install commands with `just`, it is recommended to first
inspect it with `--dry-run` and see if you're fine with the changes. Install the
basics needed to start working on this project with:

```shell
just install-workspace
```

This command will install git hooks, the necessary components for the `stable`,
`nightly` toolchain and the current MSRV toolchain, run some checks for tools
which need to be installed, ...

To get an overview over all possible `just` rules run `just -l` or directly
inspect the `Justfile` in the root of this project.

You should also set up your editor to use nightly rustfmt and clippy from the
rust `stable` toolchain for example with `rust-analyzer` server overrides. An
alternative to server overrides is to use tasks which are executed on save and
use `just fmt` to format the project with nightly `rustfmt` and `just lint` to
lint with stable clippy.

Some examples for `rust-analyzer` server overrides:

### Configure VSCode server overrides

Go to settings, choose workspace or folder settings and edit the `settings.json`
file:

```json
{
    "settings": {
        "rust-analyzer.check.overrideCommand": [
            "cargo",
            "+stable",
            "clippy",
            "--message-format=json",
            "--all-features",
            "--all-targets",
            "--workspace"
        ],
        "rust-analyzer.rustfmt.overrideCommand": [
            "rustfmt",
            "+nightly",
            "--emit",
            "stdout"
        ]
    }
}
```

### Configure Neovim server overrides

In neovim you need to set the override commands in the `rust-analyzer` server
table. The specifics depend on the plugin which is used to install/configure
`rust-analyzer`, i.e. `mason` or `rustaceanvim`, but the principle stays the
same:

```lua
["rust-analyzer"] = {
    check = {
        overrideCommand = {
            "cargo",
            "+stable",
            "clippy",
            "--message-format=json",
            "--all-features",
            "--all-targets",
            "--workspace",
        },
    },
    rustfmt = {
        overrideCommand = {
            "rustfmt",
            "+nightly",
            "--emit",
            "stdout",
        },
    },
}
```

## Working on the guide

The main documentation of Gungraun is in the [guide]. The source code lives in
the `docs/src` subdirectory of this repo.

To start working on the guide, ensure you have everything necessary installed
with `just book-install`. After everything's installed, you need two terminal
windows. In the first run `just book-watch` and in the second run
`just book-serve-github`. The second command makes the rendered guide available
at `http://localhost:4000/gungraun` (this'll redirect you to the `index.html`).
You can now start making changes to the source code and the changes are
reflected after a second or two.

Please run `just book-tests` from time to time if you make changes to rust
codeblocks and especially before pushing to the pr.

Have a look at the existing code blocks for examples. Every rust code block
requires `# extern crate gungraun;` as first line if you want to access the
`gungraun` api. Also, ensure to not actually run the benchmark harness with

```rust
#...

# fn main() {
main!(library_benchmark_groups = some_group);
# }
```

## Testing

Patches have to include tests to verify (at a minimum) that the whole pipeline
runs through without errors.

The benches in the `benchmark-tests` package are system tests and run the whole
pipeline. We use a wrapper around `cargo bench`
(`crates/benchmark-tests/src/bench`) to run the `benchmark-tests`. In order to
run a single benchmark-tests use `just full-bench-test $BENCHMARK_NAME` or all
with `just full-bench-test-all` (This might take a while). See the
[`README`](./crates/benchmark-tests/DEVELOPERS.md) of the benchmark-tests
package for more details.

The user interface is tested in `crates/gungraun/tests/ui`. The ui tests error
fixtures are fixed to the MSRV compiler since the compiler error messages differ
between the rust toolchains. For example to run the ui tests

```shell
just test-ui
```

or overwrite the error message fixtures:

`just test-ui-overwrite`

If you made changes in the `gungraun-runner` package, you can point the
`GUNGRAUN_RUNNER` environment variable to your modified version of the
`gungraun-runner` binary and run the benchmark-tests with:

```shell
cargo build -p gungraun-runner --release
GUNGRAUN_RUNNER=$(realpath target/release/gungraun-runner) cargo bench -p benchmark-tests
```

or with `just` in a single command:

```shell
just bench-test-all
```

or a specific bench of the benchmark-test package

```shell
just bench-test test_lib_bench_tools
```

The concrete results of the benchmarks are not checked when running the
benchmark tests that way. You can use

```shell
just full-bench-test test_lib_bench_tools
```

which actually verifies the stdout/stderr and/or output files of the benchmark
run according to the configuration of the benchmark (the
`$BENCHMARK_NAME.conf.yml` files in the respective benchmark folder). Depending
on the test configuration, benchmarks are sometimes run multiple times.

## Contact

If there are any outstanding questions about contributing to gungraun, they can
be asked on the [gungraun issue tracker][gungraun-issues].

[changelog]: https://github.com/gungraun/gungraun/blob/main/CHANGELOG.md
[guide]: https://gungraun.github.io/gungraun/
[gungraun-help-wanted]:
    https://github.com/gungraun/gungraun/labels/help%20wanted
[gungraun-issues]: https://github.com/gungraun/gungraun/issues
[gungraun-issues-bug]:
    https://github.com/gungraun/gungraun/issues?q=is%3Aissue+is%3Aopen+label%3Abug
[gungraun-issues-enhancement]:
    https://github.com/gungraun/gungraun/issues?q=is%3Aissue+is%3Aopen+label%3Aenhancement
[gungraun-issues-first]:
    https://github.com/gungraun/gungraun/labels/good%20first%20issue
[gungraun-pull]: https://github.com/gungraun/gungraun/pulls
[just]: https://github.com/casey/just
[rust-analyzer]:
    https://github.com/rust-lang/rust-analyzer/blob/master/CONTRIBUTING.md
