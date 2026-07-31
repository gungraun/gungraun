use std::hint::black_box;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;

use gungraun::OutputFormat;
use gungraun::prelude::*;

/// Suppose this is your library
pub mod my_lib {
    use std::io;
    use std::path::Path;
    use std::process::ExitStatus;

    /// A function executing the crate's binary `cat`
    pub fn cat(file: &Path) -> io::Result<ExitStatus> {
        std::process::Command::new(env!("CARGO_BIN_EXE_cat"))
            .arg(file)
            .status()
    }
}

/// Create a file `/tmp/foo.txt` with some content
fn create_file() -> PathBuf {
    let path = PathBuf::from("/tmp/foo.txt");
    std::fs::write(&path, "some content").unwrap();
    path
}

#[library_benchmark]
#[bench::some(setup = create_file)]
fn bench_subprocess(path: PathBuf) -> io::Result<ExitStatus> {
    black_box(my_lib::cat(black_box(&path)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_subprocess);
main!(
    config = LibraryBenchmarkConfig::default()
        .output_format(OutputFormat::default().show_intermediate(true)),
    library_benchmark_groups = my_group
);
