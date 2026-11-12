//! The host side of a boundary: the declaration's own fields become handles.

use crate::bell::Doorbell;
use crate::boundary::{Boundary, FINGERPRINT_WORD, PortKind, queue};
use crate::codec::{self, DecodeError};
use crate::hw::{HwType, LayoutError};
use facet::{Facet, Partial};
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};
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
/// The interrupt lives on the bell's thread, so a blocked waiter never holds
/// up register traffic.
struct Bus {
    transport: Mutex<Box<dyn Transport>>,
    bell: Arc<Doorbell>,
    /// Without a line every wait is a timed re-poll.
    has_irq: bool,
    _owner: File,
}

impl Bus {
    fn lock(&self) -> std::sync::MutexGuard<'_, Box<dyn Transport>> {
        self.transport.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.bell.close();
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

/// A queue end's view of its ring, behind a lock so the end can be shared
/// (`&self`): a second producer or consumer in this process is serialized
/// here, and the window lock covers other processes.
#[doc(hidden)]
#[derive(Facet)]
pub struct Cursor {
    #[facet(opaque)]
    counts: Mutex<Counts>,
}

#[derive(Default)]
struct Counts {
    head: u32,
    tail: u32,
    /// Only used by [`Answers`].
    inbox: Inbox,
}

impl Cursor {
    fn new(head: u32, tail: u32) -> Self {
        Self {
            counts: Mutex::new(Counts {
                head,
                tail,
                inbox: Inbox::default(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Counts> {
        self.counts.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Host -> fabric SPSC ring of `N` entries. The fabric sees `<name>`,
/// `<name>_valid` and drives `<name>_ready`; an entry is consumed on a cycle
/// where both are high.
///
/// `head` is our own count (we're the only producer, so it's authoritative);
/// `tail` is the fabric's as last read, only refreshed when the ring looks
/// full, so a push normally costs no reads at all.
#[derive(Facet)]
pub struct ToFabricQueue<T, const N: usize> {
    port: Port,
    cursor: Cursor,
    _t: PhantomData<T>,
}

/// Fabric -> host SPSC ring of `N` entries. The fabric drives `<name>` and
/// `<name>_valid` and sees `<name>_ready` (not full); an entry is pushed on a
/// cycle where both are high.
///
/// `head` is the fabric's count as last read, only refreshed when the ring
/// looks empty; `tail` is our own.
#[derive(Facet)]
pub struct FromFabricQueue<T, const N: usize> {
    port: Port,
    cursor: Cursor,
    _t: PhantomData<T>,
}

/// The answers to an `#[import]`: a fabric -> host ring where entry `k`
/// answers request `k`, claimed by that number instead of popped in order.
///
/// That's what lets calls overlap. Each caller waits for its own answer;
/// whoever pops someone else's parks it for them, and an answer whose caller
/// gave up (or belongs to a previous process) is dropped on arrival. Popping
/// in plain FIFO order would hand those to whoever asked next, which is why
/// this isn't a `FromFabricQueue`.
#[derive(Facet)]
pub struct Answers<T, const N: usize> {
    port: Port,
    cursor: Cursor,
    _t: PhantomData<T>,
}

/// Tickets of calls still waiting, and answers popped before their caller
/// came for them (still encoded).
#[derive(Default)]
struct Inbox {
    waiting: BTreeSet<u32>,
    parked: BTreeMap<u32, Vec<u32>>,
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

impl Port {
    fn bell(&self) -> &Doorbell {
        &self.bus.bell
    }

    /// Resolves once the fabric might have changed: the irq rang, or (with
    /// `tick`, or without a line at all) a short poll interval passed.
    pub(crate) async fn changed(&self, seen: u64, tick: bool) -> Result<(), PortError> {
        Ok(self.bell().rang(seen, tick || !self.bus.has_irq).await?)
    }

    /// `true` if the ring has room, refreshing our view of `tail` if it
    /// looked full.
    fn has_room(&self, c: &mut Counts, depth: usize) -> Result<bool, PortError> {
        if (c.head.wrapping_sub(c.tail) as usize) < depth {
            return Ok(true);
        }
        let (head, tail) = self.queue_counters(depth)?;
        if head != c.head {
            return Err(self.desync(head, tail, "fabric head differs from ours; another producer?"));
        }
        c.tail = tail;
        Ok((head.wrapping_sub(tail) as usize) < depth)
    }

    /// Writes entry `c.head` and advances it. The caller checked for room.
    fn push_words(&self, c: &mut Counts, words: &[u32]) -> Result<(), PortError> {
        let next = c.head.wrapping_add(1);
        if words.is_empty() {
            // A `()` entry has no last word to commit on, so the push is the
            // new absolute head, idempotent like a pop's tail write.
            self.bus.lock().write(self.word + queue::HEAD, next)?;
        } else {
            self.write_words(self.word + queue::ENTRY, words)?;
        }
        c.head = next;
        Ok(())
    }

    /// The oldest entry's words and its number, or `None` if the fabric
    /// hasn't produced one.
    fn pop_words(&self, c: &mut Counts, depth: usize) -> Result<Option<(u32, Vec<u32>)>, PortError> {
        if c.head == c.tail {
            let (head, tail) = self.queue_counters(depth)?;
            if tail != c.tail {
                return Err(self.desync(head, tail, "fabric tail differs from ours; another consumer?"));
            }
            c.head = head;
            if head == tail {
                return Ok(None);
            }
        }
        let index = c.tail;
        let next = index.wrapping_add(1);
        // The pop is committed before decoding: an entry that doesn't decode
        // is garbage either way, and leaving it would wedge the queue.
        let words = self.read_words(self.word + queue::ENTRY)?;
        self.bus.lock().write(self.word + queue::TAIL, next)?;
        c.tail = next;
        Ok(Some((index, words)))
    }

    fn decode<T: Facet<'static>>(&self, words: &[u32]) -> Result<T, PortError> {
        Ok(codec::decode(words, &self.ty)?)
    }
}

impl<T: Facet<'static>, const N: usize> ToFabricQueue<T, N> {
    /// Entries this ring has ever taken (a free-running count).
    pub fn pushed(&self) -> u32 {
        self.cursor.lock().head
    }

    pub fn push(&self, value: &T) -> Result<(), PushError> {
        self.push_numbered(value, |_| {}).map(drop)
    }

    /// Pushes, calling `numbered` with the entry's number just before the
    /// fabric can see it.
    pub(crate) fn push_numbered(&self, value: &T, numbered: impl FnOnce(u32)) -> Result<u32, PushError> {
        let mut c = self.cursor.lock();
        if !self.port.has_room(&mut c, N)? {
            return Err(PushError::Full);
        }
        let index = c.head;
        numbered(index);
        self.port.push_words(&mut c, &codec::encode(value, &self.port.ty))?;
        Ok(index)
    }

    /// Pushes, waiting for room. The fabric has no "room again" interrupt, so
    /// a full ring is re-polled every few tens of µs.
    pub async fn send(&self, value: &T) -> Result<(), PortError> {
        self.send_numbered(value, |_| {}).await.map(drop)
    }

    pub(crate) async fn send_numbered(&self, value: &T, mut numbered: impl FnMut(u32)) -> Result<u32, PortError> {
        loop {
            let seen = self.port.bell().rung();
            match self.push_numbered(value, &mut numbered) {
                Ok(index) => return Ok(index),
                Err(PushError::Full) => self.port.changed(seen, true).await?,
                Err(PushError::Port(e)) => return Err(e),
            }
        }
    }
}

impl<T: Facet<'static>, const N: usize> FromFabricQueue<T, N> {
    /// Entries ever popped from this ring (a free-running count).
    pub fn popped(&self) -> u32 {
        self.cursor.lock().tail
    }

    /// The oldest entry, or `None` if the fabric hasn't produced one.
    pub fn pop(&self) -> Result<Option<T>, PortError> {
        let popped = self.port.pop_words(&mut self.cursor.lock(), N)?;
        popped.map(|(_, words)| self.port.decode(&words)).transpose()
    }

    /// The oldest entry, waiting on the boundary's interrupt until there is
    /// one. Without an interrupt from the transport it polls instead, so it
    /// works anywhere.
    ///
    /// One line serves every `FromFabricQueue` in the boundary, so a waiter
    /// can be woken by another queue's entry; it re-checks its own and sleeps
    /// again. While that other entry sits unconsumed the line stays high, so
    /// keep every such queue drained or it turns this into polling.
    pub async fn recv(&self) -> Result<T, PortError> {
        loop {
            let seen = self.port.bell().rung();
            if let Some(v) = self.pop()? {
                return Ok(v);
            }
            self.port.changed(seen, false).await?;
        }
    }

    /// [`recv`](Self::recv) for plain threads: `Ok(None)` once `timeout` passes.
    pub fn pop_wait(&self, timeout: std::time::Duration) -> Result<Option<T>, PortError> {
        crate::block_on_timeout(self.recv(), timeout).transpose()
    }

    /// Start of an idle wait: see [`crate::rpc::Idle`].
    pub(crate) fn idle(&self) -> (&Port, u64) {
        (&self.port, self.port.bell().rung())
    }
}

impl<T: Facet<'static>, const N: usize> Answers<T, N> {
    /// Answers ever popped from this ring, claimed or not.
    pub fn popped(&self) -> u32 {
        self.cursor.lock().tail
    }

    /// Marks `ticket` as a call whose answer to keep. Must happen before the
    /// request is pushed, or another waiter could pop the answer first and
    /// drop it as nobody's.
    pub(crate) fn expect(&self, ticket: u32) {
        self.cursor.lock().inbox.waiting.insert(ticket);
    }

    /// The caller gave up on `ticket`: its answer is dropped, now or when it
    /// arrives.
    pub(crate) fn forget(&self, ticket: u32) {
        let mut c = self.cursor.lock();
        c.inbox.waiting.remove(&ticket);
        c.inbox.parked.remove(&ticket);
    }

    /// `ticket`'s answer, popping (and parking or dropping) everything
    /// ahead of it, or `None` if it hasn't arrived.
    pub(crate) fn take(&self, ticket: u32) -> Result<Option<T>, PortError> {
        let mut c = self.cursor.lock();
        loop {
            if let Some(words) = c.inbox.parked.remove(&ticket) {
                c.inbox.waiting.remove(&ticket);
                return self.port.decode(&words).map(Some);
            }
            let Some((index, words)) = self.port.pop_words(&mut c, N)? else {
                return Ok(None);
            };
            if c.inbox.waiting.contains(&index) {
                c.inbox.parked.insert(index, words);
            }
        }
    }

    /// Resolves once `ticket`'s answer might have arrived.
    pub(crate) async fn changed(&self, seen: u64) -> Result<(), PortError> {
        self.port.changed(seen, false).await
    }

    pub(crate) fn rung(&self) -> u64 {
        self.port.bell().rung()
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

    let irq = transport.interrupt();
    let bus = Arc::new(Bus {
        has_irq: irq.is_some(),
        bell: Doorbell::start(irq),
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
                    .and_then(|p| p.set(Cursor::new(head, tail)))
                    .and_then(|p| p.end())
                    .map_err(reflect)?;
                2
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
