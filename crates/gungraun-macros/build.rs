//! The build script

use rustc_version::version;

fn main() {
    let version = version().expect("The rustc version should be present");
    if version.major >= 1 && version.minor >= 82 {
        println!("cargo:rustc-cfg=unsafe_keyword_needed");
        println!("cargo:rustc-check-cfg=cfg(unsafe_keyword_needed)");
    }
}
