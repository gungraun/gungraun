fn main() {
    println!(
        "cargo:rustc-env=GR_BUILD_TRIPLE={}",
        std::env::var("TARGET").unwrap()
    );
}
