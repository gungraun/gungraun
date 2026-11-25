#![allow(unused)]
// cspell:words prctl pdeathsig getppid reparented subreaper

#[inline(never)]
pub fn calibrate() {
    let lock = crate::perf_enable!();
    let r = noop();
    crate::perf_disable!(lock);
    let _ = r;
}

#[cfg(all(target_os = "linux", feature = "perf"))]
#[doc(hidden)]
pub fn init() {
    use nix::sys::prctl::set_pdeathsig;
    use nix::sys::signal::Signal;
    use nix::unistd::getppid;

    set_pdeathsig(Some(Signal::SIGTERM))
        .expect("Failed to set the perf harness parent-death signal");
    // Catch early parent loss when the harness was reparented to PID 1, not to a subreaper.
    if getppid().as_raw() == 1 {
        std::process::exit(128 + Signal::SIGTERM as i32);
    }
}

#[cfg(not(all(target_os = "linux", feature = "perf")))]
#[doc(hidden)]
pub fn init() {}

#[inline(never)]
fn noop() -> u64 {
    std::hint::black_box(42)
}
