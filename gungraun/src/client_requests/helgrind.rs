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
// This file is distributed under the same License as the rest of `gungraun`.
//
// ----------------------------------------------------------------
//
//! All public client requests from the `helgrind.h` header file
//!
//! See also [Helgrind Client
//! Requests](https://valgrind.org/docs/manual/hg-manual.html#hg-manual.client-requests)
use super::arch::valgrind_do_client_request_stmt;
use super::{bindings, fatal_error};

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
        bindings::GR_HelgrindClientRequest::GR_HG_CLEAN_MEMORY,
        start as usize,
        len,
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
        bindings::GR_HelgrindClientRequest::GR_HG_USERSO_SEND_PRE,
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
        bindings::GR_HelgrindClientRequest::GR_HG_USERSO_RECV_POST,
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
        bindings::GR_HelgrindClientRequest::GR_HG_USERSO_FORGET_ALL,
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
        bindings::GR_HelgrindClientRequest::GR_HG_CLEAN_MEMORY,
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
        bindings::GR_HelgrindClientRequest::GR_HG_PTHREAD_RWLOCK_INIT_POST,
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
        bindings::GR_HelgrindClientRequest::GR_HG_PTHREAD_RWLOCK_DESTROY_PRE,
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
        bindings::GR_HelgrindClientRequest::GR_HG_PTHREAD_RWLOCK_ACQUIRED,
        lock as usize,
        usize::from(is_writer_lock),
        0,
        0,
        0
    );
}

/// Report that the lock at address `lock` is about to be released
///
/// If `is_writer_lock` is true then it is a writer lock else it is a reader lock.
///
/// See also [`annotate_rwlock_create`]
#[inline(always)]
pub fn annotate_rwlock_released(lock: *const (), is_writer_lock: bool) {
    do_client_request!(
        "helgrind::annotate_rwlock_released",
        bindings::GR_HelgrindClientRequest::GR_HG_PTHREAD_RWLOCK_RELEASED,
        lock as usize,
        usize::from(is_writer_lock),
        0,
        0,
        0
    );
}
