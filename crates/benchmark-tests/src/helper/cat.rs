//! Prints the contents of a file to stdout.
//!
//! A simple `cat`-like utility. Usable for testing if a file is present or the content of a file is
//! as expected.
//!
//! # Arguments
//!
//! * `<file>` - Path to the file to read and print to stdout.

use std::fs::File;
use std::io::{BufReader, BufWriter, Write, copy, stdout};

fn main() {
    let mut args_iter = std::env::args().skip(1);
    let file_arg = args_iter.next().expect("File argument should be present");

    let file = File::open(file_arg).expect("Opening file should succeed");
    let stdout = stdout().lock();

    let mut writer = BufWriter::new(stdout);
    copy(&mut BufReader::new(file), &mut writer).expect("Printing file to stdout should succeed");

    writer.flush().expect("Flushing writer should succeed");
}
