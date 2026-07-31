//! Idiomatic Rust bindings for [Valgrind's Client Request Mechanism][client-requests] with
//! zero-indirection execution and zero-cost fallback.
//!
//!
//! Valgrind has a trapdoor mechanism via which the client program can pass all manner of requests
//! and queries to Valgrind and the current tool. The so-called client requests are provided to
//! allow you to tell Valgrind facts about the behavior of your program, and also to make queries.
//! In particular, your program can tell Valgrind about things that it otherwise would not know,
//! leading to better results.
//!
//! # Installation/Building
//!
//! ```toml
//! [dependencies]
//! valgrind-requests = "1.0"
//! ```
//!
//! or
//!
//! ```shell
//! cargo add valgrind-requests
//! ```
//!
//! `valgrind-requests` does not depend on any specific version of Valgrind. However, not all client
//! requests are available in all Valgrind versions and this crate will abort the execution
//! producing a panic message if a client request is used but not available in the header
//! file.
//!
//! The client requests need to be built with the Valgrind header files. Usually, these header files
//! are installed by your distribution's package manager with the Valgrind package into a global
//! include path, and you don't need to do anything. Note that the used headers need to match the
//! used Valgrind version.
//!
//! If you encounter problems because the Valgrind header files cannot be found, first ensure you
//! have installed Valgrind and your package manager's package includes the header files. If not or
//! you use a custom build of Valgrind, you can point the `VALGRIND_REQUESTS_VALGRIND_INCLUDE` or
//! the `VALGRIND_REQUESTS_<triple>_VALGRIND_INCLUDE` environment variables to the include path
//! where the Valgrind headers can be found. For example, if the Valgrind header files reside in
//! `/home/foo/repo/valgrind/{valgrind.h, callgrind.h, ...}`, then the environment variable has to
//! point to `VALGRIND_REQUESTS_VALGRIND_INCLUDE=/home/foo/repo` and not
//! `VALGRIND_REQUESTS_VALGRIND_INCLUDE=/home/foo/repo/valgrind`.
//!
//! # Module Organization
//!
//! The client requests are organized into modules representing the source header file. So, if you
//! search for a client request originating from the `valgrind.h` header file, the client request
//! can be found in the [`valgrind`] module. `valgrind-requests` is a complete implementation of all
//! client requests which can be found in the original header files.
//!
//! | Module | Header | Description |
//! | ------ | ------ | ----------- |
//! | [`valgrind`] | `valgrind.h` | Core client requests ([The Client request mechanism][client-requests]) |
//! | [`memcheck`] | `memcheck.h` | [Memcheck: a memory error detector][memcheck-docs] |
//! | [`callgrind`] | `callgrind.h` | [Callgrind: a call-graph generating cache and branch prediction profiler][callgrind-docs] |
//! | [`cachegrind`] | `cachegrind.h` | [Cachegrind: a high-precision tracing profiler][cachegrind-docs] |
//! | [`helgrind`] | `helgrind.h` | [Helgrind: a thread error detector][helgrind-docs] |
//! | [`drd`] | `drd.h` | [DRD: a thread error detector][drd-docs] |
//! | [`dhat`] | `dhat.h` | [DHAT: a dynamic heap analysis tool][dhat-docs] |
//!
//! Instead of using macros like in Valgrind we're using functions and small letter names, stripping
//! the prefix if it is equal to the enclosing module. For example the client request
//! `RUNNING_ON_VALGRIND` from the `valgrind.h` file equals [`valgrind::running_on_valgrind`]
//! and `VALGRIND_COUNT_ERRORS` from the same `valgrind.h` header file equals
//! [`valgrind::count_errors`].
//!
//! The only exception to this rule are the [`valgrind_printf`] macro and its descendants like
//! [`valgrind_printf_unchecked`] which can be found in the crate root.
//!
//! # Features
//!
//! Core client request APIs are `no_std` compatible. The `std` feature which implies the `alloc`
//! feature are enabled by default.
//!
//! This crate provides two execution feature levels:
//!
//! - **`act`** *(default)*: Enables actual execution of client requests when running under
//!   Valgrind. Implies `stubs`.
//! - **`stubs`**: Enables the same public API surface and build-time code generation, but all
//!   client requests compile to no-ops that return default values. The compiler will optimize them
//!   away entirely, making this a zero-cost option suitable for production code.
//!
//! Formatting convenience macros such as [`valgrind_printf`] and [`valgrind_println`] require the
//! **`alloc`** feature because they allocate owned C strings. In allocation-free builds, you can
//! use [`valgrind_print`] or [`valgrind_print_backtrace`] instead.
//!
//! To use the zero-cost fallback, for example if you want to use the client requests for tests or
//! benchmarks and need to make annotations in production code:
//!
//! ```toml
//! [dependencies]
//! valgrind-requests = { version = "1.0", default-features = false, features = ["stubs"] }
//!
//! [dev-dependencies]
//! valgrind-requests = { version = "1.0" }
//! ```
//!
//! The stubs compile down to nothing and your production code is as performant as without any
//! annotations. If your production code uses the formatting convenience macros, enable both `stubs`
//! and `alloc` with `features = ["stubs", "alloc"]`.
//!
//! # Performance and implementation details
//!
//! If possible, client requests execute with zero indirection and the same overhead as the original
//! Valgrind C macros usable [even in high performance code][client-requests]. On
//! Valgrind-supported platforms for which zero-indirection isn't implemented by us, a native C FFI
//! binding is used which introduces at least an additional frame on the stack and the costs for the
//! function call. That means all targets covered by Valgrind are also covered by
//! `valgrind-requests`. Targets not supported by Valgrind produce a compile error.
//!
//! | Target               | Zero-indirection | Notes                                         |
//! | -------------------- | ---------------- | --------------------------------------------- |
//! | `x86_64/linux`       | yes              | -                                             |
//! | `x86_64/android`     | yes              | except the x32 ABI                            |
//! | `x86_64/freebsd`     | yes              | -                                             |
//! | `x86_64/macos`       | yes              | the versions supported by Valgrind            |
//! | `x86_64/windows+gnu` | yes              | -                                             |
//! | `x86_64/solaris`     | yes              | -                                             |
//! | `x86/linux`          | yes              | -                                             |
//! | `x86/android`        | yes              | -                                             |
//! | `x86/freebsd`        | yes              | -                                             |
//! | `x86/macos`          | yes              | the versions supported by Valgrind            |
//! | `x86/windows+gnu`    | yes              | -                                             |
//! | `x86/solaris`        | yes              | -                                             |
//! | `arm/linux`          | yes              | -                                             |
//! | `arm/android`        | yes              | -                                             |
//! | `aarch64/linux`      | yes              | -                                             |
//! | `aarch64/android`    | yes              | -                                             |
//! | `aarch64/freebsd`    | yes              | -                                             |
//! | `aarch64/macos`      | yes              | [LouisBrunner/valgrind-macos][valgrind-macos] |
//! | `riscv64/linux`      | yes              | -                                             |
//! | `s390x/linux`        | yes              | -                                             |
//! | `powerpc/linux`      | yes              | rust >= 1.95.0                                |
//! | `powerpc64/linux`    | yes              | rust >= 1.95.0                                |
//! | `powerpc64le/linux`  | yes              | rust >= 1.95.0                                |
//! | `mips32/linux`       | no               | no rust inline assembly available             |
//! | `mips64/linux`       | no               | no rust inline assembly available             |
//! | `nanomips/linux`     | no               | no zero-indirection planned                   |
//! | `x86/windows+msvc`   | no               | no zero-indirection planned                   |
//!
//! To disable the native C FFI binding as fallback you can set the environment variable
//! `VALGRIND_REQUESTS_STRATEGY=strict` (possible values are: `strict`, `fallback`).
//!
//! # Sources and additional documentation
//!
//! A lot of the library documentation of the client requests within this module and its submodules
//! is taken from the online manual and the Valgrind header files. For more details see also [The
//! Client Request mechanism][client-requests]
//!
//! [callgrind-docs]:
//!     https://valgrind.org/docs/manual/cl-manual.html#cl-manual.clientrequests
//! [cachegrind-docs]:
//!     https://valgrind.org/docs/manual/cg-manual.html#cg-manual.clientrequests
//! [client-requests]:
//!     https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq
//! [dhat-docs]: https://valgrind.org/docs/manual/dh-manual.html
//! [drd-docs]:
//!     https://valgrind.org/docs/manual/drd-manual.html#drd-manual.clientreqs
//! [helgrind-docs]:
//!     https://valgrind.org/docs/manual/hg-manual.html#hg-manual.client-requests
//! [memcheck-docs]:
//!     https://valgrind.org/docs/manual/mc-manual.html#mc-manual.clientreqs
//! [valgrind-macos]: https://github.com/LouisBrunner/valgrind-macos

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(test(attr(warn(unused))))]
#![doc(test(attr(allow(unused_extern_crates))))]
#![cfg_attr(not(feature = "std"), no_std)]
#![expect(clippy::arbitrary_source_item_ordering)]

