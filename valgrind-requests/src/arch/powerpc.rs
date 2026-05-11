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
            "rlwinm 0,0,3,0,31",
            "rlwinm 0,0,13,0,31",
            "rlwinm 0,0,29,0,31",
            "rlwinm 0,0,19,0,31",
            "or 1,1,1",
            lateout("r3") result,
            in("r4") args.as_ptr(),
            in("r3") default,
        };
    }
    result
}
