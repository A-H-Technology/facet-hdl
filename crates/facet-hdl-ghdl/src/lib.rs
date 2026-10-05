//! Co-simulation: GHDL runs the fabric design, and a generated testbench turns
//! line commands on its stdin into AXI4-Lite transactions on the design's
//! slave port. [`SimTransport`] speaks that protocol, so the host code under
//! test is exactly the code that will run on the HPS.
//!
//! The design under test must have the same interface the generated
//! `<boundary>_regs` entity has: an `ADDR_WIDTH` generic, `clk`, `rst_n` and
//! the `s_axi_*` ports. Any other outputs are left open, so a full top level
//! with LEDs etc. works as-is; extra inputs need default values.

use facet_hdl::Transport;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

const TB: &str = "facet_hdl_tb";

pub struct Sim {
    workdir: PathBuf,
}

pub struct SimBuilder {
    workdir: PathBuf,
    sources: Vec<PathBuf>,
    top: String,
    addr_width: u32,
    generics: Vec<(String, String)>,
}

impl Sim {
    /// `sources` in compile order; `top` is the entity the testbench drives.
    pub fn builder(workdir: impl Into<PathBuf>, top: &str, addr_width: u32) -> SimBuilder {
        SimBuilder {
            workdir: workdir.into(),
            sources: Vec::new(),
            top: top.into(),
            addr_width,
            generics: Vec::new(),
        }
    }

    /// Starts the simulation and waits for reset to be released.
    pub fn spawn(&self) -> io::Result<(SimTransport, SimClock)> {
        let mut child = Command::new("ghdl")
            .args(["-r", "--std=08", &format!("--workdir={}", self.workdir.display()), TB])
            .current_dir(&self.workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = BufReader::new(child.stdout.take().expect("piped"));
        let mut link = Link { child, stdin, stdout };
        link.expect("READY")?;
        let link = Arc::new(Mutex::new(link));
        Ok((SimTransport(link.clone()), SimClock(link)))
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

    /// Extra generic for the top entity, as a VHDL expression.
    pub fn generic(mut self, name: &str, value: &str) -> Self {
        self.generics.push((name.into(), value.into()));
        self
    }

    /// Analyzes and elaborates everything into `workdir`.
    pub fn build(self) -> io::Result<Sim> {
        std::fs::create_dir_all(&self.workdir)?;
        let tb = self.workdir.join(format!("{TB}.vhd"));
        std::fs::write(&tb, testbench(&self.top, self.addr_width, &self.generics))?;
        let wd = format!("--workdir={}", self.workdir.display());
        let mut analyze = Command::new("ghdl");
        analyze.args(["-a", "--std=08", &wd]).args(&self.sources).arg(&tb);
        run(analyze)?;
        let mut elab = Command::new("ghdl");
        elab.args(["-e", "--std=08", &wd, TB]).current_dir(&self.workdir);
        run(elab)?;
        Ok(Sim { workdir: self.workdir })
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

impl Link {
    fn send(&mut self, cmd: &str) -> io::Result<String> {
        writeln!(self.stdin, "{cmd}")?;
        self.stdin.flush()?;
        let mut line = String::new();
        if self.stdout.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("simulator exited during `{cmd}`"),
            ));
        }
        Ok(line.trim().to_owned())
    }

    fn expect(&mut self, want: &str) -> io::Result<()> {
        let mut line = String::new();
        self.stdout.read_line(&mut line)?;
        if line.trim() == want {
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
            "{what}: AXI response {r} (2=SLVERR, 3=DECERR)"
        ))),
    }
}

pub struct SimTransport(Arc<Mutex<Link>>);

impl Transport for SimTransport {
    fn read(&mut self, word: u32) -> io::Result<u32> {
        let line = self.0.lock().unwrap().send(&format!("R {word}"))?;
        let mut it = line.split_whitespace();
        let (Some("D"), Some(hex), Some(resp)) = (it.next(), it.next(), it.next()) else {
            return Err(io::Error::other(format!("bad read reply {line:?}")));
        };
        resp_err(resp, format!("read word {word}"))?;
        u32::from_str_radix(hex, 16).map_err(io::Error::other)
    }

