//! Co-simulation: Icarus Verilog runs the fabric design, and a generated
//! testbench turns line commands on its stdin into AXI4 transactions on the
//! design's slave port. [`SimTransport`] speaks that protocol, so the host code
//! under test is exactly the code that will run on the HPS.
//!
//! The design under test has the flat AXI4 slave ports of the DE25-Nano's
//! lightweight bridge as `s_axi_*` (12-bit IDs, 32-bit data, no size, burst,
//! strobe or last signals: the generated register file ignores those), plus
//! `clk`, an active-high `rst`, and optionally `irq`. Every transaction the
//! testbench starts carries a new ID, and a response that doesn't echo it is
//! reported as response 9.

use facet_hdl::{Interrupt, Transport};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TB: &str = "facet_hdl_tb";

pub struct Sim {
    irq: bool,
    workdir: PathBuf,
}

pub struct SimBuilder {
    workdir: PathBuf,
    sources: Vec<PathBuf>,
    top: String,
    irq: bool,
}

impl Sim {
    /// `top` is the module the testbench drives.
    pub fn builder(workdir: impl Into<PathBuf>, top: &str) -> SimBuilder {
        SimBuilder {
            workdir: workdir.into(),
            sources: Vec::new(),
            top: top.into(),
            irq: false,
        }
    }

    /// Starts the simulation and waits for reset to be released.
    pub fn spawn(&self) -> io::Result<(SimTransport, SimClock)> {
        let mut child = Command::new("vvp")
            .args(["-n", "sim.vvp"])
            .current_dir(&self.workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = BufReader::new(child.stdout.take().expect("piped"));
        let lock = self.workdir.join(format!("sim-{}.lock", child.id()));
        let mut link = Link { child, stdin, stdout };
        link.expect("READY")?;
        let link = Arc::new(Mutex::new(link));
        Ok((
            SimTransport {
                link: link.clone(),
                lock,
                irq: self.irq,
            },
            SimClock(link),
        ))
    }
}

impl SimBuilder {
    pub fn source(mut self, path: impl AsRef<Path>) -> Self {
        self.sources.push(path.as_ref().to_owned());
        self
    }

    pub fn sources<P: AsRef<Path>>(mut self, paths: impl IntoIterator<Item = P>) -> Self {
        self.sources.extend(paths.into_iter().map(|p| p.as_ref().to_owned()));
        self
    }

    /// The top has an `irq` output; the transport then offers it as an
    /// [`Interrupt`].
    pub fn irq(mut self) -> Self {
        self.irq = true;
        self
    }

    /// Compiles everything into `workdir/sim.vvp`.
    pub fn build(self) -> io::Result<Sim> {
        std::fs::create_dir_all(&self.workdir)?;
        let tb = self.workdir.join(format!("{TB}.sv"));
        std::fs::write(&tb, testbench(&self.top, self.irq))?;
        let mut cc = Command::new("iverilog");
        cc.args(["-g2012", "-o"])
            .arg(self.workdir.join("sim.vvp"))
            .args(["-s", TB])
            .arg(&tb)
            .args(&self.sources);
        run(cc)?;
        Ok(Sim {
            workdir: self.workdir,
            irq: self.irq,
        })
    }
}

fn run(mut cmd: Command) -> io::Result<()> {
    let out = cmd.output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{cmd:?} failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )))
    }
}

struct Link {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// Marks the testbench's replies. The simulator prints warnings (and the
/// design's own `$display`s) on the same stdout, so anything unmarked is
/// passed through to stderr rather than parsed.
const REPLY: &str = "@@ ";

impl Link {
    fn send(&mut self, cmd: &str) -> io::Result<String> {
        writeln!(self.stdin, "{cmd}")?;
        self.stdin.flush()?;
        self.reply(cmd)
    }

