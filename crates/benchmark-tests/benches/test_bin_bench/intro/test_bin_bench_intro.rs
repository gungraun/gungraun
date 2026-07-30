//! This is an introductory example for setting up binary benchmarks with the `#[binary_benchmark]`
//! macro. There's a lot of overlap with the way how library benchmarks are set up, so I recommend
//! reading the documentation for library benchmarks first. Or, have a look at the equivalent of
//! this file but for library benchmarks in
//! `benchmark-tests/benches/test_lib_bench/intro/test_lib_bench_intro.rs`
//!
//! It's best to read all the comments from top to bottom to get a better understanding of the api.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use gungraun::prelude::*;
use gungraun::{Pipe, Sandbox, Stdin, Stdio};

// This constant stores the absolute path to our crate's binary `echo`. Our `echo` is really simple
// and prints all arguments to stdout as the original `echo` would do. In case of multiple arguments
// the output string separates each argument by a space. `echo foo bar` => `foo bar\n`
const ECHO_EXE: &str = env!("CARGO_BIN_EXE_echo");
// Another binary of our crate. The read-file binary reads the content of a file with the path to it
// as its first argument. Then it compares the content of the file with its second argument exiting
// with an error if they don't match.
const READ_FILE_EXE: &str = env!("CARGO_BIN_EXE_read-file");
/// The last of our crate's binaries which reads its input from `Stdin`. Then it echoes back to
/// `Stdout` what it has read.
const PIPE_EXE: &str = env!("CARGO_BIN_EXE_pipe");

// The most simple usage of the `#[binary_benchmark]` macro. No `#[bench]` or `#[benches]` required.
// In contrast to library benchmarks, all functions annotated with `#[binary_benchmark]` need to
// return an `gungraun::Command`.
#[binary_benchmark]
fn simple_bench() -> gungraun::Command {
    // Within the `simple_bench` function we're building the `Command`, but nothing's getting
    // executed, yet.
    gungraun::Command::new(ECHO_EXE)
}

// Let's take it a step further and make use of the `#[bench]` attribute. As in library benchmarks
// the argument(s) of #`[bench]` are passed to the function as parameters.
#[binary_benchmark]
// This'll benchmark `echo` printing `foo`
#[bench::foo("foo")]
// This'll benchmark `echo` printing `bar`
#[bench::bar("bar")]
// Specifying multiple inputs at once with `#[benches]` is also possible. Each argument in the list
// will be passed to the function `bench_with_bench` and produces a single benchmark
#[benches::multiple("aaaa", "aaaaa")]
fn bench_with_bench(arg: &str) -> gungraun::Command {
    gungraun::Command::new(ECHO_EXE).arg(arg).build()
}

// The first group. As in library benchmarks, specify all benchmark functions you want to put into
// the same group in the `benchmark` parameter
binary_benchmark_group!(
    // A group needs a unique name which can be later used in the `main!` macro
    name = echo_group,
    benchmarks = [simple_bench, bench_with_bench]
);

fn setup_foo() {
    std::fs::write("foo.txt", "some content").unwrap();
}

fn create_file_with_content(path: &str, content: &str) {
    std::fs::write(path, content).unwrap();
}

// Let's benchmark a binary which needs a little bit of setup. Since we need to create files during
// the `setup` we chose to run the `setup` and `Command` of all benchmarks in this
// `#[binary_benchmark]` in a `Sandbox` to avoid polluting our project's workspace with files
// needed only by the benchmarks. Also, very convenient, we don't need to delete the files
// individually after the benchmark run, because the sandbox is cleaned up automatically after each
// benchmark run.
#[binary_benchmark(config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)))]
// As soon as you need to specify other parameters than arguments in the `#[bench]` attribute, it is
// required to specify the arguments with the secondary syntax `args = (...)`. The arguments need to
// be enclosed in parentheses even when there is only a single argument. Note that `setup_foo()` is
// not evaluated right away! The expression passed to the `setup` parameter is just executed before
// the `Command` is executed.
#[bench::foo(args = ("foo.txt", "some content"), setup = setup_foo())]
// The above bench uses `setup_foo()` but what if we need to set up multiple benches the same way
// based on the `args`? Simply use the function pointer or function path (the function without the
// parentheses), like below. `create_file_with_content` without parentheses causes the `args` to be
// passed to the function `bench_with_setup` itself AND `create_file_with_content`
#[bench::bar(args = ("bar.txt", "some bar content"), setup = create_file_with_content)]
// Much easier to create another benchmark case, isn't it?
#[bench::baz(args = ("baz.txt", "some baz content"), setup = create_file_with_content)]
// Or multiple benches at once. The alternate syntax for `args` in the `#[benches]` attribute is
// `args = [(...), (...), ...]` (An array of tuples)
#[benches::multiple(
    args = [("aaa.txt", "aaa"), ("aaaa.txt", "aaa")],
    setup = create_file_with_content
)]
fn bench_with_setup(path: &str, content: &str) -> gungraun::Command {
    gungraun::Command::new(READ_FILE_EXE)
        .args([path, content])
        .build()
}

