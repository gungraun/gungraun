// Copyright (C) 2000-2017 Julian Seward.  All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of conditions
//    and the following disclaimer.
//
// 2. The origin of this software must not be misrepresented; you must not claim that you wrote the
//    original software.  If you use this software in a product, an acknowledgment in the product
//    documentation would be appreciated but is not required.
//
// 3. Altered source versions must be plainly marked as such, and must not be misrepresented as
//    being the original software.
//
// 4. The name of the author may not be used to endorse or promote products derived from this
//    software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS
// OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
// GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
// NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// ----------------------------------------------------------------
//
// We're using a lot of the original documentation from the `memcheck.h` header file with some small
// adjustments, so above is the original license from `memcheck.h` file.
//
// This file is distributed under the same License as the rest of `valgrind-requests`.
//
// ----------------------------------------------------------------
//
//! All public client requests from the `memcheck.h` header file
//!
//! See also [Memcheck Client
//! Requests](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.clientreqs)

use core::ffi::CStr;

use super::{
    bindings, fatal_error, valgrind_do_client_request_expr, valgrind_do_client_request_stmt,
};

/// The [`BlockHandle`] type as returned by [`create_block`]
///
/// You can pass this [`BlockHandle`] to [`discard`]
pub type BlockHandle = usize;

/// The `LeakCounts` as returned by [`count_leaks`] and [`count_leak_blocks`]
///
/// These client request fills in the four fields of [`LeakCounts`] with the number of bytes of
/// memory found by the previous leak check to be leaked (i.e. the sum of direct leaks and indirect
/// leaks), dubious, reachable and suppressed.
#[derive(Debug, Default, PartialEq, Eq, Clone, Hash)]
pub struct LeakCounts {
    /// The number of bytes of memory of dubious leaks
    pub dubious: cty::c_ulong,
    /// The number of bytes of memory of direct and indirect leaks
    pub leaked: cty::c_ulong,
    /// The number of bytes of memory of reachable leaks
    pub reachable: cty::c_ulong,
    /// The number of bytes of memory of suppressed leaks
    pub suppressed: cty::c_ulong,
}

/// Mark memory `addr` as unaddressable for `len` bytes
#[inline(always)]
pub fn make_mem_noaccess(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::make_mem_noaccess",
        0,
        bindings::VR_MemcheckClientRequest::VR_MAKE_MEM_NOACCESS,
        addr as usize,
        len,
        0,
        0,
        0
    )
}

/// Mark memory at `addr` as addressable but undefined for `len` bytes
#[inline(always)]
pub fn make_mem_undefined(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::make_mem_undefined",
        0,
        bindings::VR_MemcheckClientRequest::VR_MAKE_MEM_UNDEFINED,
        addr as usize,
        len,
        0,
        0,
        0
    )
}

/// Mark memory at `addr` as addressable and defined for `len` bytes.
#[inline(always)]
pub fn make_mem_defined(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::make_mem_defined",
        0,
        bindings::VR_MemcheckClientRequest::VR_MAKE_MEM_DEFINED,
        addr as usize,
        len,
        0,
        0,
        0
    )
}

/// Similar to [`make_mem_defined`] except that addressability is not altered
///
/// Bytes which are addressable are marked as defined, but those which are not addressable are left
/// unchanged.
#[inline(always)]
pub fn make_mem_defined_if_addressable(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::make_mem_defined_if_addressable",
        0,
        bindings::VR_MemcheckClientRequest::VR_MAKE_MEM_DEFINED_IF_ADDRESSABLE,
        addr as usize,
        len,
        0,
        0,
        0
    )
}

/// Create a [`BlockHandle`].
///
/// The `desc` is a C string which is included in any messages pertaining to addresses within the
/// specified memory range. This client request has no other effect on the properties of the memory
/// range.
///
/// The specified address range is associated with the `desc` string. When Memcheck reports
/// invalid access to an address in the range, it will describe it in terms of this block rather
/// than in terms of any other block it knows about. Note that the use of this macro does not
/// actually change the state of memory in any way -- it merely gives a name for the range. At some
/// point you may want Memcheck to stop reporting errors in terms of the block named by
/// `create_block`. To make this possible, `create_block` returns a [`BlockHandle`]. You can pass
/// this [`BlockHandle`] to [`discard`]. After doing so, Valgrind will no longer relate addressing
/// errors in the specified range to the block.
#[inline(always)]
pub fn create_block<T>(addr: *const (), len: usize, desc: T) -> BlockHandle
where
    T: AsRef<CStr>,
{
    do_client_request!(
        "memcheck::create_block",
        0,
        bindings::VR_MemcheckClientRequest::VR_CREATE_BLOCK,
        addr as usize,
        len,
        desc.as_ref().as_ptr() as usize,
        0,
        0
    )
}

