<!-- markdownlint-disable MD025 MD042 -->

# Summary

- [Introduction](./intro.md)
- [Getting Help](./getting_help.md)

# Installation

- [Prerequisites](./installation/prerequisites.md)
- [Gungraun](./installation/gungraun.md)
- [CI](./installation/ci.md)

# Benchmarks

- [Overview](./benchmarks/overview.md)
- [Benchmarking Best Practices](./best_practices.md)
- [Library Benchmarks](./benchmarks/library_benchmarks.md)
    - [Important Default Behaviour](./benchmarks/library_benchmarks/important.md)
    - [Quickstart](./benchmarks/library_benchmarks/quickstart.md)
    - [Structure of a Library Benchmark](./benchmarks/library_benchmarks/structure.md)
    - [The Macros in More Detail](./benchmarks/library_benchmarks/macros.md)
    - [Setup and Teardown](./benchmarks/library_benchmarks/setup_and_teardown.md)
    - [Specify Multiple Benches at Once](./benchmarks/library_benchmarks/multiple_benches.md)
    - [Generic Benchmark Functions](./benchmarks/library_benchmarks/generic.md)
    - [Comparing Benchmark Functions](./benchmarks/library_benchmarks/compare_by_id.md)
    - [Configuration](./benchmarks/library_benchmarks/configuration.md)
        - [Output Format/Cache Misses](./benchmarks/library_benchmarks/configuration/output_format.md)
        - [Sandbox](./benchmarks/library_benchmarks/configuration/sandbox.md)
    - [Custom Entry Points](./benchmarks/library_benchmarks/custom_entry_point.md)
    - [Multi-Threaded and Multi-Process Applications](./benchmarks/library_benchmarks/threads_and_subprocesses.md)
    - [More Examples, please!](./benchmarks/library_benchmarks/examples.md)
- [Binary Benchmarks](./benchmarks/binary_benchmarks.md)
    - [Important Default Behaviour](./benchmarks/binary_benchmarks/important.md)
    - [Quickstart](./benchmarks/binary_benchmarks/quickstart.md)
    - [Differences to Library Benchmarks](./benchmarks/binary_benchmarks/differences.md)
    - [The Command's Stdin and Simulating Piped Input](./benchmarks/binary_benchmarks/stdin_and_pipe.md)
    - [Configuration](./benchmarks/binary_benchmarks/configuration.md)
        - [Delay the Command](./benchmarks/binary_benchmarks/configuration/delay.md)
        - [Configure the Exit Code of the Command](./benchmarks/binary_benchmarks/configuration/exit_code.md)
    - [Low-Level API](./benchmarks/binary_benchmarks/low_level.md)
    - [More Examples Needed?](./benchmarks/binary_benchmarks/examples.md)

- [Detecting Performance Regressions](./regressions.md)
- [Cachegrind](./cachegrind.md)
- [Heap Profiling with DHAT](./dhat.md)
- [Perf](./perf.md)
- [Other Valgrind Tools](./tools.md)
- [Valgrind Client Requests](./client_requests.md)
- [Callgrind Flamegraphs](./flamegraphs.md)

# Command-line and environment variables

- [Basic Usage and Exit Codes](./cli_and_env/basics.md)
- [Comparing with Baselines](./cli_and_env/baselines.md)
- [Running Benchmarks in Parallel](./cli_and_env/parallel.md)
- [Running Tools with a Custom Runner](./cli_and_env/tool_runner.md)
- [Controlling the Output of Gungraun](./cli_and_env/output.md)
    - [Customize the Output Directory](./cli_and_env/output/out_directory.md)
    - [Machine-Readable Output](./cli_and_env/output/machine_readable.md)
    - [Showing Terminal Output of Benchmarks](./cli_and_env/output/terminal_output.md)
    - [Changing the Color Output](./cli_and_env/output/color.md)
    - [Changing the Logging Output](./cli_and_env/output/logging.md)

# Migration

- [Migrating from Iai-Callgrind to Gungraun](./migration/iai-callgrind-to-gungraun.md)

# Troubleshooting

- [I'm Getting the Error `Sentinel ... Not Found`](./troubleshooting/im-getting-the-error-sentinel-not-found.md)
- [Running `cargo bench` Results in an "Unrecognized Option" Error](./troubleshooting/running-cargo-bench-results-in-an-unrecognized-option-error.md)

# Comparison

- [Criterion](./comparison/criterion.md)
- [Iai](./comparison/iai.md)
