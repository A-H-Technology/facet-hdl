//! Runtime for `#[facet_hdl::rpc]`.
//!
//! A call is a pair of queues, `<fn>_req` and `<fn>_resp`, pointing whichever
//! way the call goes. For an `#[export]` the fabric pushes arguments and pops
//! a [`CallResult`]; for an `#[import]` the host pushes arguments and pops
//! the fabric's answer as-is (the fabric can't panic). Rings are FIFO, so
//! several calls can be in flight and answers come back in order. A
//! `oneway` fn has no `_resp`.

use crate::port::{FromFabricQueue, PortError, PushError, ToFabricQueue};
use facet::Facet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

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

/// Pushes, waiting for room until `deadline`.
fn send_by<A: Facet<'static>, const N: usize>(
    q: &mut ToFabricQueue<A, N>,
    v: &A,
    deadline: Instant,
    timeout: Duration,
) -> Result<(), PortError> {
    loop {
        match q.push(v) {
            Ok(()) => return Ok(()),
            Err(PushError::Full) if Instant::now() < deadline => std::thread::sleep(Duration::from_micros(50)),
            Err(PushError::Full) => return Err(PortError::Timeout(timeout)),
            Err(PushError::Port(e)) => return Err(e),
        }
    }
}

/// A oneway `#[import]`: hand the fabric its arguments, waiting up to
/// `timeout` for room.
pub fn send<A: Facet<'static>, const N: usize>(
    q: &mut ToFabricQueue<A, N>,
    args: &A,
    timeout: Duration,
) -> Result<(), PortError> {
    send_by(q, args, Instant::now() + timeout, timeout)
}

/// An `#[import]` round trip: push the arguments, wait for the answer.
///
/// A call that timed out may still be answered later, and that late answer
/// must not be taken as this call's. Every request gets exactly one answer, so
/// `sent - taken` on the two rings is how many are still owed; they're
/// drained first. That needs no state beyond the rings' own counts, so it
/// also holds across a host restart (`bind` starts from the fabric's counts).
pub fn invoke<A: Facet<'static>, R: Facet<'static>, const N: usize, const M: usize>(
    req: &mut ToFabricQueue<A, N>,
    args: &A,
    resp: &mut FromFabricQueue<R, M>,
    timeout: Duration,
) -> Result<R, PortError> {
    let deadline = Instant::now() + timeout;
    let take = |resp: &mut FromFabricQueue<R, M>| -> Result<R, PortError> {
        resp.pop_wait(deadline.saturating_duration_since(Instant::now()))?
            .ok_or(PortError::Timeout(timeout))
    };
    while resp.popped() != req.pushed() {
        take(resp)?;
    }
    send_by(req, args, deadline, timeout)?;
    take(resp)
}
