<!-- spellchecker: ignore nofile nocapture -->

<h1 align="center">Gungraun</h1>

<div align="center">High-precision and consistent benchmarking framework/harness for Rust</div>

This package is not published and exists merely for testing the client requests
implementation of the [gungraun](../gungraun) package.

# For developers

To be able to run these tests you need
[cross](https://github.com/cross-rs/cross) and the rust stable toolchain
installed. For all currently supported test targets consult the
[Cross.toml](../Cross.toml) file.

We don't need to test the client requests itself, that's the duty of `Valgrind`.
We only need to test, that we're executing the correct client requests if we're
running the tests under Valgrind and that we're using the correct magic sequence
in assembly for optimized targets.

## Structure

We use a custom build based on the official cross docker `main` images.

First we need Valgrind to be built for the target and finally installed in the
`/target` directory. We move the temporary Valgrind directory into the `/target`
folder during the build in [build.rs](./build.rs). The `/target` is the only
directory which is made available for us in the `qemu` system image.

Secondly, we adjust the `qemu` initrd image a little bit to be able to run
Valgrind `--tool=memcheck`. For a full run-down consult the content of the
[docker](../docker) folder.

We also wrap the actual Valgrind invocation in a `valgrind-wrapper` binary to be
able to filter the output of Valgrind, so it is architecture agnostic. All other
binaries in the `bin` directory are test binaries.

The exact numbers, backtraces and the other filtered output is not of interest,
since we just check for side effects to ensure we've executed the client
request. For example for some client requests, Valgrind prints some output to
the logging output and when running it with `--verbose`.

We end all test binaries with a call to
`std::process::exit(client_requests::valgrind::running_on_valgrind() as i32)`,
so we exit with `0` when not running under Valgrind and with `1` when running
under Valgrind. There's no need for us to test `valgrind` in `valgrind` and
higher exit codes than `1`.

## Running the tests

Testing for `x86_64`:

`cross +stable test -p client-request-tests --test tests --target x86_64-unknown-linux-gnu --release -- --nocapture`

Testing for `s390x`:

`cross +stable test -p client-request-tests --test tests --target s390x-unknown-linux-gnu --release -- --nocapture`

You might need to export the environment variable
`CROSS_CONTAINER_OPTS='--ulimit nofile=1024:4096'` to be able to run the tests.

See also the [github workflow](../.github/workflows/cicd.yml)
