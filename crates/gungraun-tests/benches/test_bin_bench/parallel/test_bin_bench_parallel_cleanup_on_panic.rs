use std::time::Duration;

use gungraun::prelude::*;
use gungraun::{Delay, DelayKind, Stdio};

#[binary_benchmark]
fn exit_with_panic() -> Command {
    Command::new(env!("CARGO_BIN_EXE_exit-with"))
        .arg("panic")
        .delay(Delay::new(DelayKind::DurationElapse(
            Duration::from_millis(500),
        )))
        .build()
}

#[binary_benchmark]
fn timeout() -> Command {
    Command::new(env!("CARGO_BIN_EXE_timeout"))
        .arg("20000")
        .stdout(Stdio::Inherit)
        .build()
}

binary_benchmark_group!(name = my_group, benchmarks = [exit_with_panic, timeout]);
main!(binary_benchmark_groups = my_group);