#[cfg(feature = "alloc")]
#[doc(hidden)]
pub extern crate alloc as __alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(not(feature = "stubs"))]
compile_error!("valgrind-requests requires either the `stubs` or `act` feature");

/// Returns `true` if a client request is defined and available in the used Valgrind version.
///
/// For internal use only!
///
/// We do this check to avoid incompatibilities with older Valgrind versions which might not have
/// all client requests available we're offering.
///
/// We're only using constant values known at compile time, which the compiler will finally optimize
/// away, so this macro costs us nothing.
macro_rules! is_def {
    ($user_req:path) => {{ $user_req as cty::c_uint > 0x1000 }};
}

/// The macro which uses [`valgrind_do_client_request_stmt`] or [`valgrind_do_client_request_expr`]
/// to execute the client request.
///
/// For internal use only!
///
/// This macro has two forms: The first takes 7 arguments `name, request, arg1, ..., arg5` and
/// returns `()`. The expanded macro of this form calls [`valgrind_do_client_request_stmt`]. The
/// second first has 8 arguments `name, default, request, arg1, ..., arg5` returning a `usize`. The
/// expanded macro of this form calls [`valgrind_do_client_request_expr`].
///
/// Both forms will raise a [`fatal_error`] in case the [`is_def`] macro returns false.
macro_rules! do_client_request {
    ($name:literal, $request:path, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr) => {{
        if is_def!($request) {
            valgrind_do_client_request_stmt(
                $request as cty::c_uint,
                $arg1,
                $arg2,
                $arg3,
                $arg4,
                $arg5,
            );
        } else {
            fatal_error($name);
        }
    }};
    (
        $name:literal,
        $default:expr,
        $request:path,
        $arg1:expr,
        $arg2:expr,
        $arg3:expr,
        $arg4:expr,
        $arg5:expr
    ) => {{
        if is_def!($request) {
            valgrind_do_client_request_expr(
                $default,
                $request as cty::c_uint,
                $arg1,
                $arg2,
                $arg3,
                $arg4,
                $arg5,
            )
        } else {
            fatal_error($name);
        }
    }};
}