/// Used to read all files with the `.fix` extension in a directory line by line
fn read_dir(path: &str) -> Vec<String> {
    let mut lines = vec![];
    // We're only interested in files having the `.fix` suffix
    for path in std::fs::read_dir(path).unwrap().filter(|entry| {
        entry
            .as_ref()
            .ok()
            .and_then(|entry| entry.path().extension().map(|extension| extension == "fix"))
            .is_some_and(|is_fix| is_fix)
    }) {
        lines.extend(
            BufReader::new(File::open(path.unwrap().path()).unwrap())
                .lines()
                // Sort out empty lines or lines that contain only whitespace
                .filter_map(|l| l.ok().and_then(|l| (!l.trim().is_empty()).then_some(l))),
        );
    }

    // We sort the lines for a consistent benchmark output and remove duplicates, but it's not
    // strictly necessary
    lines.sort_unstable();
    lines.dedup();
    lines
}

// Again, we use a `Sandbox` for the benches in this `binary_benchmark`.
#[binary_benchmark(config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)))]
// The `iter` parameter. However, that's not very different from the `args` parameter.
#[benches::from_array(iter = ["1".to_owned(), "2".to_owned()])]
// This example couldn't have been achieved without `iter`. Each element of the iterator argument
// (anything that implements `IntoIterator`) represents a benchmark case. Here we read all files in
// the `benches/fixtures` directory line by line. These files contain a number on each line.
#[benches::from_iter(iter = read_dir("benches/fixtures"))]
fn benches_from_iter(line: String) -> gungraun::Command {
    gungraun::Command::new(ECHO_EXE).arg(line).build()
}

// A small helper function because we need to do the same in the benchmark function
// `benches_from_file` and the setup function `setup_file` and the moment the input file format
// changes we only need to change the code in this function.
fn split_line(line: &str) -> (&str, &str) {
    line.split_once(";").unwrap()
}

fn setup_file(line: String) {
    let (path, content) = split_line(&line);
    // Let's reuse the setup function `create_file_with_content`.
    create_file_with_content(path, content);
}

#[binary_benchmark(config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)))]
// If you want/need to store and read benchmark inputs from a file, you can do so with the `file`
// parameter of the `#[benches]` attribute. Note this parameter is not available in `#[bench]`. Each
// line in the input file represents a single benchmark. The whole line as `String` is passed to the
// function. Here, we also pass the line to the `setup_file` function. The file has to be encoded in
// valid UTF-8.
#[benches::file(file = "benchmark-tests/benches/fixtures/file_content.inputs", setup = setup_file)]
fn benches_from_file(line: String) -> gungraun::Command {
    // As opposed to library benchmarks, we can put any code in this function, since this function
    // is evaluated only once when gungraun collects all benchmarks. The function's sole
    // purpose is to __build__ the `Command` which is getting executed later.
    let (path, content) = split_line(&line);
    gungraun::Command::new(READ_FILE_EXE)
        .args([path, content])
        .build()
}

// Nothing in this example forces us to create another group for the `read-file` binary benchmarks,
// but we would like to show the possibility of doing so. The different groups are also visible in
// the resulting output directories of `gungraun`. Unless otherwise specified, the output files
// are stored in the
// `$WORKSPACE_ROOT/target/gungraun/$BENCHMARK_FILE/$GROUP/$FUNCTION_NAME.$BENCH_ID`
// directory hierarchy. The individual groups therefore save their files in a separate directory,
// and the benchmark output also displays the different groups in different ways. An important
// reason for using different groups is that each group can have its own `BinaryBenchmarkConfig` via
// the `config` parameter. The `BinaryBenchmarkConfig` at group level configures all benchmarks of
// the `benchmarks` parameter.
binary_benchmark_group!(
    name = read_file_group,
    benchmarks = [bench_with_setup, benches_from_file, benches_from_iter]
);

fn setup_pipe(additional_content: &str) {
    println!(
        "The output of this function to `Stdout` is the input for the `Stdin` of the `Command`"
    );
    println!("{additional_content}");
}

fn check_output(expected: &str) {
    let content = std::fs::read_to_string("output").unwrap();
    assert!(content.ends_with(&format!("{expected}\n")));
}

// Finally, we're using the output of the `setup` as input for the `Command`. We also specify a
// global setup function which is applied to all benches in this `#[binary_benchmark]`. We could
// overwrite this global `setup` function in a `#[bench]` or `#[benches]` if we would need to do so.
// The same applies to the `teardown` function.
#[binary_benchmark(setup = setup_pipe, teardown = check_output)]
#[bench::foo("foo")]
#[bench::bar("bar")]
// We need the `args` from the benches only for `setup` and `teardown` but not in the function, so
// we can ignore them.
fn bench_pipe(_: &str) -> gungraun::Command {
    gungraun::Command::new(PIPE_EXE)
        // Here, we configure the output of the `setup` function to `Stdout` so that it is the
        // `Stdin` of the `Command`. This changes the execution order of `setup` and `Command`.
        // Usually, `setup` is executed, then `Command`, then `teardown`. Now, `setup` and `Command`
        // are run in parallel to be able to create a pipe between them.
        .stdin(Stdin::Setup(Pipe::Stdout))
        // Since the `pipe` binary echo's the read `Stdin` back to `Stdout` we route the `Stdout` to
        // a file which we pickup in the `teardown` to be able to check, that the `pipe` binary has
        // done what it is expected to do.
        .stdout(Stdio::File(PathBuf::from("output")))
        // Just showing off another possibility and route the `Stderr` to `/dev/null`
        .stderr(Stdio::Null)
        .build()
}

// Our last group
binary_benchmark_group!(
    name = pipe_group,
    // We specify at group level that all benchmarks in this group are run in a `Sandbox`
    config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    benchmarks = bench_pipe
);

// Collect all the groups by name in the `binary_benchmark_groups` parameter
main!(binary_benchmark_groups = [echo_group, read_file_group, pipe_group]);
