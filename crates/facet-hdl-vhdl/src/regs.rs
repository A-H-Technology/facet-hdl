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
use facet_hdl::{Boundary, Direction, FINGERPRINT_WORD, PortDecl, PortKind, queue};
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
    let mut fabric = String::new();

    for p in &b.ports {
        if let PortKind::Queue { depth } = p.kind {
            let q = queue_port(p, depth, ns)?;
            for (to, from) in [
                (&mut ports, q.ports),
                (&mut signals, q.signals),
                (&mut outputs, q.outputs),
                (&mut resets, q.resets),
                (&mut vars, q.vars),
                (&mut fabric, q.fabric),
                (&mut writes, q.writes),
                (&mut reads, q.reads),
            ] {
                to.push_str(&from);
            }
            continue;
        }
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
{fabric}
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
            .filter(|p| p.direction == Direction::ToFabric && p.kind == PortKind::Register)
            .map(|p| format!("      {}_written <= '0';\n", p.name))
            .collect::<String>(),
    ))
}

/// What one port contributes to each section of the register entity.
#[derive(Default)]
struct Parts {
    ports: String,
    signals: String,
    outputs: String,
    resets: String,
    vars: String,
    /// Statements run every cycle (outside the bus handshake): the fabric
    /// side of a queue pushing or popping.
    fabric: String,
    writes: String,
    reads: String,
}

