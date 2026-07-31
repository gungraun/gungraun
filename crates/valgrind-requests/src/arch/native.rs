//! Provide the native implementation of `valgrind_do_client_request_expr`
use crate::native_bindings;

/// Valgrind's native implementation of `valgrind_do_client_request_expr`
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
    // SAFETY: This call is as safe as valgrind's implementation of
    // `valgrind_do_client_request_expr`. The `request` parameter is `cty::c_uint`
    // (matching the bindgen-generated request codes) and is cast to `usize` to
    // match the C function signature which uses `size_t`.
    unsafe {
        native_bindings::valgrind_do_client_request_expr(
            default,
            request as usize,
            arg1,
            arg2,
            arg3,
            arg4,
            arg5,
        )
    }
}