/// Discard a [`BlockHandle`] previously acquired with [`create_block`]
///
/// Returns 1 for an invalid handle, 0 for a valid handle. Passing invalid handles to [`discard`] is
/// harmless.
///
/// See also [`create_block`]
#[inline(always)]
pub fn discard(handle: BlockHandle) -> usize {
    do_client_request!(
        "memcheck::discard",
        0,
        bindings::VR_MemcheckClientRequest::VR_DISCARD,
        0,
        handle,
        0,
        0,
        0
    )
}

/// Checks that memory at `addr` is addressable for `len` bytes.
///
/// If suitable addressibility is not established, Valgrind prints an error message and returns the
/// address of the first offending byte. Otherwise, it returns zero.
#[inline(always)]
pub fn check_mem_is_addressable(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::check_mem_is_addressable",
        0,
        bindings::VR_MemcheckClientRequest::VR_CHECK_MEM_IS_ADDRESSABLE,
        addr as usize,
        len,
        0,
        0,
        0
    )
}

/// Check that memory at `addr` is addressable and defined for `len` bytes.
///
/// If suitable addressibility and definedness are not established, Valgrind prints an error message
/// and returns the address of the first offending byte. Otherwise, it returns zero.
#[inline(always)]
pub fn check_mem_is_defined(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::check_mem_is_defined",
        0,
        bindings::VR_MemcheckClientRequest::VR_CHECK_MEM_IS_DEFINED,
        addr as usize,
        len,
        0,
        0,
        0
    )
}

/// Use this macro to force the definedness and addressibility of a `value` to be checked.
///
/// If suitable addressibility and definedness are not established, Valgrind prints an error message
/// and returns the address of the first offending byte. Otherwise, it returns zero.
#[inline(always)]
pub fn check_value_is_defined<T>(value: &T) -> usize {
    do_client_request!(
        "memcheck::check_value_is_defined",
        0,
        bindings::VR_MemcheckClientRequest::VR_CHECK_MEM_IS_DEFINED,
        core::ptr::from_ref::<T>(value) as usize,
        core::mem::size_of::<T>(),
        0,
        0,
        0
    )
}

/// Do a full memory leak check (like `--leak-check=full`) mid-execution
///
/// This is useful for incrementally checking for leaks between arbitrary places in the program's
/// execution.
#[inline(always)]
pub fn do_leak_check() {
    do_client_request!(
        "memcheck::do_leak_check",
        bindings::VR_MemcheckClientRequest::VR_DO_LEAK_CHECK,
        0,
        0,
        0,
        0,
        0
    );
}

/// Same as [`do_leak_check`] but only showing the entries for which there was an increase in leaked
/// bytes or leaked nr of blocks since the previous leak search.
#[inline(always)]
pub fn do_added_leak_check() {
    do_client_request!(
        "memcheck::do_added_leak_check",
        bindings::VR_MemcheckClientRequest::VR_DO_LEAK_CHECK,
        0,
        1,
        0,
        0,
        0
    );
}

/// Same as [`do_added_leak_check`] but showing entries with increased or decreased leaked
/// bytes/blocks since previous leak search.
#[inline(always)]
pub fn do_changed_leak_check() {
    do_client_request!(
        "memcheck::do_changed_leak_check",
        bindings::VR_MemcheckClientRequest::VR_DO_LEAK_CHECK,
        0,
        2,
        0,
        0,
        0
    );
}

/// Same as [`do_leak_check`] but only showing new entries i.e. loss records that were not there in
/// the previous leak search.
#[inline(always)]
pub fn do_new_leak_check() {
    do_client_request!(
        "memcheck::do_new_leak_check",
        bindings::VR_MemcheckClientRequest::VR_DO_LEAK_CHECK,
        0,
        3,
        0,
        0,
        0
    );
}

/// Do a summary memory leak check (like `--leak-check=summary`) mid-execution
#[inline(always)]
pub fn do_quick_leak_check() {
    do_client_request!(
        "memcheck::do_quick_leak_check",
        bindings::VR_MemcheckClientRequest::VR_DO_LEAK_CHECK,
        1,
        0,
        0,
        0,
        0
    );
}

