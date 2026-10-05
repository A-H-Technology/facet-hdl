//! The host side of a boundary: the declaration's own fields become handles.

use crate::boundary::{Boundary, FINGERPRINT_WORD, PortKind, queue};
use crate::codec::{self, DecodeError};
use crate::hw::{HwType, LayoutError};
use facet::{Facet, Partial};
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Word-addressed access to a boundary's register window.
pub trait Transport: Send {
    fn read(&mut self, word: u32) -> io::Result<u32>;
    fn write(&mut self, word: u32, value: u32) -> io::Result<()>;

    /// The file whose exclusive `flock` stands for owning this register
    /// window. Every transport reaching the same window must name the same
    /// file; `bind` holds the lock for as long as the binding lives.
    fn lock_path(&self) -> PathBuf;

    /// The boundary's `irq` line, if this transport can wait on it. `bind`
    /// takes it once; without one, waiting falls back to polling.
    fn interrupt(&mut self) -> Option<Box<dyn Interrupt>> {
        None
    }
}

/// The generated `irq` output: high while any `FromFabricQueue` holds an
/// entry.
///
/// Level, not edge, is the whole point. A consumer checks its queue, finds it
/// empty and goes to wait; if the entry lands in between, the line is already
/// high when the wait starts and the wait returns at once. An edge would have
/// come and gone, and the wakeup would be lost.
pub trait Interrupt: Send {
    /// Returns `true` as soon as the line is high (immediately if it already
    /// is), `false` once `timeout` passes with it low. Spurious `true`s are
    /// allowed; callers re-check their queue either way.
    fn wait(&mut self, timeout: Duration) -> io::Result<bool>;
}

/// Multi-word ports are why both locks exist: the fabric only commits a write
/// on the port's last word and snapshots a read on its first, so interleaved
/// words from two accessors commit or return values neither side meant.
///
/// The mutex orders accessors within this binding. The flock on `_owner`
/// keeps out every other binding, in this process or another, and the kernel
/// drops it when the holder dies (a lock register in the fabric would stay
/// set after a crash). One edge: a fork elsewhere in this process copies the
/// fd, so until that child execs (or forever, if it never does) the lock
/// outlives this binding.
///
/// The interrupt has its own lock so a blocked waiter never holds up register
/// traffic on other ports.
struct Bus {
    transport: Mutex<Box<dyn Transport>>,
    irq: Option<Mutex<Box<dyn Interrupt>>>,
    _owner: File,
}

impl Bus {
    fn lock(&self) -> std::sync::MutexGuard<'_, Box<dyn Transport>> {
        self.transport.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Sleeps until the irq line might mean there's work, or `timeout`.
    fn wait_irq(&self, timeout: Duration) -> io::Result<()> {
        match &self.irq {
            Some(irq) => irq.lock().unwrap_or_else(|e| e.into_inner()).wait(timeout).map(drop),
            // No line to sleep on: give other threads the CPU and let the
            // caller poll again.
            None => {
                std::thread::yield_now();
                Ok(())
            }
        }
    }
}

fn take_ownership(transport: &dyn Transport) -> Result<File, BindError> {
    let path = transport.lock_path();
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("opening lock file {}: {e}", path.display())))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(BindError::Busy { lock: path }),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}

#[doc(hidden)]
#[derive(Facet)]
pub struct Port {
    #[facet(opaque)]
    bus: Arc<Bus>,
    #[facet(opaque)]
    ty: Arc<HwType>,
    word: u32,
    name: &'static str,
}

impl Port {
    fn write_words(&self, at: u32, words: &[u32]) -> io::Result<()> {
        let mut bus = self.bus.lock();
        for (i, w) in words.iter().enumerate() {
            bus.write(at + i as u32, *w)?;
        }
        Ok(())
    }

    fn read_words(&self, at: u32) -> io::Result<Vec<u32>> {
        let mut bus = self.bus.lock();
        (0..codec::words_for(self.ty.width()))
            .map(|i| bus.read(at + i))
            .collect()
    }

    /// The fabric's `(head, tail)` for a queue port. A count gap wider than the
    /// ring means the fabric (or a raw poke) broke the protocol; it's
    /// reported, never used as an index.
    fn queue_counters(&self, depth: usize) -> Result<(u32, u32), PortError> {
        let (head, tail) = {
            let mut bus = self.bus.lock();
            let tail = bus.read(self.word + queue::TAIL)?;
            (bus.read(self.word + queue::HEAD)?, tail)
        };
        if head.wrapping_sub(tail) as usize > depth {
            return Err(self.desync(head, tail, "head - tail exceeds the depth"));
        }
        Ok((head, tail))
    }

