//! A word array standing in for a register window, shared by the test binaries.
#![allow(dead_code)] // each test binary uses a different subset

use facet::Facet;
use facet_hdl::{Boundary, Transport};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Mem {
    pub words: Arc<Mutex<Vec<u32>>>,
    pub lock: PathBuf,
}

impl Mem {
    /// A window sized for `B` with its fingerprint in place, locked by `lock`.
    pub fn for_boundary<B: Facet<'static>>(lock: PathBuf) -> (Self, Boundary) {
        let b = Boundary::of::<B>().unwrap();
        let mut words = vec![0; b.words() as usize];
        words[0] = b.fingerprint();
        let mem = Self {
            words: Arc::new(Mutex::new(words)),
            lock,
        };
        (mem, b)
    }

    /// As [`Mem::for_boundary`], with a lock file no other test shares.
    pub fn fresh<B: Facet<'static>>() -> (Self, Boundary) {
        Self::for_boundary::<B>(unique_lock())
    }

    pub fn set(&self, word: u32, value: u32) {
        self.words.lock().unwrap()[word as usize] = value;
    }

    pub fn get(&self, word: u32) -> u32 {
        self.words.lock().unwrap()[word as usize]
    }
}

pub fn unique_lock() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "facet-hdl-test-{}-{}.lock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

impl Transport for Mem {
    fn read(&mut self, word: u32) -> io::Result<u32> {
        Ok(self.get(word))
    }
    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        self.set(word, value);
        Ok(())
    }
    fn lock_path(&self) -> PathBuf {
        self.lock.clone()
    }
}
