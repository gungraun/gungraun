//! Common libs of gungraun crates

use std::fmt::Display;

#[cfg(feature = "strum")]
use strum::EnumIter;
#[cfg(feature = "strum")]
pub use strum::IntoEnumIterator;

/// TODO: DOCS
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "strum", derive(EnumIter))]
pub enum ValgrindSupport {
    /// TODO: DOCS
    Arm,
    /// TODO: DOCS
    Aarch64,
    /// TODO: DOCS
    X86,
    /// TODO: DOCS
    X86_64,
    /// TODO: DOCS
    Riscv64,
    /// TODO: DOCS
    S390x,
    /// TODO: DOCS
    Powerpc,
    /// TODO: DOCS
    Powerpc64, // little and big endian
}

impl ValgrindSupport {
    /// TODO: DOCS
    pub fn new() -> Option<Self> {
        Self::from_target(
            env!("__GUNGRAUN_COMMON_TARGET_ARCH"),
            env!("__GUNGRAUN_COMMON_TARGET_OS"),
            env!("__GUNGRAUN_COMMON_TARGET_ENV"),
            env!("__GUNGRAUN_COMMON_TARGET_VENDOR"),
            env!("__GUNGRAUN_COMMON_TARGET_ABI"),
        )
    }

    /// TODO: DOCS
    pub fn from_target(arch: &str, os: &str, env: &str, vendor: &str, abi: &str) -> Option<Self> {
        // Note this table uses Valgrind support as priority. For example some targets might not be
        // supported by Rust like i686-unknown-illumos. They are added nonetheless to this table
        // because Valgrind supports them and they might be added by Rust in the future.
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

/// TODO: DOCS
pub fn is_perf_supported() -> bool {
    cfg!(target_os = "linux")
}
