//! A public Rust declaration used to compare standalone and harness execution.

use std::path::PathBuf;

use gungraun::prelude::*;
use gungraun::{Callgrind, EntryPoint, Tool};

#[binary_benchmark(
    config = BinaryBenchmarkConfig::default()
        .default_tool(Tool::Callgrind)
        .tool(Callgrind::default().entry_point(EntryPoint::Custom("guard_region".to_owned())))
)]
fn rust_declared() -> Command {
    let root = PathBuf::from(std::env::var_os("GUNGRAUN_STANDALONE_ROOT").unwrap());
    Command::new(root.join("./workload"))
        .args([
            "context",
            "argument with spaces",
            "${LITERAL}",
            "$(touch SH_INJECTION)",
            "*",
        ])
        .current_dir(root.join("working directory"))
        .envs([
            ("DEFAULT_ENV", "default value"),
            ("LC_ALL", "C"),
            ("LITERAL", "$HOME ${USER} *"),
            ("OVERRIDE", "case value"),
        ])
        .build()
}

binary_benchmark_group!(name = standalone; benchmarks = rust_declared);
main!(binary_benchmark_groups = standalone);
