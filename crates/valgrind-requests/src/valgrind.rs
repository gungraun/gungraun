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
// We're using a lot of the original documentation from the `valgrind.h` header file with some small
// adjustments, so above is the original license from `valgrind.h` file.
//
// This file is distributed under the same License as the rest of `valgrind-requests`.
//
// ----------------------------------------------------------------
//
//! All public client requests from the `valgrind.h` header file
//!
//! See also [The client request
//! mechanism](https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq)

/// The `MempoolFlags` usable in [`create_mempool_ext`] as `flags`.
#[expect(non_snake_case)]
pub mod MempoolFlags {
    /// When `MempoolFlags` is `DEFAULT`, the behavior is identical to [`super::create_mempool`].
    pub const DEFAULT: u8 = 0;

    /// A [`METAPOOL`] can also be marked as an `auto free` pool
    ///
    /// This flag, which must be OR-ed together with the [`METAPOOL`].
    ///
    /// For an `auto free` pool, [`super::mempool_free`] will automatically free the second level
    /// blocks that are contained inside the first level block freed with [`super::mempool_free`].
    /// In other words, calling [`super::mempool_free`] will cause implicit calls to
    /// [`super::freelike_block`] for all the second level blocks included in the first level block.
    ///
    /// Note: it is an error to use this flag without the [`METAPOOL`] flag.
    pub const AUTOFREE: u8 = 1;

    /// The flag [`super::MempoolFlags::METAPOOL`]
    ///
    /// The flag [`super::MempoolFlags::METAPOOL`] specifies that the pieces of memory associated
    /// with the pool using [`super::mempool_alloc`] will be used by the application as superblocks
    /// to dole out [`super::malloclike_block`] blocks using [`super::malloclike_block`].
    ///
    /// In other words, a meta pool is a "2 levels" pool : first level is the blocks described by
    /// [`super::mempool_alloc`] The second level blocks are described using
    /// [`super::malloclike_block`]. Note that the association between the pool and the second level
    /// blocks is implicit : second level blocks will be located inside first level blocks. It is
    /// necessary to use the `METAPOOL` flag for such 2 levels pools, as otherwise Valgrind will
    /// detect overlapping memory blocks, and will abort execution (e.g. during leak search).
    pub const METAPOOL: u8 = 2;
}

use core::ffi::CStr;

use super::{
    RawFd, StackId, ThreadId, bindings, fatal_error, valgrind_do_client_request_expr,
    valgrind_do_client_request_stmt,
};

/// Returns the number of Valgrinds this code is running under
///
/// That is, 0 if running natively, 1 if running under Valgrind, 2 if running under Valgrind which
/// is running under another Valgrind, etc.
#[inline(always)]
pub fn running_on_valgrind() -> usize {
    do_client_request!(
        "valgrind::running_on_valgrind",
        0,
        bindings::VR_ValgrindClientRequest::VR_RUNNING_ON_VALGRIND,
        0,
        0,
        0,
        0,
        0
    )
}

/// Discard translation of code in the range [addr ... addr + len - 1].
///
/// Useful if you are debugging a `JITter` or some such, since it provides a way to make sure
/// Valgrind will retranslate the invalidated area.
#[inline(always)]
pub fn discard_translations(addr: *const (), len: usize) {
    do_client_request!(
        "valgrind::discard_translations",
        bindings::VR_ValgrindClientRequest::VR_DISCARD_TRANSLATIONS,
        addr as usize,
        len,
        0,
        0,
        0
    );
}

/// Returns `true` if the tool replaces malloc (e.g., Memcheck).
///
/// Returns `false` if the tool does not replace malloc (e.g., Cachegrind and Callgrind) or if the
/// executable is not running under Valgrind.
#[inline(always)]
pub fn replaces_malloc() -> bool {
    do_client_request!(
        "valgrind::replaces_malloc",
        0,
        bindings::VR_ValgrindClientRequest::VR_VALGRIND_REPLACES_MALLOC,
        0,
        0,
        0,
        0,
        0
    ) != 0
}