/// Convenience macro to create a `\0`-byte terminated `alloc::ffi::CString` from a literal string
///
/// This macro requires the `alloc` feature. In allocation-free builds, use [`valgrind_print`]
/// instead.
///
/// The string literal passed to this macro must not contain or end with a `\0`-byte. If you need a
/// checked version of `alloc::ffi::CString` you can use `alloc::ffi::CString::new`.
///
/// # Safety
///
/// This macro is unsafe but convenient and efficient. It is your responsibility to ensure that the
/// input string literal does not contain any `\0` bytes.
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! cstring {
    ($string:literal) => {{
        $crate::__alloc::ffi::CString::from_vec_with_nul_unchecked(
            concat!($string, "\0").as_bytes().to_vec(),
        )
    }};
}

/// Convenience macro to create a `\0`-byte terminated `alloc::ffi::CString` from a format string
///
/// This macro requires the `alloc` feature. In allocation-free builds, use [`valgrind_print`]
/// instead.
///
/// The format string passed to this macro must not contain or end with a `\0`-byte.
///
/// # Safety
///
/// The same safety conditions as to the [`cstring`] macro apply here
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! format_cstring {
    ($($args:tt)*) => {{
        $crate::__alloc::ffi::CString::from_vec_with_nul_unchecked(
            $crate::__alloc::format!("{}\0", format_args!($($args)*)).into_bytes()
        )
    }};
}

