use std::collections::HashMap;
use std::hint::black_box;

use gungraun::OutputFormat;
use gungraun::prelude::*;

fn make_hashmap(num: usize) -> HashMap<String, usize> {
    (0..num).fold(HashMap::new(), |mut acc, e| {
        acc.insert(format!("element: {e}"), e);
        acc
    })
}

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default()
            .tolerance(0.9)
        )
)]
#[bench::tolerance(make_hashmap(100))]
fn bench_hash_map(map: HashMap<String, usize>) -> Option<usize> {
    black_box(
        map.iter()
            .find_map(|(key, value)| (key == "element: 12345").then_some(*value)),
    )
}

library_benchmark_group!(name = my_group, benchmarks = bench_hash_map);
main!(library_benchmark_groups = my_group);
