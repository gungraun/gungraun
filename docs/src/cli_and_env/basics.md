# Basic Usage

It's possible to pass arguments to Gungraun separated by `--`
(`cargo bench -- ARGS`). If you're running into the error `Unrecognized Option`,
see
[Troubleshooting](../troubleshooting/running-cargo-bench-results-in-an-unrecognized-option-error.md).
For a complete rundown of possible arguments, execute `gungraun-runner --help`.
Almost all command-line arguments have a corresponding environment variable. The
environment variables which don't have a corresponding command-line argument
are:

- `GUNGRAUN_COLOR`: [Control the colored output of Gungraun](./output/color.md)
  (Default is `auto`)
- `GUNGRAUN_LOG`: [Define the log level](./output/logging.md) (Default is
  `WARN`)

## Exit Codes

- **0**: Success
- **1**: All other errors
- **2**: Parsing command-line arguments failed
- **3**: One or more regressions occurred

## The Command-Line Arguments

For an up-to-date list for your Gungraun version run `gungraun-runner --help`:

<!-- start: gungraun-runner-help -->

````text
High-precision, one-shot and consistent benchmarking framework/harness for Rust

Boolish command line arguments take also one of `y`, `yes`, `t`, `true`, `on`, `1`
instead of `true` and one of `n`, `no`, `f`, `false`, `off`, and `0` instead of
`false`

Usage: cargo bench ... -- [OPTIONS | FILTER]

Arguments:
  [FILTER]
          If specified, only run benchmarks matching this wildcard pattern

          The wildcard pattern can contain `*` to match any amount of characters, `?` to match a
          single character and simple classes `[...]` like `[abc] `to match the characters `a` or `b`
          or `c`. Character classes can contain ranges, so `[abc]` could be rewritten as `[a-c]` and
          they can be negated with `[!...]` to not match the contained characters.

          This pattern matches the whole module path of benchmarks. A list of all benchmarks with
          their module path as recognized by this option can be obtained by running `--list`. The
          general structure of the module path of a benchmark is:

          `FILENAME::GROUP::FUNCTION::ID`

          Examples:
            * `*::my_benchmark_id` runs all benchmarks with the id `my_benchmark_id`
            * `gungraun_benchmarks::*` runs all benchmarks in the file `gungraun_benchmarks`
            * `my_file::some_group::*` runs all benchmarks in the file `my_file` and the group
              `some_group`

          [env: GUNGRAUN_FILTER=]