cfg_if! {
    if #[cfg(feature = "act")] {
        /// Prints to the Valgrind log.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print`] instead.
        ///
        /// This macro is a safe variant of the `VALGRIND_PRINTF` function, checking for `\0` bytes
        /// in the formatting string. However, if you're sure there are no `\0` bytes present you
        /// can safely use [`crate::valgrind_printf_unchecked`] which performs better compared to
        /// this macro.
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf {
            ($($args:tt)*) => {{
                match $crate::__alloc::ffi::CString::from_vec_with_nul(
                    $crate::__alloc::format!("{}\0", format_args!($($args)*)).into_bytes()
                ) {
                    Ok(c_string) => {
                        unsafe {
                            $crate::__valgrind_print(
                                c_string.as_ptr()
                            );
                        }
                        Ok(())
                    },
                    Err(error) => Err(
                        $crate::error::ClientRequestError::from(error)
                    )
                }
            }};
        }

        /// Prints to the Valgrind log.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print`] instead.
        ///
        /// Use this macro only if you are sure there are no `\0`-bytes in the formatted string. If
        /// unsure use the safe [`crate::valgrind_printf`] variant.
        ///
        /// This variant performs better than [`crate::valgrind_printf`].
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf_unchecked {
            ($($args:tt)*) => {{
                let string = $crate::__alloc::format!("{}\0", format_args!($($args)*));
                $crate::__valgrind_print(
                    string.as_ptr() as *const $crate::__cty::c_char
                );
            }};
        }

        /// Prints to the Valgrind log ending with a newline.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print`] instead.
        ///
        /// See also [`crate::valgrind_printf`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println {
            () => { $crate::valgrind_printf!("\n") };
            ($($arg:tt)*) => {{
                match $crate::__alloc::ffi::CString::from_vec_with_nul(
                    $crate::__alloc::format!("{}\n\0", format_args!($($arg)*)).into_bytes()
                ) {
                    Ok(c_string) => {
                        unsafe {
                            $crate::__valgrind_print(
                                c_string.as_ptr()
                            );
                        }
                        Ok(())
                    },
                    Err(error) => Err(
                        $crate::error::ClientRequestError::from(error)
                    )
                }
            }};
        }

        /// Prints to the Valgrind log ending with a newline.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print`] instead.
        ///
        /// See also [`crate::valgrind_printf_unchecked`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println_unchecked {
            () => { $crate::valgrind_printf_unchecked!("\n") };
            ($($args:tt)*) => {{
                let string = $crate::__alloc::format!("{}\n\0", format_args!($($args)*));
                $crate::__valgrind_print(
                    string.as_ptr() as *const $crate::__cty::c_char
                );
            }};
        }

        /// Prints to the Valgrind log with a backtrace.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf_backtrace {
            ($($arg:tt)*) => {{
                match $crate::__alloc::ffi::CString::from_vec_with_nul(
                    $crate::__alloc::format!("{}\0", format_args!($($arg)*)).into_bytes()
                ) {
                    Ok(c_string) => {
                        unsafe {
                            $crate::__valgrind_print_backtrace(
                                c_string.as_ptr()
                            );
                        }
                        Ok(())
                    },
                    Err(error) => Err(
                        $crate::error::ClientRequestError::from(error)
                    )
                }
            }};
        }

        /// Prints to the Valgrind log with a backtrace.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf_unchecked`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf_backtrace_unchecked {
            ($($arg:tt)*) => {{
                let string = $crate::__alloc::format!("{}\0", format_args!($($arg)*));
                $crate::__valgrind_print_backtrace(
                    string.as_ptr() as *const $crate::__cty::c_char
                );
            }};
        }

        /// Prints to the Valgrind log with a backtrace, ending the formatted string with a newline.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println_backtrace {
            () => { $crate::valgrind_printf_backtrace!("\n") };
            ($($arg:tt)*) => {{
                match $crate::__alloc::ffi::CString::from_vec_with_nul(
                    $crate::__alloc::format!("{}\n\0", format_args!($($arg)*)).into_bytes()
                ) {
                    Ok(c_string) => {
                        unsafe {
                            $crate::__valgrind_print_backtrace(
                                c_string.as_ptr()
                            );
                        }
                        Ok(())
                    },
                    Err(error) => Err(
                        $crate::error::ClientRequestError::from(error)
                    )
                }
            }};
        }

        /// Prints to the Valgrind log with a backtrace, ending the formatted string with a newline.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf_unchecked`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println_backtrace_unchecked {
            () => { $crate::valgrind_printf_backtrace_unchecked!("\n") };
            ($($arg:tt)*) => {{
                let string = $crate::__alloc::format!("{}\n\0", format_args!($($arg)*));
                unsafe {
                    $crate::__valgrind_print_backtrace(
                        string.as_ptr() as *const $crate::__cty::c_char
                    );
                }
            }};
        }
    } else {
        /// No-op variant of [`valgrind_printf`] for stub builds.
        ///
        /// This macro requires the `alloc` feature to preserve the same fallible API as the active
        /// formatting macro. In allocation-free builds, use [`valgrind_print`] instead.
        ///
        /// This macro is a safe variant of the `VALGRIND_PRINTF` function, checking for `\0` bytes
        /// in the formatting string. However, if you're sure there are no `\0` bytes present you
        /// can safely use [`crate::valgrind_printf_unchecked`] which performs better compared to
        /// this macro.
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf {
            ($($arg:tt)*) => {{
                let res: Result<(), $crate::error::ClientRequestError> = Ok(());
                res
            }};
        }

        /// No-op variant of [`valgrind_printf_unchecked`] for stub builds.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print`] instead.
        ///
        /// Use this macro only if you are sure there are no `\0`-bytes in the formatted string. If
        /// unsure use the safe [`crate::valgrind_printf`] variant.
        ///
        /// This variant performs better than [`crate::valgrind_printf`].
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf_unchecked {
            ($($arg:tt)*) => {{ $crate::__no_op() }};
        }

        /// No-op variant of [`valgrind_println`] for stub builds.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print`] instead.
        ///
        /// See also [`crate::valgrind_printf`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println {
            ($($arg:tt)*) => {{
                let res: Result<(), $crate::error::ClientRequestError> = Ok(());
                res
            }};
        }

        /// No-op variant of [`valgrind_println_unchecked`] for stub builds.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print`] instead.
        ///
        /// See also [`crate::valgrind_printf_unchecked`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println_unchecked {
            ($($arg:tt)*) => {{ $crate::__no_op() }};
        }

        /// No-op variant of [`valgrind_printf_backtrace`] for stub builds.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf_backtrace {
            ($($arg:tt)*) => {{
                let res: Result<(), $crate::error::ClientRequestError> = Ok(());
                res
            }};
        }

        /// No-op variant of [`valgrind_printf_backtrace_unchecked`] for stub builds.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf_unchecked`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_printf_backtrace_unchecked {
            ($($arg:tt)*) => {{ $crate::__no_op() }};
        }

        /// No-op variant of [`valgrind_println_backtrace`] for stub builds.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println_backtrace {
            ($($arg:tt)*) => {{
                let res: Result<(), $crate::error::ClientRequestError> = Ok(());
                res
            }};
        }

        /// No-op variant of [`valgrind_println_backtrace_unchecked`] for stub builds.
        ///
        /// This macro requires the `alloc` feature. In allocation-free builds, use
        /// [`valgrind_print_backtrace`] instead.
        ///
        /// See also [`crate::valgrind_printf_unchecked`]
        #[cfg(feature = "alloc")]
        #[macro_export]
        macro_rules! valgrind_println_backtrace_unchecked {
            ($($arg:tt)*) => {{ $crate::__no_op() }};
        }
    }
}

