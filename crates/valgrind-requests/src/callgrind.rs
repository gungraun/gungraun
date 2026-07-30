// Copyright (C) 2003-2017 Josef Weidendorfer.  All rights reserved.
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
// THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS`` AND ANY EXPRESS
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
// We're using a lot of the original documentation from the `callgrind.h` header file with some
// small adjustments, so above is the original license from `callgrind.h` file.
//
// This file is distributed under the same License as the rest of `valgrind-requests`.
//
// ----------------------------------------------------------------
//
//! All client requests from the `callgrind.h` header file
//!
//! See also [Callgrind specific client
//! requests](https://valgrind.org/docs/manual/cl-manual.html#cl-manual.clientrequests)

use core::ffi::CStr;

use super::{bindings, fatal_error, valgrind_do_client_request_stmt};

/// Dump current state of cost centers, and zero them afterward
///
/// Force generation of a profile dump at specified position in code, for the current thread only.
/// Written counters will be reset to zero.
#[inline(always)]
pub fn dump_stats() {
    do_client_request!(
        "callgrind::dump_stats",
        bindings::VR_CallgrindClientRequest::VR_DUMP_STATS,
        0,
        0,
        0,
        0,
        0
    );
}

/// Dump current state of cost centers, and zero them afterward stating a reason
///
/// Same as [`dump_stats`], but allows you to specify a string to be able to distinguish profile
/// dumps.
///
/// The argument is appended to a string stating the reason which triggered the dump. This string is
/// written as a description field into the profile data dump.
#[inline(always)]
pub fn dump_stats_at<T>(c_str: T)
where
    T: AsRef<CStr>,
{
    do_client_request!(
        "callgrind::dump_stats_at",
        bindings::VR_CallgrindClientRequest::VR_DUMP_STATS_AT,
        c_str.as_ref().as_ptr() as usize,
        0,
        0,
        0,
        0
    );
}

/// Zero cost centers
///
/// Reset the profile counters for the current thread to zero.
#[inline(always)]
pub fn zero_stats() {
    do_client_request!(
        "callgrind::zero_stats",
        bindings::VR_CallgrindClientRequest::VR_ZERO_STATS,
        0,
        0,
        0,
        0,
        0
    );
}

/// Toggles collection state.
///
/// The collection state specifies whether the happening of events should be noted or if they are to
/// be ignored. Events are noted by increment of counters in a cost center
///
/// This allows you to ignore events with regard to profile counters. See also valgrind command line
/// options `--collect-atstart` and `--toggle-collect`.
#[inline(always)]
pub fn toggle_collect() {
    do_client_request!(
        "callgrind::toggle_collect",
        bindings::VR_CallgrindClientRequest::VR_TOGGLE_COLLECT,
        0,
        0,
        0,
        0,
        0
    );
}

/// Start full Callgrind instrumentation if not already switched on
///
/// When cache simulation is done, it will flush the simulated cache; this will lead to an
/// artificial cache warmup phase afterward with cache misses which would not have happened in
/// reality.
#[inline(always)]
pub fn start_instrumentation() {
    do_client_request!(
        "callgrind::start_instrumentation",
        bindings::VR_CallgrindClientRequest::VR_START_INSTRUMENTATION,
        0,
        0,
        0,
        0,
        0
    );
}

/// Stop full Callgrind instrumentation if not already switched off
///
/// This flushes Valgrind's translation cache, and does no additional instrumentation afterward,
/// which effectively will run at the same speed as the "none" tool (i.e. at minimal slowdown). Use
/// this to bypass Callgrind aggregation for uninteresting code parts. To start Callgrind in this
/// mode to ignore the setup phase, use the Valgrind command line option `--instr-atstart=no`.
#[inline(always)]
pub fn stop_instrumentation() {
    do_client_request!(
        "callgrind::stop_instrumentation",
        bindings::VR_CallgrindClientRequest::VR_STOP_INSTRUMENTATION,
        0,
        0,
        0,
        0,
        0
    );
}
