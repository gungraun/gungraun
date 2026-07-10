#[inline(never)]
fn noop() -> u64 {
    std::hint::black_box(42)
}

#[inline(never)]
pub fn calibrate() {
    let lock = crate::perf_enable!();
    let r = noop();
    crate::perf_disable!(lock);
    let _ = r;
}