/// Get the running tool name as a string.
///
/// Returns the required length (including terminating nul) for the input `buffer`. Returns `0` and
/// the contents of `buffer` are not modified if not running under Valgrind. `buffer` may be
/// [`core::ptr::null_mut`], which can be used to query the required buffer length before making a
/// real request for the tool name.
///
/// `len` should be the length of `buffer`. The returned string in `buffer` will be nul terminated.
/// If `buffer` is too small to contain the tool name then the name will be truncated.
#[inline(always)]
pub fn get_toolname(buffer: *mut u8, len: usize) -> usize {
    do_client_request!(
        "valgrind::get_toolname",
        0,
        bindings::VR_ValgrindClientRequest::VR_VALGRIND_GET_TOOLNAME,
        buffer as usize,
        len,
        0,
        0,
        0
    )
}

/// Allow control to move from the simulated CPU to the real CPU, calling an arbitrary function.
///
/// Note that the current [`ThreadId`] is inserted as the first argument.
/// So this call:
///
/// `non_simd_call0(func)`
///
/// requires func to have this signature:
///
/// `usize func(ThreadId tid)`
///
/// Note that these client requests are not entirely reliable. For example, if you call a function
/// with them that subsequently calls `printf()`, there's a high chance Valgrind will crash.
/// Generally, your prospects of these working are made higher if the called function does not refer
/// to any global variables, and does not refer to other functions (print! et al.).
#[inline(always)]
pub fn non_simd_call0(func: fn(ThreadId) -> usize) -> usize {
    do_client_request!(
        "valgrind::non_simd_call0",
        0,
        bindings::VR_ValgrindClientRequest::VR_CLIENT_CALL0,
        func as *const () as usize,
        0,
        0,
        0,
        0
    )
}

/// Allow control to move from the simulated CPU to the real CPU, calling an arbitrary function.
///
/// See also [`non_simd_call0`]
///
/// # Examples
///
/// ```rust,no_run
/// let num = 42i32;
/// let res = valgrind_requests::valgrind::non_simd_call1(
///     |_tid, a| unsafe { ((a as *const i32).as_ref().unwrap() + 2) as usize },
///     (&num) as *const i32 as usize,
/// );
/// assert_eq!(res, 44);
/// ```
#[inline(always)]
pub fn non_simd_call1(func: fn(ThreadId, usize) -> usize, arg1: usize) -> usize {
    do_client_request!(
        "valgrind::non_simd_call1",
        0,
        bindings::VR_ValgrindClientRequest::VR_CLIENT_CALL1,
        func as *const () as usize,
        arg1,
        0,
        0,
        0
    )
}

/// Allow control to move from the simulated CPU to the real CPU, calling an arbitrary function.
///
/// See also [`non_simd_call0`] and [`non_simd_call1`]
#[inline(always)]
pub fn non_simd_call2(
    func: fn(ThreadId, usize, usize) -> usize,
    arg1: usize,
    arg2: usize,
) -> usize {
    do_client_request!(
        "valgrind::non_simd_call2",
        0,
        bindings::VR_ValgrindClientRequest::VR_CLIENT_CALL2,
        func as *const () as usize,
        arg1,
        arg2,
        0,
        0
    )
}