mod arch;
mod bindings;
pub mod cachegrind;
pub mod callgrind;
pub mod dhat;
pub mod drd;
#[cfg(feature = "alloc")]
pub mod error;
pub mod helgrind;
pub mod memcheck;
#[cfg(feature = "act")]
mod native_bindings;
pub mod valgrind;
use core::ffi::CStr;

use arch::imp::valgrind_do_client_request_expr;
use arch::valgrind_do_client_request_stmt;
use cfg_if::cfg_if;
#[doc(hidden)]
pub use cty as __cty;

/// The `ThreadId` is used by some client requests to represent the `tid` which Valgrind uses or
/// returns
///
/// This type has no relationship to `std::thread::ThreadId`!
pub type ThreadId = usize;

/// The `StackId` is used and returned by some client requests and represents an id on Valgrind's
/// stack
pub type StackId = usize;

/// The raw file descriptor number
///
/// This type has no relationship to the standard library type definition of `RawFd` besides they
/// are wrapping the same type on unix systems.
pub type RawFd = cty::c_int;

/// Valgrind's version number from the `valgrind.h` file
///
/// Note that the version numbers were introduced at Valgrind version 3.6 and so would not exist in
/// version 3.5 or earlier. `VALGRIND_VERSION` is None is this case, else it is a tuple `(MAJOR,
/// MINOR)`
pub const VALGRIND_VERSION: Option<(u32, u32)> = {
    if bindings::VR_VALGRIND_MAJOR == 0 {
        None
    } else {
        Some((bindings::VR_VALGRIND_MAJOR, bindings::VR_VALGRIND_MINOR))
    }
};