    fn desync(&self, head: u32, tail: u32, why: &'static str) -> PortError {
        PortError::Desync {
            port: self.name,
            head,
            tail,
            why,
        }
    }
}

/// A value the host writes and the fabric reads.
#[derive(Facet)]
pub struct ToFabric<T> {
    port: Port,
    _t: PhantomData<T>,
}

/// A value the fabric drives and the host reads.
#[derive(Facet)]
pub struct FromFabric<T> {
    port: Port,
    _t: PhantomData<T>,
}

/// Host -> fabric SPSC ring of `N` entries. The fabric sees `<name>`,
/// `<name>_valid` and drives `<name>_ready`; an entry is consumed on a cycle
/// where both are high.
#[derive(Facet)]
pub struct ToFabricQueue<T, const N: usize> {
    port: Port,
    /// Our own count. We're the only producer, so it's authoritative.
    head: u32,
    /// The fabric's consumer count as last read; only refreshed when the ring
    /// looks full, so a push normally costs no reads at all.
    tail: u32,
    _t: PhantomData<T>,
}

/// Fabric -> host SPSC ring of `N` entries. The fabric drives `<name>` and
/// `<name>_valid` and sees `<name>_ready` (not full); an entry is pushed on a
/// cycle where both are high.
#[derive(Facet)]
pub struct FromFabricQueue<T, const N: usize> {
    port: Port,
    /// The fabric's producer count as last read; only refreshed when the ring
    /// looks empty.
    head: u32,
    /// Our own count. We're the only consumer, so it's authoritative.
    tail: u32,
    _t: PhantomData<T>,
}

impl<T: Facet<'static>> ToFabric<T> {
    pub fn write(&self, value: &T) -> Result<(), PortError> {
        Ok(self
            .port
            .write_words(self.port.word, &codec::encode(value, &self.port.ty))?)
    }
}

impl<T: Facet<'static>> FromFabric<T> {
    pub fn read(&self) -> Result<T, PortError> {
        Ok(codec::decode(&self.port.read_words(self.port.word)?, &self.port.ty)?)
    }

    /// Re-reads until `done` accepts the value. A read returns whatever the
    /// fabric drives *now*, never a response to an earlier write, so
    /// request/response over plain ports needs a field the fabric changes
    /// once its answer is valid (a generation counter) and this to wait for it.
    pub fn read_until(&self, timeout: Duration, mut done: impl FnMut(&T) -> bool) -> Result<T, PortError> {
        let deadline = Instant::now() + timeout;
        loop {
            let value = self.read()?;
            if done(&value) {
                return Ok(value);
            }
            if Instant::now() >= deadline {
                return Err(PortError::Timeout(timeout));
            }
        }
    }
}

// `&mut self` on both queue ends: each is a field the binding owns, so a
// second producer or consumer in this process is a borrow error, and #1's
// window lock covers other processes.
impl<T: Facet<'static>, const N: usize> ToFabricQueue<T, N> {
    pub fn push(&mut self, value: &T) -> Result<(), PushError> {
        if self.head.wrapping_sub(self.tail) as usize >= N {
            let (head, tail) = self.port.queue_counters(N)?;
            if head != self.head {
                return Err(self
                    .port
                    .desync(head, tail, "fabric head differs from ours; another producer?")
                    .into());
            }
            self.tail = tail;
            if head.wrapping_sub(tail) as usize >= N {
                return Err(PushError::Full);
            }
        }
        let words = codec::encode(value, &self.port.ty);
        self.port
            .write_words(self.port.word + queue::ENTRY, &words)
            .map_err(PortError::from)?;
        self.head = self.head.wrapping_add(1);
        Ok(())
    }
}

