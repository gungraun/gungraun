//! Provide the assembly optimized implementation of `valgrind_do_client_request_expr`
//! spell-checker: ignore srli norvc

use core::arch::asm;

/// The optimized implementation of `valgrind_do_client_request_expr`
#[inline(always)]
#[expect(clippy::similar_names)]
pub fn valgrind_do_client_request_expr(
    default: usize,
    request: cty::c_uint,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> usize {
    let args: [usize; 6] = [request as usize, arg1, arg2, arg3, arg4, arg5];
    let result;
    // SAFETY: These assembly instructions do nothing when not run under valgrind
    unsafe {
        asm! {
            ".option push",
            ".option norvc",
            "srli zero, zero, 3",
            "srli zero, zero, 13",
            "srli zero, zero, 51",
            "srli zero, zero, 61",
            "or a0, a0, a0",
            ".option pop",
            lateout("a3") result,
            in("a3") default,
            in("a4") args.as_ptr(),
        };
    }
    result
}
