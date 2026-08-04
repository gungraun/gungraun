mod assert;
mod config;
mod expected_files;
mod filter;
mod io;
mod runner;

use anyhow::{Context, anyhow, bail};
use config::Partition;
use runner::SystemTestRunner;

fn main() -> anyhow::Result<()> {
    // The cli args:
    // positional arguments
    let mut benches = Vec::default();
    // --filter=some_wildcard_filter_*
    let mut filter = Option::default();
    // --partition=x/y
    let mut partition = Option::default();
    // --continue
    let mut resume = false;

    for arg in std::env::args().skip(1) {
        match arg.split_once('=') {
            Some(("--filter", value)) => filter = Some(value.to_owned()),
            Some(("--partition", value)) => {
                let (part_str, total_str) = value
                    .split_once('/')
                    .ok_or_else(|| anyhow!("Invalid partition: {value}"))?;
                let part = part_str
                    .parse::<usize>()
                    .with_context(|| format!("Invalid partition part: {part_str}"))?;
                let total = total_str
                    .parse::<usize>()
                    .with_context(|| format!("Invalid partition total: {total_str}"))?;

                if total == 0 {
                    bail!("The total of a partition should be greater than zero");
                }
                if part == 0 || part > total {
                    bail!("The part of a partition should be within bounds: 0 < x <= total");
                }

                partition = Some(Partition { part, total });
            }
            Some(_) => bail!("Invalid argument: {arg}"),
            None if arg == "--continue" => resume = true,
            None => benches.push(arg),
        }
    }

    SystemTestRunner::new(&benches, filter.as_deref(), partition, resume)?.run()
}