impl<T: Facet<'static>, const N: usize> FromFabricQueue<T, N> {
    /// The oldest entry, or `None` if the fabric hasn't produced one.
    pub fn pop(&mut self) -> Result<Option<T>, PortError> {
        if self.head == self.tail {
            let (head, tail) = self.port.queue_counters(N)?;
            if tail != self.tail {
                return Err(self
                    .port
                    .desync(head, tail, "fabric tail differs from ours; another consumer?"));
            }
            self.head = head;
            if head == tail {
                return Ok(None);
            }
        }
        let next = self.tail.wrapping_add(1);
        // The pop is committed before decoding: an entry that doesn't decode
        // is garbage either way, and leaving it would wedge the queue.
        let words = {
            let words = self.port.read_words(self.port.word + queue::ENTRY)?;
            self.port.bus.lock().write(self.port.word + queue::TAIL, next)?;
            words
        };
        self.tail = next;
        Ok(Some(codec::decode(&words, &self.port.ty)?))
    }

    /// Like [`pop`](Self::pop), but sleeps on the boundary's interrupt until
    /// an entry arrives or `timeout` passes (`Ok(None)`). Without an
    /// interrupt from the transport it polls instead, so it works anywhere.
    ///
    /// One line serves every `FromFabricQueue` in the boundary, so a waiter
    /// can be woken by another queue's entry; it re-checks its own and sleeps
    /// again. While that other entry sits unconsumed the line stays high, so
    /// keep every such queue drained or it turns this into polling.
    pub fn pop_wait(&mut self, timeout: Duration) -> Result<Option<T>, PortError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(v) = self.pop()? {
                return Ok(Some(v));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            self.port.bus.wait_irq(left)?;
        }
    }
}

/// Turn a boundary declaration into live handles over `transport`, after
/// checking the fabric was generated from this same declaration.
pub fn bind<B: Facet<'static>>(transport: impl Transport + 'static) -> Result<B, BindError> {
    let boundary = Boundary::of::<B>()?;
    let owner = take_ownership(&transport)?;
    let mut transport: Box<dyn Transport> = Box::new(transport);
    let found = transport.read(FINGERPRINT_WORD)?;
    let expected = boundary.fingerprint();
    if found != expected {
        return Err(BindError::Fingerprint {
            boundary: boundary.name,
            expected,
            found,
        });
    }

    let bus = Arc::new(Bus {
        irq: transport.interrupt().map(Mutex::new),
        transport: Mutex::new(transport),
        _owner: owner,
    });
    let mut p = Partial::alloc::<B>().map_err(reflect)?;
    for (i, decl) in boundary.ports.iter().enumerate() {
        let port = Port {
            bus: bus.clone(),
            ty: decl.ty.clone(),
            word: decl.word,
            name: decl.name,
        };
        // A queue's ends start from the fabric's counters, not zero, so a host
        // that restarts resumes where the ring actually is.
        let counters = match decl.kind {
            PortKind::Register => None,
            PortKind::Queue { depth } => Some(port.queue_counters(depth as usize)?),
        };
        p = p.begin_nth_field(i).map_err(reflect)?;
        p = p
            .begin_nth_field(0)
            .and_then(|p| p.set(port))
            .and_then(|p| p.end())
            .map_err(reflect)?;
        let phantom = match counters {
            None => 1,
            Some((head, tail)) => {
                p = p
                    .begin_nth_field(1)
                    .and_then(|p| p.set(head))
                    .and_then(|p| p.end())
                    .and_then(|p| p.begin_nth_field(2))
                    .and_then(|p| p.set(tail))
                    .and_then(|p| p.end())
                    .map_err(reflect)?;
                3
            }
        };
        p = p
            .begin_nth_field(phantom)
            .and_then(|p| p.set_default())
            .and_then(|p| p.end())
            .and_then(|p| p.end())
            .map_err(reflect)?;
    }
    p.build().map_err(reflect)?.materialize().map_err(reflect)
}

#[derive(Debug, thiserror::Error)]
pub enum PortError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error("the fabric did not produce the awaited value within {0:?}")]
    Timeout(Duration),
    #[error("queue `{port}` is out of step with the fabric (head {head}, tail {tail}): {why}")]
    Desync {
        port: &'static str,
        head: u32,
        tail: u32,
        why: &'static str,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    /// Every slot holds an entry the fabric hasn't consumed yet. Nothing was written.
    #[error("queue is full")]
    Full,
    #[error(transparent)]
    Port(#[from] PortError),
}

#[derive(Debug, thiserror::Error)]
pub enum BindError {
    #[error(transparent)]
    Layout(#[from] LayoutError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(
        "fabric fingerprint {found:#010x} != {expected:#010x} for `{boundary}`: the bitstream was built from a different declaration"
    )]
    Fingerprint {
        boundary: &'static str,
        expected: u32,
        found: u32,
    },
    #[error(
        "another binding owns this register window (lock {} is held); interleaved multi-word accesses would tear",
        lock.display()
    )]
    Busy { lock: PathBuf },
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("reflection failed while binding (a facet-hdl bug): {0}")]
    Reflect(String),
}

fn reflect(e: impl std::fmt::Display) -> BindError {
    BindError::Reflect(e.to_string())
}
