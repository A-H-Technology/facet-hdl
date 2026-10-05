//! [`Transport`](facet_hdl::Transport)s for Linux on an SoC FPGA whose fabric
//! sits behind a memory-mapped bridge:
//!
//! - [`DevMem`] maps the bridge through `/dev/mem`. It needs no kernel setup
//!   beyond the bridge being enabled (U-Boot does that on mercury), but it
//!   needs root and has no interrupt, so `pop_wait` polls.
//! - [`Uio`] maps it through a `generic-uio` device-tree node, which also
//!   brings the boundary's `irq` line, so `pop_wait` sleeps.
//!
//! Both lock the same file for the same physical window, so they exclude
//! each other as well as themselves.

mod devmem;
mod uio;

pub use devmem::{AGILEX5_LWH2F, DevMem};
pub use uio::Uio;

use memmap2::MmapRaw;
use std::io;
use std::path::PathBuf;

/// One lock per physical window, whichever way it's mapped. /run/lock is
/// root-only on NixOS, which is fine: both ways of mapping need root or a
/// device node someone deliberately opened up.
fn window_lock(phys: u64) -> PathBuf {
    PathBuf::from(format!("/run/lock/facet-hdl-window-{phys:#x}.lock"))
}

/// A mapped register window: bounds-checked, volatile, 32-bit accesses.
struct Window {
    map: MmapRaw,
    words: u32,
    phys: u64,
}

impl Window {
    fn ptr(&self, word: u32) -> io::Result<*mut u32> {
        if word >= self.words {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("word {word} is outside the {}-word window", self.words),
            ));
        }
        Ok(self.map.as_mut_ptr().cast::<u32>().wrapping_add(word as usize))
    }

    fn read(&self, word: u32) -> io::Result<u32> {
        let p = self.ptr(word)?;
        // SAFETY: in bounds of a live mapping whose base is page-aligned, so
        // 4-aligned. Volatile because each access is a bus transaction the
        // fabric observes.
        Ok(unsafe { p.read_volatile() })
    }

    fn write(&self, word: u32, value: u32) -> io::Result<()> {
        let p = self.ptr(word)?;
        // SAFETY: as in `read`.
        unsafe { p.write_volatile(value) };
        Ok(())
    }
}