/// Allow control to move from the simulated CPU to the real CPU, calling an arbitrary function.
///
/// See also [`non_simd_call0`] and [`non_simd_call1`]
#[inline(always)]
pub fn non_simd_call3(
    func: fn(ThreadId, usize, usize, usize) -> usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> usize {
    do_client_request!(
        "valgrind::non_simd_call3",
        0,
        bindings::VR_ValgrindClientRequest::VR_CLIENT_CALL3,
        func as *const () as usize,
        arg1,
        arg2,
        arg3,
        0
    )
}

/// Counts the number of errors that have been recorded by a tool.
///
/// Can be useful to e.g. can send output to /dev/null and still count errors.
///
/// The tool must record the errors with `VG_(maybe_record_error)()` or `VG_(unique_error)()` for
/// them to be counted. These are to my best knowledge (as of Valgrind 3.22) `Memcheck`, `DRD` and
/// `Helgrind`.
#[inline(always)]
pub fn count_errors() -> usize {
    do_client_request!(
        "valgrind::count_errors",
        0,
        bindings::VR_ValgrindClientRequest::VR_COUNT_ERRORS,
        0,
        0,
        0,
        0,
        0
    )
}

/// Several Valgrind tools (Memcheck, Massif, Helgrind, DRD) rely on knowing when heap blocks are
/// allocated to give accurate results.
///
/// Ignored if addr == 0.
///
/// The following description is taken almost untouched from the docs in the `valgrind.h` header
/// file.
///
/// This happens automatically for the standard allocator functions such as `malloc()`, `calloc()`,
/// `realloc()`, `memalign()`, `new`, `new[]`, `free()`, `delete`, `delete[]`, etc.
///
/// But if your program uses a custom allocator, this doesn't automatically happen, and Valgrind
/// will not do as well. For example, if you allocate superblocks with `mmap()` and then allocates
/// chunks of the superblocks, all Valgrind's observations will be at the `mmap()` level, and it
/// won't know that the chunks should be considered separate entities.  In Memcheck's case, that
/// means you probably won't get heap block overrun detection (because there won't be redzones
/// marked as unaddressable) and you definitely won't get any leak detection.
///
/// The following client requests allow a custom allocator to be annotated so that it can be handled
/// accurately by Valgrind.
///
/// [`malloclike_block`] marks a region of memory as having been allocated by a malloc()-like
/// function. For Memcheck (an illustrative case), this does two things:
///
/// - It records that the block has been allocated.  This means any addresses within the block
///   mentioned in error messages will be identified as belonging to the block.  It also means that
///   if the block isn't freed it will be detected by the leak checker.
/// - It marks the block as being addressable and undefined (if `is_zeroed` is not set), or
///   addressable and defined (if `is_zeroed` is set). This controls how accesses to the block by
///   the program are handled.
///
/// `addr` is the start of the usable block (ie. after any redzone), `size` is its size. `redzone`
/// is the redzone size if the allocator can apply redzones -- these are blocks of padding at the
/// start and end of each block. Adding redzones is recommended as it makes it much more likely
/// Valgrind will spot block overruns. `is_zeroed` indicates if the memory is zeroed (or filled
/// with another predictable value), as is the case for `calloc()`.
///
/// [`malloclike_block`] should be put immediately after the point where a heap block -- that will
/// be used by the client program -- is allocated. It's best to put it at the outermost level of the
/// allocator if possible; for example, if you have a function `my_alloc()` which calls
/// `internal_alloc()`, and the client request is put inside `internal_alloc()`, stack traces
/// relating to the heap block will contain entries for both `my_alloc()` and `internal_alloc()`,
/// which is probably not what you want.
///
/// For Memcheck users: if you use [`malloclike_block`] to carve out custom blocks from within a
/// heap block, B, that has been allocated with malloc/calloc/new/etc, then block B will be
/// *ignored* during leak-checking -- the custom blocks will take precedence.
///
/// In many cases, these three client requests (`malloclike_block`, [`resizeinplace_block`],
/// [`freelike_block`]) will not be enough to get your allocator working well with Memcheck. More
/// specifically, if your allocator writes to freed blocks in any way then a
/// [`super::memcheck::make_mem_undefined`] call will be necessary to mark the memory as addressable
/// just before the zeroing occurs, otherwise you'll get a lot of invalid write errors.  For
/// example, you'll need to do this if your allocator recycles freed blocks, but it zeroes them
/// before handing them back out (via `malloclike_block`). Alternatively, if your allocator reuses
/// freed blocks for allocator-internal data structures, [`super::memcheck::make_mem_undefined`]
/// calls will also be necessary.
///
/// Really, what's happening is a blurring of the lines between the client program and the
/// allocator... after [`freelike_block`] is called, the memory should be considered unaddressable
/// to the client program, but the allocator knows more than the rest of the client program and so
/// may be able to safely access it. Extra client requests are necessary for Valgrind to understand
/// the distinction between the allocator and the rest of the program.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools) and [Memcheck:
/// Client requests](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.clientreqs)
#[inline(always)]
pub fn malloclike_block(addr: *const (), size: usize, redzone: usize, is_zeroed: bool) {
    do_client_request!(
        "valgrind::malloclike_block",
        bindings::VR_ValgrindClientRequest::VR_MALLOCLIKE_BLOCK,
        addr as usize,
        size,
        redzone,
        usize::from(is_zeroed),
        0
    );
}

/// `resizeinplace_block` informs a tool about reallocation.
///
/// The following description is taken almost untouched from the docs in the `valgrind.h` header
/// file.
///
/// For Memcheck, it does four things:
///
/// - It records that the size of a block has been changed. This assumes that the block was
///   annotated as having been allocated via [`malloclike_block`]. Otherwise, an error will be
///   issued.
/// - If the block shrunk, it marks the freed memory as being unaddressable.
/// - If the block grew, it marks the new area as undefined and defines a red zone past the end of
///   the new block.
/// - The V-bits of the overlap between the old and the new block are preserved.
///
/// `resizeinplace_block` should be put after allocation of the new block and before deallocation of
/// the old block.
///
/// See also [`malloclike_block`] for more details
#[inline(always)]
pub fn resizeinplace_block(addr: *const (), old_size: usize, new_size: usize, redzone: usize) {
    do_client_request!(
        "valgrind::resizeinplace_block",
        bindings::VR_ValgrindClientRequest::VR_RESIZEINPLACE_BLOCK,
        addr as usize,
        old_size,
        new_size,
        redzone,
        0
    );
}

/// `freelike_block` is the partner to [`malloclike_block`]. For Memcheck, it does two things:
///
/// The following description is taken almost untouched from the docs in the `valgrind.h` header
/// file.
///
/// - It records that the block has been deallocated. This assumes that the block was annotated as
///   having been allocated via [`malloclike_block`]. Otherwise, an error will be issued.
/// - It marks the block as being unaddressable.
///
/// `freelike_block` should be put immediately after the point where a heap block is deallocated.
///
/// See also [`malloclike_block`] for more details
#[inline(always)]
pub fn freelike_block(addr: *const (), redzone: usize) {
    do_client_request!(
        "valgrind::freelike_block",
        bindings::VR_ValgrindClientRequest::VR_FREELIKE_BLOCK,
        addr as usize,
        redzone,
        0,
        0,
        0
    );
}

/// Create a memory pool
///
/// This request registers the address `pool` as the anchor address for a memory pool. It also
/// provides a size `redzone`, specifying how large the redzones placed around chunks allocated from
/// the pool should be. Finally, it provides an `is_zeroed` argument that specifies whether the
/// pool's chunks are zeroed (more precisely: defined) when allocated. Upon completion of this
/// request, no chunks are associated with the pool. The request simply tells Memcheck that the pool
/// exists, so that subsequent calls can refer to it as a pool.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn create_mempool(pool: *const (), redzone: usize, is_zeroed: bool) {
    do_client_request!(
        "valgrind::create_mempool",
        bindings::VR_ValgrindClientRequest::VR_CREATE_MEMPOOL,
        pool as usize,
        redzone,
        usize::from(is_zeroed),
        0,
        0
    );
}

