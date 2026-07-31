use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::{Callgrind, EntryPoint, OutputFormat};

/// Suppose this is your library
pub mod my_lib {
    /// Return true if `num` is a prime number
    pub fn is_prime(num: u64) -> bool {
        if num <= 1 {
            return false;
        }

        for i in 2..=(num as f64).sqrt() as u64 {
            if num % i == 0 {
                return false;
            }
        }

        true
    }

    /// Find and return all prime numbers in the inclusive range `low` to `high`
    pub fn find_primes(low: u64, high: u64) -> Vec<u64> {
        (low..=high).filter(|n| is_prime(*n)).collect()
    }

    /// Return the prime numbers in the range `0..(num_threads * 10000)`
    pub fn find_primes_multi_thread(num_threads: usize) -> Vec<u64> {
        let mut handles = vec![];
        let mut low = 0;
        for _ in 0..num_threads {
            let handle = std::thread::spawn(move || find_primes(low, low + 10000));
            handles.push(handle);

            low += 10000;
        }

        let mut primes = vec![];
        for handle in handles {
            let result = handle.join();
            primes.extend(result.unwrap())
        }

        primes
    }
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::with_args(["--collect-atstart=yes"])
            .entry_point(EntryPoint::None)
        )
)]
#[bench::two_threads(2)]
fn bench_threads(num_threads: usize) -> Vec<u64> {
    black_box(my_lib::find_primes_multi_thread(black_box(num_threads)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_threads);
main!(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default().show_intermediate(true)),
    library_benchmark_groups = my_group
);
