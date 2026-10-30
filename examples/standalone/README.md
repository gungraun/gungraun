<!-- spell-checker: ignore DCMAKE -->

# Standalone C example

This directory is a complete non-Rust consumer: a C workload, its CMake build,
and a TOML benchmark manifest. It needs a `gungraun-runner` build containing the
standalone commands and a supported Valgrind installation. Published `0.20.0`
binaries do not contain these commands.

From this directory, with that runner in `PATH`:

```shell
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
gungraun-runner run --config perf-guard.toml
gungraun-runner check --config perf-guard.toml
```

Use `--allow-aslr` on either command if your container forbids disabling ASLR.
There is no Cargo project to generate or compile. To build without CMake:

```shell
mkdir -p build
cc -std=c11 -O2 -g workload.c -o build/workload
```

The workload creates its input array before `gungraun_example_region`, sums it
inside the region, and checks the result afterwards. The two cases contain 32
and 1000 additions respectively. Gungraun reports the complete measured
instruction count; it does not divide by either number. In a C++ program, use
`extern "C"` for a similarly simple symbol name and keep the function from being
inlined or stripped.

The generous example caps demonstrate enforcement; they are not suggested limits
for another program. Remove the two `hard_limits` tables to try uncapped
capture. `run` identifies the uncapped cases; `check` rejects that manifest
before execution. Setting a cap to `1` demonstrates an exceeded-limit failure.
Neither command changes the manifest.

Profiles, logs, and native `summary.json` files for parsed measurements are
written below `target/gungraun/standalone/cases`. They remain available after a
failed guard. An execution failure may prevent a summary from being produced;
the exit status, diagnostics, and any native files that were created remain the
evidence of that failed run.

The configuration file's resolved directory anchors the executable, working
directory, and output paths, so invocation from another directory also works:

```shell
gungraun-runner check --config /absolute/path/to/standalone/perf-guard.toml
```

See the [standalone guide] for the schema, installation, CI, failure contract,
and a migration example.

[standalone guide]: ../../docs/src/benchmarks/binary_benchmarks/standalone.md
