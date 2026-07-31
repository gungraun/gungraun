// Copyright (C) 2007-2017 OpenWorks LLP
//    info@open-works.co.uk
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
// We're using a lot of the original documentation from the `helgrind.h` header file with some
// small adjustments, so above is the original license from `helgrind.h` file.
//
// This file is distributed under the same License as the rest of `valgrind-requests`.
//
// ----------------------------------------------------------------
//
//! All public client requests from the `helgrind.h` header file
//!
//! The client requests which are for internal use only (e.g. condition variable, spin lock) are not
//! available.
//!
//! See also [Helgrind Client
//! Requests](https://valgrind.org/docs/manual/hg-manual.html#hg-manual.client-requests)
use super::{
    bindings, fatal_error, valgrind_do_client_request_expr, valgrind_do_client_request_stmt,
};

/// Notify immediately after mutex creation
///
/// `mb_rec` indicates whether the mutex is recursive or not.
#[inline(always)]
pub fn mutex_init_post(mutex: *const (), mb_rec: bool) {
    do_client_request!(
        "helgrind::mutex_init_post",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_MUTEX_INIT_POST,
        mutex as usize,
        usize::from(mb_rec),
        0,
        0,
        0
    );
}

/// Notify immediately before mutex acquisition
///
/// `is_try_lock` indicates whether this is a try-lock operation (true) or a normal lock (false).
#[inline(always)]
pub fn mutex_lock_pre(mutex: *const (), is_try_lock: bool) {
    do_client_request!(
        "helgrind::mutex_lock_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_MUTEX_LOCK_PRE,
        mutex as usize,
        usize::from(is_try_lock),
        0,
        0,
        0
    );
}

