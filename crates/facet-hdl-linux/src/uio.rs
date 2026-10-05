use crate::{Window, window_lock};
use facet::Facet;
use facet_hdl::{BindError, Boundary, Interrupt, Transport};
use memmap2::MmapOptions;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The bridge window through a UIO device: a device-tree node with
/// `compatible = "generic-uio"`, the window as `reg`, and the boundary's
/// `irq` as a level-high interrupt, bound by `uio_pdrv_genirq`.
pub struct Uio {
    window: Window,
    file: File,
}

impl Uio {
    /// Opens the UIO device called `name` and maps exactly the register
    /// window `B` declares.
    pub fn open_for<B: Facet<'static>>(name: &str) -> Result<Self, BindError> {
        let words = Boundary::of::<B>()?.words();
        Ok(Self::open(name, words)?)
    }

    pub fn open(name: &str, words: u32) -> io::Result<Self> {
        Self::open_in(Path::new("/sys/class/uio"), Path::new("/dev"), name, words)
    }

    /// `sysfs` is the `/sys/class/uio` directory and `dev` the directory
    /// holding the `uioN` nodes; separate so tests can fake both.
    pub fn open_in(sysfs: &Path, dev: &Path, name: &str, words: u32) -> io::Result<Self> {
        let uio = find(sysfs, name)?;
        let map0 = sysfs.join(&uio).join("maps/map0");
        let phys = read_hex(&map0.join("addr"))?;
        let size = read_hex(&map0.join("size"))?;
        let need = words as u64 * 4;
        if size < need {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name}: map0 is {size:#x} bytes but the boundary needs {need:#x}; fix the node's reg"),
            ));
        }
        let file = OpenOptions::new().read(true).write(true).open(dev.join(&uio))?;
        // UIO selects map N with offset N * page size; map0 is offset 0.
        let map = MmapOptions::new().len(need as usize).map_raw(&file)?;
        Ok(Self {
            window: Window { map, words, phys },
            file,
        })
    }
}

/// The `uioN` whose `name` is `name`.
fn find(sysfs: &Path, name: &str) -> io::Result<String> {
    let entries = fs::read_dir(sysfs).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "listing {}: {e} (is uio_pdrv_genirq loaded and the node bound?)",
                sysfs.display()
            ),
        )
    })?;
    for entry in entries {
        let entry = entry?;
        if fs::read_to_string(entry.path().join("name"))?.trim() == name {
            return Ok(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no UIO device named {name:?} under {}", sysfs.display()),
    ))
}

fn read_hex(path: &Path) -> io::Result<u64> {
    let text = fs::read_to_string(path)?;
    let digits = text.trim().trim_start_matches("0x");
    u64::from_str_radix(digits, 16)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display())))
}

impl Transport for Uio {
    fn read(&mut self, word: u32) -> io::Result<u32> {
        self.window.read(word)
    }

    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        self.window.write(word, value)
    }

    fn lock_path(&self) -> PathBuf {
        window_lock(self.window.phys)
    }

    fn interrupt(&mut self) -> Option<Box<dyn Interrupt>> {
        // A failed dup just means no interrupt: pop_wait falls back to polling.
        self.file
            .try_clone()
            .ok()
            .map(|f| Box::new(UioIrq(f)) as Box<dyn Interrupt>)
    }
}

/// The UIO device's interrupt.
///
/// `uio_pdrv_genirq` masks a level interrupt each time it fires, because it
/// can't clear the source itself. Writing 1 to the device unmasks it, and a
/// line that's still high fires again at once. So every wait unmasks first,
/// which is what keeps the level semantics `Interrupt` promises.
struct UioIrq(File);

impl Interrupt for UioIrq {
    fn wait(&mut self, timeout: Duration) -> io::Result<bool> {
        self.0.write_all(&1u32.to_ne_bytes())?;
        let mut pfd = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: one valid pollfd for the duration of the call.
        match unsafe { libc::poll(&mut pfd, 1, ms) } {
            0 => Ok(false),
            n if n > 0 => {
                // Consume the event count so the next poll blocks again.
                let mut count = [0u8; 4];
                self.0.read_exact(&mut count)?;
                Ok(true)
            }
            _ => match io::Error::last_os_error() {
                // A signal isn't an error; callers re-check their queue anyway.
                e if e.kind() == io::ErrorKind::Interrupted => Ok(true),
                e => Err(e),
            },
        }
    }
}
