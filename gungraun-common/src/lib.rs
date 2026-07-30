//! Shared target-support detection and transport types for Gungraun crates.
//!
//! The benchmark harness is compiled for the benchmark target and uses this crate to determine
//! which tool families that target supports. It serializes that information as [`SupportedTools`]
//! for the runner. This describes compile-target support only; it does not guarantee that a tool
//! executable is installed or that the current process has permission to use it.

use std::fmt::Display;
use std::str::FromStr;

#[cfg(feature = "strum")]
use strum::EnumIter;
#[cfg(feature = "strum")]
pub use strum::IntoEnumIterator;

/// Tool families supported by the benchmark's compilation target.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct SupportedTools {
    /// Whether the target supports Linux Perf.
    pub perf: bool,
    /// Whether the target supports Valgrind-based tools.
    pub valgrind: bool,
}

/// Valgrind client-request implementation available for a target.
///
/// Each variant identifies the architecture-specific implementation selected by
/// [`ValgrindSupport::from_target`]. A value indicates that Valgrind supports the target tuple; it
/// does not indicate that a Valgrind executable is installed on the host.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "strum", derive(EnumIter))]
pub enum ValgrindSupport {
    /// 32-bit Arm targets.
    Arm,
    /// 64-bit Arm targets.
    Aarch64,
    /// 32-bit x86 targets.
    X86,
    /// 64-bit x86 targets.
    X86_64,
    /// 64-bit RISC-V targets.
    Riscv64,
    /// IBM Z 64-bit targets.
    S390x,
    /// 32-bit PowerPC targets.
    Powerpc,
    /// 64-bit PowerPC targets, both little- and big-endian.
    Powerpc64,
}

impl SupportedTools {
    /// Returns `true` when at least one recognized tool family supports the target.
    pub fn has_at_least_one(&self) -> bool {
        self.perf || self.valgrind
    }

    /// Returns the comma-separated names of all tool families recognized by the transport format.
    ///
    /// Unlike [`Display`], this list is independent of a particular target's support flags.
    pub fn tools_list() -> String {
        "perf,valgrind".to_owned()
    }
}

impl Display for SupportedTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("tools=")?;
        f.write_str(
            &[
                self.perf.then_some("perf"),
                self.valgrind.then_some("valgrind"),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<&str>>()
            .join(","),
        )
    }
}

impl FromStr for SupportedTools {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let prefix = "Invalid format for supported tools";

        let (key, value) = s
            .split_once('=')
            .ok_or_else(|| format!("{prefix}: Expected an equals sign `tools=foo,bar`"))?;

        if key != "tools" {
            return Err(format!(
                "{prefix}: Expected the `tools` keyword buf found '{key}'"
            ));
        }

        let mut this = Self::default();

        for tool in value.split(',').filter(|s| !s.is_empty()) {
            match tool {
                "perf" => this.perf = true,
                "valgrind" => this.valgrind = true,
                _ => {
                    return Err(format!(
                        "{prefix}: Expected one of `perf`, `valgrind` but found '{tool}'"
                    ));
                }
            }
        }

        Ok(this)
    }
}

impl ValgrindSupport {
    /// Returns the Valgrind implementation for the current compilation target.
    ///
    /// Target components are supplied by this crate's build script so the result describes the
    /// Cargo target rather than the host running the compiler.
    pub fn new() -> Option<Self> {
        Self::from_target(
            env!("__GUNGRAUN_COMMON_TARGET_ARCH"),
            env!("__GUNGRAUN_COMMON_TARGET_OS"),
            env!("__GUNGRAUN_COMMON_TARGET_ENV"),
            env!("__GUNGRAUN_COMMON_TARGET_VENDOR"),
            env!("__GUNGRAUN_COMMON_TARGET_ABI"),
        )
    }

    /// Returns the Valgrind implementation for the supplied target components.
    ///
    /// The arguments correspond to Cargo's target architecture, operating system, environment,
    /// vendor, and ABI configuration values. `None` means that the target tuple is not in
    /// Valgrind's supported platform set. The table prioritizes Valgrind support and may include
    /// Valgrind platforms for which Rust does not yet provide an official target. For example, at
    /// the time of writing this documentation, `i686-unknown-illumos`. They are added nonetheless
    /// to this table because Valgrind supports them and they might be added by Rust in the future.
    pub fn from_target(arch: &str, os: &str, env: &str, vendor: &str, abi: &str) -> Option<Self> {
        if arch == "x86_64"
            && (((os == "linux" || os == "android") && abi != "x32")
                || os == "freebsd"
                || (vendor == "apple" && os == "macos")
                || (os == "windows" && env == "gnu")
                || os == "illumos"
                || ((vendor == "sun" || vendor == "pc") && os == "solaris"))
        {
            Some(Self::X86_64)
        } else if arch == "x86"
            && (os == "linux"
                || os == "freebsd"
                || os == "android"
                || (vendor == "apple" && os == "macos")
                || (os == "windows" && env == "gnu")
                || os == "illumos"
                || ((vendor == "sun" || vendor == "pc") && os == "solaris"))
        {
            Some(Self::X86)
        } else if arch == "arm" && (os == "linux" || os == "android") {
            Some(Self::Arm)
        } else if arch == "aarch64"
            && ((os == "linux")
                || os == "freebsd"
                || os == "android"
                || (vendor == "apple" && os == "macos"))
        {
            Some(Self::Aarch64)
        } else if arch == "riscv64" && os == "linux" {
            Some(Self::Riscv64)
        } else if arch == "s390x" && os == "linux" {
            Some(Self::S390x)
        } else if arch == "powerpc" && os == "linux" {
            Some(Self::Powerpc)
            // Note arch matches both little and big endian
        } else if arch == "powerpc64" && os == "linux" {
            Some(Self::Powerpc64)
        } else {
            None
        }
    }
}

impl Display for ValgrindSupport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let support = format!("{self:?}").to_lowercase();
        f.write_str(&support)
    }
}

/// Returns whether Linux Perf supports the current compilation target.
///
/// This checks target-level support only. It does not inspect the `perf` executable, permissions,
/// kernel settings, or requested events.
pub fn is_perf_supported() -> bool {
    cfg!(target_os = "linux")
}

/// Detects all tool families supported by the current compilation target.
///
/// The benchmark harness sends the returned value to the runner, which uses it to discard
/// configured tools that cannot run on the target before resolving an effective default tool.
pub fn supported_tools() -> SupportedTools {
    SupportedTools {
        perf: is_perf_supported(),
        valgrind: ValgrindSupport::new().is_some(),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    impl SupportedTools {
        fn new(perf: bool, valgrind: bool) -> Self {
            Self { perf, valgrind }
        }
    }

    #[rstest]
    #[case::empty_tools("tools=", SupportedTools::default())]
    #[case::perf("tools=perf", SupportedTools::new(true, false))]
    #[case::valgrind("tools=valgrind", SupportedTools::new(false, true))]
    #[case::perf_and_valgrind("tools=perf,valgrind", SupportedTools::new(true, true))]
    #[case::valgrind_and_perf("tools=valgrind,perf", SupportedTools::new(true, true))]
    fn test_supported_tools_from_str(#[case] input: &str, #[case] expected: SupportedTools) {
        assert_eq!(SupportedTools::from_str(input).unwrap(), expected);
    }
}
