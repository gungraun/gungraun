use std::ffi::OsString;
use std::path::PathBuf;

use fs_extra::dir::CopyOptions;

fn set_env_var<K, V>(key: K, value: V)
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    println!("cargo:rustc-env={}={}", key.as_ref(), value.as_ref());
}

pub fn get_rust_version() -> String {
    let output = std::process::Command::new(
        std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc")),
    )
    .arg("--version")
    .output()
    .expect("Unable to run rustc");

    String::from_utf8_lossy(&output.stdout)
        .split(' ')
        .nth(1)
        .expect("The rust version should be present")
        .to_string()
}

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTUP_TOOLCHAIN");
    println!("cargo:rerun-if-env-changed=RUSTC");
    println!("cargo:rerun-if-env-changed=CARGO_MANIFEST_DIR");
    println!("cargo:rerun-if-env-changed=CROSS_RUNNER");

    let fixtures = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .expect("Environment variable CARGO_MANIFEST_DIR should exist"),
    )
    .join("tests/fixtures");

    if let Ok(env_var) = std::env::var("CROSS_RUNNER") {
        // cross-runner can be `qemu-user`, `qemu-system`, `native`
        if env_var == "qemu-system" {
            let target = PathBuf::from("/target");
            let fixtures_dest = PathBuf::from(&target).join("fixtures");
            if fixtures_dest.exists() {
                std::fs::remove_dir_all(&fixtures_dest).unwrap();
            }
            fs_extra::copy_items(&[fixtures], &target, &CopyOptions::new()).unwrap();

            set_env_var(
                "CLIENT_REQUEST_TESTS_FIXTURES",
                fixtures_dest.display().to_string(),
            );
        } else {
            set_env_var(
                "CLIENT_REQUEST_TESTS_FIXTURES",
                fixtures.display().to_string(),
            );
        }
    } else {
        set_env_var(
            "CLIENT_REQUEST_TESTS_FIXTURES",
            fixtures.display().to_string(),
        );
    }

    let rust_version = get_rust_version();
    set_env_var("CLIENT_REQUEST_TESTS_RUST_VERSION", rust_version);
}
