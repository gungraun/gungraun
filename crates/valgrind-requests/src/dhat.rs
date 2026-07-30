// Copyright (C) 2020 Nicholas Nethercote.  All rights reserved.
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
// We're using a lot of the original documentation from the `dhat.h` header file with some
// small adjustments, so above is the original license from `dhat.h` file.
//
// This file is distributed under the same License as the rest of `valgrind-requests`.
//
// ----------------------------------------------------------------
//
//! All client requests from the `dhat.h` header file
//!
//! See also the [DHAT documentation](https://valgrind.org/docs/manual/dh-manual.html#dh-manual)

use super::{bindings, fatal_error, valgrind_do_client_request_stmt};

/// Record an ad hoc event
///
/// If DHAT is invoked with `--mode=ad-hoc`, instead of profiling heap operations (allocations and
/// deallocations), it profiles calls to this `ad_hoc_event` client request.
///
/// The meaning of the `weight` argument will depend on what the event represents, which is up to
/// the user. If no meaningful `weight` argument exists, just use 1.
///
/// See also
/// [Ad hoc profiling](https://valgrind.org/docs/manual/dh-manual.html#dh-manual.ad-hoc-profiling)
#[inline(always)]
pub fn ad_hoc_event(weight: usize) {
    do_client_request!(
        "dhat::ad_hoc_event",
        bindings::VR_DHATClientRequest::VR_DHAT_AD_HOC_EVENT,
        weight,
        0,
        0,
        0,
        0
    );
}

/// For access to count histograms of memory larger than 1k
///
/// The size of the blocks that measure and display access counts is limited to 1024 bytes. This is
/// done to limit the performance overhead and also to keep the size of the generated output
/// reasonable. However, it is possible to override this limit using this client request. The
/// use-case for this is to first run DHAT normally, and then identify any large blocks that you
/// would like to further investigate with access count histograms. The function call should be
/// placed immediately after the call to the allocator, and use the pointer returned by the
/// allocator.
///
/// See also [Access Counts](https://valgrind.org/docs/manual/dh-manual.html#dh-access-counts)
#[inline(always)]
pub fn histogram_memory(addr: *const ()) {
    do_client_request!(
        "dhat::histogram_memory",
        bindings::VR_DHATClientRequest::VR_DHAT_HISTOGRAM_MEMORY,
        addr as usize,
        0,
        0,
        0,
        0
    );
}
