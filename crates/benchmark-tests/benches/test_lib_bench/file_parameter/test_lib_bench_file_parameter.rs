use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use gungraun::prelude::*;

fn consume_line(path: &PathBuf) -> String {
    let mut file = File::open(path).expect("Opening the file for reading should succeed");

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .expect("Reading the content of the file to a string should succeed");
    std::fs::remove_file(path).expect("Deleting the file should succeed");
    if let Some((line, remainder)) = buffer.split_once("\n") {
        if !remainder.is_empty() {
            let mut file = File::create(path).expect("(Re-)creating the file should succeed");
            file.write_all(remainder.as_bytes())
                .expect("Writing to the file should succeed");
        }
        line.trim_end().to_owned()
    } else {
        buffer
    }
}

fn string_to_u64(line: String) -> u64 {
    line.parse().unwrap()
}

#[library_benchmark]
#[benches::one_line(file = "benchmark-tests/benches/fixtures/one_line.fix")]
fn one_line(value: String) {
    assert_eq!(value, "1".to_owned());
}

#[library_benchmark]
#[benches::one_line(file = "benchmark-tests/benches/fixtures/one_line.fix", setup = string_to_u64)]
fn one_line_with_setup(value: u64) {
    assert_eq!(value, 1);
}

#[library_benchmark]
#[benches::two_lines(file = "benchmark-tests/benches/fixtures/two_lines.fix")]
fn two_lines(value: String) {
    let path = PathBuf::from("/tmp/gungraun.two_lines.fix");
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap())
            .expect("The temporary directory should exist or being creatable");
        std::fs::copy("benches/fixtures/two_lines.fix", &path)
            .expect("Copying the fixture should succeed");
    }

    let expected = consume_line(&path);
    assert_eq!(value, expected);
}

#[library_benchmark]
#[benches::two_lines(
    file = "benchmark-tests/benches/fixtures/two_lines.fix",
    setup = string_to_u64
)]
fn two_lines_with_setup(value: u64) {
    let path = PathBuf::from("/tmp/gungraun.two_lines.fix");
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap())
            .expect("The temporary directory should exist or being creatable");
        std::fs::copy("benches/fixtures/two_lines.fix", &path)
            .expect("Copying the fixture should succeed");
    }

    let expected = consume_line(&path);
    assert_eq!(value, expected.parse::<u64>().unwrap());
}

library_benchmark_group!(
    name = bench_group,
    benchmarks = [
        one_line,
        one_line_with_setup,
        two_lines,
        two_lines_with_setup
    ]
);

main!(library_benchmark_groups = bench_group);
