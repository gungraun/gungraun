//! The build script for the `valgrind-requests` crate

// spell-checker: ignore idirafter idiraftervalgrind isystem rustified

#[cfg(feature = "stubs")]
mod imp {
    use std::borrow::Cow;
    use std::fmt::Display;
    use std::io::{BufRead, BufReader, Cursor};
    use std::path::PathBuf;

    use bindgen::{Bindings, builder};
    use gungraun_common::ValgrindSupport;
    use rustc_version::{Version, version};
    use strum::{EnumIter, IntoEnumIterator};

    use crate::BuildResult;

    #[derive(Debug, PartialEq, Eq)]
    enum Strategy {
        Strict,
        Fallback,
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
        Powerpc64,
        Native,
        No,
    }

    #[derive(Debug)]
    struct Target {
        abi: String,
        arch: String,
        env: String,
        os: String,
        triple: String,
        vendor: String,
    }

    impl Strategy {
        fn from_env() -> BuildResult<Self> {
            match std::env::var("VALGRIND_REQUESTS_STRATEGY") {
                Ok(value) => match value.as_str() {
                    "strict" => Ok(Self::Strict),
                    "fallback" => Ok(Self::Fallback),
                    _ => Err(format!(
                        "invalid value for VALGRIND_REQUESTS_STRATEGY: {value}; valid values: \
                         'strict', 'fallback'"
                    )
                    .into()),
                },
                Err(std::env::VarError::NotPresent) => Ok(Self::Fallback),
                Err(error) => Err(format!("invalid VALGRIND_REQUESTS_STRATEGY: {error}").into()),
            }
        }
    }

    impl Display for Support {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let support = format!("{self:?}").to_lowercase();
            f.write_str(&support)
        }
    }

    impl From<ValgrindSupport> for Support {
        fn from(value: ValgrindSupport) -> Self {
            match value {
                ValgrindSupport::Arm => Self::Arm,
                ValgrindSupport::Aarch64 => Self::Aarch64,
                ValgrindSupport::X86 => Self::X86,
                ValgrindSupport::X86_64 => Self::X86_64,
                ValgrindSupport::Riscv64 => Self::Riscv64,
                ValgrindSupport::S390x => Self::S390x,
                ValgrindSupport::Powerpc => Self::Powerpc,
                ValgrindSupport::Powerpc64 => Self::Powerpc64,
            }
        }
    }

    impl Target {
        fn triple_to_env_key(&self) -> String {
            self.triple.replace('-', "_").to_ascii_uppercase()
        }

        fn from_env() -> Self {
            Self {
                abi: std::env::var("CARGO_CFG_TARGET_ABI").unwrap(),
                arch: std::env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
                env: std::env::var("CARGO_CFG_TARGET_ENV").unwrap(),
                os: std::env::var("CARGO_CFG_TARGET_OS").unwrap(),
                vendor: std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap(),
                triple: std::env::var("TARGET").unwrap(),
            }
        }
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
            .use_core()
            .ctypes_prefix("cty")
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

    #[cfg(feature = "act")]
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

    #[cfg(not(feature = "act"))]
    fn build_native(_target: &Target) {}

    // Return the rust version if running rustc was successful
    fn get_rust_version() -> Option<Version> {
        version().ok()
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

    pub fn main() -> BuildResult<()> {
        print_migration_warnings();

        let target = Target::from_env();
        let triple_env_key = target.triple_to_env_key();
        let strategy = Strategy::from_env()?;

        println!("cargo:rerun-if-changed=valgrind/wrapper.h");
        println!("cargo:rerun-if-changed=valgrind/native.c");
        println!("cargo:rerun-if-env-changed=VALGRIND_REQUESTS_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=VALGRIND_REQUESTS_{triple_env_key}_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=GUNGRAUN_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=GUNGRAUN_{triple_env_key}_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=IAI_CALLGRIND_VALGRIND_INCLUDE");
        println!("cargo:rerun-if-env-changed=IAI_CALLGRIND_{triple_env_key}_VALGRIND_INCLUDE");

        println!("cargo:rerun-if-env-changed=TARGET");
        println!("cargo:rerun-if-env-changed=VALGRIND_REQUESTS_STRATEGY");

        let rust_version = get_rust_version();

        let values = Support::iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<String>>()
            .join(",");
        println!("cargo:rustc-check-cfg=cfg(client_requests_support,values({values}))");

        // When building the docs on docs.rs we can take a shortcut
        if std::env::var("DOCS_RS").is_ok() {
            print_client_requests_support(&Support::X86_64);
            build_bindings(&target);
            build_native(&target);
            return Ok(());
        }

        let bindings = build_bindings(&target);

        // Note the `gungraun_common` table uses Valgrind support as priority. For example some
        // targets might not be supported by Rust like i686-unknown-illumos.
        let support = match ValgrindSupport::from_target(
            &target.arch,
            &target.os,
            &target.env,
            &target.vendor,
            &target.abi,
        ) {
            Some(key @ (ValgrindSupport::Powerpc | ValgrindSupport::Powerpc64))
                if rust_version
                    .as_ref()
                    .is_some_and(|r| r.major >= 1 && r.minor >= 95) =>
            {
                Some(key.into())
            }
            Some(
                key @ (ValgrindSupport::Arm
                | ValgrindSupport::Aarch64
                | ValgrindSupport::X86
                | ValgrindSupport::X86_64
                | ValgrindSupport::Riscv64
                | ValgrindSupport::S390x),
            ) => Some(key.into()),
            None | Some(_) => {
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
            }
        };

        match (strategy, support) {
            (_, Some(Support::No)) => {
                return Err(
                    format!("target '{}' is unsupported by Valgrind", target.triple).into(),
                );
            }
            (Strategy::Strict, Some(Support::Native)) => {
                return Err(format!(
                    "target '{}' doesn't have zero-indirection support and strict strategy is set",
                    target.triple
                )
                .into());
            }
            (_, Some(support)) => {
                print_client_requests_support(&support);
                build_native(&target);
            }
            (_, None) => {
                return Err("unable to determine client requests support".into());
            }
        }

        Ok(())
    }

    fn print_client_requests_support(value: &Support) {
        println!("cargo:rustc-cfg=client_requests_support=\"{value}\"");
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
}

#[cfg(not(feature = "stubs"))]
mod imp {
    pub fn main() -> crate::BuildResult<()> {
        Ok(())
    }
}

use std::error::Error;
type BuildResult<T> = Result<T, Box<dyn Error>>;

fn main() -> BuildResult<()> {
    imp::main()
}
