//! Bubble-sorts a worst-case (reverse-sorted) array.
//!
//! Creates a descending array from `start-1` down to `0` (for positive `start`) and sorts it using
//! bubble sort. The resulting array is printed to stdout. Used to do some work and allocate memory.
//!
//! # Arguments
//!
//! * `<start>` - The starting number for the array. Creates an array of size `abs(start)`.

use benchmark_tests::{bubble_sort, setup_worst_case_array};

fn main() {
    let mut iter = std::env::args().skip(1);
    let start = iter.next().unwrap().parse().unwrap();
    let sorted = bubble_sort(setup_worst_case_array(start));
    println!("{sorted:?}");
}
