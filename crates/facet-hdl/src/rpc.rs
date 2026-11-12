//! Runtime for `#[facet_hdl::rpc]`: the fabric calls Rust functions.
//!
//! A call is a pair of queues: the fabric pushes the arguments onto
//! `<fn>_req` (a `FromFabricQueue`) and pops a [`CallResult`] from
//! `<fn>_resp` (a `ToFabricQueue`). Rings are FIFO, so several calls can be in
//! flight and the answers come back in order. A `oneway` fn has no `_resp`.

use crate::port::{FromFabricQueue, PortError, PushError, ToFabricQueue};
use facet::Facet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

/// What the fabric gets back from a call. It's always told, so a `()` fn
/// still answers: the fabric can sequence on "it finished" and see a panic.
#[derive(Facet, Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum CallResult<T> {
    Returned(T),
    /// The function panicked. The panic hook has already printed it.
    Panicked,
}

/// Runs `f`, turning a panic into [`CallResult::Panicked`] so one bad call
/// can't take the dispatcher down and leave the fabric waiting forever.
pub fn call<T>(f: impl FnOnce() -> T) -> CallResult<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => CallResult::Returned(v),
        Err(_) => CallResult::Panicked,
    }
}

/// Pushes a response, waiting while the fabric hasn't drained earlier ones.
/// There's no interrupt for "room again" (see the README), so this polls.
pub fn respond<T: Facet<'static>, const N: usize>(q: &mut ToFabricQueue<T, N>, v: &T) -> Result<(), PortError> {
    loop {
        match q.push(v) {
            Ok(()) => return Ok(()),
            Err(PushError::Full) => std::thread::sleep(Duration::from_micros(50)),
            Err(PushError::Port(e)) => return Err(e),
        }
    }
}

/// Sleeps until any request queue in the boundary might hold a call, or
/// `timeout`. The generated `serve` loop's idle wait.
pub fn wait<T: Facet<'static>, const N: usize>(q: &FromFabricQueue<T, N>, timeout: Duration) -> Result<(), PortError> {
    q.wait_irq(timeout)
}
