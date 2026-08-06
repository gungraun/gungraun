<!-- spell-checker:ignore rmdirs -->

# Overview

This is the package for system tests of the interaction of `gungraun`,
`gungraun-runner` and `gungraun-macros`. Most of the benchmarks in this package
can be run as usual with `cargo bench` or `just bench $BENCH_NAME`. But, to be
able to intercept and validate the output (and other validations) of the
`cargo bench` run of a system test there is a wrapper around `cargo bench` in
`gungraun-tests/src/bench/main.rs` with which the system tests should be run.
For example, you can use `just system-test $BENCH_NAME`.

## Notes

This wrapper is and always will be an ongoing work and extended by need. There
is still room for improvements in the testing practice. This document gives a
brief introduction. All documentation can be found in the source code in
`src/bench`.

## Usage

Basically

`$ cargo run -p gungraun-tests --profile=bench bench [-- [FLAGS] [BENCH]]`

but far better is to use

`$ just system-test`, `$ just system-test-all`.

See the output of `just --show system-test`, ... for the command description and
arguments.

The positional `BENCH` can be one or more system tests. If no positional
argument is given all benchmarks are run.

`FLAGS` are described in `src/bench/main.rs`.

## Adding a new system test

### Basic structure

Library system tests go into `benches/test_lib_bench` and binary system tests
into `benches/test_bin_bench`. The naming scheme of a new file is for example
for a binary benchmark `benches/test_bin_bench/foo/test_bin_bench_foo.rs` and
for a library benchmark `benches/test_lib_bench/foo/test_lib_bench_foo.rs`.
After you have created the new directory and file, have a look at the
`Cargo.toml` of this package and then add

```toml
[[bench]]
harness = false
name = "test_bin_bench_foo"
path = "benches/test_bin_bench/foo/test_bin_bench_foo.rs"
```

You can now start to set up your test case in the benchmark file. Run the
benchmark for example with `just bench test_lib_bench_foo`.

### Configuration

In the current state this new benchmark won't run in the CI or with
`just system-test`. Adding a yaml file with the same name as the benchmark file
but with the extension `.conf.yml` is required to register this benchmark as
system test.

For example, if the benchmark file name is
`benches/test_bin_bench/foo/test_bin_bench_foo.rs`, the configuration file name
is `benches/test_bin_bench/foo/test_bin_bench_foo.conf.yml`.

The basic structure of this file is fully documented in `src/bench/config.rs`.

An example of a configuration file which runs two benchmark suites for the
benchmark file within the same folder. We're not testing much here besides that
the benchmark suites don't cause an exit with error or panic. Setting the
expectation values would be required to validate the output of the benchmark
runs, check that all expected files are present etc.

```yaml
groups:
    - runs:
          - args: ["--nocapture"]
    # The output files of the previous group run(s) are deleted
    - runs:
          - args: ["--callgrind-args='--toggle-collect=main'"]
```

An example of a configuration file which runs the benchmark in the same folder
twice without deleting the output files.

```yaml
groups:
    - runs:
          - args: ["--nocapture"]
          # The output files of the previous benchmark run are NOT deleted
          - args: ["--callgrind-args='--toggle-collect=main'"]
```

#### Expected values

All possible values are documented in `src/bench/config.rs`.

##### Expected Stdout/Stderr

The expected output can be stored in a file in the same directory as the
configuration file. For example a file
`benches/test_bin_bench/foo/expected_stdout` can be configured like that to be
the expected stdout of this benchmark run. Likewise
`benches/test_bin_bench/foo/expected_stderr` for the `stderr` of the benchmark
run. It's usually not a bad idea to run the benchmarks with `--nocapture` if you
define an expected `stdout/stderr` but also depends on the test.

```yaml
groups:
    - runs:
          - args: ["--nocapture"]
            expected:
                stdout: expected_stdout
          - args: ["--nocapture"]
            expected:
                stdout: expected_stdout
```

The expected `stdout` is sanitized from numbers:

If the original output is

```text
test_bin_bench_foo::group::function id:() -> target/release/echo
  Instructions:                   1|N/A             (*********)
  L1 Hits:                        2|N/A             (*********)
  LL Hits:                        3|N/A             (*********)
  RAM Hits:                       4|N/A             (*********)
  Total read+write:               5|N/A             (*********)
  Estimated Cycles:               6|N/A             (*********)
```

then the expected stdout is

```text
test_bin_bench_foo::group::function id:() -> target/release/echo
  Instructions:                    |N/A             (*********)
  L1 Hits:                         |N/A             (*********)
  LL Hits:                         |N/A             (*********)
  RAM Hits:                        |N/A             (*********)
  Total read+write:                |N/A             (*********)
  Estimated Cycles:                |N/A             (*********)
```

We do this, because the numbers can differ a little bit depending on the target,
toolchain in use etc. Having all system tests to update every time something
changes by `1` or `2` up or down is unmanageable. So, this is a simple method to
check if there are numbers, but we do not check the numbers themselves. Most
often, this is sufficient. But, we also check if all numbers of a single tool
are 0 which is usually an indicator for something going wrong.

The expected stdout is currently also sanitized from factors (the `[1.000000x]`
part after the percentages `(10.000000%)` and the `L2`, `RAM`,
`Estimated Cycles` change reports as seen below). Here the second run of the
above benchmark

```text
test_bin_bench_foo::group::function id:() -> target/release/echo
  Instructions:                    |                (No change)
  L1 Hits:                         |                (No change)
  LL Hits:                         |                (         )
  RAM Hits:                        |                (         )
  Total read+write:                |                (No change)
  Estimated Cycles:                |                (         )
```

##### Expected files

The manifest for expected files is fully described in
`src/bench/expected_files.rs`. A small usage example:

```yaml
groups:
    - runs:
          - args: []
            expected:
                files: expected_files.yml
```

##### Expected exit code

```yaml
groups:
    - runs:
          - args: []
            expected:
                exit_code: 0
```

See for example `benches/test_bin_bench/exit_with`

#### Templated benchmarks

```yaml
template: test_bin_bench_foo.rs.j2
groups:
    - runs:
          - args: []
            template_data:
                foo: "1234"
```

See for example `benches/test_lib_bench/regression`

#### Other configuration values

All configuration values are fully documented in `src/bench/config.rs`
