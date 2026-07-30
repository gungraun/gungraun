use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use gungraun::prelude::*;

const GROUP_SETUP_FILE: &str = "/tmp/gungraun.group_setup.tmp";

#[library_benchmark]
fn simple_bench() {
    let mut file = File::open(GROUP_SETUP_FILE).unwrap();
    let mut actual = String::new();
    file.read_to_string(&mut actual).unwrap();
    let expected = format!("simple_group_with_setup: {GROUP_SETUP_FILE}");
    assert_eq!(expected, actual);
}

#[library_benchmark]
fn check_file_exists() {
    if !PathBuf::from(GROUP_SETUP_FILE).exists() {
        panic!("The setup file '{GROUP_SETUP_FILE}' should exist");
    }
}

#[library_benchmark]
fn check_file_not_exists() {
    if PathBuf::from(GROUP_SETUP_FILE).exists() {
        panic!("The setup file '{GROUP_SETUP_FILE}' should not exist");
    }
}

#[library_benchmark]
fn create_file_bench() {
    std::fs::write(GROUP_SETUP_FILE, "content").unwrap();
}

#[library_benchmark]
fn delete_file_bench() {
    std::fs::remove_file(GROUP_SETUP_FILE).unwrap();
}

fn group_setup() {
    println!("GROUP SETUP");
    let mut file = File::create(GROUP_SETUP_FILE).unwrap();
    write!(file, "simple_group_with_setup: {GROUP_SETUP_FILE}").unwrap();
}

fn group_teardown() {
    println!("GROUP TEARDOWN");
    std::fs::remove_file(GROUP_SETUP_FILE).unwrap();
}

library_benchmark_group!(
    name = simple_group_with_setup,
    setup = group_setup(),
    teardown = group_teardown(),
    benchmarks = [simple_bench, check_file_exists]
);

// No setup and teardown
library_benchmark_group!(name = check_group, benchmarks = check_file_not_exists);

library_benchmark_group!(
    name = group_only_setup,
    setup = group_setup(),
    benchmarks = delete_file_bench
);

library_benchmark_group!(
    name = group_only_teardown,
    teardown = group_teardown(),
    benchmarks = create_file_bench
);

fn main_setup() {
    println!("MAIN SETUP");
}

fn main_teardown() {
    println!("MAIN TEARDOWN");
}

main!(
    setup = main_setup(),
    teardown = main_teardown(),
    library_benchmark_groups = [
        simple_group_with_setup,
        // Check group is supposed to run directory after `simple_group_with_setup`
        check_group,
        // The groups below can be run in any order
        group_only_setup,
        group_only_teardown
    ]
);
