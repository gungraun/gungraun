use std::hint::black_box;

use gungraun::prelude::*;

const TMP_ENV_CLEAR: &str = "/tmp/gungraun_test_lib_bench_envs.env-clear";

fn expected_env_clear_value(env_clear: bool) -> bool {
    if let Ok(value) = std::fs::read_to_string(TMP_ENV_CLEAR) {
        println!("Found env clear override: {value}");
        value == "true"
    } else {
        env_clear
    }
}

#[library_benchmark]
#[bench::yes_default(true)]
#[bench::yes_explicit(args = [true], config = LibraryBenchmarkConfig::default().env_clear(true))]
#[bench::no(args = [false], config = LibraryBenchmarkConfig::default().env_clear(false))]
fn env_clear(env_clear: bool) {
    benchmark_tests::check_env(black_box(expected_env_clear_value(env_clear)))
}

library_benchmark_group!(name = my_group, benchmarks = [env_clear]);
main!(library_benchmark_groups = my_group);
