use facet::Facet;
use facet_hdl::{BindError, Boundary, FromFabric, PortError, ToFabric, Transport, bind};
use std::io;

use std::path::PathBuf;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Facet, Debug, PartialEq)]
#[repr(u8)]
pub enum Mode {
    Off,
    On,
    Blink,
}

#[derive(Facet, Debug, PartialEq)]
pub struct Wide {
    pub mode: Mode,
    pub a: u32,
    pub b: u32,
}

#[derive(Facet)]
pub struct Regs {
    pub ctl: ToFabric<Wide>,
    pub status: FromFabric<Wide>,
}

/// Shared word array standing in for the register window.
#[derive(Clone)]
struct Mem {
    words: Arc<Mutex<Vec<u32>>>,
    lock: PathBuf,
}

impl Transport for Mem {
    fn read(&mut self, word: u32) -> io::Result<u32> {
        Ok(self.words.lock().unwrap()[word as usize])
    }
    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        self.words.lock().unwrap()[word as usize] = value;
        Ok(())
    }
    fn lock_path(&self) -> PathBuf {
        self.lock.clone()
    }
}

/// A fresh window; its lock file is unique per call so tests don't contend.
fn fabric() -> (Mem, Boundary) {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let lock = std::env::temp_dir().join(format!(
        "facet-hdl-test-{}-{}.lock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    window_at(lock)
}

fn window_at(lock: PathBuf) -> (Mem, Boundary) {
    let b = Boundary::of::<Regs>().unwrap();
    let mut words = vec![0; b.words() as usize];
    words[0] = b.fingerprint();
    (
        Mem {
            words: Arc::new(Mutex::new(words)),
            lock,
        },
        b,
    )
}
#[test]
fn writes_land_where_the_layout_says_and_reads_decode() {
    let (mem, b) = fabric();
    let regs: Regs = bind(mem.clone()).unwrap();
    let ctl = Wide {
        mode: Mode::Blink,
        a: 0xDEAD_BEEF,
        b: 7,
    };
    regs.ctl.write(&ctl).unwrap();

    // Simulate the fabric echoing ctl back on status.
    let (c, s) = (&b.ports[0], &b.ports[1]);
    {
        let mut w = mem.words.lock().unwrap();
        for i in 0..c.words {
            w[(s.word + i) as usize] = w[(c.word + i) as usize];
        }
    }
    assert_eq!(regs.status.read().unwrap(), ctl);
}

#[test]
fn refuses_a_fabric_built_from_another_declaration() {
    let (mem, _) = fabric();
    mem.words.lock().unwrap()[0] ^= 1;
    assert!(matches!(bind::<Regs>(mem), Err(BindError::Fingerprint { .. })));
}

#[test]
fn surfaces_garbage_discriminants_from_the_fabric() {
    let (mem, b) = fabric();
    let regs: Regs = bind(mem.clone()).unwrap();
    mem.words.lock().unwrap()[b.ports[1].word as usize] = 0b11;
    assert!(matches!(regs.status.read(), Err(PortError::Decode(_))));
}

#[test]
fn a_second_binding_of_the_same_window_is_refused_until_the_first_drops() {
    let (mem, _) = fabric();
    let first: Regs = bind(mem.clone()).unwrap();
    assert!(matches!(bind::<Regs>(mem.clone()), Err(BindError::Busy { .. })));
    drop(first);
    let _second: Regs = bind(mem).expect("lock released with the first binding");
}