/// Notify here immediately after a successful mutex acquisition
#[inline(always)]
pub fn mutex_lock_post(mutex: *const ()) {
    do_client_request!(
        "helgrind::mutex_lock_post",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_MUTEX_LOCK_POST,
        mutex as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately before mutex release
#[inline(always)]
pub fn mutex_unlock_pre(mutex: *const ()) {
    do_client_request!(
        "helgrind::mutex_unlock_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_MUTEX_UNLOCK_PRE,
        mutex as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately after mutex release
#[inline(always)]
pub fn mutex_unlock_post(mutex: *const ()) {
    do_client_request!(
        "helgrind::mutex_unlock_post",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_MUTEX_UNLOCK_POST,
        mutex as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately before mutex destruction
#[inline(always)]
pub fn mutex_destroy_pre(mutex: *const ()) {
    do_client_request!(
        "helgrind::mutex_destroy_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_MUTEX_DESTROY_PRE,
        mutex as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately after semaphore creation
///
/// `value` is the initial value of the semaphore.
#[inline(always)]
pub fn sem_init_post(sem: *const (), value: usize) {
    do_client_request!(
        "helgrind::sem_init_post",
        bindings::VR_HelgrindClientRequest::VR_HG_POSIX_SEM_INIT_POST,
        sem as usize,
        value,
        0,
        0,
        0
    );
}

/// Notify here immediately after a semaphore wait (an acquire-style operation)
#[inline(always)]
pub fn sem_wait_post(sem: *const ()) {
    do_client_request!(
        "helgrind::sem_wait_post",
        bindings::VR_HelgrindClientRequest::VR_HG_POSIX_SEM_ACQUIRED,
        sem as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately before semaphore post (a release-style operation)
#[inline(always)]
pub fn sem_post_pre(sem: *const ()) {
    do_client_request!(
        "helgrind::sem_post_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_POSIX_SEM_RELEASED,
        sem as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately before semaphore destruction
#[inline(always)]
pub fn sem_destroy_pre(sem: *const ()) {
    do_client_request!(
        "helgrind::sem_destroy_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_POSIX_SEM_DESTROY_PRE,
        sem as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately before barrier creation
///
/// `count` is the barrier capacity. `resizable` indicates whether the barrier may be resized
/// or not.
#[inline(always)]
pub fn barrier_init_pre(bar: *const (), count: usize, resizable: bool) {
    do_client_request!(
        "helgrind::barrier_init_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_BARRIER_INIT_PRE,
        bar as usize,
        count,
        usize::from(resizable),
        0,
        0
    );
}

/// Notify here immediately before arrival at a barrier
#[inline(always)]
pub fn barrier_wait_pre(bar: *const ()) {
    do_client_request!(
        "helgrind::barrier_wait_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_BARRIER_WAIT_PRE,
        bar as usize,
        0,
        0,
        0,
        0
    );
}

/// Notify here immediately before a barrier resize (change of barrier capacity)
///
/// If `new_count` >= the existing capacity, there is no change in the state of any threads
/// waiting at the barrier. If `new_count` < the existing capacity and >= `new_count` threads are
/// currently waiting, this is considered to also have the effect of telling the checker that all
/// waiting threads have now moved past the barrier.
#[inline(always)]
pub fn barrier_resize_pre(bar: *const (), new_count: usize) {
    do_client_request!(
        "helgrind::barrier_resize_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_BARRIER_RESIZE_PRE,
        bar as usize,
        new_count,
        0,
        0,
        0
    );
}

/// Notify here immediately before barrier destruction
#[inline(always)]
pub fn barrier_destroy_pre(bar: *const ()) {
    do_client_request!(
        "helgrind::barrier_destroy_pre",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_BARRIER_DESTROY_PRE,
        bar as usize,
        0,
        0,
        0,
        0
    );
}

/// Clean memory state
///
/// This makes Helgrind forget everything it knew about the specified memory range. Effectively this
/// announces that the specified memory range now "belongs" to the calling thread, so that: (1) the
/// calling thread can access it safely without synchronisation, and (2) all other threads must sync
/// with this one to access it safely.
///
/// This is particularly useful for memory allocators that wish to recycle memory.
#[inline(always)]
pub fn clean_memory(start: *const (), len: usize) {
    do_client_request!(
        "helgrind::clean_memory",
        bindings::VR_HelgrindClientRequest::VR_HG_CLEAN_MEMORY,
        start as usize,
        len,
        0,
        0,
        0
    );
}

/// Clean memory state for a heap block starting at `blockstart`
///
/// The same as [`clean_memory`] but for a heap block starting at `blockstart`. This allows
/// painting when we only know the address of an object, but not its size, which is sometimes the
/// case in C++ code involving inheritance, and in which RTTI is not, for whatever reason,
/// available.
///
/// Returns the number of bytes painted, which can be zero for a zero-sized block. Hence, return
/// values >= 0 indicate success (the block was found), and the value -1 indicates block not found,
/// and -2 is returned when not running on Helgrind.
#[inline(always)]
#[expect(clippy::cast_possible_wrap)]
pub fn clean_memory_heapblock(blockstart: *const ()) -> isize {
    do_client_request!(
        "helgrind::clean_memory_heapblock",
        usize::MAX - 1,
        bindings::VR_HelgrindClientRequest::VR_HG_CLEAN_MEMORY_HEAPBLOCK,
        blockstart as usize,
        0,
        0,
        0,
        0
    ) as isize
}

/// Tell Helgrind that an address range is not to be "tracked" until further notice.
///
/// This puts it in the NOACCESS state, in which case we ignore all reads and writes to it. Useful
/// for ignoring ranges of memory where there might be races we don't want to see. If the memory is
/// subsequently reallocated via malloc/new/stack allocation, then it is put back in the trackable
/// state. Hence it is safe in the situation where checking is disabled, the containing area is
/// deallocated and later reallocated for some other purpose.
#[inline(always)]
pub fn disable_checking(start: *const (), len: usize) {
    do_client_request!(
        "helgrind::disable_checking",
        bindings::VR_HelgrindClientRequest::VR_HG_ARANGE_MAKE_UNTRACKED,
        start as usize,
        len,
        0,
        0,
        0
    );
}

/// Re-enable race checking for an address range.
///
/// That is, make it once again subject to the normal race-checking machinery. This puts it in the
/// same state as new memory allocated by this thread -- that is, basically owned exclusively by
/// this thread. See also [`disable_checking`]
#[inline(always)]
pub fn enable_checking(start: *const (), len: usize) {
    do_client_request!(
        "helgrind::enable_checking",
        bindings::VR_HelgrindClientRequest::VR_HG_ARANGE_MAKE_TRACKED,
        start as usize,
        len,
        0,
        0,
        0
    );
}

/// Checks the accessibility bits for addresses [addr..addr+len-1].
///
/// If `abits` array is provided, copy the accessibility bits in `abits`.
///
/// Return values:
///   -2   if not running on helgrind
///   -1   if any part of `abits` is not addressable
///   >= 0 : success.
///
/// When success, it returns the number of addressable bytes found. So, to check that a whole range
/// is addressable, check
///
/// ```ignore
/// get_abits(addr, core::ptr::null_mut(), len) == len
/// ```
///
/// In addition, if you want to examine the addressability of each byte of the range, you need to
/// provide a non null ptr as second argument, pointing to an array of unsigned char of length len.
/// Addressable bytes are indicated with 0xff. Non-addressable bytes are indicated with 0x00.
#[inline(always)]
#[expect(clippy::cast_possible_wrap)]
pub fn get_abits(addr: *const (), abits: *mut u8, len: usize) -> isize {
    do_client_request!(
        "helgrind::get_abits",
        usize::MAX - 1,
        bindings::VR_HelgrindClientRequest::VR_HG_GET_ABITS,
        addr as usize,
        abits as usize,
        len,
        0,
        0
    ) as isize
}

/// End-user request for Ada applications compiled with GNAT.
///
/// Helgrind understands the Ada concept of Ada task dependencies and terminations. See Ada
/// Reference Manual section 9.3 "Task Dependence - Termination of Tasks".
///
/// However, in some cases, the master of (terminated) tasks completes only when the application
/// exits. An example of this is dynamically allocated tasks with an access type defined at Library
/// Level. By default, the state of such tasks in Helgrind will be 'exited but join not done yet'.
/// Many tasks in such a state are however causing Helgrind CPU and memory to increase
/// significantly. `gnat_dependent_master_join` can be used to indicate to Helgrind that a not yet
/// completed master has however already 'seen' the termination of a dependent : this is
/// conceptually the same as a `pthread_join` and causes the cleanup of the dependent as done by
/// Helgrind when a master completes. This allows to avoid the overhead in helgrind caused by such
/// tasks. A typical usage for a master to indicate it has done conceptually a join with a dependent
/// task before the master completes is:
///
/// ```text
///    while not Dep_Task'Terminated loop
///       ... do whatever to wait for Dep_Task termination.
///    end loop;
///    gnat_dependent_master_join
///      (Dep_Task'Identity,
///       Ada.Task_Identification.Current_Task);
/// ```
#[inline(always)]
pub fn gnat_dependent_master_join(dep: *const (), master: *const ()) {
    do_client_request!(
        "helgrind::gnat_dependent_master_join",
        bindings::VR_HelgrindClientRequest::VR_HG_GNAT_DEPENDENT_MASTER_JOIN,
        dep as usize,
        master as usize,
        0,
        0,
        0
    );
}

/// Create completely arbitrary happens-before edges between threads
///
/// If threads T1 ... Tn all do `annotate_happens_before` and later (w.r.t. some notional global
/// clock for the computation) thread Tm does [`annotate_happens_after`], then Helgrind will regard
/// all memory accesses done by T1 ... Tn before the ...BEFORE... call as happening-before all
/// memory accesses done by Tm after the ...AFTER... call. Hence, Helgrind won't complain about
/// races if Tm's accesses afterward are to the same locations as accesses before by any of T1 ...
/// Tn.
///
/// `obj` is a machine word and completely arbitrary, and denotes the identity of some
/// synchronisation object you're modelling.
///
/// You must do the _BEFORE call just before the real sync event on the signaller's side, and _AFTER
/// just after the real sync event on the waiter's side.
///
/// If none of the rest of these macros make sense to you, at least take the time to understand
/// these two.  They form the very essence of describing arbitrary inter-thread synchronisation
/// events to Helgrind.  You can get a long way just with them alone.
#[inline(always)]
pub fn annotate_happens_before(obj: *const ()) {
    do_client_request!(
        "helgrind::annotate_happens_before",
        bindings::VR_HelgrindClientRequest::VR_HG_USERSO_SEND_PRE,
        obj as usize,
        0,
        0,
        0,
        0
    );
}

/// See [`annotate_happens_before`]
#[inline(always)]
pub fn annotate_happens_after(obj: *const ()) {
    do_client_request!(
        "helgrind::annotate_happens_after",
        bindings::VR_HelgrindClientRequest::VR_HG_USERSO_RECV_POST,
        obj as usize,
        0,
        0,
        0,
        0
    );
}

/// This is interim until such time as bug 243935 is fully resolved
///
/// It instructs Helgrind to forget about any [`annotate_happens_before`] calls on the specified
/// object, in effect putting it back in its original state. Once in that state, a use of
/// [`annotate_happens_after`] on it has no effect on the calling thread.
///
/// An implementation may optionally release resources it has associated with 'obj' when
/// `annotate_happens_before_forget_all` happens. Users are recommended to use
/// `annotate_happens_before_forget_all` to indicate when a synchronisation object is no longer
/// needed, to avoid potential indefinite resource leaks.
#[inline(always)]
pub fn annotate_happens_before_forget_all(obj: *const ()) {
    do_client_request!(
        "helgrind::annotate_happens_before_forget_all",
        bindings::VR_HelgrindClientRequest::VR_HG_USERSO_FORGET_ALL,
        obj as usize,
        0,
        0,
        0,
        0
    );
}

/// Report that a new memory at `addr` of size `size` has been allocated.
///
/// This might be used when the memory has been retrieved from a free list and is about to be
/// reused, or when a locking discipline for a variable changes.
///
/// This is the same as [`clean_memory`].
#[inline(always)]
pub fn annotate_new_memory(addr: *const (), size: usize) {
    do_client_request!(
        "helgrind::annotate_new_memory",
        bindings::VR_HelgrindClientRequest::VR_HG_CLEAN_MEMORY,
        addr as usize,
        size,
        0,
        0,
        0
    );
}

/// Report that a lock has just been created at address `lock`
///
/// Annotation for describing behaviour of user-implemented lock primitives. In all cases, the
/// `lock` argument is a completely arbitrary machine word and can be any value which gives a unique
/// identity to the lock objects being modelled.
///
/// We just pretend they're ordinary posix rwlocks. That'll probably give some rather confusing
/// wording in error messages, claiming that the arbitrary `lock` values are `pthread_rwlock_t*`'s,
/// when in fact they are not. Ah, well.
#[inline(always)]
pub fn annotate_rwlock_create(lock: *const ()) {
    do_client_request!(
        "helgrind::annotate_rwlock_create",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_RWLOCK_INIT_POST,
        lock as usize,
        0,
        0,
        0,
        0
    );
}

/// Report that the lock at address `lock` is about to be destroyed
///
/// See also [`annotate_rwlock_create`]
#[inline(always)]
pub fn annotate_rwlock_destroy(lock: *const ()) {
    do_client_request!(
        "helgrind::annotate_rwlock_destroy",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_RWLOCK_DESTROY_PRE,
        lock as usize,
        0,
        0,
        0,
        0
    );
}

/// Report that the lock at address `lock` has just been acquired
///
/// If `is_writer_lock` is true then it is a writer lock else it is a reader lock.
///
/// See also [`annotate_rwlock_create`]
#[inline(always)]
pub fn annotate_rwlock_acquired(lock: *const (), is_writer_lock: bool) {
    do_client_request!(
        "helgrind::annotate_rwlock_acquired",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_RWLOCK_ACQUIRED,
        lock as usize,
        usize::from(is_writer_lock),
        0,
        0,
        0
    );
}

/// Report that the lock at address `lock` is about to be released
///
/// `is_writer_lock` is ignored.
///
/// See also [`annotate_rwlock_create`]
#[inline(always)]
pub fn annotate_rwlock_released(lock: *const (), _is_writer_lock: bool) {
    do_client_request!(
        "helgrind::annotate_rwlock_released",
        bindings::VR_HelgrindClientRequest::VR_HG_PTHREAD_RWLOCK_RELEASED,
        lock as usize,
        0,
        0,
        0,
        0
    );
}
