//! Provide the platform dependent implementations of the valgrind.h header macros and functions
//!
//! The rust `asm!` macro is not stable yet for all platforms which valgrind supports, so we can't
//! deliver the client requests for all platforms. We fall back to `native` for all these platforms.

// The `client_requests_support` cfg is set in the build script
cfg_if::cfg_if! {
    if #[cfg(not(feature = "act"))] {
        pub mod imp {
            #[inline(always)]
            #[expect(clippy::similar_names)]
            pub fn valgrind_do_client_request_expr(
                default: usize,
                _request: cty::c_uint,
                _arg1: usize,
                _arg2: usize,
                _arg3: usize,
                _arg4: usize,
                _arg5: usize,
            ) -> usize {
                default
            }
        }
    } else if #[cfg(client_requests_support = "x86_64")] {
        #[path = "x86_64.rs"]
        pub mod imp;
    } else if #[cfg(client_requests_support = "x86")] {
        #[path = "x86.rs"]
        pub mod imp;
    } else if #[cfg(client_requests_support = "arm")] {
        #[path = "arm.rs"]
        pub mod imp;
    } else if #[cfg(client_requests_support = "aarch64")] {
        #[path = "aarch64.rs"]
        pub mod imp;
    } else if #[cfg(client_requests_support = "riscv64")] {
        #[path = "riscv64.rs"]
        pub mod imp;
    } else if #[cfg(client_requests_support = "s390x")] {
        #[path = "s390x.rs"]
        pub mod imp;
    } else if #[cfg(client_requests_support = "native")] {
        #[path = "native.rs"]
        pub mod imp;
    } else {
        // We're here when `client_requests_support = "no"`
        compile_error!("Client requests are not supported on this platform");
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "act")] {
        #[inline(always)]
        pub fn valgrind_do_client_request_stmt(
            request: cty::c_uint,
            arg1: usize,
            arg2: usize,
            arg3: usize,
            arg4: usize,
            arg5: usize,
        ) {
            imp::valgrind_do_client_request_expr(0, request, arg1, arg2, arg3, arg4, arg5);
        }
    } else {
        #[inline(always)]
        pub fn valgrind_do_client_request_stmt(
            _request: cty::c_uint,
            _arg1: usize,
            _arg2: usize,
            _arg3: usize,
            _arg4: usize,
            _arg5: usize,
        ) {}
    }
}
