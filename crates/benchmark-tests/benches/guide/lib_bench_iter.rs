mod my_lib {
    pub fn count_lines_fast(_path: std::path::PathBuf) -> usize {
        0
    }
}

use std::hint::black_box;
use std::path::PathBuf;

use gungraun::prelude::*;

fn read_dir() -> Vec<PathBuf> {
    std::fs::read_dir("benches/fixtures")
        .unwrap()
        .map(|d| d.unwrap().path())
        .collect()
}

#[library_benchmark]
#[benches::from_iter(iter = read_dir())]
fn bench_count_lines_fast(path: PathBuf) -> usize {
    black_box(my_lib::count_lines_fast(black_box(path)))
}

library_benchmark_group!(name = my_group; benchmarks = bench_count_lines_fast);
main!(library_benchmark_groups = my_group);
