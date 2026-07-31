//! Echoes command-line arguments to stdout.
//!
//! Prints all provided arguments to stdout joined by spaces, followed by a newline.

use std::ffi::OsString;
use std::io::{BufWriter, Write, stdout};
use std::os::unix::ffi::OsStrExt;

fn main() {
    let mut args_iter = std::env::args_os().skip(1);
    let mut joined = if let Some(next) = args_iter.next() {
        args_iter.fold(next, |mut a, b| {
            a.push(" ");
            a.push(b);
            a
        })
    } else {
        OsString::new()
    };
    joined.push("\n");

    let mut writer = BufWriter::new(stdout().lock());
    writer
        .write_all(joined.as_bytes())
        .expect("Writing to stdout should succeed");

    writer.flush().expect("Flushing writer should succeed");
}
