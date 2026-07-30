//! The build script for the gungraun library

fn print_env(key: &str, value: &str) {
    println!("cargo:rustc-env={key}={value}");
}

fn main() {
    print_env(
        "__GUNGRAUN_COMMON_TARGET_ABI",
        &std::env::var("CARGO_CFG_TARGET_ABI")
            .expect("The build environment variable should be available"),
    );
    print_env(
        "__GUNGRAUN_COMMON_TARGET_ARCH",
        &std::env::var("CARGO_CFG_TARGET_ARCH")
            .expect("The build environment variable should be available"),
    );
    print_env(
        "__GUNGRAUN_COMMON_TARGET_ENV",
        &std::env::var("CARGO_CFG_TARGET_ENV")
            .expect("The build environment variable should be available"),
    );
    print_env(
        "__GUNGRAUN_COMMON_TARGET_OS",
        &std::env::var("CARGO_CFG_TARGET_OS")
            .expect("The build environment variable should be available"),
    );
    print_env(
        "__GUNGRAUN_COMMON_TARGET_VENDOR",
        &std::env::var("CARGO_CFG_TARGET_VENDOR")
            .expect("The build environment variable should be available"),
    );
}
