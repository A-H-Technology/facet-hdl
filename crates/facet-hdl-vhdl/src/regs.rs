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

use crate::names::{NameError, Namespace, snake, type_mark};
use crate::package::{pack, package_name, unpack};
use facet_hdl::{Boundary, Direction, FINGERPRINT_WORD};
use std::fmt::Write;

pub fn entity_name(b: &Boundary) -> String {
    format!("{}_regs", snake(b.name))
}

pub fn generate(b: &Boundary, ns: &mut Namespace) -> Result<String, NameError> {
    let entity = entity_name(b);
    let pkg = package_name(b);
    let stem = snake(b.name);
    ns.claim(&entity, format!("the register entity for `{}`", b.name))?;
    for p in ["clk", "rst_n", "axi_in", "axi_out"] {
        ns.claim(p, "the bus interface")?;
    }
    for internal in ["word", "s2m"] {
        ns.claim(internal, "register-file internals")?;
    }

    let (mut ports, mut signals, mut outputs, mut resets) =
        (String::new(), String::new(), String::new(), String::new());
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
                let (reg, shadow, written) = (
                    format!("{name}_reg"),
                    format!("{name}_shadow"),
                    format!("{name}_written"),
                );
                ns.claim(&reg, origin("the committed register"))?;
                ns.claim(&written, origin("the commit strobe"))?;
                writeln!(ports, "    -- Host -> fabric, words {}..={last}\n    {name} : out {mark};\n    {written} : out std_logic;", p.word).unwrap();
                writeln!(
                    signals,
                    "  signal {reg} : std_logic_vector({} downto 0) := (others => '0');",
                    w - 1
                )
                .unwrap();
                writeln!(
                    outputs,
                    "  {name} <= {};",
                    unpack(&reg, "0", &(w - 1).to_string(), &p.ty)
                )
                .unwrap();
                writeln!(resets, "          {reg} <= (others => '0');").unwrap();
                writeln!(resets, "          {written} <= '0';").unwrap();
                if p.words > 1 {
                    ns.claim(&shadow, origin("the write buffer"))?;
                    writeln!(
                        signals,
                        "  signal {shadow} : std_logic_vector({} downto 0) := (others => '0');",
                        (p.words - 1) * 32 - 1
                    )
                    .unwrap();
                }
                for k in 0..p.words {
                    let word = p.word + k;
                    let (lo, hi) = (k * 32, k * 32 + 31);
                    if word == last {
                        let prefix = if p.words > 1 {
                            format!("axi_in.wdata & {shadow}")
                        } else {
                            "axi_in.wdata".into()
                        };
                        writeln!(
                            writes,
                            "            when {word} =>\n              {full} := {prefix};\n              {reg} <= {full}({} downto 0);\n              {written} <= '1';",
                            w - 1
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            writes,
                            "            when {word} =>\n              {shadow}({hi} downto {lo}) <= axi_in.wdata;"
                        )
                        .unwrap();
                    }
                    writeln!(
                        reads,
                        "            when {word} =>\n              {full} := (others => '0');\n              {full}({} downto 0) := {reg};\n              s2m.rdata <= {full}({hi} downto {lo});",
                        w - 1
                    )
                    .unwrap();
                }
            }
            Direction::FromFabric => {
                let snap = format!("{name}_snap");
                ns.claim(&snap, origin("the read snapshot"))?;
                writeln!(
                    ports,
                    "    -- Fabric -> host, words {}..={last}\n    {name} : in {mark};",
                    p.word
                )
                .unwrap();
                if p.words > 1 {
                    writeln!(
                        signals,
                        "  signal {snap} : std_logic_vector({span_hi} downto 0) := (others => '0');",
                        span_hi = span - 1
                    )
                    .unwrap();
                }
                for k in 0..p.words {
                    let word = p.word + k;
                    let (lo, hi) = (k * 32, k * 32 + 31);
                    if k == 0 {
                        let mut s = format!(
                            "            when {word} =>\n              {full} := (others => '0');\n              {}\n              s2m.rdata <= {full}(31 downto 0);",
                            pack(&full, "0", &(w - 1).to_string(), name, &p.ty)
                        );
                        if p.words > 1 {
                            write!(s, "\n              {snap} <= {full};").unwrap();
                        }
                        writeln!(reads, "{s}").unwrap();
                    } else {
                        writeln!(
                            reads,
                            "            when {word} =>\n              s2m.rdata <= {snap}({hi} downto {lo});"
                        )
                        .unwrap();
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
use work.facet_hdl_axil_pkg.all;
use work.{pkg}.all;

entity {entity} is
  port (
    clk     : in  std_logic;
    rst_n   : in  std_logic;
    axi_in  : in  axil_m2s_t;
    axi_out : out axil_s2m_t;

{ports}  );
end entity;

architecture rtl of {entity} is
  constant OKAY   : std_logic_vector(1 downto 0) := "00";
  constant SLVERR : std_logic_vector(1 downto 0) := "10";

  signal s2m : axil_s2m_t := axil_s2m_idle;
{signals}begin
  axi_out <= s2m;
{outputs}
  -- One transaction per channel in flight. Ready is raised for exactly one
  -- cycle after valid is seen, and the handshake cycle is where the access
  -- happens, so the response always follows its handshake.
  process (clk)
    variable word : natural;
{vars}  begin
    if rising_edge(clk) then
      s2m.awready <= '0';
      s2m.wready  <= '0';
      s2m.arready <= '0';
{strobe_clear}
      if rst_n = '0' then
        s2m.bvalid <= '0';
        s2m.rvalid <= '0';
{resets}      else
        if s2m.bvalid = '1' then
          if axi_in.bready = '1' then
            s2m.bvalid <= '0';
          end if;
        elsif s2m.awready = '1' then
          word := to_integer(unsigned(axi_in.awaddr(31 downto 2)));
          s2m.bvalid <= '1';
          s2m.bresp <= OKAY;
          case word is
{writes}            when others =>
              s2m.bresp <= SLVERR;
          end case;
        elsif axi_in.awvalid = '1' and axi_in.wvalid = '1' then
          s2m.awready <= '1';
          s2m.wready  <= '1';
        end if;

        if s2m.rvalid = '1' then
          if axi_in.rready = '1' then
            s2m.rvalid <= '0';
          end if;
        elsif s2m.arready = '1' then
          word := to_integer(unsigned(axi_in.araddr(31 downto 2)));
          s2m.rvalid <= '1';
          s2m.rresp <= OKAY;
          case word is
            when {fp_word} =>
              s2m.rdata <= {stem}_fingerprint;
{reads}            when others =>
              s2m.rdata <= (others => '0');
              s2m.rresp <= SLVERR;
          end case;
        elsif axi_in.arvalid = '1' then
          s2m.arready <= '1';
        end if;
      end if;
    end if;
  end process;
end architecture;
"#,
        name = b.name,
        fp_word = FINGERPRINT_WORD,
        strobe_clear = b
            .ports
            .iter()
            .filter(|p| p.direction == Direction::ToFabric)
            .map(|p| format!("      {}_written <= '0';\n", p.name))
            .collect::<String>(),
    ))
}
