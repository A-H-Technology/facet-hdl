use facet::Facet;
use facet_hdl::{BindError, Boundary, FromFabric, PortError, ToFabric, Transport, bind};
use std::io;
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
struct Mem(Arc<Mutex<Vec<u32>>>);

impl Transport for Mem {
    fn read(&mut self, word: u32) -> io::Result<u32> {
        Ok(self.0.lock().unwrap()[word as usize])
    }
    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        self.0.lock().unwrap()[word as usize] = value;
        Ok(())
    }
}

fn fabric() -> (Mem, Boundary) {
    let b = Boundary::of::<Regs>().unwrap();
    let mut words = vec![0; b.words() as usize];
    words[0] = b.fingerprint();
    (Mem(Arc::new(Mutex::new(words))), b)
}

#[test]
fn writes_land_where_the_layout_says_and_reads_decode() {
    let (mem, b) = fabric();
    let regs: Regs = bind(mem.clone()).unwrap();
    let ctl = Wide { mode: Mode::Blink, a: 0xDEAD_BEEF, b: 7 };
    regs.ctl.write(&ctl).unwrap();

    // Simulate the fabric echoing ctl back on status.
    let (c, s) = (&b.ports[0], &b.ports[1]);
    {
        let mut w = mem.0.lock().unwrap();
        for i in 0..c.words {
            w[(s.word + i) as usize] = w[(c.word + i) as usize];
        }
    }
    assert_eq!(regs.status.read().unwrap(), ctl);
}

#[test]
fn refuses_a_fabric_built_from_another_declaration() {
    let (mem, _) = fabric();
    mem.0.lock().unwrap()[0] ^= 1;
    assert!(matches!(bind::<Regs>(mem), Err(BindError::Fingerprint { .. })));
}

#[test]
fn surfaces_garbage_discriminants_from_the_fabric() {
    let (mem, b) = fabric();
    let regs: Regs = bind(mem.clone()).unwrap();
    mem.0.lock().unwrap()[b.ports[1].word as usize] = 0b11;
    assert!(matches!(regs.status.read(), Err(PortError::Decode(_))));
}
