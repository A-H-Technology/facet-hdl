//! Runtime for `#[facet_hdl::rpc]`.
//!
//! A call is a pair of queues, `<fn>_req` and `<fn>_resp`, pointing whichever
//! way the call goes. For an `#[export]` the fabric pushes arguments and pops
//! a [`CallResult`]; for an `#[import]` the host pushes arguments and pops
//! the fabric's answer as-is (the fabric can't panic), claimed by request
//! number so calls can overlap (see [`Answers`]). A `oneway` fn has no
//! `_resp`.

pub use crate::port::Answers;
use crate::port::{FromFabricQueue, Port, PortError, PushError, ToFabricQueue};
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
/// There's no interrupt for "room again" (see the README), so this polls. It
/// blocks rather than awaits because it runs inside the sync `poll`; a fabric
/// that doesn't drain its answers stalls the host either way.
pub fn respond<T: Facet<'static>, const N: usize>(q: &ToFabricQueue<T, N>, v: &T) -> Result<(), PortError> {
    loop {
        match q.push(v) {
            Ok(()) => return Ok(()),
            Err(PushError::Full) => std::thread::sleep(Duration::from_micros(50)),
            Err(PushError::Port(e)) => return Err(e),
        }
    }
}

/// The generated `serve` loop's idle wait. Started *before* polling the
/// request queues, so a call that lands in between still wakes it.
pub struct Idle<'a> {
    port: &'a Port,
    seen: u64,
}

impl<'a> Idle<'a> {
    /// `q` is any request queue of the boundary: they share one line.
    pub fn start<T: Facet<'static>, const N: usize>(q: &'a FromFabricQueue<T, N>) -> Self {
        let (port, seen) = q.idle();
        Self { port, seen }
    }

    /// Resolves once any request queue might hold a call.
    pub async fn wait(self) -> Result<(), PortError> {
        self.port.changed(self.seen, false).await
    }
}

/// An `#[import]` round trip: push the arguments, await the answer.
///
/// Calls can overlap, from several tasks or several threads; each gets the
/// answer to its own request. Dropping the future abandons the call (the
/// request may already be with the fabric, but its answer is thrown away when
/// it comes), so a timeout is just your runtime's timeout around this.
/// Answers owed to a previous process are dropped the same way.
pub async fn invoke<A: Facet<'static>, R: Facet<'static>, const N: usize, const M: usize>(
    req: &ToFabricQueue<A, N>,
    args: &A,
    resp: &Answers<R, M>,
) -> Result<R, PortError> {
    // Numbered only once there's room, right before the write, so a call
    // dropped while waiting for room never took a ticket.
    let mut issued = None;
    let ticket = match req
        .send_numbered(args, |k| {
            resp.expect(k);
            issued = Some(k);
        })
        .await
    {
        Ok(t) => t,
        Err(e) => {
            if let Some(k) = issued {
                resp.forget(k);
            }
            return Err(e);
        }
    };
    let claim = Claim { resp, ticket };
    loop {
        let seen = resp.rung();
        if let Some(r) = resp.take(ticket)? {
            std::mem::forget(claim);
            return Ok(r);
        }
        resp.changed(seen).await?;
    }
}

/// Forgets the ticket if the call is dropped (or fails) before its answer
/// is taken.
struct Claim<'a, R: Facet<'static>, const M: usize> {
    resp: &'a Answers<R, M>,
    ticket: u32,
}

impl<R: Facet<'static>, const M: usize> Drop for Claim<'_, R, M> {
    fn drop(&mut self) {
        self.resp.forget(self.ticket);
    }
}
