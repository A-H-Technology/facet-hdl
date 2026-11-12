//! Driving the async API from plain threads, without pulling in a runtime.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Runs `fut` to completion on this thread.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    run(fut, None).expect("no deadline")
}

/// Runs `fut` until it completes or `timeout` passes. On a timeout `fut` is
/// dropped, which for a call means abandoning it: its answer, if one comes,
/// is thrown away rather than handed to a later call.
pub fn block_on_timeout<F: Future>(fut: F, timeout: Duration) -> Option<F::Output> {
    run(fut, Some(Instant::now() + timeout))
}

fn run<F: Future>(fut: F, deadline: Option<Instant>) -> Option<F::Output> {
    let mut fut = pin!(fut);
    let waker = Arc::new(Unpark(thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return Some(v);
        }
        // Spurious unparks are fine: the future is just polled again.
        match deadline {
            None => thread::park(),
            Some(d) => {
                let left = d.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return None;
                }
                thread::park_timeout(left);
            }
        }
    }
}
