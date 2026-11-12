//! One `irq` line, any number of waiters.
//!
//! The boundary has a single level-sensitive line for every
//! `FromFabricQueue`, and an [`Interrupt`] can only be waited on by one
//! thread at a time. So one thread per binding does the waiting and every
//! waiter, sync or async, just waits for that thread to ring. Waiters don't
//! learn *which* queue has work; they re-check their own after every ring.
//!
//! No executor is involved: the thread wakes [`Waker`]s directly, so the
//! futures built on this run under tokio, smol or [`block_on`](crate::block_on)
//! alike.

use crate::port::Interrupt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::Duration;

/// How often a waiter that the irq can't wake gets re-polled: no line at all,
/// or waiting for room in a `ToFabricQueue` (the fabric has no "room again"
/// interrupt).
const TICK: Duration = Duration::from_micros(50);

/// Upper bound on one irq wait, so a dropped binding's thread notices within
/// this long.
const IDLE: Duration = Duration::from_millis(250);

pub(crate) struct Doorbell {
    state: Mutex<State>,
    /// Signalled when the first waiter arrives, so the thread only touches the
    /// line while someone cares. A level line that stays high with nobody
    /// draining it would otherwise make the thread spin forever.
    arrived: Condvar,
}

#[derive(Default)]
struct State {
    /// Bumped on every ring. A waiter remembers the count from before it last
    /// checked its queue; any later count means "look again".
    rung: u64,
    /// Keyed by waiter, not deduplicated by `Waker::will_wake`: futures
    /// joined in one task share a waker, and one of them dropping must not
    /// unregister the others.
    wakers: Vec<(u64, Waker)>,
    next_id: u64,
    /// Some current waiter needs re-polling every [`TICK`].
    ticking: bool,
    closed: bool,
    /// The line failed. `io::Error` isn't `Clone`, so each waiter gets a copy
    /// made from these.
    failed: Option<(io::ErrorKind, String)>,
}

impl Doorbell {
    pub(crate) fn start(irq: Option<Box<dyn Interrupt>>) -> Arc<Self> {
        let bell = Arc::new(Self {
            state: Mutex::default(),
            arrived: Condvar::new(),
        });
        let theirs = bell.clone();
        thread::Builder::new()
            .name("facet-hdl-irq".into())
            .spawn(move || theirs.run(irq))
            .expect("spawning the irq thread");
        bell
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn run(&self, mut irq: Option<Box<dyn Interrupt>>) {
        loop {
            let ticking = {
                let mut s = self.lock();
                while s.wakers.is_empty() && !s.closed {
                    s = self.arrived.wait(s).unwrap_or_else(|e| e.into_inner());
                }
                if s.closed {
                    return;
                }
                s.ticking
            };
            let waited = match &mut irq {
                Some(irq) => irq.wait(if ticking { TICK } else { IDLE }).map(drop),
                None => {
                    thread::sleep(TICK);
                    Ok(())
                }
            };
            let mut s = self.lock();
            if let Err(e) = waited {
                s.failed = Some((e.kind(), e.to_string()));
                s.closed = true;
            }
            // Wake everyone, even on a timeout: the callers re-check either
            // way and an unneeded poll is cheap next to a missed one.
            s.rung += 1;
            s.ticking = false;
            for (_, w) in s.wakers.drain(..) {
                w.wake();
            }
        }
    }

    /// Take this *before* checking a queue, then wait with [`Self::rang`]: an
    /// entry that lands in between is either seen by the check or rings after
    /// this count, so it can't be missed.
    pub(crate) fn rung(&self) -> u64 {
        self.lock().rung
    }

    /// Resolves once the bell rings after `seen`. `tick` asks for a ring
    /// within [`TICK`] even without the irq.
    pub(crate) fn rang(&self, seen: u64, tick: bool) -> Rang<'_> {
        Rang {
            bell: self,
            seen,
            tick,
            id: None,
        }
    }

    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.arrived.notify_one();
    }
}

pub(crate) struct Rang<'a> {
    bell: &'a Doorbell,
    seen: u64,
    tick: bool,
    id: Option<u64>,
}

impl Future for Rang<'_> {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Every field is Unpin.
        let this = self.get_mut();
        let bell = this.bell;
        let mut s = bell.lock();
        if let Some((kind, msg)) = &s.failed {
            return Poll::Ready(Err(io::Error::new(*kind, format!("waiting on the irq: {msg}"))));
        }
        if s.rung != this.seen {
            return Poll::Ready(Ok(()));
        }
        if s.closed {
            return Poll::Ready(Err(io::Error::other("the binding was dropped")));
        }
        let id = *this.id.get_or_insert_with(|| {
            s.next_id += 1;
            s.next_id
        });
        match s.wakers.iter_mut().find(|(i, _)| *i == id) {
            Some((_, w)) => w.clone_from(cx.waker()),
            None => s.wakers.push((id, cx.waker().clone())),
        }
        s.ticking |= this.tick;
        bell.arrived.notify_one();
        Poll::Pending
    }
}

/// A waiter that gives up stops counting as one, so with nobody left the
/// thread stops touching the line (and a simulation stops advancing).
impl Drop for Rang<'_> {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            self.bell.lock().wakers.retain(|(i, _)| *i != id);
        }
    }
}