/// Create a memory pool like [`create_mempool`] with some [`MempoolFlags`] specifying extended
/// behavior.
///
/// See also [`create_mempool`], [`MempoolFlags`] and [Memory Pools: describing and working with
/// custom allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn create_mempool_ext(pool: *const (), redzone: usize, is_zeroed: bool, flags: u8) {
    do_client_request!(
        "valgrind::create_mempool_ext",
        bindings::VR_ValgrindClientRequest::VR_CREATE_MEMPOOL,
        pool as usize,
        redzone,
        usize::from(is_zeroed),
        flags as usize,
        0
    );
}

/// Destroy a memory pool
///
/// This request tells Memcheck that a pool is being torn down. Memcheck then removes all records of
/// chunks associated with the pool, as well as its record of the pool's existence. While destroying
/// its records of a mempool, Memcheck resets the redzones of any live chunks in the pool to
/// `NOACCESS`.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn destroy_mempool(pool: *const ()) {
    do_client_request!(
        "valgrind::destroy_mempool",
        bindings::VR_ValgrindClientRequest::VR_DESTROY_MEMPOOL,
        pool as usize,
        0,
        0,
        0,
        0
    );
}

/// Associate a piece of memory with a memory `pool`
///
/// This request informs Memcheck that a size-byte chunk has been allocated at `addr`, and
/// associates the chunk with the specified `pool`. If the `pool` was created with nonzero redzones,
/// Memcheck will mark the bytes before and after the chunk as `NOACCESS`. If the pool was created
/// with the `is_zeroed` argument set, Memcheck will mark the chunk as `DEFINED`, otherwise Memcheck
/// will mark the chunk as `UNDEFINED`.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn mempool_alloc(pool: *const (), addr: *const (), size: usize) {
    do_client_request!(
        "valgrind::mempool_alloc",
        bindings::VR_ValgrindClientRequest::VR_MEMPOOL_ALLOC,
        pool as usize,
        addr as usize,
        size,
        0,
        0
    );
}

