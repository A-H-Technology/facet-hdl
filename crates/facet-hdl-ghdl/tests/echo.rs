//! Every encodable shape goes host -> VHDL record -> host unchanged. The
//! fabric side is just a wire from the ToFabric record to the FromFabric one,
//! so any disagreement between `facet_hdl::codec` and the generated VHDL
//! pack/unpack functions shows up as a mismatch.

use facet::Facet;
use facet_hdl::{FromFabric, ToFabric, bind};
use facet_hdl_ghdl::Sim;
use facet_hdl_vhdl::Vhdl;
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
}

#[derive(Facet)]
pub struct Echo {
    pub input: ToFabric<Everything>,
    pub output: FromFabric<Everything>,
    /// A constant on the fabric side, rendered from a Rust value: reading it
    /// checks VHDL packing alone, independent of VHDL unpacking.
    pub golden: FromFabric<Everything>,
    /// Fabric-side `input = golden`: checks VHDL unpacking alone.
    pub matches_golden: FromFabric<bool>,
}

const TOP: &str = r#"
library ieee;
use ieee.std_logic_1164.all;
use work.echo_pkg.all;
use work.echo_golden_pkg.all;

entity echo_top is
  generic (ADDR_WIDTH : positive := 8);
  port (
    clk, rst_n : in std_logic;
    s_axi_awaddr : in std_logic_vector(ADDR_WIDTH - 1 downto 0);
    s_axi_awvalid : in std_logic; s_axi_awready : out std_logic;
    s_axi_wdata : in std_logic_vector(31 downto 0); s_axi_wstrb : in std_logic_vector(3 downto 0);
    s_axi_wvalid : in std_logic; s_axi_wready : out std_logic;
    s_axi_bresp : out std_logic_vector(1 downto 0); s_axi_bvalid : out std_logic; s_axi_bready : in std_logic;
    s_axi_araddr : in std_logic_vector(ADDR_WIDTH - 1 downto 0);
    s_axi_arvalid : in std_logic; s_axi_arready : out std_logic;
    s_axi_rdata : out std_logic_vector(31 downto 0); s_axi_rresp : out std_logic_vector(1 downto 0);
    s_axi_rvalid : out std_logic; s_axi_rready : in std_logic
  );
end entity;

architecture rtl of echo_top is
  signal v : everything_t;
  signal m : std_logic;
begin
  m <= '1' when v = GOLDEN else '0';

  regs : entity work.echo_regs
    generic map (ADDR_WIDTH => ADDR_WIDTH)
    port map (
      clk => clk, rst_n => rst_n,
      s_axi_awaddr => s_axi_awaddr, s_axi_awvalid => s_axi_awvalid, s_axi_awready => s_axi_awready,
      s_axi_wdata => s_axi_wdata, s_axi_wstrb => s_axi_wstrb, s_axi_wvalid => s_axi_wvalid, s_axi_wready => s_axi_wready,
      s_axi_bresp => s_axi_bresp, s_axi_bvalid => s_axi_bvalid, s_axi_bready => s_axi_bready,
      s_axi_araddr => s_axi_araddr, s_axi_arvalid => s_axi_arvalid, s_axi_arready => s_axi_arready,
      s_axi_rdata => s_axi_rdata, s_axi_rresp => s_axi_rresp, s_axi_rvalid => s_axi_rvalid, s_axi_rready => s_axi_rready,
      input => v, input_written => open, output => v,
      golden => GOLDEN, matches_golden => m
    );
end architecture;
"#;

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
        }
    }
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

#[test]
fn rust_and_vhdl_agree_on_every_bit() {
    let golden = Rng(SEED ^ 0xFFFF).everything();
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("echo-sim");
    std::fs::create_dir_all(&work).unwrap();
    let golden_pkg = work.join("echo_golden_pkg.vhd");
    std::fs::write(
        &golden_pkg,
        format!(
            "use work.echo_pkg.all;\nlibrary ieee;\nuse ieee.std_logic_1164.all;\nuse ieee.numeric_std.all;\npackage echo_golden_pkg is\n  constant GOLDEN : everything_t := {};\nend package;\n",
            facet_hdl_vhdl::literal(&golden).unwrap()
        ),
    )
    .unwrap();
    let top = work.join("echo_top.vhd");
    std::fs::write(&top, TOP).unwrap();
    let generated = Vhdl::of::<Echo>().unwrap().write_to(&work.join("gen")).unwrap();
    let sim = Sim::builder(&work, "echo_top", 8)
        .sources(generated)
        .source(&golden_pkg)
        .source(&top)
        .build()
        .unwrap();
    let (transport, _clk) = sim.spawn().unwrap();
    let echo: Echo = bind(transport).unwrap();

    assert_eq!(
        echo.golden.read().unwrap(),
        golden,
        "VHDL to_slv disagrees with the Rust decoder"
    );
    echo.input.write(&golden).unwrap();
    assert!(
        echo.matches_golden.read().unwrap(),
        "VHDL to_<type> disagrees with the Rust encoder"
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
