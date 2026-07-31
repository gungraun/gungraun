//! Tests Callgrind client requests for instrumentation control.
//!
//! Demonstrates starting and stopping Callgrind instrumentation around a code region.

fn main() {
    gungraun::client_requests::callgrind::start_instrumentation();
    println!("Hello World.");
    gungraun::client_requests::callgrind::stop_instrumentation();
}
