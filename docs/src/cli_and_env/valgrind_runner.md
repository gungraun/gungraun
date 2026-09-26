<!-- spell-checker: ignore unsetenv -->

# Running Valgrind with a Custom Runner

By default, Gungraun invokes Valgrind directly to run your benchmarks. However,
if Valgrind is not available on your host system, you can use the
`--valgrind-runner` option to run Valgrind through a container or alternative
execution environment.

This is controlled by several command-line options and their respective
environment variables:

- `--valgrind-runner` (env: `GUNGRAUN_VALGRIND_RUNNER`): Specify the runner
  executable. This can be anything from a script to `docker`, `sudo`, ...
- `--valgrind-runner-args` (env: `GUNGRAUN_VALGRIND_RUNNER_ARGS`): Pass
  additional arguments to the runner (supports environment variable
  interpolation)
- `--valgrind-bin` (env: `GUNGRAUN_VALGRIND_BIN`): Specify the Valgrind
  executable path.
- `--valgrind-runner-dest` (env: `GUNGRAUN_VALGRIND_RUNNER_DEST`): Override the
  destination directory of Valgrind output files
- `--valgrind-runner-root` (env: `GUNGRAUN_VALGRIND_RUNNER_ROOT`): Override the
  path to the workspace root directory

The command-line options take precedence over environment variables.

If using a custom runner, the custom runner is responsible for a stable
benchmark runtime environment:

- clear the environment variables
- disable ASLR, if asked to do so
- if `--valgrind-runner-root` or `--valgrind-runner-dest` are used, to create
  these directories
- eventually run Valgrind to create the output files. Failing to produce the
  output files will fail the benchmark.

## How Gungraun Invokes the Runner

When a custom runner is specified, Gungraun invokes it as follows:

```shell
<RUNNER> [RUNNER_ARGS] <VALGRIND_BIN> [VALGRIND_ARGS] <BENCHMARK> [BENCHMARK_ARGS]
```

Key points:

- `<RUNNER_ARGS>` are additional arguments for the runner from
  `--valgrind-runner-args`
- `<VALGRIND_BIN>` is the resolved path to Valgrind (from `--valgrind-bin` or
  system default)
- Your normal `<VALGRIND_ARGS>` for example set with `--callgrind-args`,
  `--cachegrind-args`, etc. are passed through to Valgrind after the runner
- The benchmark binary is passed as Valgrind's first positional argument. After
  that all arguments to the benchmark binary are added as last arguments.
- All paths set by Gungraun in the Valgrind arguments are absolute paths
- Environment variables are still cleared by default. Configure this behavior
  with [`LibraryBenchmarkConfig`] or [`BinaryBenchmarkConfig`] in the benchmarks
  or in the CLI with `--env-clear` (env: `GUNGRAUN_ENV_CLEAR`)

## Environment Variables for Runners

Gungraun sets the following environment variables for the custom Valgrind runner
process even if the environment variables have been asked to be cleared. The
custom Valgrind runner is responsible for clearing the environment variables.

- `GUNGRAUN_VR_DEST_DIR`: Destination directory for output files
- `GUNGRAUN_VR_HOME`: The gungraun home directory
- `GUNGRAUN_VR_WORKSPACE_ROOT`: The project's workspace root directory
- `GUNGRAUN_ALLOW_ASLR`: `yes` or `no` based on `--allow-aslr` setting

These can be interpolated in `--valgrind-runner-args` like any other environment
variable using the `${VAR}` syntax.

## Passing Additional Arguments

Use `--valgrind-runner-args` to pass additional arguments to your wrapper:

```bash
cargo bench -- \
    --valgrind-runner=~/bin/docker-valgrind \
    --valgrind-runner-args='--volume=/my/project:/project:ro'
```

### Environment Variable Interpolation

The `--valgrind-runner-args` option supports environment variable interpolation
using the `${VAR}` syntax. Variables are resolved in this order:

1. `GUNGRAUN_VR_*` variables set by Gungraun and `GUNGRAUN_ALLOW_ASLR`
2. Variables specified via `--envs` and in the benchmark with a
   `LibraryBenchmarkConfig` or `BinaryBenchmarkConfig`
3. System environment variables. This happens before the environment variables
   are cleared, so all environment variables passed to the `cargo bench`
   invocation are available for interpolation.