fn fatal_error(func: &str) -> ! {
    if let Some((major, minor)) = VALGRIND_VERSION {
        panic!(
            "{0}: FATAL: {0}::{func} not available! To be able to use this client request, a \
             newer Valgrind version is required. The detected Valgrind version of the \
             `valgrind.h` header file is {major}.{minor}. Aborting...",
            module_path!(),
        );
    } else {
        panic!(
            "{0}: FATAL: {0}::{func} not available! The Valgrind headers could not be found or \
             the Valgrind version is too old. Check your Valgrind installation or set \
             VALGRIND_REQUESTS_VALGRIND_INCLUDE to the Valgrind include path. Aborting...",
            module_path!(),
        );
    }
}

cfg_if! {
    if #[cfg(feature = "act")] {
        /// Prints a C string to the Valgrind log.
        ///
        /// This function is the allocation-free equivalent of [`valgrind_printf_unchecked`]. It
        /// accepts any value that can be borrowed as a [`CStr`], so it can be used in `no_std`
        /// builds without the `alloc` feature. The stub implementation of this function is a no-op
        /// and compiles away.
        ///
        /// The provided string must be NUL-terminated and must not contain interior NUL bytes, as
        /// required by [`CStr`].
        ///
        /// # Examples
        ///
        /// ```rust
        /// valgrind_requests::valgrind_print(c"hello from Valgrind\n");
        /// ```
        #[inline]
        pub fn valgrind_print<T>(c_string: T)
        where
            T: AsRef<CStr>,
        {
            // SAFETY: `CStr` guarantees a valid NUL-terminated byte sequence for the duration of
            // the call.
            unsafe { __valgrind_print(c_string.as_ref().as_ptr()) }
        }

        /// Prints a C string with a backtrace to the Valgrind log.
        ///
        /// This function is the allocation-free equivalent of
        /// [`valgrind_printf_backtrace_unchecked`]. It accepts any value that can be borrowed as a
        /// [`CStr`], so it can be used in `no_std` builds without the `alloc` feature. The stub
        /// implementation of this function is a no-op and compiles away.
        ///
        /// The provided string must be NUL-terminated and must not contain interior NUL bytes, as
        /// required by [`CStr`].
        ///
        /// # Examples
        ///
        /// ```rust
        /// valgrind_requests::valgrind_print_backtrace(c"important checkpoint\n");
        /// ```
        #[inline]
        pub fn valgrind_print_backtrace<T>(c_string: T)
        where
            T: AsRef<CStr>,
        {
            // SAFETY: `CStr` guarantees a valid NUL-terminated byte sequence for the duration of
            // the call.
            unsafe { __valgrind_print_backtrace(c_string.as_ref().as_ptr()) }
        }
    } else {
        /// No-op variant of [`valgrind_print`] for stub builds.
        ///
        /// This function preserves the same allocation-free API surface as active builds and
        /// compiles away entirely.
        #[inline]
        pub fn valgrind_print<T>(_c_string: T)
        where
            T: AsRef<CStr>,
        {}

        /// No-op variant of [`valgrind_print_backtrace`] for stub builds.
        ///
        /// This function preserves the same allocation-free API surface as active builds and
        /// compiles away entirely.
        #[inline]
        pub fn valgrind_print_backtrace<T>(_c_string: T)
        where
            T: AsRef<CStr>,
        {}
    }
}

#[cfg(feature = "act")]
#[doc(hidden)]
#[inline(always)]
pub unsafe fn __valgrind_print(ptr: *const cty::c_char) {
    // SAFETY: The safety of this function must be ensured by the caller
    unsafe {
        native_bindings::valgrind_printf(ptr);
    }
}

#[cfg(feature = "act")]
#[doc(hidden)]
#[inline(always)]
pub unsafe fn __valgrind_print_backtrace(ptr: *const cty::c_char) {
    // SAFETY: The safety of this function must be ensured by the caller
    unsafe {
        native_bindings::valgrind_printf_backtrace(ptr);
    }
}

#[doc(hidden)]
#[inline(always)]
pub unsafe fn __no_op() {}
