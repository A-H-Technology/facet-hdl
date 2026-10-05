use crate::{Window, window_lock};
use facet::Facet;
use facet_hdl::{BindError, Boundary, Transport};
use memmap2::MmapOptions;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

/// Agilex 5 lightweight HPS-to-FPGA bridge window (512 MiB). From the Altera
/// Agilex 5 GSRD address map; a Platform Designer base address for the
/// boundary component is an offset into this.
pub const AGILEX5_LWH2F: u64 = 0x2000_0000;

/// The bridge through `/dev/mem`. Needs root (or CAP_SYS_RAWIO).
pub struct DevMem(Window);

impl DevMem {
    /// Maps exactly the register window `B` declares, at physical `base`.
    pub fn open_for<B: Facet<'static>>(base: u64) -> Result<Self, BindError> {
        let words = Boundary::of::<B>()?.words();
        Ok(Self::open(base, words)?)
    }

    pub fn open(base: u64, words: u32) -> io::Result<Self> {
        let page = page_size();
        if !base.is_multiple_of(page) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{base:#x} isn't page-aligned; place the component on a {page:#x} boundary"),
            ));
        }
        // O_SYNC is what makes the kernel map /dev/mem uncached; device
        // registers must never sit in a CPU cache line.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_SYNC)
            .open("/dev/mem")?;
        let map = MmapOptions::new().offset(base).len(words as usize * 4).map_raw(&file)?;
        Ok(Self(Window { map, words, phys: base }))
    }
}

impl Transport for DevMem {
    fn read(&mut self, word: u32) -> io::Result<u32> {
        self.0.read(word)
    }

    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        self.0.write(word, value)
    }

    fn lock_path(&self) -> PathBuf {
        window_lock(self.0.phys)
    }
}

fn page_size() -> u64 {
    // SAFETY: sysconf has no preconditions.
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
}