This allows passing dynamic configuration to your runner:

```bash
cargo bench -- \
    --valgrind-runner=./docker-wrapper \
    --valgrind-runner-args='--allow-aslr=${GUNGRAUN_ALLOW_ASLR} --dest=${GUNGRAUN_VR_DEST_DIR}'
```

## Path Substitution

When running in containers, paths on the host may differ from paths inside the
container. Use these options to help the runner use the correct directories and
files. If using docker as custom runner, then most likely you will need to
specify all of them. Note that Gungraun doesn't create any directories given by
`--valgrind-runner-dest` and `--valgrind-runner-root`. It is your responsibility
to create these directories before the first custom runner execution.

### `--valgrind-runner-dest`

Specifies a path prefix that replaces all occurrences of the value of
`GUNGRAUN_VR_DEST_DIR` in the Valgrind command line arguments. Useful when using
a container volume mount that is different from the destination directory on the
host. The destination directory on the host is guaranteed to be different for
each benchmark and Valgrind runner execution, so it is safe to mount the
original `GUNGRAUN_VR_DEST_DIR` to the same directory for each invocation.
However, make sure you have write permissions to that directory and it is empty
before the first invocation of the custom Valgrind runner.

> **Warning** Setting this variable to a non-empty directory or to a directory
> which contains important data will potentially destroy the contained data.
> Gungraun repeatedly dumps files in this directory and then either deletes
> **all** of the contained files or moves **all** content to the final
> destination directory on the host.

If the directory `/tmp/dest` in the docker container exists:

```bash
cargo bench -- \
    --valgrind-runner=docker \
    --valgrind-runner-dest=/tmp/dest
    --valgrind-runner-args='run --volume ${GUNGRAUN_VR_DEST_DIR}:/tmp/dest alpine'
```

### `--valgrind-runner-root`

Specifies a path prefix that replaces all occurrences in paths with the value of
`GUNGRAUN_VR_WORKSPACE_ROOT` in the Valgrind command line arguments. Useful when
a container mount of the workspace root is different from the host workspace
root. When running library benchmarks, the compiled benchmark binary (somewhere
located in `${GUNGRAUN_VR_WORKSPACE_ROOT}/target/release/deps/some_name-1a2b3c`)
needs to be executed in the container which you could achieve with something
similar to:

```bash
cargo bench -- \
    --valgrind-runner=docker \
    --valgrind-runner-root=/workspace
    --valgrind-runner-args='run --volume ${GUNGRAUN_VR_WORKSPACE_ROOT}:/workspace alpine'
```

## Running Valgrind in a Container

### Docker/Podman Example

> **Note:** You'll need a Docker image with Valgrind installed. For example:
>
> ```dockerfile
> FROM ubuntu:24.04
> RUN apt-get update && \
> 	  apt-get install -y libc6-dbg valgrind && \
> 	  mkdir -p /workspace /var/gungraun-dest
> ```
>
> Build and tag the image: `docker build -t valgrind-image .`

Then use it with Gungraun and execute the following in your workspace root (the
directory with the top-level `Cargo.toml` file). Here, `--valgrind-bin` points
to the Valgrind executable in the container although the path might be the same
on the host:

```bash
cargo bench -- \
      --clear-env=no \
      --valgrind-runner=docker \
      --valgrind-bin=/usr/bin/valgrind \
      --valgrind-runner-root=/workspace \
      --valgrind-runner-dest=/var/gungraun-dest \
      --valgrind-runner-args='run --unsetenv-all' \
      --valgrind-runner-args='-v ${GUNGRAUN_VR_WORKSPACE_ROOT}:/workspace' \
      --valgrind-runner-args='-v ${GUNGRAUN_VR_DEST_DIR}:/var/gungraun-dest' \
      --valgrind-runner-args='valgrind-image'
```

`--clear-env=no` is set, since `docker` needs some of the environment variables
and we can configure `docker` with `--unsetenv-all` to clear the environment
variables for the container.

[`LibraryBenchmarkConfig`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.LibraryBenchmarkConfig.html
[`BinaryBenchmarkConfig`]:
    https://docs.rs/gungraun/0.20.0/gungraun/struct.BinaryBenchmarkConfig.html
