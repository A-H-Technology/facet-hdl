//! `<boundary>_regs`: an AXI4-Lite slave exposing the boundary's ports as
//! typed VHDL signals.
//!
//! Protocol, matching `facet_hdl::port`:
//! - Word 0 reads the fingerprint.
//! - A `ToFabric` port buffers every word but its last; writing the last word
//!   commits all of them at once and pulses `<port>_written` for one cycle.
//!   The host always writes words in ascending order.
//! - Reading word 0 of a `FromFabric` port snapshots the whole value; its
//!   later words are served from that snapshot. The host reads in ascending order.
//! - Writes are whole-word only (`wstrb` is ignored). Unmapped addresses, and
//!   writes to `FromFabric` ports, answer SLVERR.

use crate::names::{Namespace, NameError, snake, type_mark};
use crate::package::{pack, package_name, unpack};
use facet_hdl::{Boundary, Direction, FINGERPRINT_WORD};
use std::fmt::Write;

pub fn entity_name(b: &Boundary) -> String {
    format!("{}_regs", snake(b.name))
}

/// Smallest byte-address width that reaches every word of the window.
pub fn min_addr_width(b: &Boundary) -> u32 {
    let bytes = b.words() * 4;
    (u32::BITS - (bytes - 1).leading_zeros()).max(3)
}

const FIXED_PORTS: &[&str] = &[
    "clk", "rst_n", "s_axi_awaddr", "s_axi_awvalid", "s_axi_awready", "s_axi_wdata", "s_axi_wstrb",
    "s_axi_wvalid", "s_axi_wready", "s_axi_bresp", "s_axi_bvalid", "s_axi_bready", "s_axi_araddr",
    "s_axi_arvalid", "s_axi_arready", "s_axi_rdata", "s_axi_rresp", "s_axi_rvalid", "s_axi_rready",
];

