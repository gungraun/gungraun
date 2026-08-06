use std::hint::black_box;

fn main() {
    let mut value = 0_u64;

    let lock = gungraun::perf_enable!();
    for i in 0..1_000 {
        value = value.wrapping_add(black_box(i));
    }
    gungraun::perf_disable!(lock);

    black_box(value);
}
