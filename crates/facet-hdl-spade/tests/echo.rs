//! Every encodable shape goes host -> Spade -> host unchanged, through the
//! generated register file in iverilog. The fabric side is a wire from the
//! ToFabric value to the FromFabric one, so any disagreement between
//! `facet_hdl::codec` and the generated pack/unpack shows up as a mismatch.

use facet::Facet;
use facet_hdl::{FromFabric, ToFabric, bind};
use facet_hdl_iverilog::Sim;
use facet_hdl_spade::{
    FLAT_AXI_GLUE, FLAT_AXI_PORTS, Spade, literal, vhdl_flat_axi_glue, vhdl_flat_axi_ports, vhdl_library, vhdl_literal,
};
use std::path::Path;

#[derive(Facet, Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Tri {
    A,
    B,
    C,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Inner {
    pub flag: bool,
    pub tri: Tri,
    pub small: i8,
}

/// Payloads of different widths share one area; `Go`'s is narrower than
/// `Set`'s, so the padding above it is exercised too.
#[derive(Facet, Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum Cmd {
    Stop,
    Set(Inner),
    Nudge(i8),
    Go(Tri),
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Everything {
    pub inner: Inner,
    pub inners: [Inner; 2],
    pub flags: [bool; 5],
    pub tris: [Tri; 3],
    pub grid: [[u8; 2]; 3],
    pub wide: u128,
    pub neg: i64,
    pub half: u16,
    pub maybe: Option<u16>,
    pub res: Result<(), Tri>,
    pub cmds: [Cmd; 3],
    pub nested: Option<Result<Inner, bool>>,
}

#[derive(Facet)]
pub struct Echo {
    pub input: ToFabric<Everything>,
    pub output: FromFabric<Everything>,
    /// A constant on the fabric side, written as a Spade literal from a Rust
    /// value: reading it checks Spade packing alone, not unpacking.
    pub golden: FromFabric<Everything>,
    /// `pack(input) == pack(golden)` on the fabric: unpacking has to agree.
    pub matches_golden: FromFabric<bool>,
}

/// xorshift64: deterministic, so a failure reproduces.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn tri(&mut self) -> Tri {
        [Tri::A, Tri::B, Tri::C][(self.next() % 3) as usize]
    }
    fn inner(&mut self) -> Inner {
        Inner {
            flag: self.next() & 1 == 1,
            tri: self.tri(),
            small: self.next() as i8,
        }
    }
    fn cmd(&mut self) -> Cmd {
        match self.next() % 4 {
            0 => Cmd::Stop,
            1 => Cmd::Set(self.inner()),
            2 => Cmd::Nudge(self.next() as i8),
            _ => Cmd::Go(self.tri()),
        }
    }
    fn nested(&mut self) -> Option<Result<Inner, bool>> {
        match self.next() % 3 {
            0 => None,
            1 => Some(Ok(self.inner())),
            _ => Some(Err(self.next() & 1 == 1)),
        }
    }
    fn everything(&mut self) -> Everything {
        Everything {
            inner: self.inner(),
            inners: [self.inner(), self.inner()],
            flags: std::array::from_fn(|_| self.next() & 1 == 1),
            tris: std::array::from_fn(|_| self.tri()),
            grid: std::array::from_fn(|_| std::array::from_fn(|_| self.next() as u8)),
            wide: (self.next() as u128) << 64 | self.next() as u128,
            neg: self.next() as i64,
            half: self.next() as u16,
            maybe: (self.next() & 1 == 1).then(|| self.next() as u16),
            res: if self.next() & 1 == 1 { Ok(()) } else { Err(self.tri()) },
            cmds: std::array::from_fn(|_| self.cmd()),
            nested: self.nested(),
        }
    }
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

