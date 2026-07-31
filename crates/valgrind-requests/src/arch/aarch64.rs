//! Provide the assembly optimized implementation of `valgrind_do_client_request_expr`

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
            "ror x12, x12, 3",
            "ror x12, x12, 13",
            "ror x12, x12, 51",
            "ror x12, x12, 61",
            "orr x10, x10, x10",
            lateout("x3") result,
            in("x3") default,
            in("x4") args.as_ptr(),
        };
    }
    result
}