    fn reply(&mut self, during: &str) -> io::Result<String> {
        loop {
            let mut line = String::new();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("simulator exited during `{during}`"),
                ));
            }
            match line.strip_prefix(REPLY) {
                Some(reply) => return Ok(reply.trim().to_owned()),
                None => eprint!("iverilog: {line}"),
            }
        }
    }

    fn expect(&mut self, want: &str) -> io::Result<()> {
        let line = self.reply("startup")?;
        if line == want {
            Ok(())
        } else {
            Err(io::Error::other(format!("simulator said {line:?}, expected {want:?}")))
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "Q");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

fn resp_err(resp: &str, what: String) -> io::Result<()> {
    match resp {
        "0" => Ok(()),
        r => Err(io::Error::other(format!(
            "{what}: AXI response {r} (2=SLVERR, 3=DECERR, 9=wrong ID echoed)"
        ))),
    }
}

pub struct SimTransport {
    link: Arc<Mutex<Link>>,
    /// One per simulator process: that process is the register window.
    lock: PathBuf,
    irq: bool,
}

impl SimTransport {
    /// Another handle onto the same running fabric, standing in for a second
    /// process mapping the same bridge window.
    pub fn share(&self) -> Self {
        Self {
            link: self.link.clone(),
            lock: self.lock.clone(),
            irq: self.irq,
        }
    }
}

impl Transport for SimTransport {
    fn interrupt(&mut self) -> Option<Box<dyn Interrupt>> {
        self.irq
            .then(|| Box::new(SimInterrupt(self.link.clone())) as Box<dyn Interrupt>)
    }

    fn lock_path(&self) -> PathBuf {
        self.lock.clone()
    }

    fn read(&mut self, word: u32) -> io::Result<u32> {
        let line = self.link.lock().unwrap().send(&format!("R {word}"))?;
        let mut it = line.split_whitespace();
        let (Some("D"), Some(hex), Some(resp)) = (it.next(), it.next(), it.next()) else {
            return Err(io::Error::other(format!("bad read reply {line:?}")));
        };
        resp_err(resp, format!("read word {word}"))?;
        // `x`/`z` digits mean the design drove undefined bits.
        u32::from_str_radix(hex, 16).map_err(|e| io::Error::other(format!("read word {word} gave {hex:?}: {e}")))
    }

    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        let line = self.link.lock().unwrap().send(&format!("W {word} {value:08X}"))?;
        match line.strip_prefix("B ") {
            Some(resp) => resp_err(resp, format!("write word {word}")),
            None => Err(io::Error::other(format!("bad write reply {line:?}"))),
        }
    }
}

/// Simulated time only moves during transactions and [`SimClock::cycles`].
pub struct SimClock(Arc<Mutex<Link>>);

impl SimClock {
    pub fn cycles(&self, n: u64) -> io::Result<()> {
        let line = self.0.lock().unwrap().send(&format!("T {n}"))?;
        if line == "T" {
            Ok(())
        } else {
            Err(io::Error::other(format!("bad tick reply {line:?}")))
        }
    }
}

impl SimTransport {
    /// One AXI4 write burst of `data.len()` beats from `word` up (a test of
    /// the slave itself; `Transport` only does single beats).
    pub fn write_burst(&mut self, word: u32, data: &[u32]) -> io::Result<()> {
        let mut cmd = format!("X {word} {}", data.len());
        for d in data {
            cmd.push_str(&format!(" {d:08X}"));
        }
        let line = self.link.lock().unwrap().send(&cmd)?;
        match line.strip_prefix("B ") {
            Some(resp) => resp_err(resp, format!("write burst at word {word}")),
            None => Err(io::Error::other(format!("bad burst reply {line:?}"))),
        }
    }

    /// One AXI4 read burst of `n` beats from `word` up.
    pub fn read_burst(&mut self, word: u32, n: usize) -> io::Result<Vec<u32>> {
        let line = self.link.lock().unwrap().send(&format!("Y {word} {n}"))?;
        let mut it = line.split_whitespace();
        if it.next() != Some("Y") {
            return Err(io::Error::other(format!("bad burst reply {line:?}")));
        }
        let words: Vec<&str> = it.collect();
        let (resp, beats) = words
            .split_last()
            .ok_or_else(|| io::Error::other("empty burst reply"))?;
        resp_err(resp, format!("read burst at word {word}"))?;
        beats
            .iter()
            .map(|h| u32::from_str_radix(h, 16).map_err(io::Error::other))
            .collect()
    }
}

mod testbench;
use testbench::testbench;

/// The top's `irq`, sampled by letting the simulator run until it rises.
/// Simulated time only advances while a wait is in progress, in bounded
/// slices so the link stays available to register traffic in between.
struct SimInterrupt(Arc<Mutex<Link>>);

impl Interrupt for SimInterrupt {
    fn wait(&mut self, timeout: Duration) -> io::Result<bool> {
        const SLICE_CYCLES: u32 = 64;
        let deadline = Instant::now() + timeout;
        // The first check runs zero cycles: a line that's already high must
        // return without letting time pass.
        let mut cycles = 0;
        loop {
            let line = self.0.lock().unwrap().send(&format!("I {cycles}"))?;
            match line.as_str() {
                "I 1" => return Ok(true),
                "I 0" if Instant::now() >= deadline => return Ok(false),
                "I 0" => cycles = SLICE_CYCLES,
                other => return Err(io::Error::other(format!("bad irq reply {other:?}"))),
            }
        }
    }
}
