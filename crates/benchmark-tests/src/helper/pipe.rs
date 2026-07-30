//! Reads stdin and echoes it back to stdout.
//!
//! Reads all content from stdin until EOF, then prints a header followed by the content.

use std::io::{Read, Write, stdin, stdout};

fn main() {
    let mut stdin = stdin().lock();

    let mut content = vec![];
    stdin.read_to_end(&mut content).unwrap();

    println!("STDIN was:");
    stdout().write_all(&content).unwrap();
}
