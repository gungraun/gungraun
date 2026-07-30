//! Multi-threaded prime number finder for benchmarking thread handling.
//!
//! Finds prime numbers across multiple threads to test threading behavior in benchmarks. Supports
//! both simple multi-threaded execution and nested thread scenarios with Callgrind instrumentation.
//!
//! # Arguments
//!
//! * `[num_threads]` - Number of threads to spawn (default: 0, uses main thread only).
//! * `--thread-in-thread` - Run a nested thread scenario with Callgrind instrumentation control.

use benchmark_tests::{find_primes_multi_thread, thread_in_thread_with_instrumentation};

fn main() {
    let mut args_iter = std::env::args().skip(1);
    match args_iter.next() {
        Some(value) if value.as_str() == "--thread-in-thread" => {
            gungraun::client_requests::callgrind::start_instrumentation();
            let result = thread_in_thread_with_instrumentation();
            gungraun::client_requests::callgrind::stop_instrumentation();
            result
        }
        Some(value) => {
            let num_threads = value.parse::<usize>().unwrap();
            find_primes_multi_thread(num_threads)
        }
        None => find_primes_multi_thread(0),
    };
}
