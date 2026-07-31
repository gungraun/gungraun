// Copyright (C) 2023-2023 Nicholas Nethercote.  All rights reserved.
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
// We're using a lot of the original documentation from the `cachegrind.h` header file with some
// small adjustments, so above is the original license from `cachegrind.h` file.
//
// This file is distributed under the same License as the rest of `valgrind-requests`.
//
// ----------------------------------------------------------------
//
//! All client requests from the `cachegrind.h` header file
//!
//! See also [Cachegrind specific client
//! requests](https://valgrind.org/docs/manual/cg-manual.html#cg-manual.clientrequests)

use super::{bindings, fatal_error, valgrind_do_client_request_stmt};

/// Start Cachegrind instrumentation if not already enabled.
///
/// Use this in combination with [`stop_instrumentation`] and `--instr-at-start` to measure only
/// part of a client program's execution.
#[inline(always)]
pub fn start_instrumentation() {
    do_client_request!(
        "cachegrind::start_instrumentation",
        bindings::VR_CachegrindClientRequest::VR_CG_START_INSTRUMENTATION,
        0,
        0,
        0,
        0,
        0
    );
}

/// Stop Cachegrind instrumentation if not already disabled.
///
/// Use this in combination with [`start_instrumentation`] and `--instr-at-start` to measure only
/// part of a client program's execution.
#[inline(always)]
pub fn stop_instrumentation() {
    do_client_request!(
        "cachegrind::stop_instrumentation",
        bindings::VR_CachegrindClientRequest::VR_CG_STOP_INSTRUMENTATION,
        0,
        0,
        0,
        0,
        0
    );
}
