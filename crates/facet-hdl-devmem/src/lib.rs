//! [`Transport`] for an SoC FPGA whose fabric sits behind a memory-mapped
//! bridge, reached through `/dev/mem`. Needs root (or CAP_SYS_RAWIO) and the
//! bridge enabled before Linux boots; on mercury U-Boot does the latter.

use facet::Facet;
use facet_hdl::{BindError, Boundary, Transport};
use memmap2::{MmapOptions, MmapRaw};
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

/// Agilex 5 lightweight HPS-to-FPGA bridge window (512 MiB). From the Altera
/// Agilex 5 GSRD address map; a Platform Designer base address for the
/// boundary component is an offset into this.
pub const AGILEX5_LWH2F: u64 = 0x2000_0000;

pub struct DevMem {
    map: MmapRaw,
    words: u32,
    base: u64,
}

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
        Ok(Self { map, words, base })
    }

    fn ptr(&self, word: u32) -> io::Result<*mut u32> {
        if word >= self.words {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("word {word} is outside the {}-word window", self.words),
            ));
        }
        Ok(self.map.as_mut_ptr().cast::<u32>().wrapping_add(word as usize))
    }
}

impl Transport for DevMem {
    /// Keyed on the physical base, so every process mapping this window
    /// contends for one lock. /run/lock is root-only on NixOS, which is fine:
    /// /dev/mem already needs root.
    fn lock_path(&self) -> PathBuf {
        PathBuf::from(format!("/run/lock/facet-hdl-devmem-{:#x}.lock", self.base))
    }

    fn read(&mut self, word: u32) -> io::Result<u32> {
        let p = self.ptr(word)?;
        // SAFETY: in bounds of a live, 4-aligned mapping (page-aligned base,
        // word index checked above). Volatile because each access is a bus
        // transaction the fabric observes.
        Ok(unsafe { p.read_volatile() })
    }

    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        let p = self.ptr(word)?;
        // SAFETY: as in `read`.
        unsafe { p.write_volatile(value) };
        Ok(())
    }
}

fn page_size() -> u64 {
    // SAFETY: sysconf has no preconditions.
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
}
