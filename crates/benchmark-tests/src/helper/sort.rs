//! Performs bubble sort with memory allocation for benchmarking.
//!
//! Sorts a worst-case array and computes a sum of the first elements.
//!
//! # Arguments
//!
//! * `[start]` - Starting number for the array (default: 4000).
//! * `[sum]` - Number of sorted elements to sum up (default: 2000).

use benchmark_tests::bubble_sort_allocate;

fn main() {
    let mut iter = std::env::args().skip(1);
    let start = iter.next().unwrap_or("4000".to_owned()).parse().unwrap();
    let sum = iter.next().unwrap_or("2000".to_owned()).parse().unwrap();

    let sum = bubble_sort_allocate(start, sum);
    println!("{sum}");
}
