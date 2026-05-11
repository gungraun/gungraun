//! The build script for the `valgrind-requests` crate

// spell-checker: ignore idirafter idiraftervalgrind isystem rustified

#[cfg(feature = "stubs")]
mod imp {
    use std::borrow::Cow;
    use std::fmt::Display;
    use std::io::{BufRead, BufReader, Cursor};
    use std::path::PathBuf;

    use bindgen::{Bindings, builder};
    use rustc_version::{Version, version};
    use strum::{EnumIter, IntoEnumIterator};

    #[derive(Debug)]
    struct Target {
        arch: String,
        env: String,
        os: String,
        triple: String,
        vendor: String,
    }

    #[derive(EnumIter, Debug, PartialEq, Eq)]
    enum Support {
        Arm,
        Aarch64,
        X86,
        X86_64,
        Riscv64,
        S390x,
        Powerpc,
        Native,
        No,
    }

    impl Display for Support {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let support = format!("{self:?}").to_lowercase();
            f.write_str(&support)
        }
    }

    impl Target {
        fn triple_to_env_key(&self) -> String {
            self.triple.replace('-', "_").to_ascii_uppercase()
        }

        fn from_env() -> Self {
            Self {
                arch: std::env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
                env: std::env::var("CARGO_CFG_TARGET_ENV").unwrap(),
                os: std::env::var("CARGO_CFG_TARGET_OS").unwrap(),
                vendor: std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap(),
                triple: std::env::var("TARGET").unwrap(),
            }
        }
    }

    pub fn print_migration_warnings() {
        for (old, new) in std::env::vars().filter_map(|(key, _)| {
            if key.starts_with("IAI_CALLGRIND_") && key.ends_with("VALGRIND_INCLUDE") {
                Some((
                    key.clone(),
                    key.replace("IAI_CALLGRIND_", "VALGRIND_REQUESTS_"),
                ))
            } else if key.starts_with("GUNGRAUN_") && key.ends_with("VALGRIND_INCLUDE") {
                Some((key.clone(), key.replace("GUNGRAUN_", "VALGRIND_REQUESTS_")))
            } else {
                None
            }
        }) {
            if std::env::var(&old).is_ok() && std::env::var(&new).is_err() {
                println!(
                    "cargo:warning=The name of the environment variable `{old}` has changed to \
                     `{new}`."
                );
            }
        }
    }

    fn print_client_requests_support(value: &Support) {
        println!("cargo:rustc-cfg=client_requests_support=\"{value}\"");
    }

    fn include_dirs(target: &Target) -> impl Iterator<Item = String> + use<> {
        let triple_env_key = target.triple_to_env_key();
        [
            Cow::Owned(format!(
                "VALGRIND_REQUESTS_{triple_env_key}_VALGRIND_INCLUDE",
            )),
            Cow::Owned(format!("GUNGRAUN_{triple_env_key}_VALGRIND_INCLUDE")),
            Cow::Owned(format!("IAI_CALLGRIND_{triple_env_key}_VALGRIND_INCLUDE")),
            Cow::Borrowed("VALGRIND_REQUESTS_VALGRIND_INCLUDE"),
            Cow::Borrowed("GUNGRAUN_VALGRIND_INCLUDE"),
            Cow::Borrowed("IAI_CALLGRIND_VALGRIND_INCLUDE"),
        ]
        .into_iter()
        .filter_map(|env| std::env::var(env.as_ref()).ok())
    }

    fn build_native(target: &Target) {
        let mut builder = cc::Build::new();

        for env in include_dirs(target) {
            builder.flag(format!("-isystem{env}"));
        }

        builder.flag("-isystem/usr/local/include");
        builder.flag("-isystem/usr/include");
        builder.flag("-idiraftervalgrind/include");

        builder
            .debug(true)
            .file("valgrind/native.c")
            .compile("native");
    }

    fn build_bindings(target: &Target) -> Bindings {
        let mut builder = builder();

        for env in include_dirs(target) {
            builder = builder.clang_arg(format!("-isystem{env}"));
        }

        // The default includes are not working in cross because the sysroot is set to a target
        // specific path like /usr/x86_64-linux-gnu/usr/include but the valgrind headers are
        // target-agnostic and usually installed in /usr/{local/}include.
        builder = builder.clang_arg("-isystem/usr/local/include");
        builder = builder.clang_arg("-isystem/usr/include");
        builder = builder.clang_arg("-idiraftervalgrind/include");

        let bindings = builder
            .header("valgrind/wrapper.h")
            .allowlist_var("VR_IS_PLATFORM_SUPPORTED_BY_VALGRIND")
            .allowlist_var("VR_VALGRIND_MAJOR")
            .allowlist_var("VR_VALGRIND_MINOR")
            .allowlist_type("VR_.*ClientRequest")
            .rustified_enum("VR_.*ClientRequest")
            .layout_tests(false)
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .generate()
            .expect("Generating binding should succeed");

        let out_dir = std::env::var("OUT_DIR").map(PathBuf::from).unwrap();
        let path = out_dir.join("bindings.rs");
        bindings.write_to_file(path).unwrap();
        bindings
    }

    // Return the rust version if running rustc was successful
    fn get_rust_version() -> Option<Version> {
        version().ok()
    }

    pub fn main() {
        print_migration_warnings();

        let target = Target::from_env();
        let triple_env_key = target.triple_to_env_key();

        println!("cargo:rerun-if-changed=valgrind/wrapper.h");
        println!("cargo:rerun-if-changed=valgrind/native.c");
        println!("cargo:rerun-if-env-changed=VALGRIND_REQUESTS_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=VALGRIND_REQUESTS_{triple_env_key}_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=GUNGRAUN_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=GUNGRAUN_{triple_env_key}_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=IAI_CALLGRIND_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=IAI_CALLGRIND_{triple_env_key}_VALGRIND_INCLUDE");

        println!("cargo:rerun-if-env-changed=TARGET");

        let rust_version = get_rust_version();

        // rustc-check-cfg is introduced in rust with version 1.80 and avoids the compiler warnings
        // in version >= 1.80.0. Printing it when compiling with versions < 1.80 triggers a warning,
        // too. To get the best of both worlds we check against the currently active rust version.
        if let Some(rust_version) = &rust_version {
            if rust_version.major >= 1 && rust_version.minor >= 80 {
                let values = Support::iter()
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<String>>()
                    .join(",");
                println!("cargo:rustc-check-cfg=cfg(client_requests_support,values({values}))");
            }
        }

        // When building the docs on docs.rs we can take a shortcut
        if std::env::var("DOCS_RS").is_ok() {
            print_client_requests_support(&Support::X86_64);
            build_bindings(&target);
            build_native(&target);
            return;
        }

        let bindings = build_bindings(&target);

        // These guards mirror the checks in the `valgrind.h` header file
        let support = if target.arch == "x86_64"
            && (target.os == "linux"
                || target.os == "freebsd"
                || (target.vendor == "apple" && target.os == "darwin")
                || (target.os == "windows" && target.env == "gnu")
                || ((target.vendor == "sun" || target.vendor == "pc") && target.os == "solaris"))
        {
            Some(Support::X86_64)
        } else if target.arch == "x86"
            && (target.os == "linux"
                || target.os == "freebsd"
                || (target.vendor == "apple" && target.os == "darwin")
                || (target.os == "windows" && target.env == "gnu")
                || ((target.vendor == "sun" || target.vendor == "pc") && target.os == "solaris"))
        {
            Some(Support::X86)
        } else if target.arch == "arm" && target.os == "linux" && target.env == "gnu" {
            Some(Support::Arm)
        } else if target.arch == "aarch64"
            && (target.os == "freebsd"
                || (target.os == "linux" && target.env == "gnu")
                || (target.vendor == "apple" && target.os == "macos"))
        {
            Some(Support::Aarch64)
        } else if target.arch == "riscv64gc" && target.os == "linux" {
            Some(Support::Riscv64)
        } else if target.arch == "s390x" && target.os == "linux" {
            Some(Support::S390x)
        } else if target.arch == "powerpc"
            && target.os == "linux"
            && rust_version.is_some_and(|r| r.major >= 1 && r.minor >= 95)
        {
            Some(Support::Powerpc)
        } else {
            let re = regex::Regex::new(
                r"VR_IS_PLATFORM_SUPPORTED_BY_VALGRIND.*?=\s*(?<value>true|false)",
            )
            .expect("Regex should compile");
            let reader = BufReader::new(Cursor::new(bindings.to_string()));
            let mut support = None;
            for line in reader.lines().map(Result::unwrap) {
                if let Some(caps) = re.captures(&line) {
                    let value = caps.name("value").unwrap().as_str();
                    if value == "false" {
                        support = Some(Support::No);
                    } else if value == "true" {
                        support = Some(Support::Native);
                    } else {
                        // do nothing
                    }
                    break;
                }
            }
            support
        };

        if let Some(support) = support {
            print_client_requests_support(&support);
            if support != Support::No {
                build_native(&target);
            }
        } else {
            eprintln!("{bindings}");
            panic!("Unable to set cfg value for client_requests_support");
        }
    }
}

#[cfg(not(feature = "stubs"))]
mod imp {
    pub fn main() {}
}

fn main() {
    imp::main();
}