/// Return [`LeakCounts`] found by all previous leak checks
///
/// This client request fills in the four fields of [`LeakCounts`] with the number of bytes of
/// memory found by the previous leak check to be leaked (i.e. the sum of direct leaks and indirect
/// leaks), dubious, reachable and suppressed.
///
/// This is useful in test harness code, after calling [`do_leak_check`] or [`do_quick_leak_check`]
#[inline(always)]
pub fn count_leaks() -> LeakCounts {
    let leaks = LeakCounts::default();
    do_client_request!(
        "memcheck::count_leaks",
        bindings::VR_MemcheckClientRequest::VR_COUNT_LEAKS,
        core::ptr::addr_of!(leaks.leaked) as usize,
        core::ptr::addr_of!(leaks.dubious) as usize,
        core::ptr::addr_of!(leaks.reachable) as usize,
        core::ptr::addr_of!(leaks.suppressed) as usize,
        0
    );
    leaks
}

/// Identical to [`count_leaks`] except that it returns the number of blocks rather than the number
/// of bytes in each category.
#[inline(always)]
pub fn count_leak_blocks() -> LeakCounts {
    let leaks = LeakCounts::default();
    do_client_request!(
        "memcheck::count_leak_blocks",
        bindings::VR_MemcheckClientRequest::VR_COUNT_LEAK_BLOCKS,
        core::ptr::addr_of!(leaks.leaked) as usize,
        core::ptr::addr_of!(leaks.dubious) as usize,
        core::ptr::addr_of!(leaks.reachable) as usize,
        core::ptr::addr_of!(leaks.suppressed) as usize,
        0
    );
    leaks
}

/// Allow you to get the V (validity) bits for an address range `[addr...addr+len-1]`
///
/// The validity data is copied into the provided `bits` slice.
///
/// Return values:
/// * 0   if not running on valgrind
/// * 1   success
/// * 2   [previously indicated unaligned arrays; these are now allowed]
/// * 3   if any parts of `addr`/`bits` are not addressable.
///
/// The metadata is not copied in cases 0, 2 or 3, so it should be impossible to segfault your
/// system by using this call.
///
/// You should probably only set V bits with [`set_vbits`] that you have got with this client
/// request.
///
/// Only for those who really know what they are doing.
#[inline(always)]
pub fn get_vbits(addr: *const (), bits: &mut [u8], len: usize) -> usize {
    do_client_request!(
        "memcheck::get_vbits",
        0,
        bindings::VR_MemcheckClientRequest::VR_GET_VBITS,
        addr as usize,
        bits.as_ptr() as usize,
        len,
        0,
        0
    )
}

/// Allow you to set the V (validity) bits for an address range `[addr...addr+len-1]`
///
/// The validity data is copied from the provided `bits` slice.
///
/// Return values:
/// * 0   if not running on valgrind
/// * 1   success
/// * 2   [previously indicated unaligned arrays;  these are now allowed]
/// * 3   if any parts of `addr`/`bits` are not addressable.
///
/// The metadata is not copied in cases 0, 2 or 3, so it should be impossible to segfault your
/// system by using this call.
///
/// You should probably only set V bits with `set_vbits` that you have got with [`get_vbits`].
///
/// Only for those who really know what they are doing.
#[inline(always)]
pub fn set_vbits(addr: *const (), bits: &[u8], len: usize) -> usize {
    do_client_request!(
        "memcheck::set_vbits",
        0,
        bindings::VR_MemcheckClientRequest::VR_SET_VBITS,
        addr as usize,
        bits.as_ptr() as usize,
        len,
        0,
        0
    )
}

/// Disables reporting of addressing errors in the specified address range.
#[inline(always)]
pub fn disable_addr_error_reporting_in_range(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::disable_addr_error_reporting_in_range",
        0,
        bindings::VR_MemcheckClientRequest::VR_DISABLE_ADDR_ERROR_REPORTING_IN_RANGE,
        addr as usize,
        len,
        0,
        0,
        0
    )
}

/// Enables reporting of addressing errors in the specified address range.
#[inline(always)]
pub fn enable_addr_error_reporting_in_range(addr: *const (), len: usize) -> usize {
    do_client_request!(
        "memcheck::enable_addr_error_reporting_in_range",
        0,
        bindings::VR_MemcheckClientRequest::VR_ENABLE_ADDR_ERROR_REPORTING_IN_RANGE,
        addr as usize,
        len,
        0,
        0,
        0
    )
}
