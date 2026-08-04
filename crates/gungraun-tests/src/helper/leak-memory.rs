//! Intentionally leaks memory for testing memory leak detection.
//!
//! Creates reference cycles that will not be freed. Used for testing DHAT and other memory
//! profiling tools' ability to detect memory leaks.
//!
//! # Arguments
//!
//! * `<num>` - Number of memory leak cycles to create.

use benchmark_tests::leak_memory;

fn main() {
    let mut args = std::env::args().skip(1);
    let num = args.next().unwrap().parse::<usize>().unwrap();

    leak_memory(num);
}