/// Disassociate a piece of memory from a memory `pool`
///
/// This request informs Memcheck that the chunk at `addr` should no longer be considered allocated.
/// Memcheck will mark the chunk associated with `addr` as `NOACCESS`, and delete its record of the
/// chunk's existence.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn mempool_free(pool: *const (), addr: *const ()) {
    do_client_request!(
        "valgrind::mempool_free",
        bindings::VR_ValgrindClientRequest::VR_MEMPOOL_FREE,
        pool as usize,
        addr as usize,
        0,
        0,
        0
    );
}

/// Disassociate any pieces outside a particular range
///
/// This request trims the chunks associated with pool. The request only operates on chunks
/// associated with pool. Trimming is formally defined as:
///
/// All chunks entirely inside the range `addr..(addr+size-1)` are preserved.
///
/// All chunks entirely outside the range `addr..(addr+size-1)` are discarded, as though
/// [`mempool_free`] was called on them.
///
/// All other chunks must intersect with the range `addr..(addr+size-1)`; areas outside the
/// intersection are marked as `NOACCESS`, as though they had been independently freed with
/// [`mempool_free`].
///
/// This is a somewhat rare request, but can be useful in implementing the type of mass-free
/// operations common in custom LIFO allocators.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn mempool_trim(pool: *const (), addr: *const (), size: usize) {
    do_client_request!(
        "valgrind::mempool_trim",
        bindings::VR_ValgrindClientRequest::VR_MEMPOOL_TRIM,
        pool as usize,
        addr as usize,
        size,
        0,
        0
    );
}

/// Resize and/or move a piece associated with a memory pool
///
/// This request informs Memcheck that the pool previously anchored at address `pool_a` has moved to
/// anchor address `pool_b`. This is a rare request, typically only needed if you realloc the header
/// of a mempool.
///
/// No memory-status bits are altered by this request.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn move_mempool(pool_a: *const (), pool_b: *const ()) {
    do_client_request!(
        "valgrind::move_mempool",
        bindings::VR_ValgrindClientRequest::VR_MOVE_MEMPOOL,
        pool_a as usize,
        pool_b as usize,
        0,
        0,
        0
    );
}