    fn write(&mut self, word: u32, value: u32) -> io::Result<()> {
        let line = self.0.lock().unwrap().send(&format!("W {word} {value:08X}"))?;
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

fn testbench(top: &str, aw: u32, generics: &[(String, String)]) -> String {
    let extra: String = generics.iter().map(|(k, v)| format!(", {k} => {v}")).collect();
    format!(
        r#"-- Generated by facet-hdl-ghdl: stdin/stdout AXI4-Lite master for co-simulation.
--   W <word> <hex32>  -> "B <resp>"
--   R <word>          -> "D <hex32> <resp>"
--   T <cycles>        -> "T"
--   Q                 -> ends the simulation
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use std.textio.all;
use std.env.all;

entity {TB} is
end entity;

architecture sim of {TB} is
  constant AW : positive := {aw};
  signal clk   : std_logic := '0';
  signal rst_n : std_logic := '0';
  signal awaddr, araddr : std_logic_vector(AW - 1 downto 0) := (others => '0');
  signal awvalid, wvalid, bready, arvalid, rready : std_logic := '0';
  signal awready, wready, bvalid, arready, rvalid : std_logic;
  signal wdata : std_logic_vector(31 downto 0) := (others => '0');
  signal rdata : std_logic_vector(31 downto 0);
  signal bresp, rresp : std_logic_vector(1 downto 0);
begin
  clk <= not clk after 5 ns;

  dut : entity work.{top}
    generic map (ADDR_WIDTH => AW{extra})
    port map (
      clk => clk, rst_n => rst_n,
      s_axi_awaddr => awaddr, s_axi_awvalid => awvalid, s_axi_awready => awready,
      s_axi_wdata => wdata, s_axi_wstrb => "1111", s_axi_wvalid => wvalid, s_axi_wready => wready,
      s_axi_bresp => bresp, s_axi_bvalid => bvalid, s_axi_bready => bready,
      s_axi_araddr => araddr, s_axi_arvalid => arvalid, s_axi_arready => arready,
      s_axi_rdata => rdata, s_axi_rresp => rresp, s_axi_rvalid => rvalid, s_axi_rready => rready
    );

  driver : process
    variable l : line;
    variable o : line;
    variable cmd : character;
    variable word : natural;
    variable n : natural;
    variable data : std_logic_vector(31 downto 0);
    variable aw_done, w_done : boolean;
  begin
    for i in 1 to 4 loop
      wait until rising_edge(clk);
    end loop;
    rst_n <= '1';
    wait until rising_edge(clk);
    write(o, string'("READY"));
    writeline(output, o);
    flush(output);

    while not endfile(input) loop
      readline(input, l);
      next when l'length = 0;
      read(l, cmd);
      case cmd is
        when 'W' =>
          read(l, word);
          hread(l, data);
          awaddr <= std_logic_vector(to_unsigned(word * 4, AW));
          wdata <= data;
          awvalid <= '1';
          wvalid <= '1';
          bready <= '1';
          aw_done := false;
          w_done := false;
          while not (aw_done and w_done) loop
            wait until rising_edge(clk);
            if awvalid = '1' and awready = '1' then
              aw_done := true;
              awvalid <= '0';
            end if;
            if wvalid = '1' and wready = '1' then
              w_done := true;
              wvalid <= '0';
            end if;
          end loop;
          loop
            wait until rising_edge(clk);
            exit when bvalid = '1';
          end loop;
          bready <= '0';
          write(o, string'("B "));
          write(o, to_integer(unsigned(bresp)));
        when 'R' =>
          read(l, word);
          araddr <= std_logic_vector(to_unsigned(word * 4, AW));
          arvalid <= '1';
          rready <= '1';
          loop
            wait until rising_edge(clk);
            exit when arready = '1';
          end loop;
          arvalid <= '0';
          loop
            wait until rising_edge(clk);
            exit when rvalid = '1';
          end loop;
          rready <= '0';
          write(o, string'("D "));
          hwrite(o, rdata);
          write(o, string'(" "));
          write(o, to_integer(unsigned(rresp)));
        when 'T' =>
          read(l, n);
          for i in 1 to n loop
            wait until rising_edge(clk);
          end loop;
          write(o, string'("T"));
        when others =>
          finish;
      end case;
      writeline(output, o);
      flush(output);
    end loop;
    finish;
  end process;
end architecture;
"#
    )
}