Options:
      --list[=<LIST>]
          Print a list of all benchmarks. With this argument no benchmarks are executed.

          The output format is intended to be the same as the output format of the libtest harness.
          However, future changes of the output format by cargo might not be incorporated into
          gungraun. As a consequence, it is not considered safe to rely on the output in scripts.

          Combine with `--format terse` for `cargo nextest`-compatible output (per-benchmark lines
          only, no trailing blank line or summary). `--list --format terse --ignored` emits an empty
          list because gungraun has no ignored-benchmark concept.

          [env: GUNGRAUN_LIST=]
          [default: false]
          [possible values: true, false]

      --allow-aslr[=<ALLOW_ASLR>]
          Allow ASLR (Address Space Layout Randomization)

          If possible, ASLR is disabled on platforms that support it (linux, freebsd) because ASLR
          could noise up the Callgrind cache simulation results a bit. Setting this option to `true`
          runs all benchmarks with ASLR enabled.

          See also [kernel.org: randomize_va_space]

          [kernel.org: randomize_va_space]:
          https://docs.kernel.org/admin-guide/sysctl/kernel.html#randomize-va-space

          [env: GUNGRAUN_ALLOW_ASLR=]
          [possible values: true, false]

      --env-clear [<ENV_CLEAR>]
          Control whether environment variables are cleared before running a benchmark

          By default (`true`), environment variables are cleared to ensure reproducible benchmark
          results across different environments. Set to `false` to preserve all environment variables
          of the `cargo bench` process.

          Examples:
            * `--env-clear` (default: clear environment)
            * `--env-clear=false` (preserve environment)

          [env: GUNGRAUN_ENV_CLEAR=]
          [possible values: true, false]

      --envs=<ENVS>
          Set environment variables for benchmarks ignoring the clearing of environment variables

          Environment variables can be specified in two forms:
          - `KEY=VALUE`: Set `KEY` to `VALUE` explicitly
          - `KEY`: Resolve `KEY` from the current environment and pass its value

          Multiple key-value pairs can be specified in a single invocation using space-separated
          values (posix-style quoting of values is supported). The `--envs` argument can also be
          specified multiple times to accumulate environment variables.

          These variables are cumulative to any environment variables configured via
          `LibraryBenchmarkConfig::env` or `BinaryBenchmarkConfig::env`.

          Examples:
            * `--envs=FOO=bar` (set FOO to "bar")
            * `--envs=FOO` (pass the original value of FOO from current environment)
            * `--envs='FOO=bar BAZ=qux'` (set multiple variables and once)
            * `--envs=FOO=bar --envs=BAZ=qux` (accumulate multiple times)

          [env: GUNGRAUN_ENVS=]

      --home <HOME>
          Specify the home directory of gungraun benchmark output files

          All output files are by default stored under the `$PROJECT_ROOT/target/gungraun` directory.
          This option lets you customize this home directory, and it will be created if it doesn't
          exist.

          [env: GUNGRAUN_HOME=]

      --parallel[=<PARALLEL>]
          Number of benchmarks to run in parallel.

          A value of `1` runs benchmarks serially which is the default if this option is not
          specified. Passing `auto` lets the runner choose the parallelism level based on available
          hardware which is the number of available logical cores.

          Note that benchmark groups are used as synchronization points and only benchmarks within
          the same group are executed in parallel.

          Valgrind and gungraun perform disk I/O even if your benchmarks don't. This is usually a
          bottleneck, so running with parallelism of 10 may provide similar speedup as 5. Actual
          results depend on the hardware and if your benchmarks are performing disk I/O, too.

          Examples: * --parallel=4 * --parallel=auto

          [env: GUNGRAUN_PARALLEL=]
          [default: 1]

      --separate-targets[=<SEPARATE_TARGETS>]
          Separate gungraun benchmark output files by target

          The default output path for files created by Gungraun and Valgrind during the benchmark is

          `target/gungraun/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`.

          This can be problematic if you're running the benchmarks not only for a single target
          because you end up comparing the benchmark runs with the wrong targets. Setting this option
          changes the default output path to

          `target/gungraun/$TARGET/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`

          Although not as comfortable and strict, you could achieve a separation by target also with
          baselines and a combination of `--save-baseline=$TARGET` and `--baseline=$TARGET` if you
          prefer having all files of a single $BENCH in the same directory.

          [env: GUNGRAUN_SEPARATE_TARGETS=]
          [default: false]
          [possible values: true, false]

      --valgrind-bin <VALGRIND_BIN>
          Specify the path to the Valgrind executable

          By default, Gungraun searches for `valgrind` in the system PATH. This option
          allows specifying an alternative Valgrind executable. When used with
          `--tool-runner`, this path is passed to the runner as the Valgrind binary
          to invoke.

          Note: The specified path is not validated for existence. If the path is invalid, the
          benchmark will fail when attempting to execute Valgrind.

          Examples:
            * `--valgrind-bin=/usr/local/bin/valgrind`
            * `--valgrind-bin=/doesnotexist` (used with `--tool-runner` for container setups)

          [env: GUNGRAUN_VALGRIND_BIN=]

      --workspace-root <WORKSPACE_ROOT>
          Override the Cargo workspace root

          By default, Gungraun queries `cargo metadata` to detect the workspace root. This is usually
          correct, but this option provides the workspace root explicitly and avoids that part of the
          Cargo metadata query.

          To run without invoking `cargo` at all, also provide an explicit output location with
          `--home`/`GUNGRAUN_HOME` or set `CARGO_TARGET_DIR`. If neither is provided, Gungraun still
          queries `cargo metadata` to detect Cargo's effective target directory.

          This can be useful when `gungraun-runner` executes in an environment where `cargo` is not
          available, such as a container, VM, or emulator image.

          Examples:
            * `--workspace-root=/project`
            * `GUNGRAUN_WORKSPACE_ROOT=/project GUNGRAUN_HOME=/tmp/gungraun cargo bench`

          [env: GUNGRAUN_WORKSPACE_ROOT=]

      --tool-runner <TOOL_RUNNER>
          Specify an alternative executable to run a tool invocation

          By default, Gungraun runs the selected tool directly. This option allows specifying an
          alternative runner executable that will be invoked instead, with the selected tool binary
          passed as an argument to the runner.

          When specified, the runner is invoked as:
            `<RUNNER> [RUNNER_ARGS...] <TOOL_BIN> [TOOL_ARGS...] <BENCHMARK>
            [BENCHMARK_ARGS...]`

          The runner receives extra environment variables that provide context:
          - `GUNGRAUN_TR_DEST_DIR`: The destination directory for tool output files
          - `GUNGRAUN_TR_HOME`: The gungraun home (`--home`) directory
          - `GUNGRAUN_TR_WORKSPACE_ROOT`: The project's workspace root directory
          - `GUNGRAUN_ALLOW_ASLR`: `yes` or `no` (the default) based on `--allow-aslr` setting

          Environment variables in `--tool-runner-args` are interpolated using `${VAR}` syntax.
          The interpolation priority is: `GUNGRAUN_TR_*` variables first, then `--envs` variables,
          then the system environment.

          This is useful for running benchmarks in containers or other environments where the tool is
          not available on the host. See the online guide for detailed examples.

          Examples:
            * --tool-runner=docker
            * --tool-runner=/path/to/wrapper
            --tool-runner-args='--some-flag=${GUNGRAUN_ALLOW_ASLR}'

          [env: GUNGRAUN_TOOL_RUNNER=]

      --tool-runner-args <TOOL_RUNNER_ARGS>
          Additional arguments to pass to the tool runner executable

          This option is only effective when `--tool-runner` is specified. The arguments are
          passed to the runner executable after `--tool-runner` and before the tool path.

          Environment variable interpolation is supported using the `${VAR}` syntax. Variables are
          resolved in this order:
          1. `GUNGRAUN_TR_*` variables set by Gungraun (see `--tool-runner` for the list)
          2. Variables specified via `--envs` and `LibraryBenchmarkConfig::envs` or
             `BinaryBenchmarkConfig::envs`
          3. System environment variables

          The interpolation allows passing dynamic values to the runner based on Gungraun's
          configuration. For example, `${GUNGRAUN_ALLOW_ASLR}` interpolation is useful for passing
          the ASLR setting to container setups.

          Examples:
            * --tool-runner=sudo --tool-runner-args='--user=foo'
            * --tool-runner=wrapper '--tool-runner-args=--allow-aslr=${GUNGRAUN_ALLOW_ASLR}'

          [env: GUNGRAUN_TOOL_RUNNER_ARGS=]

      --tool-runner-dest <TOOL_RUNNER_DEST>
          Override the destination directory path for tool runner output files

          This option is only effective when `--tool-runner` is specified. By default, tool
          output files are written to paths under the gungraun home directory or in temporary
          directories. This option allows substituting this path with a custom directory.

          When specified, any occurrence of this path prefix in tool arguments will be replaced
          with the directory path specified by `--tool-runner-dest`.

          WARNING: Make sure the directory of this argument exists, is empty and doesn't point to a
          directory with important files in it! This directory is managed by Gungraun and Gungraun
          might delete **all** files in this directory. More details can be found in the online
          guide.

          Examples:
            * `--tool-runner-dest=/tmp/results`

          [env: GUNGRAUN_TOOL_RUNNER_DEST=]

      --tool-runner-root <TOOL_RUNNER_ROOT>
          Override the workspace root path for the tool runner

          This option is only effective when `--tool-runner` is specified. It allows substituting
          the workspace root path prefix in the benchmark executable path and all other tool
          arguments.

          This can be useful for container setups where the workspace is mounted at a different
          location inside the container.

          Examples:
            * `--tool-runner-root=/workspace`

          [env: GUNGRAUN_TOOL_RUNNER_ROOT=]

      --baseline[=<BASELINE>]
          Compare benchmark results against a previously saved baseline

          This option compares the current benchmark run against a named baseline from a previous run
          without modifying the saved baseline. Baselines store benchmark results for future
          comparisons, useful for tracking performance over time or comparing against fixed reference
          points like a release tag or main branch.

          If this option is specified but no baseline with that name exists yet, Gungraun creates a
          new baseline with the current results instead of comparing.

          See also: `--save-baseline` to create or update a baseline, `--load-baseline` to compare
          existing baselines without running benchmarks.

          Examples: * `--baseline` (uses the default baseline name "default") * `--baseline=main`
          (compares against baseline saved as "main") * `--baseline=v1.0` (compares against baseline
          saved as "v1.0")

          [env: GUNGRAUN_BASELINE=]

      --load-baseline[=<LOAD_BASELINE>]
          Load an existing baseline instead of running new benchmarks

          This option loads benchmark results from a previously saved baseline and uses them as the
          "new" data for comparison against another baseline. This allows comparing two existing
          baselines without re-running any benchmarks.

          This option requires `--baseline` to be specified, which provides the "old" baseline to
          compare against.

          This is useful for: - Re-comparing existing baselines with different comparison targets -
          Comparing two previously saved baselines against each other - Avoiding expensive benchmark
          re-runs when only analysis is needed

          See also: `--baseline` to compare against a baseline while running new benchmarks,
          `--save-baseline` to create or update a baseline.

          Examples: * `--load-baseline --baseline=main` (loads "default", compares against "main") *
          `--load-baseline=feature --baseline=main` (loads "feature", compares against "main") *
          `--load-baseline=v1.1 --baseline=v1.0` (loads "v1.1", compares against "v1.0")

          [env: GUNGRAUN_LOAD_BASELINE=]

      --save-baseline[=<SAVE_BASELINE>]
          Save benchmark results as a named baseline for future comparisons

          If a baseline with this name already exists, Gungraun first compares against it before
          overwriting with the new results. If this option is used together with `--baseline`,
          Gungraun compares against the baseline selected by `--baseline` and saves the new results
          with the name selected by `--save-baseline`.

          This option is useful for creating reference measurements (like from the main branch or a
          release tag) that you can later compare against using `--baseline`.

          This option conflicts with `--load-baseline`. See `--baseline` to compare against a saved
          baseline without modifying it and `--load-baseline` to compare existing baselines without
          running benchmarks.

          Examples: * `--save-baseline` (uses the default baseline name "default") *
          `--save-baseline=main` (saves as baseline "main") * `--save-baseline=v1.0` (saves as
          baseline "v1.0") * `--save-baseline=pr_1234 --baseline=main` (compares against "main",
          saves as `pr_1234`)

          [env: GUNGRAUN_SAVE_BASELINE=]

      --nocapture[=<NOCAPTURE>]
          Don't capture terminal output of benchmarks

          Possible values are one of [true, false, stdout, stderr].

          This option is currently restricted to the Callgrind run of benchmarks. The output of
          additional tool runs like DHAT, Memcheck, ... is still captured, to prevent showing the
          same output of benchmarks multiple times. Use `GUNGRAUN_LOG=info` to also show captured and
          logged output.

          If no value is given, the default missing value is `true` and doesn't capture stdout and
          stderr. Besides `true` or `false` you can specify the special values `stdout` or `stderr`.
          If `--nocapture=stdout` is given, the output to `stdout` won't be captured and the output
          to `stderr` will be discarded. Likewise, if `--nocapture=stderr` is specified, the output
          to `stderr` won't be captured and the output to `stdout` will be discarded.

          [env: GUNGRAUN_NOCAPTURE=]
          [default: false]

      --nosummary[=<NOSUMMARY>]
          Suppress the summary showing regressions and execution time at the end of a benchmark run

          Note, that a summary is only printed if the `--output-format` is not JSON.

          The summary described by `--nosummary` is different from `--save-summary` and they do not
          affect each other.

          [env: GUNGRAUN_NOSUMMARY=]
          [default: false]
          [possible values: true, false]

      --output-format <OUTPUT_FORMAT>
          The terminal output format in default human-readable format or in machine-readable json
          format

          # The JSON Output Format

          The json terminal output schema is the same as the schema with the `--save-summary`
          argument when saving to a `summary.json` file. All other output than the json output goes
          to stderr and only the summary output goes to stdout. When not printing pretty json, each
          line is a dictionary summarizing a single benchmark. You can combine all lines (benchmarks)
          into an array for example with `jq`

          `cargo bench -- --output-format=json | jq -s`

          which transforms `{...}\n{...}` into `[{...},{...}]`

          Possible values:
          - default:     The default terminal output
          - json:        Json terminal output
          - pretty-json: Pretty json terminal output

          [env: GUNGRAUN_OUTPUT_FORMAT=]
          [default: default]

      --save-summary[=<SAVE_SUMMARY>]
          Save a machine-readable summary of each benchmark run to a JSON file

          This option saves a structured JSON summary of each benchmark run alongside the usual
          benchmark output. The summary file contains benchmark results, metrics, detected
          regressions, and other metadata in a machine-readable format.

          The summary file is saved as `summary.json` in the benchmark's output directory next to the
          other usual benchmark output.

          Available formats: - `json`: Compact JSON without newlines (space-efficient) -
          `pretty-json`: Pretty-printed JSON with indentation (human-readable)

          See also `--output-format` for printing JSON summaries to the terminal instead of saving to
          a file.

          Examples: * `--save-summary` (saves as compact JSON) * `--save-summary=json` (saves as
          compact JSON) * `--save-summary=pretty-json` (saves as pretty-printed JSON)

          Possible values:
          - json:        The format in a space optimal json representation without newlines
          - pretty-json: The format in pretty printed json

          [env: GUNGRAUN_SAVE_SUMMARY=]

      --show-grid[=<SHOW_GRID>]
          Show an ascii grid in the benchmark terminal output

          A matter of taste but the guiding lines can also be helpful reading benchmark output when
          running multiple tools with multiple threads and subprocesses for example by using
          `--show-intermediate`.

          [env: GUNGRAUN_SHOW_GRID=]
          [possible values: true, false]

      --show-intermediate[=<SHOW_INTERMEDIATE>]
          Show intermediate metrics from parts, subprocesses, threads, ... (Default: false)

          In Callgrind, threads are treated as separate units (similar to subprocesses) and the
          metrics for them are dumped into an own file. Other Valgrind tools usually separate the
          output files only by subprocesses. Use this option, to also show the metrics of any
          intermediate fragments and not just the total over all of them.

          Temporarily setting `show_intermediate` to `true` can help to find misconfigurations in
          multi-thread/multi-process benchmarks.

          [env: GUNGRAUN_SHOW_INTERMEDIATE=]
          [possible values: true, false]

      --show-only-comparison[=<SHOW_ONLY_COMPARISON>]
          Show only the comparison between different benchmarks when using `compare_by_id`

          If you're only interested in the comparisons between different benchmarks but not the
          metric
          differences between the self comparisons of the new and old benchmark run, use this option.
          This option is only useful if `compare_by_id` is used in the `library_benchmark_group!` or
          `binary_benchmark_group!`. Note, that it does not prevent any benchmarks to be run,
          especially benchmarks which are not compared to another benchmark. Such benchmarks have
          only
          the usual benchmark headline printed.

          [env: GUNGRAUN_SHOW_ONLY_COMPARISON=]
          [possible values: true, false]

      --tolerance[=<TOLERANCE>]
          Show changes only when they are above the `tolerance` level

          If no value is specified, the default value of `0.000_009_999_999_999_999_999` is based on
          the number of decimal places of the percentages displayed in the terminal output in case of
          differences.

          Negative tolerance values are converted to their absolute value.

          Examples:
            * --tolerance (applies the default value)
            * --tolerance=0.1 (set the tolerance level to `0.1`)

          [env: GUNGRAUN_TOLERANCE=]

      --truncate-description[=<TRUNCATE_DESCRIPTION>]
          Adjust, enable or disable the truncation of the description in the Gungraun output

          The default is to truncate the description to the size of 50 ascii characters. A false
          value disables the truncation entirely and a value will truncate the description to the
          given amount of characters excluding the ellipsis.

          To clarify which part of the output is meant by `DESCRIPTION`:

          ```text
          benchmark_file::group_name::function_name id:DESCRIPTION
            Instructions:              352135|352135          (No change)
            ...
          ```

          Examples:
            * --truncate-description=no (disables truncation)
            * --truncate-description=100 (set the truncation to 100 ascii chars)
            * --truncate-description (this is the default and sets the size of 50 ascii chars)

          [env: GUNGRAUN_TRUNCATE_DESCRIPTION=]

      --cachegrind-metrics <CACHEGRIND_METRICS>...
          Define the Cachegrind metrics and the order in which they are displayed

          This is a `,`-separated list of Cachegrind metric groups and event kinds which are allowed
          to appear in the terminal output of Cachegrind.

          See `--callgrind-metrics` for more details and
          <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetrics.html>
          respectively
          <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetric.html> for valid
          metrics and group members.

          The `group` names, their abbreviations if present and `event` kinds are exactly the same as
          described in the `--cachegrind-limits` option.

          Examples:
            * --cachegrind-metrics='ir' to show only `Instructions`
          * --cachegrind-metrics='@all' to show all possible Cachegrind metrics
            * --cachegrind-metrics='@default,@mr' to show cache miss rates in addition to the
            defaults

          [env: GUNGRAUN_CACHEGRIND_METRICS=]

      --callgrind-metrics <CALLGRIND_METRICS>...
          Define the Callgrind metrics and the order in which they are displayed

          This is a `,`-separated list of Callgrind metric groups and event kinds which are allowed
          to appear in the terminal output of Callgrind. Group names need to be prefixed with '@'.
          The order matters and the Callgrind metrics are shown in their insertion order of this
          option. More precisely, in case of duplicate metrics, the first specified one wins.

          The `group` names, their abbreviations if present and `event` kinds are exactly the same as
          described in the `--callgrind-limits` option.

          For a list of valid metrics, groups and their members see the docs of `CallgrindMetrics`
          (<https://docs.rs/gungraun/latest/gungraun/enum.CallgrindMetrics.html>) and
          `EventKind` <https://docs.rs/gungraun/latest/gungraun/enum.EventKind.html>.

          Note that setting the metrics here does not imply that these metrics are actually
          collected. This option just sets the order and appearance of metrics in case they are
          collected. To activate the collection of specific metrics you need to use
          `--callgrind-args`.

          Examples:
            * --callgrind-metrics='ir' to show only `Instructions`
          * --callgrind-metrics='@all' to show all possible Callgrind metrics
            * --callgrind-metrics='@default,@mr' to show cache miss rates in addition to the defaults

          [env: GUNGRAUN_CALLGRIND_METRICS=]

      --dhat-metrics <DHAT_METRICS>...
          Define the DHAT metrics and the order in which they are displayed

          This is a `,`-separated list of DHAT metric groups and event kinds which are allowed to
          appear in the terminal output of DHAT.

          See `--callgrind-metrics` for more details and
          <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetrics.html> respectively
          <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetric.html> for valid metrics
          and group members.

          The `group` names, their abbreviations if present and `event` kinds are exactly the same as
          described in the `--dhat-limits` option.

          Examples:
            * --dhat-metrics='totalbytes' to show only `Total Bytes`
          * --dhat-metrics='@all' to show all possible DHAT metrics
            * --dhat-metrics='@default,mb' to show maximum bytes in addition to the defaults

          [env: GUNGRAUN_DHAT_METRICS=]

      --drd-metrics <DRD_METRICS>...
          Define the DRD error metrics and the order in which they are displayed

          This is a `,`-separated list of error metrics which are allowed to appear in the terminal
          output of DRD. The `group` and `event` are the same as for `--memcheck-metrics`.

          See `--callgrind-metrics` for more details and
          <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid error
          metrics.

          Since this is a very small set of metrics, there is only one `group`: `@all`

          Examples:
            * --drd-metrics='errors' to show only `Errors`
            * --drd-metrics='@all' to show all possible error metrics (the default)
            * --drd-metrics='err,ctx' to show only errors and contexts

          [env: GUNGRAUN_DRD_METRICS=]

      --helgrind-metrics <HELGRIND_METRICS>...
          Define the Helgrind error metrics and the order in which they are displayed

          This is a `,`-separated list of error metrics which are allowed to appear in the terminal
          output of Helgrind. The `group` and `event` are the same as for `--memcheck-metrics`.

          See `--callgrind-metrics` for more details and
          <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid error
          metrics.

          Examples:
            * --helgrind-metrics='errors' to show only `Errors`
            * --helgrind-metrics='@all' to show all possible error metrics (the default)
            * --helgrind-metrics='err,ctx' to show only errors and contexts

          [env: GUNGRAUN_HELGRIND_METRICS=]

      --memcheck-metrics <MEMCHECK_METRICS>...
          Define the Memcheck error metrics and the order in which they are displayed

          This is a `,`-separated list of error metrics which are allowed to appear in the terminal
          output of Memcheck.

          Since this is a very small set of metrics, there is only one `group`: `@all`

          group ::= "@all"
          event ::=   ( "errors" | "err" )
                    | ( "contexts" | "ctx" )
                    | ( "suppressederrors" | "serr")
                    | ( "suppressedcontexts" | "sctx" )

          See `--callgrind-metrics` for more details and
          <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid
          metrics.

          Examples:
            * --memcheck-metrics='errors' to show only `Errors`
            * --memcheck-metrics='@all' to show all possible error metrics (the default)
            * --memcheck-metrics='err,ctx' to show only errors and contexts

          [env: GUNGRAUN_MEMCHECK_METRICS=]

      --default-tool <DEFAULT_TOOL>
          The default tool used to run the benchmarks

          The standard tool to run the benchmarks is Callgrind but can be overridden with this
          option. Any Valgrind tool can be used:
            * callgrind
            * cachegrind
            * dhat
            * memcheck
            * helgrind
            * drd
            * massif
            * exp-bbv

          This argument matches the tool case-insensitive. Note that using Cachegrind with this
          option to benchmark library functions needs adjustments to the benchmarking functions with
          client-requests to measure the counts correctly. If you want to switch permanently to
          Cachegrind, it is usually better to activate the `cachegrind` feature of gungraun in
          your Cargo.toml. However, setting a tool with this option overrides Cachegrind set with the
          gungraun feature. See the guide for all details.

          [env: GUNGRAUN_DEFAULT_TOOL=]

      --tools <TOOLS>...
          A comma separated list of tools to run additionally to Callgrind or another default tool

          The tools specified here take precedence over the tools in the benchmarks. The Valgrind
          tools which are allowed here are the same as the ones listed in the documentation of
          --default-tool.

          Examples
            * --tools dhat
            * --tools memcheck,drd

          [env: GUNGRAUN_TOOLS=]

      --bbv-args <BBV_ARGS>
          The command-line arguments to pass through to the experimental BBV

          <https://valgrind.org/docs/manual/bbv-manual.html#bbv-manual.usage>. See also the
          description for --callgrind-args for more details and restrictions.

          Examples:
            * --bbv-args=--interval-size=10000
            * --bbv-args='--interval-size=10000 --instr-count-only=yes'

          [env: GUNGRAUN_BBV_ARGS=]

      --cachegrind-args <CACHEGRIND_ARGS>
          The command-line arguments to pass through to Cachegrind

          <https://valgrind.org/docs/manual/cg-manual.html#cg-manual.cgopts>. See also the
          description for --callgrind-args for more details and restrictions.

          Examples:
            * --cachegrind-args=--instr-at-start=no
            * --cachegrind-args='--branch-sim=yes --instr-at-start=no'

          [env: GUNGRAUN_CACHEGRIND_ARGS=]

      --callgrind-args <CALLGRIND_ARGS>
          The command-line arguments to pass through to Callgrind

          <https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options> and the core valgrind
          command-line arguments
          <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>. Note that not all
          command-line arguments are supported especially the ones which change output paths.
          Unsupported arguments will be ignored printing a warning.

          Examples:
            * --callgrind-args=--dump-instr=yes
            * --callgrind-args='--dump-instr=yes --collect-systime=yes'

          [env: GUNGRAUN_CALLGRIND_ARGS=]

      --dhat-args <DHAT_ARGS>
          The command-line arguments to pass through to DHAT

          <https://valgrind.org/docs/manual/dh-manual.html#dh-manual.options>. See also the
          description for --callgrind-args for more details and restrictions.

          Examples:
            * --dhat-args=--mode=ad-hoc

          [env: GUNGRAUN_DHAT_ARGS=]

      --drd-args <DRD_ARGS>
          The command-line arguments to pass through to DRD

          <https://valgrind.org/docs/manual/drd-manual.html#drd-manual.options>. See also the
          description for --callgrind-args for more details and restrictions.

          Examples:
            * --drd-args=--exclusive-threshold=100
            * --drd-args='--exclusive-threshold=100 --free-is-write=yes'

          [env: GUNGRAUN_DRD_ARGS=]

      --helgrind-args <HELGRIND_ARGS>
          The command-line arguments to pass through to Helgrind

          <https://valgrind.org/docs/manual/hg-manual.html#hg-manual.options>. See also the
          description for --callgrind-args for more details and restrictions.

          Examples:
            * --helgrind-args=--free-is-write=yes
            * --helgrind-args='--conflict-cache-size=100000 --free-is-write=yes'

          [env: GUNGRAUN_HELGRIND_ARGS=]

      --massif-args <MASSIF_ARGS>
          The command-line arguments to pass through to Massif

          <https://valgrind.org/docs/manual/ms-manual.html#ms-manual.options>. See also the
          description for --callgrind-args for more details and restrictions.

          Examples:
            * --massif-args=--heap=no
            * --massif-args='--heap=no --threshold=2.0'

          [env: GUNGRAUN_MASSIF_ARGS=]

      --memcheck-args <MEMCHECK_ARGS>
          The command-line arguments to pass through to Memcheck

          <https://valgrind.org/docs/manual/mc-manual.html#mc-manual.options>. See also the
          description for --callgrind-args for more details and restrictions.

          Examples:
            * --memcheck-args=--leak-check=full
            * --memcheck-args='--leak-check=yes --show-leak-kinds=all'

          [env: GUNGRAUN_MEMCHECK_ARGS=]

      --valgrind-args <VALGRIND_ARGS>
          The command-line arguments to pass through to all tools

          The core Valgrind command-line arguments
          <https://valgrind.org/docs/manual/manual-core.html#manual-core.options> which are
          recognized by all tools. More specific arguments for example set with --callgrind-args
          override the arguments with the same name specified with this option.

          Examples:
            * --valgrind-args=--time-stamp=yes
            * --valgrind-args='--error-exitcode=202 --num-callers=50'

          [env: GUNGRAUN_VALGRIND_ARGS=]

      --cachegrind-limits <CACHEGRIND_LIMITS>
          Set performance regression limits for specific Cachegrind metrics

          This is a `,` separate list of CachegrindMetric=limit or CachegrindMetrics=limit
          (key=value) pairs. See the description of --callgrind-limits for the details and
          <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetrics.html>
          respectively <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetric.html>
          for valid metrics and group members.

          See the guide
          (<https://gungraun.github.io/gungraun/latest/html/regressions.html>) for all
          details or replace the format spec in `--callgrind-limits` with the following:

          group ::= "@" ( "default"
                        | "all"
                        | ("cachemisses" | "misses" | "ms")
                        | ("cachemissrates" | "missrates" | "mr")
                        | ("cachehits" | "hits" | "hs")
                        | ("cachehitrates" | "hitrates" | "hr")
                        | ("cachesim" | "cs")
                        | ("branchsim" | "bs")
                        )
          event ::= CachegrindMetric

          Examples:
            * --cachegrind-limits='ir=0.0%'
            * --cachegrind-limits='ir=10000,EstimatedCycles=10%'
            * --cachegrind-limits='@all=10%,ir=10000,EstimatedCycles=10%'

          [env: GUNGRAUN_CACHEGRIND_LIMITS=]

      --callgrind-limits <CALLGRIND_LIMITS>
          Set performance regression limits for specific `EventKinds`

          This is a `,` separate list of EventKind=limit or CallgrindMetrics=limit (key=value) pairs
          with the limit being a soft limit if the number suffixed with a `%` or a hard limit if it
          is a bare number. It is possible to specify hard and soft limits in one go with the `|`
          operator (e.g. `ir=10%|10000`). Groups (CallgrindMetrics) are prefixed with `@`. List of
          allowed groups and events with their abbreviations:

          group ::= "@" ( "default"
                        | "all"
                        | ("cachemisses" | "misses" | "ms")
                        | ("cachemissrates" | "missrates" | "mr")
                        | ("cachehits" | "hits" | "hs")
                        | ("cachehitrates" | "hitrates" | "hr")
                        | ("cachesim" | "cs")
                        | ("cacheuse" | "cu")
                        | ("systemcalls" | "syscalls" | "sc")
                        | ("branchsim" | "bs")
                        | ("writebackbehaviour" | "writeback" | "wb")
                        )
          event ::= EventKind

          See the guide (<https://gungraun.github.io/gungraun/latest/html/regressions.html>)
          for more details, the docs of `CallgrindMetrics`
          (<https://docs.rs/gungraun/latest/gungraun/enum.CallgrindMetrics.html>) and
          `EventKind` <https://docs.rs/gungraun/latest/gungraun/enum.EventKind.html> for a
          list of metrics and groups with their members.

          A performance regression check for an `EventKind` fails if the limit is exceeded. If
          limits are defined and one or more regressions have occurred during the benchmark run,
          the whole benchmark is considered to have failed and the program exits with error and
          exit code `3`.

          Examples:
            * --callgrind-limits='ir=5.0%'
            * --callgrind-limits='ir=10000,EstimatedCycles=10%'
            * --callgrind-limits='@all=10%,ir=5%|10000'

          [env: GUNGRAUN_CALLGRIND_LIMITS=]

      --dhat-limits <DHAT_LIMITS>
          Set performance regression limits for specific DHAT metrics

          This is a `,` separate list of DhatMetrics=limit or DhatMetric=limit (key=value) pairs. See
          the description of --callgrind-limits for the details and
          <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetrics.html> respectively
          <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetric.html> for valid metrics
          and group members.

          See the guide
          (<https://gungraun.github.io/gungraun/latest/html/regressions.html>) for all
          details or replace the format spec in `--callgrind-limits` with the following:

          group ::= "@" ( "default" | "all" )
          event ::=   ( "totalunits" | "tun" )
                    | ( "totalevents" | "tev" )
                    | ( "totalbytes" | "tb" )
                    | ( "totalblocks" | "tbk" )
                    | ( "attgmaxbytes" | "gb" )
                    | ( "attgmaxblocks" | "gbk" )
                    | ( "attendbytes" | "eb" )
                    | ( "attendblocks" | "ebk" )
                    | ( "readsbytes" | "rb" )
                    | ( "writesbytes" | "wb" )
                    | ( "totallifetimes" | "tl" )
                    | ( "maximumbytes" | "mb" )
                    | ( "maximumblocks" | "mbk" )

          `events` with a long name have their allowed abbreviations placed in the same parentheses.

          Examples:
            * --dhat-limits='totalbytes=0.0%'
            * --dhat-limits='totalbytes=10000,totalblocks=5%'
            * --dhat-limits='@all=10%,totalbytes=5000,totalblocks=5%'

          [env: GUNGRAUN_DHAT_LIMITS=]

      --regression-fail-fast[=<REGRESSION_FAIL_FAST>]
          Fail the entire benchmark run on the first performance regression

          When enabled, this option causes Gungraun to stop immediately when a performance regression
          is detected, rather than continuing to run all benchmarks and reporting regressions at the
          end. The program exits with exit code `3` to indicate that one or more regressions
          occurred.

          Performance regressions are defined by limits set via `--callgrind-limits`,
          `--cachegrind-limits`, `--dhat-limits`, and similar options. Without this option, Gungraun
          completes all benchmarks and reports all regressions in a summary at the end.

          See also: `--callgrind-limits`, `--cachegrind-limits`, `--dhat-limits` for defining
          regression limits.

          Examples: * `--regression-fail-fast` (fail on first regression) *
          `--regression-fail-fast=false` (continue running, report at end - default)

          [env: GUNGRAUN_REGRESSION_FAIL_FAST=]
          [possible values: true, false]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

  Exit codes:
      0: Success
      1: All other errors
      2: Parsing command-line arguments failed
      3: One or more regressions occurred

````

<!-- end: gungraun-runner-help -->