/// A queue port: `depth` slots of the payload's width, a free-running head
/// and tail, and the counter/entry words at the offsets in
/// [`facet_hdl::queue`]. Full and empty are only ever reported through the
/// counters, never as an AXI error, because on arm64 an error response can
/// surface as an SError that takes the kernel down.
fn queue_port(p: &PortDecl, depth: u32, ns: &mut Namespace) -> Result<Parts, NameError> {
    let mut out = Parts::default();
    let name = p.name;
    let origin = |what: &str| format!("{what} of queue `{name}`");
    let [valid, ready, mem, mem_t, head, tail, full, shadow, snap, incoming] = [
        "valid", "ready", "mem", "mem_t", "head", "tail", "full", "shadow", "snap", "in",
    ]
    .map(|s| format!("{name}_{s}"));
    ns.claim(name, origin("the entry signal"))?;
    for (n, what) in [
        (&valid, "the valid flag"),
        (&ready, "the ready flag"),
        (&mem, "the storage"),
        (&mem_t, "the storage type"),
        (&head, "the producer count"),
        (&tail, "the consumer count"),
        (&full, "the padded entry"),
    ] {
        ns.claim(n, origin(what))?;
    }

    let mark = type_mark(&p.ty);
    let w = p.ty.width();
    let hi = (w - 1).to_string();
    let entries = p.value_words();
    let index_bits = depth.trailing_zeros();
    let idx = |counter: &str| format!("to_integer({counter}({} downto 0))", index_bits - 1);
    let has_room = format!("{head} - {tail} < {depth}");
    let (tail_w, head_w, entry_w) = (p.word + queue::TAIL, p.word + queue::HEAD, p.word + queue::ENTRY);
    let last = entry_w + entries - 1;

    writeln!(
        out.signals,
        "  type {mem_t} is array (0 to {}) of std_logic_vector({hi} downto 0);\n  signal {mem} : {mem_t};\n  signal {head}, {tail} : unsigned(31 downto 0) := (others => '0');",
        depth - 1
    )
    .unwrap();
    writeln!(
        out.vars,
        "    variable {full} : std_logic_vector({} downto 0);",
        entries * 32 - 1
    )
    .unwrap();
    writeln!(
        out.resets,
        "          {head} <= (others => '0');\n          {tail} <= (others => '0');"
    )
    .unwrap();
    for (word, counter) in [(tail_w, &tail), (head_w, &head)] {
        writeln!(
            out.reads,
            "            when {word} =>\n              s2m.rdata <= std_logic_vector({counter});"
        )
        .unwrap();
    }

    match p.direction {
        Direction::ToFabric => {
            writeln!(
                out.ports,
                "    -- Host -> fabric queue of {depth}, words {}..={last}; consumed when valid and ready\n    {name} : out {mark};\n    {valid} : out std_logic;\n    {ready} : in std_logic;",
                p.word
            )
            .unwrap();
            writeln!(
                out.outputs,
                "  {name} <= {};\n  {valid} <= '1' when {head} /= {tail} else '0';",
                unpack(&format!("{mem}({})", idx(&tail)), "0", &hi, &p.ty)
            )
            .unwrap();
            writeln!(
                out.fabric,
                "        if {head} /= {tail} and {ready} = '1' then\n          {tail} <= {tail} + 1;\n        end if;"
            )
            .unwrap();
            if entries > 1 {
                ns.claim(&shadow, origin("the write buffer"))?;
                writeln!(
                    out.signals,
                    "  signal {shadow} : std_logic_vector({} downto 0) := (others => '0');",
                    (entries - 1) * 32 - 1
                )
                .unwrap();
            }
            for k in 0..entries {
                let word = entry_w + k;
                if word == last {
                    let prefix = if entries > 1 {
                        format!("axi_in.wdata & {shadow}")
                    } else {
                        "axi_in.wdata".into()
                    };
                    // A push into a full ring is dropped, not an error: the host
                    // checks room first, and notices a dropped push as a desync.
                    writeln!(
                        out.writes,
                        "            when {word} =>\n              if {has_room} then\n                {full} := {prefix};\n                {mem}({}) <= {full}({hi} downto 0);\n                {head} <= {head} + 1;\n              end if;",
                        idx(&head)
                    )
                    .unwrap();
                } else {
                    writeln!(
                        out.writes,
                        "            when {word} =>\n              {shadow}({} downto {}) <= axi_in.wdata;",
                        k * 32 + 31,
                        k * 32
                    )
                    .unwrap();
                }
                writeln!(
                    out.reads,
                    "            when {word} =>\n              s2m.rdata <= (others => '0');"
                )
                .unwrap();
            }
        }
        Direction::FromFabric => {
            ns.claim(&incoming, origin("the packed incoming entry"))?;
            writeln!(
                out.ports,
                "    -- Fabric -> host queue of {depth}, words {}..={last}; pushed when valid and ready\n    {name} : in {mark};\n    {valid} : in std_logic;\n    {ready} : out std_logic;",
                p.word
            )
            .unwrap();
            writeln!(out.outputs, "  {ready} <= '1' when {has_room} else '0';").unwrap();
            writeln!(out.vars, "    variable {incoming} : std_logic_vector({hi} downto 0);").unwrap();
            writeln!(
                out.fabric,
                "        if {valid} = '1' and {has_room} then\n          {}\n          {mem}({}) <= {incoming};\n          {head} <= {head} + 1;\n        end if;",
                pack(&incoming, "0", &hi, name, &p.ty),
                idx(&head)
            )
            .unwrap();
            // Pop = write the new absolute tail. Anything that isn't a step
            // forward within what's been produced is ignored, so a retried
            // write can't consume a second entry.
            writeln!(
                out.writes,
                "            when {tail_w} =>\n              if unsigned(axi_in.wdata) - {tail} <= {head} - {tail} then\n                {tail} <= unsigned(axi_in.wdata);\n              end if;"
            )
            .unwrap();
            if entries > 1 {
                ns.claim(&snap, origin("the read snapshot"))?;
                writeln!(
                    out.signals,
                    "  signal {snap} : std_logic_vector({} downto 0) := (others => '0');",
                    entries * 32 - 1
                )
                .unwrap();
            }
            for k in 0..entries {
                let word = entry_w + k;
                if k == 0 {
                    let mut s = format!(
                        "            when {word} =>\n              {full} := (others => '0');\n              {full}({hi} downto 0) := {mem}({});\n              s2m.rdata <= {full}(31 downto 0);",
                        idx(&tail)
                    );
                    if entries > 1 {
                        write!(s, "\n              {snap} <= {full};").unwrap();
                    }
                    writeln!(out.reads, "{s}").unwrap();
                } else {
                    writeln!(
                        out.reads,
                        "            when {word} =>\n              s2m.rdata <= {snap}({} downto {});",
                        k * 32 + 31,
                        k * 32
                    )
                    .unwrap();
                }
            }
        }
    }
    Ok(out)
}