/// Resize and/or move a piece associated with a memory pool
///
/// This request informs Memcheck that the chunk previously allocated at address `addr_a` within
/// pool has been moved and/or resized, and should be changed to cover the region
/// `addr_b..(addr_b+size-1)`. This is a rare request, typically only needed if you realloc a
/// superblock or wish to extend a chunk without changing its memory-status bits.
///
/// No memory-status bits are altered by this request.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn mempool_change(pool: *const (), addr_a: *const (), addr_b: *const (), size: usize) {
    do_client_request!(
        "valgrind::mempool_change",
        bindings::VR_ValgrindClientRequest::VR_MEMPOOL_CHANGE,
        pool as usize,
        addr_a as usize,
        addr_b as usize,
        size,
        0
    );
}

/// Returns `true` if a mempool exists, else false.
///
/// This request informs the caller whether or not Memcheck is currently tracking a mempool at
/// anchor address pool. It evaluates to `true` when there is a mempool associated with that
/// address, `false` otherwise. This is a rare request, only useful in circumstances when client
/// code might have lost track of the set of active mempools.
///
/// See also [Memory Pools: describing and working with custom
/// allocators](https://valgrind.org/docs/manual/mc-manual.html#mc-manual.mempools)
#[inline(always)]
pub fn mempool_exists(pool: *const ()) -> bool {
    do_client_request!(
        "valgrind::mempool_exists",
        0,
        bindings::VR_ValgrindClientRequest::VR_MEMPOOL_EXISTS,
        pool as usize,
        0,
        0,
        0,
        0
    ) != 0
}

/// Mark a piece of memory as being a stack. Returns a [`super::StackId`]
///
/// `start` is the lowest addressable stack byte, `end` is the highest addressable stack byte.
///
/// Registers a new stack. Informs Valgrind that the memory range between `start` and `end` is a
/// unique stack. Returns a stack identifier that can be used with the other [`stack_change`] and
/// [`stack_deregister`] client requests. Valgrind will use this information to determine if a
/// change to the stack pointer is an item pushed onto the stack or a change over to a new stack.
/// Use this if you're using a user-level thread package and are noticing crashes in stack trace
/// recording or spurious errors from Valgrind about uninitialized memory reads.
///
/// Warning: Unfortunately, this client request is unreliable and best avoided.
#[inline(always)]
pub fn stack_register(start: usize, end: usize) -> StackId {
    do_client_request!(
        "valgrind::stack_register",
        0,
        bindings::VR_ValgrindClientRequest::VR_STACK_REGISTER,
        start,
        end,
        0,
        0,
        0
    )
}

/// Unmark the piece of memory associated with a [`StackId`] as being a stack
///
/// Deregisters a previously registered stack. Informs Valgrind that previously registered memory
/// range with [`StackId`] id is no longer a stack.
///
/// Warning: Unfortunately, this client request is unreliable and best avoided.
#[inline(always)]
pub fn stack_deregister(stack_id: StackId) {
    do_client_request!(
        "valgrind::stack_deregister",
        bindings::VR_ValgrindClientRequest::VR_STACK_DEREGISTER,
        stack_id,
        0,
        0,
        0,
        0
    );
}

/// Change the `start` and `end` address of the [`StackId`]
///
/// `start` is the new lowest addressable stack byte, `end` is the new highest addressable stack
/// byte.
///
/// Changes a previously registered stack. Informs Valgrind that the previously registered stack
/// with [`StackId`] has changed its `start` and `end` values. Use this if your user-level thread
/// package implements stack growth.
///
/// Warning: Unfortunately, this client request is unreliable and best avoided.
#[inline(always)]
pub fn stack_change(stack_id: StackId, start: usize, end: usize) {
    do_client_request!(
        "valgrind::stack_change",
        bindings::VR_ValgrindClientRequest::VR_STACK_CHANGE,
        stack_id,
        start,
        end,
        0,
        0
    );
}

