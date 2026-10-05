//! The host side of a boundary: the declaration's own fields become handles.

use crate::boundary::{Boundary, FINGERPRINT_WORD};
use crate::codec::{self, DecodeError};
use crate::hw::{HwType, LayoutError};
use facet::{Facet, Partial};
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Word-addressed access to a boundary's register window.
pub trait Transport: Send {
    fn read(&mut self, word: u32) -> io::Result<u32>;
    fn write(&mut self, word: u32, value: u32) -> io::Result<()>;

    /// The file whose exclusive `flock` stands for owning this register
    /// window. Every transport reaching the same window must name the same
    /// file; `bind` holds the lock for as long as the binding lives.
    fn lock_path(&self) -> PathBuf;
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
struct Bus {
    transport: Mutex<Box<dyn Transport>>,
    _owner: File,
}

impl Bus {
    fn lock(&self) -> std::sync::MutexGuard<'_, Box<dyn Transport>> {
        self.transport.lock().unwrap_or_else(|e| e.into_inner())
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

impl<T: Facet<'static>> ToFabric<T> {
    pub fn write(&self, value: &T) -> Result<(), PortError> {
        let words = codec::encode(value, &self.port.ty);
        let mut bus = self.port.bus.lock();
        for (i, w) in words.iter().enumerate() {
            bus.write(self.port.word + i as u32, *w)?;
        }
        Ok(())
    }
}

impl<T: Facet<'static>> FromFabric<T> {
    pub fn read(&self) -> Result<T, PortError> {
        let n = codec::words_for(self.port.ty.width());
        let words = {
            let mut bus = self.port.bus.lock();
            (0..n)
                .map(|i| bus.read(self.port.word + i))
                .collect::<io::Result<Vec<_>>>()?
        };
        Ok(codec::decode(&words, &self.port.ty)?)
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
        transport: Mutex::new(transport),
        _owner: owner,
    });
    let mut p = Partial::alloc::<B>().map_err(reflect)?;
    for (i, decl) in boundary.ports.iter().enumerate() {
        let port = Port {
            bus: bus.clone(),
            ty: decl.ty.clone(),
            word: decl.word,
        };
        p = p
            .begin_nth_field(i)
            .and_then(|p| p.begin_nth_field(0))
            .and_then(|p| p.set(port))
            .and_then(|p| p.end())
            .and_then(|p| p.begin_nth_field(1))
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
    #[error("reflection failed while binding (a facet-hdl bug): {0}")]
    Reflect(String),
}

fn reflect(e: impl std::fmt::Display) -> BindError {
    BindError::Reflect(e.to_string())
}
