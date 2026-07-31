//! The build script for the gungraun library

fn main() {
    println!(
        "cargo:rustc-env=__GUNGRAUN_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