#[test]
fn rust_and_spade_agree_on_every_bit() {
    let golden = Rng(SEED ^ 0xFFFF).everything();
    let spade = Spade::of::<Echo>().unwrap();
    let top = format!(
        r#"use echo::{{echo_regs, Cmd, Everything, Inner, Tri, pack_everything}};

#[no_mangle(all)]
entity echo_top(
    clk: clock,
    rst: bool,
{FLAT_AXI_PORTS}
    irq: inv bool,
) {{
{FLAT_AXI_GLUE}
    let p = inst echo_regs(clk, rst, axi);
    set axi_out_inv = p.axi_out;
    set irq = p.irq;
    let golden: Everything = {golden};
    set p.output = p.input;
    set p.golden = golden;
    set p.matches_golden = pack_everything(p.input) == pack_everything(golden);
}}
"#,
        golden = literal(&golden).unwrap()
    );
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("spade-echo");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("echo.spade"), &spade.code).unwrap();
    std::fs::write(work.join("top.spade"), &top).unwrap();
    let sv = spade.compile(&top).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(work.join("spade.sv"), sv).unwrap();
    let sim = Sim::builder(&work, "echo_top")
        .source(work.join("spade.sv"))
        .build()
        .unwrap();
    let (transport, _clk) = sim.spawn().unwrap();
    let echo: Echo = bind(transport).unwrap();

    assert_eq!(
        echo.golden.read().unwrap(),
        golden,
        "Spade pack disagrees with the Rust decoder"
    );
    echo.input.write(&golden).unwrap();
    assert!(
        echo.matches_golden.read().unwrap(),
        "Spade unpack disagrees with the Rust encoder"
    );

    let mut rng = Rng(SEED);
    for i in 0..200 {
        let v = rng.everything();
        echo.input.write(&v).unwrap();
        assert_eq!(echo.output.read().unwrap(), v, "iteration {i}");
        assert!(
            !echo.matches_golden.read().unwrap(),
            "iteration {i} isn't the golden value"
        );
    }
}

/// The same for the VHDL codec: a VHDL fabric wires `input` to `output`
/// through the generated records, `golden` is a VHDL aggregate built from the
/// Rust value (so reading it checks `pack_*` alone), and `matches_golden`
/// compares the unpacked record with that aggregate (`unpack_*` alone).
#[test]
fn rust_and_vhdl_agree_on_every_bit() {
    let golden = Rng(SEED ^ 0xFFFF).everything();
    let spade = Spade::of::<Echo>().unwrap();
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("vhdl-echo");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("echo_core.sv"), spade.compile_core().unwrap_or_else(|e| panic!("{e}"))).unwrap();
    std::fs::write(work.join("facet_hdl_pkg.vhd"), vhdl_library()).unwrap();
    std::fs::write(work.join("echo.vhd"), spade.vhdl().unwrap()).unwrap();
    let top = format!(
        r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.facet_hdl_pkg.all;
use work.echo_pkg.all;

entity echo_top is
  port (
    clk, rst : in std_logic;
{ports}    irq : out std_logic
  );
end entity;

architecture sim of echo_top is
  signal axi_in  : axi_in_t;
  signal axi_out : axi_out_t;
  signal t       : echo_to_fabric_t;
  signal f       : echo_from_fabric_t;
  constant golden : everything_t := {golden};
begin
{glue}
  regs : entity work.echo_regs port map (clk, rst, axi_in, axi_out, irq, t, f);
  f.output         <= t.input;
  f.golden         <= golden;
  f.matches_golden <= '1' when t.input = golden else '0';
end architecture;
"#,
        ports = vhdl_flat_axi_ports(),
        glue = vhdl_flat_axi_glue(),
        golden = vhdl_literal(&golden).unwrap(),
    );
    std::fs::write(work.join("echo_top.vhd"), top).unwrap();
    let sim = Sim::builder(&work, "echo_top")
        .source(work.join("echo_core.sv"))
        .vhdl(
            [work.join("facet_hdl_pkg.vhd"), work.join("echo.vhd"), work.join("echo_top.vhd")],
            "echo_top",
        )
        .unwrap_or_else(|e| panic!("{e}"))
        .build()
        .unwrap();
    let (transport, _clk) = sim.spawn().unwrap();
    let echo: Echo = bind(transport).unwrap();

    assert_eq!(echo.golden.read().unwrap(), golden, "VHDL pack disagrees with the Rust decoder");
    echo.input.write(&golden).unwrap();
    assert!(echo.matches_golden.read().unwrap(), "VHDL unpack disagrees with the Rust encoder");

    let mut rng = Rng(SEED);
    for i in 0..200 {
        let v = rng.everything();
        echo.input.write(&v).unwrap();
        assert_eq!(echo.output.read().unwrap(), v, "iteration {i}");
        assert!(!echo.matches_golden.read().unwrap(), "iteration {i} isn't the golden value");
    }
}
