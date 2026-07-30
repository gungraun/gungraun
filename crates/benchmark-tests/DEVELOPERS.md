<!-- spell-checker:ignore rmdirs -->

# Overview

This is the package for system tests of the interaction of `gungraun`,
`gungraun-runner` and `gungraun-macros`. Most of the benchmarks in this package
can be run as usual with `cargo bench` or `just bench-test $BENCH_NAME`. But, to
be able to intercept and validate the output (and other validations) of the
`cargo bench` run of a benchmark test there is a wrapper around `cargo bench` in
`benchmark-test/src/bench.rs` with which the benchmarks tests should be run. For
example, you can use `just full-bench-test $BENCH_NAME`.

## Notes

This wrapper was extended by need from pr to pr and hasn't experienced much
refactoring, so it is not in the best shape. There is still room for
improvements in the testing practice.

## Usage

`$ cargo run -p benchmark-tests --profile=bench bench [-- [FLAGS] [BENCH]]`

or

`$ just full-bench-test`, `$ just full-bench-test-all`.

See the output of `just --show full-bench-test`, ... for the command description
and arguments.

The positional `BENCH` can be a single benchmark. If no positional argument is
given all benchmarks are run.

Other `FLAGS`:

- `--filter=FILTER`: `FILTER` can be a glob expression matching the full
  benchmark name. For example `--filter=test_lib_bench_*` will run all library
  benchmarks.

- `--partition=PARTITION`: `PARTITION` is expected to be in the format
  `part/total` where `total` is the total number of parts in which the matched
  benchmarks should be split. `part` is the selected part to be run. For example
  `--partition=1/2` splits the benchmarks in half and then runs the first half
  of it.

## Adding a new benchmark test

### Basic structure

Library benchmark tests go into `benches/lib_bench` and binary benchmark tests
into `benches/bin_bench`. The naming scheme of a new file is for example for a
binary benchmark `benches/bin_bench/foo/test_bin_bench_foo.rs` and for a library
benchmark `benches/lib_bench/foo/test_lib_bench_foo.rs`. After you have created
the new directory and file, have a look at the `Cargo.toml` of this package and
then add

```toml
[[bench]]
harness = false
name = "test_bin_bench_foo"
path = "benches/test_bin_bench/foo/test_bin_bench_foo.rs"
```

You can now start to set up your test case in the benchmark file. Run the
benchmark for example with `just bench-test test_lib_bench_foo`.

### Configuration

In the current state this new benchmark won't run in the CI or with
`just full-bench-test`. Adding a yaml file with the same name as the benchmark
file but with the extension `.conf.yml` is required, too.

For example, if the benchmark file name is
`benches/test_bin_bench/foo/test_bin_bench_foo.rs`, the configuration file name
is `benches/test_bin_bench/foo/test_bin_bench_foo.conf.yml`.

The basic structure of this configuration file:

```yaml
# Top-level (Mandatory)
groups:
    # An array of benchmark suites.
    #
    # Each suite creates a new pristine state and the benchmark output files are
    # deleted.
    - runs:
          # An array of benchmark runs. The output files are not deleted after a
          # benchmark run here.
          #
          # `args` (Mandatory): The arguments for `cargo bench -- ARGS`. ARGS are
          # passed to gungraun
          - args: []
            # `expected` (Optional): Define the expectation values for this benchmark
            # run.
            #
            # TODO: Add missing `expected` values
            expected:
                # `files` (Optional): Takes a path to a file in the same folder as the
                # conf file containing the expected output files of this benchmark
                # run.
                files: expected_files.1.yml
                # `stdout` (Optional): A path to a file in the same folder as the
                # conf file containing the expected stdout of this benchmark run.
                stdout: expected_stdout.1
```

An example of a configuration file which runs two benchmark suites for the
benchmark file within the same folder. We're not testing much here besides that
the benchmark suites don't cause an exit with error or panic. Setting the
expectation values would be required to validate the output of the benchmark
runs, check that all expected files are present etc.

```yaml
groups:
    - runs:
          - args: ["--nocapture"]
    # The output files of the previous benchmark suite are deleted
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
toolchain in use etc. Having all benchmark tests to update every time something
changes by `1` or `2` up or down is unmanageable. So, this is a simple method to
check if there are numbers, but we do not check the numbers themselves. Most
often, this is sufficient but stills needs improvement. For example being able
to check if all numbers are 0 which is usually an indicator for something going
wrong.

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

```yaml
groups:
    - runs:
          - args: []
            expected:
                files: expected_files
```

TODO

See for other examples in the `benches` folder.

##### Expected exit code

```yaml
groups:
    - runs:
          - args: []
            expected:
                exit_code: 0
```

TODO

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

TODO:

See for example `benches/test_bin_bench/exit_with`

#### Other configuration values

##### bench args

Arguments passed to the `cargo bench` invocation

```yaml
groups:
    - runs:
          - args: []
            cargo_args: ["--features", "cachegrind"]
```

##### rust version

```yaml
groups:
    - runs:
          - args: []
            rust_version: ">=1.73"
          - args: ["--nocapture"]
            rust_version: "<1.73"
```

The first benchmark will only run if the rust version is `>= 1.73`. The second
benchmark will only run if the rust version is `< 1.73`. You can then set the
expected values depending on the rust version.

##### runs_on

```yaml
groups:
    - runs:
          - args: []
            runs_on: "x86_64-unknown-linux-gnu"
```

The above benchmark run will only run on the `x86_64-unknown-linux-gnu` target.
Or, for all benchmarks in a run group:

```yaml
groups:
    - runs_on: "x86_64-unknown-linux-gnu"
      runs:
          - args: []
```

The target can be prefixed with a `!` to indicate to not run on this target.

```yaml
groups:
    - runs_on: "x86_64-unknown-linux-gnu"
      runs:
          - args: ["--nocapture"]
    - runs_on: "!x86_64-unknown-linux-gnu"
      runs:
          - args: []
```

##### rmdirs

```yaml
groups:
    - runs:
          - args: []
            rmdirs: ["/tmp/gungraun_test_dir"]
```

This instruction is used to remove directories before a benchmark run.

##### flaky

If tests are flaky, they can be tried multiple times:

```yaml
groups:
    - runs:
          - args: []
            flaky: 3
```