pub fn generate(b: &Boundary, ns: &mut Namespace) -> Result<String, NameError> {
    let entity = entity_name(b);
    let pkg = package_name(b);
    let stem = snake(b.name);
    ns.claim(&entity, format!("the register entity for `{}`", b.name))?;
    for p in FIXED_PORTS {
        ns.claim(p, "the AXI4-Lite interface")?;
    }
    for internal in ["word", "awready_i", "wready_i", "bvalid_i", "bresp_i", "arready_i", "rvalid_i", "rresp_i", "rdata_i"] {
        ns.claim(internal, "register-file internals")?;
    }

    let (mut ports, mut signals, mut outputs, mut resets) = (String::new(), String::new(), String::new(), String::new());
    let (mut vars, mut writes, mut reads) = (String::new(), String::new(), String::new());

    for p in &b.ports {
        let name = p.name;
        let origin = |what: &str| format!("{what} of port `{name}`");
        ns.claim(name, origin("the signal"))?;
        let mark = type_mark(&p.ty);
        let w = p.ty.width();
        let full = format!("{name}_full");
        let span = p.words * 32;
        ns.claim(&full, origin("the padded value"))?;
        writeln!(vars, "    variable {full} : std_logic_vector({} downto 0);", span - 1).unwrap();
        let last = p.word + p.words - 1;

        match p.direction {
            Direction::ToFabric => {
                let (reg, shadow, written) = (format!("{name}_reg"), format!("{name}_shadow"), format!("{name}_written"));
                ns.claim(&reg, origin("the committed register"))?;
                ns.claim(&written, origin("the commit strobe"))?;
                writeln!(ports, "    -- Host -> fabric, words {}..={last}\n    {name} : out {mark};\n    {written} : out std_logic;", p.word).unwrap();
                writeln!(signals, "  signal {reg} : std_logic_vector({} downto 0) := (others => '0');", w - 1).unwrap();
                writeln!(outputs, "  {name} <= {};", unpack(&reg, "0", &(w - 1).to_string(), &p.ty)).unwrap();
                writeln!(resets, "          {reg} <= (others => '0');").unwrap();
                writeln!(resets, "          {written} <= '0';").unwrap();
                if p.words > 1 {
                    ns.claim(&shadow, origin("the write buffer"))?;
                    writeln!(signals, "  signal {shadow} : std_logic_vector({} downto 0) := (others => '0');", (p.words - 1) * 32 - 1).unwrap();
                }
                for k in 0..p.words {
                    let word = p.word + k;
                    let (lo, hi) = (k * 32, k * 32 + 31);
                    if word == last {
                        let prefix = if p.words > 1 { format!("s_axi_wdata & {shadow}") } else { "s_axi_wdata".into() };
                        writeln!(
                            writes,
                            "            when {word} =>\n              {full} := {prefix};\n              {reg} <= {full}({} downto 0);\n              {written} <= '1';",
                            w - 1
                        )
                        .unwrap();
                    } else {
                        writeln!(writes, "            when {word} =>\n              {shadow}({hi} downto {lo}) <= s_axi_wdata;").unwrap();
                    }
                    writeln!(
                        reads,
                        "            when {word} =>\n              {full} := (others => '0');\n              {full}({} downto 0) := {reg};\n              rdata_i <= {full}({hi} downto {lo});",
                        w - 1
                    )
                    .unwrap();
                }
            }
            Direction::FromFabric => {
                let snap = format!("{name}_snap");
                ns.claim(&snap, origin("the read snapshot"))?;
                writeln!(ports, "    -- Fabric -> host, words {}..={last}\n    {name} : in {mark};", p.word).unwrap();
                if p.words > 1 {
                    writeln!(signals, "  signal {snap} : std_logic_vector({span_hi} downto 0) := (others => '0');", span_hi = span - 1).unwrap();
                }
                for k in 0..p.words {
                    let word = p.word + k;
                    let (lo, hi) = (k * 32, k * 32 + 31);
                    if k == 0 {
                        let mut s = format!(
                            "            when {word} =>\n              {full} := (others => '0');\n              {}\n              rdata_i <= {full}(31 downto 0);",
                            pack(&full, "0", &(w - 1).to_string(), name, &p.ty)
                        );
                        if p.words > 1 {
                            write!(s, "\n              {snap} <= {full};").unwrap();
                        }
                        writeln!(reads, "{s}").unwrap();
                    } else {
                        writeln!(reads, "            when {word} =>\n              rdata_i <= {snap}({hi} downto {lo});").unwrap();
                    }
                }
            }
        }
    }

    // VHDL separates interface elements rather than terminating them.
    let ports = format!("{}\n", ports.trim_end().trim_end_matches(';'));

    Ok(format!(
        r#"-- Generated by facet-hdl from `{name}`. Do not edit; change the Rust
-- declaration and regenerate.
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.{pkg}.all;

entity {entity} is
  generic (
    -- Byte-address width; wider than the minimum is fine, the full width is decoded.
    ADDR_WIDTH : positive := {addr}
  );
  port (
    clk   : in std_logic;
    rst_n : in std_logic;

    s_axi_awaddr  : in  std_logic_vector(ADDR_WIDTH - 1 downto 0);
    s_axi_awvalid : in  std_logic;
    s_axi_awready : out std_logic;
    s_axi_wdata   : in  std_logic_vector(31 downto 0);
    s_axi_wstrb   : in  std_logic_vector(3 downto 0);
    s_axi_wvalid  : in  std_logic;
    s_axi_wready  : out std_logic;
    s_axi_bresp   : out std_logic_vector(1 downto 0);
    s_axi_bvalid  : out std_logic;
    s_axi_bready  : in  std_logic;
    s_axi_araddr  : in  std_logic_vector(ADDR_WIDTH - 1 downto 0);
    s_axi_arvalid : in  std_logic;
    s_axi_arready : out std_logic;
    s_axi_rdata   : out std_logic_vector(31 downto 0);
    s_axi_rresp   : out std_logic_vector(1 downto 0);
    s_axi_rvalid  : out std_logic;
    s_axi_rready  : in  std_logic;

{ports}  );
end entity;

architecture rtl of {entity} is
  constant OKAY   : std_logic_vector(1 downto 0) := "00";
  constant SLVERR : std_logic_vector(1 downto 0) := "10";

  signal awready_i, wready_i, bvalid_i, arready_i, rvalid_i : std_logic := '0';
  signal bresp_i, rresp_i : std_logic_vector(1 downto 0) := OKAY;
  signal rdata_i : std_logic_vector(31 downto 0) := (others => '0');
{signals}begin
  s_axi_awready <= awready_i;
  s_axi_wready  <= wready_i;
  s_axi_bvalid  <= bvalid_i;
  s_axi_bresp   <= bresp_i;
  s_axi_arready <= arready_i;
  s_axi_rvalid  <= rvalid_i;
  s_axi_rresp   <= rresp_i;
  s_axi_rdata   <= rdata_i;
{outputs}
  -- One transaction per channel in flight. Ready is raised for exactly one
  -- cycle after valid is seen, and the handshake cycle is where the access
  -- happens, so the response always follows its handshake.
  process (clk)
    variable word : natural;
{vars}  begin
    if rising_edge(clk) then
      awready_i <= '0';
      wready_i  <= '0';
      arready_i <= '0';
{strobe_clear}
      if rst_n = '0' then
        bvalid_i <= '0';
        rvalid_i <= '0';
{resets}      else
        if bvalid_i = '1' then
          if s_axi_bready = '1' then
            bvalid_i <= '0';
          end if;
        elsif awready_i = '1' then
          word := to_integer(unsigned(s_axi_awaddr(ADDR_WIDTH - 1 downto 2)));
          bvalid_i <= '1';
          bresp_i <= OKAY;
          case word is
{writes}            when others =>
              bresp_i <= SLVERR;
          end case;
        elsif s_axi_awvalid = '1' and s_axi_wvalid = '1' then
          awready_i <= '1';
          wready_i  <= '1';
        end if;

        if rvalid_i = '1' then
          if s_axi_rready = '1' then
            rvalid_i <= '0';
          end if;
        elsif arready_i = '1' then
          word := to_integer(unsigned(s_axi_araddr(ADDR_WIDTH - 1 downto 2)));
          rvalid_i <= '1';
          rresp_i <= OKAY;
          case word is
            when {fp_word} =>
              rdata_i <= {stem}_fingerprint;
{reads}            when others =>
              rdata_i <= (others => '0');
              rresp_i <= SLVERR;
          end case;
        elsif s_axi_arvalid = '1' then
          arready_i <= '1';
        end if;
      end if;
    end if;
  end process;
end architecture;
"#,
        name = b.name,
        addr = min_addr_width(b),
        fp_word = FINGERPRINT_WORD,
        strobe_clear = b
            .ports
            .iter()
            .filter(|p| p.direction == Direction::ToFabric)
            .map(|p| format!("      {}_written <= '0';\n", p.name))
            .collect::<String>(),
    ))
}