/// Loads PDB debug info for `Wine PE image_map`.
///
/// # Panics
///
/// When the raw file descriptor `fd` is smaller than 0
#[inline(always)]
pub fn load_pdb_debuginfo(fd: RawFd, ptr: *const (), total_size: usize, delta: usize) {
    do_client_request!(
        "valgrind::load_pdb_debuginfo",
        bindings::VR_ValgrindClientRequest::VR_LOAD_PDB_DEBUGINFO,
        fd.try_into().expect("A file descriptor should be >= 0"),
        ptr as usize,
        total_size,
        delta,
        0
    );
}

/// Map a code address to a source file name and line number
///
/// `buf64` must point to a 64-byte buffer in the caller's address space. The result will be dumped
/// in there and is guaranteed to be zero terminated. If no info is found, the first byte is set to
/// zero.
#[inline(always)]
pub fn map_ip_to_srcloc(addr: *const (), buf64: *const ()) -> usize {
    do_client_request!(
        "valgrind::map_ip_to_srcloc",
        0,
        bindings::VR_ValgrindClientRequest::VR_MAP_IP_TO_SRCLOC,
        addr as usize,
        buf64 as usize,
        0,
        0,
        0
    )
}

/// Disable error reporting for this thread.
///
/// Behaves in a stack like way, so you can safely call this multiple times provided that
/// [`enable_error_reporting`] is called the same number of times to re-enable reporting. The
/// first call of this macro disables reporting. Subsequent calls have no effect except to increase
/// the number of [`enable_error_reporting`] calls needed to re-enable reporting. Child
/// threads do not inherit this setting from their parents -- they are always created with reporting
/// enabled.
#[inline(always)]
pub fn disable_error_reporting() {
    do_client_request!(
        "valgrind::disable_error_reporting",
        bindings::VR_ValgrindClientRequest::VR_CHANGE_ERR_DISABLEMENT,
        1,
        0,
        0,
        0,
        0
    );
}

/// Re-enable error reporting
///
/// See also [`disable_error_reporting`]
#[inline(always)]
pub fn enable_error_reporting() {
    do_client_request!(
        "valgrind::enable_error_reporting",
        bindings::VR_ValgrindClientRequest::VR_CHANGE_ERR_DISABLEMENT,
        usize::MAX, // The original code in `valgrind.h` used `-1` as value
        0,
        0,
        0,
        0
    );
}

/// Execute a monitor command from the client program
///
/// If a connection is opened with GDB, the output will be sent according to the output mode set for
/// vgdb. If no connection is opened, output will go to the log output. Returns `false` if command
/// not recognized, `true` otherwise. Note the return value deviates from the original in
/// `valgrind.h` which returns 1 if the command was not recognized and 0 otherwise.
///
/// See also [Valgrind monitor commands][valgrind-monitor]
///
/// [valgrind-monitor]:
/// https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.valgrind-monitor-commands
#[inline(always)]
pub fn monitor_command<T>(command: T) -> bool
where
    T: AsRef<CStr>,
{
    do_client_request!(
        "valgrind::monitor_command",
        0,
        bindings::VR_ValgrindClientRequest::VR_GDB_MONITOR_COMMAND,
        command.as_ref().as_ptr() as usize,
        0,
        0,
        0,
        0
    ) != 1
}

/// Change the value of a dynamic command line option
///
/// The value of some command line options can be changed dynamically while your program is running
/// under Valgrind. The dynamically changeable options of the Valgrind core and a given tool can be
/// listed using option --help-dyn-options,
///
/// Note that unknown or not dynamically changeable options will cause a warning message to be
/// output.
///
/// See also [Dynamically changing
/// options](https://valgrind.org/docs/manual/manual-core.html#manual-core.dynopts)
#[inline(always)]
pub fn clo_change<T>(option: T)
where
    T: AsRef<CStr>,
{
    do_client_request!(
        "valgrind::clo_change",
        bindings::VR_ValgrindClientRequest::VR_CLO_CHANGE,
        option.as_ref().as_ptr() as usize,
        0,
        0,
        0,
        0
    );
}
