//! VHDL for fabric written in VHDL: one VHDL type per Rust type (records,
//! enumerations, arrays, `unsigned`/`signed`), `pack_*`/`unpack_*` functions
//! mirroring `facet_hdl::codec`, and a `<module>_regs` entity around the
//! compiled `<module>_core` whose ports are those types:
//!
//! - `to_fabric`: everything the fabric reads. A host -> fabric queue `q` is
//!   `q_valid` and `q`; a fabric -> host queue's `q_ready`; a host -> fabric
//!   register `r` is `r` and `r_written`.
//! - `from_fabric`: everything the fabric drives. A host -> fabric queue's
//!   `q_ready`; a fabric -> host queue's `q_valid` and `q`; a fabric -> host
//!   register `r`.
//!
//! A tagged type (`Option`, `Result`, a Rust enum with payloads) is a record:
//! `tag`, an enumeration of the variants, and one field per variant that has
//! a payload, named after the variant. Only the field `tag` names is
//! meaningful; the others unpack as zeros.

use crate::core::AXI_PINS;
use crate::names::{NameError, snake, stem};
use crate::types::collect;
use facet::Facet;
use facet_hdl::{Boundary, Direction, HwType, LayoutError, PortKind, codec};
use std::collections::HashSet;
use std::fmt::Write;

/// VHDL-2019's reserved words, which can't name anything.
const KEYWORDS: &[&str] = &[
    "abs", "access", "after", "alias", "all", "and", "architecture", "array", "assert", "assume", "attribute",
    "begin", "block", "body", "buffer", "bus", "case", "component", "configuration", "constant", "context", "cover",
    "default", "disconnect", "downto", "else", "elsif", "end", "entity", "exit", "fairness", "file", "for", "force",
    "function", "generate", "generic", "group", "guarded", "if", "impure", "in", "inertial", "inout", "is", "label",
    "library", "linkage", "literal", "loop", "map", "mod", "nand", "new", "next", "nor", "not", "null", "of", "on",
    "open", "or", "others", "out", "package", "parameter", "port", "postponed", "private", "procedure", "process",
    "property", "protected", "pure", "range", "record", "register", "reject", "release", "rem", "report",
    "restrict", "return", "rol", "ror", "select", "sequence", "severity", "shared", "signal", "sla", "sll", "sra",
    "srl", "strong", "subtype", "then", "to", "transport", "type", "unaffected", "units", "until", "use", "variable",
    "view", "vmode", "vprop", "vunit", "wait", "when", "while", "with", "xnor", "xor",
];

/// A Rust name as a VHDL identifier, or why it can't be one.
fn ident(name: &str, origin: &str) -> Result<String, NameError> {
    let why = if KEYWORDS.contains(&name.to_ascii_lowercase().as_str()) {
        Some("is a VHDL reserved word")
    } else if name.starts_with('_') || name.ends_with('_') || name.contains("__") {
        Some("has a leading, trailing or doubled underscore, which VHDL doesn't allow")
    } else {
        None
    };
    match why {
        Some(why) => Err(NameError::Vhdl {
            ident: name.to_owned(),
            origin: origin.to_owned(),
            why,
        }),
        None => Ok(name.to_owned()),
    }
}

/// VHDL ignores case, so names that differ only in case collide.
struct Scope(HashSet<String>);

/// The codec functions' own names. An enum literal spelled the same would
/// be hidden inside them, so every scope starts with these taken.
const INTERNALS: &[&str] = &["fh_bits", "fh_norm", "fh_value", "fh_zeros"];

impl Default for Scope {
    fn default() -> Self {
        Self(INTERNALS.iter().map(|s| s.to_string()).collect())
    }
}

impl Scope {
    fn claim(&mut self, name: &str, origin: &str) -> Result<String, NameError> {
        let name = ident(name, origin)?;
        if !self.0.insert(name.to_ascii_lowercase()) {
            return Err(NameError::Vhdl {
                ident: name,
                origin: origin.to_owned(),
                why: "collides with another name once case is ignored, as VHDL does",
            });
        }
        Ok(name)
    }
}

pub fn vhdl_type(ty: &HwType) -> String {
    match ty {
        HwType::Bool => "std_logic".into(),
        HwType::Int { bits, signed } => format!("{}({} downto 0)", if *signed { "signed" } else { "unsigned" }, bits - 1),
        HwType::Unit => unreachable!("() has no VHDL type; it's never a field"),
        _ => format!("{}_t", stem(ty)),
    }
}

/// `expr` (of type `ty`) written into bits `hi..=lo` of `target`.
fn put(target: &str, lo: u32, ty: &HwType, expr: &str, op: &str) -> String {
    let w = ty.width();
    match ty {
        _ if w == 0 => String::new(),
        HwType::Bool => format!("{target}({lo}) {op} {expr};"),
        HwType::Int { .. } => format!("{target}({} downto {lo}) {op} std_logic_vector({expr});", lo + w - 1),
        _ => format!("{target}({} downto {lo}) {op} pack_{}({expr});", lo + w - 1, stem(ty)),
    }
}

/// Bits `lo..` of `src` read back as a `ty`.
fn get(src: &str, lo: u32, ty: &HwType) -> String {
    let w = ty.width();
    let hi = (lo + w).wrapping_sub(1);
    match ty {
        _ if w == 0 => format!("unpack_{}(\"\")", stem(ty)),
        HwType::Bool => format!("{src}({lo})"),
        HwType::Int { signed: false, .. } => format!("unsigned({src}({hi} downto {lo}))"),
        HwType::Int { signed: true, .. } => format!("signed({src}({hi} downto {lo}))"),
        _ => format!("unpack_{}({src}({hi} downto {lo}))", stem(ty)),
    }
}

/// Declarations for the package, and function bodies for its body.
#[derive(Default)]
struct Out {
    decls: String,
    bodies: String,
}

fn declare(out: &mut Out, ty: &HwType, globals: &mut Scope) -> Result<(), NameError> {
    let (st, t, w) = (stem(ty), vhdl_type(ty), ty.width());
    let what = format!("the VHDL for `{ty}`");
    globals.claim(&t, &what)?;
    globals.claim(&format!("pack_{st}"), &what)?;
    globals.claim(&format!("unpack_{st}"), &what)?;
    let (mut pack, mut unpack) = (String::new(), String::new());
    match ty {
        HwType::Enum { variants, .. } => {
            let mut lits = Scope::default();
            let names = variants
                .iter()
                .map(|v| lits.claim(v, &format!("a variant of `{ty}`")))
                .collect::<Result<Vec<_>, _>>()?;
            writeln!(out.decls, "  type {t} is ({});", names.join(", ")).unwrap();
            writeln!(pack, "    return std_logic_vector(to_unsigned({t}'pos(fh_value), {w}));").unwrap();
            // Codes past the last variant can't come from the host; fold them
            // onto the first, as the Spade side does.
            writeln!(
                unpack,
                "    if to_integer(unsigned(fh_bits)) < {} then\n      return {t}'val(to_integer(unsigned(fh_bits)));\n    end if;\n    return {};",
                variants.len(),
                names[0]
            )
            .unwrap();
            write_codec(out, &st, &t, w, "", &pack, "", &unpack);
            return Ok(());
        }
        HwType::Struct { fields, .. } => {
            let mut scope = Scope::default();
            writeln!(out.decls, "  type {t} is record").unwrap();
            let mut lo = 0;
            for (name, f) in fields {
                let name = scope.claim(name, &format!("a field of `{ty}`"))?;
                writeln!(out.decls, "    {name} : {};", vhdl_type(f)).unwrap();
                push_line(&mut pack, &put("fh_bits", lo, f, &format!("fh_value.{name}"), ":="));
                writeln!(unpack, "    fh_value.{name} := {};", get("fh_norm", lo, f)).unwrap();
                lo += f.width();
            }
            writeln!(out.decls, "  end record;").unwrap();
        }
        HwType::Array { elem, len } => {
            writeln!(out.decls, "  type {t} is array (0 to {}) of {};", len - 1, vhdl_type(elem)).unwrap();
            for i in 0..*len as u32 {
                push_line(&mut pack, &put("fh_bits", i * elem.width(), elem, &format!("fh_value({i})"), ":="));
                writeln!(unpack, "    fh_value({i}) := {};", get("fh_norm", i * elem.width(), elem)).unwrap();
            }
        }
        HwType::Tagged { variants, .. } => {
            let (tw, n) = (ty.tag_width(), variants.len());
            let tag_t = globals.claim(&format!("{st}_tag_t"), &what)?;
            let mut lits = Scope::default();
            let names = variants
                .iter()
                .map(|(v, _)| lits.claim(v, &format!("a variant of `{ty}`")))
                .collect::<Result<Vec<_>, _>>()?;
            writeln!(out.decls, "  type {tag_t} is ({});", names.join(", ")).unwrap();
            writeln!(out.decls, "  type {t} is record\n    tag : {tag_t};").unwrap();
            let mut fields = Scope::default();
            fields.claim("tag", "")?;
            let mut pack_case = String::new();
            let mut unpack_case = String::new();
            for ((v, payload), lit) in variants.iter().zip(&names) {
                let Some(p) = payload else { continue };
                let f = fields.claim(&snake(v), &format!("the payload field of variant `{v}` of `{ty}`"))?;
                writeln!(out.decls, "    {f} : {};", vhdl_type(p)).unwrap();
                writeln!(unpack, "    fh_value.{f} := {};", get("fh_zeros", tw, p)).unwrap();
                writeln!(pack_case, "      when {lit} =>\n        {}", put("fh_bits", tw, p, &format!("fh_value.{f}"), ":=")).unwrap();
                writeln!(unpack_case, "      when {lit} =>\n        fh_value.{f} := {};", get("fh_norm", tw, p)).unwrap();
            }
            writeln!(out.decls, "  end record;").unwrap();
            let pack_tag = if tw > 0 {
                format!("    fh_bits({} downto 0) := std_logic_vector(to_unsigned({tag_t}'pos(fh_value.tag), {tw}));\n", tw - 1)
            } else {
                String::new()
            };
            pack = format!(
                "{pack_tag}{}",
                if pack_case.is_empty() {
                    String::new()
                } else {
                    format!("    case fh_value.tag is\n{pack_case}      when others =>\n        null;\n    end case;\n")
                }
            );
            let unpack_tag = if tw > 0 {
                format!(
                    "    if to_integer(unsigned(fh_norm({h} downto 0))) < {n} then\n      fh_value.tag := {tag_t}'val(to_integer(unsigned(fh_norm({h} downto 0))));\n    else\n      fh_value.tag := {first};\n    end if;\n",
                    h = tw - 1,
                    first = names[0]
                )
            } else {
                format!("    fh_value.tag := {};\n", names[0])
            };
            unpack = format!(
                "{unpack_tag}{unpack}{}",
                if unpack_case.is_empty() {
                    String::new()
                } else {
                    format!("    case fh_value.tag is\n{unpack_case}      when others =>\n        null;\n    end case;\n")
                }
            );
            let zeros = format!("    constant fh_zeros : std_logic_vector({} downto 0) := (others => '0');\n", w.max(1) - 1);
            write_codec(
                out,
                &st,
                &t,
                w,
                "    variable fh_bits : std_logic_vector(W downto 0) := (others => '0');\n",
                &pack,
                &format!("    alias fh_norm : std_logic_vector(fh_bits'length - 1 downto 0) is fh_bits;\n{zeros}    variable fh_value : {t};\n"),
                &unpack,
            );
            return Ok(());
        }
        HwType::Bool | HwType::Int { .. } | HwType::Unit => unreachable!("no VHDL type of its own"),
    }
    write_codec(
        out,
        &st,
        &t,
        w,
        "    variable fh_bits : std_logic_vector(W downto 0);\n",
        &pack,
        &format!("    alias fh_norm : std_logic_vector(fh_bits'length - 1 downto 0) is fh_bits;\n    variable fh_value : {t};\n"),
        &unpack,
    );
    Ok(())
}

fn push_line(out: &mut String, stmt: &str) {
    if !stmt.is_empty() {
        writeln!(out, "    {stmt}").unwrap();
    }
}

/// `pack_vars` may say `W`, the top bit's index.
#[allow(clippy::too_many_arguments)]
fn write_codec(out: &mut Out, st: &str, t: &str, w: u32, pack_vars: &str, pack: &str, unpack_vars: &str, unpack: &str) {
    let pack_vars = pack_vars.replace("(W downto", &format!("({} downto", w as i64 - 1));
    let ret = if pack_vars.is_empty() { "" } else { "    return fh_bits;\n" };
    writeln!(out.decls, "  function pack_{st}(fh_value : {t}) return std_logic_vector;").unwrap();
    writeln!(out.decls, "  function unpack_{st}(fh_bits : std_logic_vector) return {t};\n").unwrap();
    writeln!(
        out.bodies,
        "  function pack_{st}(fh_value : {t}) return std_logic_vector is\n{pack_vars}  begin\n{pack}{ret}  end function;\n"
    )
    .unwrap();
    writeln!(
        out.bodies,
        "  function unpack_{st}(fh_bits : std_logic_vector) return {t} is\n{unpack_vars}  begin\n{unpack}{}  end function;\n",
        if unpack_vars.is_empty() { "" } else { "    return fh_value;\n" }
    )
    .unwrap();
}

/// `facet_hdl_pkg.vhd`: the bus as two records, shared by every boundary.
pub fn library() -> String {
    let record = |name: &str, output: bool| {
        let fields: String = AXI_PINS
            .iter()
            .filter(|(_, o, _)| *o == output)
            .map(|(n, _, w)| format!("    {} : {};\n", &n["s_axi_".len()..], slv(*w)))
            .collect();
        format!("  type {name} is record\n{fields}  end record;\n")
    };
    format!(
        r#"-- facet-hdl's VHDL library: the AXI4 slave port of every `<boundary>_regs`,
-- as one record each way. The field names are the AXI signal names without
-- `s_axi_`; see `flat_axi_ports`/`flat_axi_glue` in facet-hdl-spade for a
-- top level that has the flat signals.
library ieee;
use ieee.std_logic_1164.all;

package facet_hdl_pkg is
{}
{}end package;
"#,
        record("axi_in_t", false),
        record("axi_out_t", true),
    )
}

fn slv(width: Option<u32>) -> String {
    width.map_or("std_logic".into(), |w| format!("std_logic_vector({} downto 0)", w - 1))
}

/// Ports for a VHDL top level with the flat `s_axi_*` signals, to splice
/// into its port list (each line ends in `;`).
pub fn flat_axi_ports() -> String {
    AXI_PINS
        .iter()
        .map(|(n, o, w)| format!("    {n:<14} : {} {};\n", if *o { "out" } else { "in " }, slv(*w)))
        .collect()
}

/// Concurrent statements joining [`flat_axi_ports`] to the signals
/// `axi_in : axi_in_t` and `axi_out : axi_out_t`.
pub fn flat_axi_glue() -> String {
    AXI_PINS
        .iter()
        .map(|(n, o, _)| {
            let f = &n["s_axi_".len()..];
            if *o {
                format!("  {n} <= axi_out.{f};\n")
            } else {
                format!("  axi_in.{f} <= {n};\n")
            }
        })
        .collect()
}

/// `<module>.vhd`: the package with the types, the codec, the port records
/// and `<module>_core`'s component, then the `<module>_regs` entity.
pub fn generate(b: &Boundary, module: &str, core_pins: &[crate::core::Pin]) -> Result<String, NameError> {
    let mut globals = Scope::default();
    for n in ["axi_in_t", "axi_out_t", "facet_hdl_pkg"] {
        globals.claim(n, "facet_hdl_pkg")?;
    }
    let origin = format!("the VHDL for `{}`", b.name);
    let pkg = globals.claim(&format!("{module}_pkg"), &origin)?;
    let regs = globals.claim(&format!("{module}_regs"), &origin)?;
    let core = globals.claim(&format!("{module}_core"), &origin)?;
    let to_t = globals.claim(&format!("{module}_to_fabric_t"), &origin)?;
    let from_t = globals.claim(&format!("{module}_from_fabric_t"), &origin)?;

    let mut used = Vec::new();
    for p in &b.ports {
        collect(&p.ty, &mut used);
    }
    let mut out = Out::default();
    for ty in used {
        declare(&mut out, ty, &mut globals)?;
    }

    // Record fields each way, and how the wrapper joins each to the core.
    let (mut to, mut from) = (String::new(), String::new());
    let (mut to_scope, mut from_scope) = (Scope::default(), Scope::default());
    let mut map = Vec::new();
    let mut signals = String::new();
    let mut converts = String::new();
    for p in &b.ports {
        let n = p.name;
        let o = format!("the VHDL fields of port `{n}`");
        let data = (p.ty.width() > 0).then(|| vhdl_type(&p.ty));
        let mut field = |to_fabric: bool, name: &str, ty: &str| -> Result<String, NameError> {
            let (rec, scope) = if to_fabric { (&mut to, &mut to_scope) } else { (&mut from, &mut from_scope) };
            let name = scope.claim(name, &o)?;
            writeln!(rec, "    {name} : {ty};").unwrap();
            Ok(format!("{}.{name}", if to_fabric { "to_fabric" } else { "from_fabric" }))
        };
        // The flat pin carrying the payload, through a `_bits` signal.
        let mut bits = |pin: String, value: &str, to_fabric: bool| {
            let sig = format!("{pin}_bits");
            writeln!(signals, "  signal {sig} : {};", slv(Some(p.ty.width()))).unwrap();
            if to_fabric {
                writeln!(converts, "  {value} <= {};", get(&sig, 0, &p.ty)).unwrap();
            } else {
                writeln!(converts, "  {}", put(&sig, 0, &p.ty, value, "<=")).unwrap();
            }
            (pin, sig)
        };
        match (&p.kind, &p.direction) {
            (PortKind::Queue { .. }, Direction::ToFabric) => {
                let valid = field(true, &format!("{n}_valid"), "std_logic")?;
                map.push((format!("{n}_valid"), valid));
                if let Some(t) = &data {
                    let value = field(true, n, t)?;
                    map.push(bits(format!("{n}_data"), &value, true));
                }
                let ready = field(false, &format!("{n}_ready"), "std_logic")?;
                map.push((format!("{n}_ready"), ready));
            }
            (PortKind::Queue { .. }, Direction::FromFabric) => {
                let valid = field(false, &format!("{n}_valid"), "std_logic")?;
                map.push((format!("{n}_valid"), valid));
                if let Some(t) = &data {
                    let value = field(false, n, t)?;
                    map.push(bits(format!("{n}_data"), &value, false));
                }
                let ready = field(true, &format!("{n}_ready"), "std_logic")?;
                map.push((format!("{n}_ready"), ready));
            }
            (PortKind::Register, Direction::ToFabric) => {
                let value = field(true, n, data.as_deref().expect("a register has bits"))?;
                map.push(bits(format!("{n}_data"), &value, true));
                let written = field(true, &format!("{n}_written"), "std_logic")?;
                map.push((format!("{n}_written"), written));
            }
            (PortKind::Register, Direction::FromFabric) => {
                let value = field(false, n, data.as_deref().expect("a register has bits"))?;
                map.push(bits(format!("{n}_data"), &value, false));
            }
        }
    }
    // VHDL has no empty records.
    for rec in [&mut to, &mut from] {
        if rec.is_empty() {
            rec.push_str("    unused : std_logic;\n");
        }
    }

    let component: String = core_pins
        .iter()
        .map(|p| format!("      {:<20} : {} {}", p.name, if p.output { "out" } else { "in " }, slv(p.width)))
        .collect::<Vec<_>>()
        .join(";\n");
    let mut port_map: Vec<String> = vec!["clk => clk".into(), "rst => rst".into()];
    for (n, o, _) in AXI_PINS {
        let f = &n["s_axi_".len()..];
        port_map.push(format!("{n} => axi_{}.{f}", if *o { "out" } else { "in" }));
    }
    port_map.push("irq => irq".into());
    port_map.extend(map.iter().map(|(pin, actual)| format!("{pin} => {actual}")));
    let Out { decls, bodies } = out;
    Ok(format!(
        r#"-- Generated by facet-hdl from `{boundary}`. Do not edit; change the Rust
-- declaration and regenerate.
--
-- Use `{regs}` from VHDL: the bus as `facet_hdl_pkg`'s records, and the
-- boundary's ports as `to_fabric` (what the fabric reads) and `from_fabric`
-- (what it drives). Queues are valid/ready streams: an entry moves on a
-- rising edge where both are '1'. rst is active high and synchronous. Needs
-- `{core}.sv` (the register file itself) and `facet_hdl_pkg.vhd`.
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

package {pkg} is
{decls}  type {to_t} is record
{to}  end record;

  type {from_t} is record
{from}  end record;

  -- The register file with flat ports, for Verilog-minded tops; `{regs}`
  -- below wraps it.
  component {core} is
    port (
{component}
    );
  end component;
end package;

package body {pkg} is
{bodies}end package body;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.facet_hdl_pkg.all;
use work.{pkg}.all;

entity {regs} is
  port (
    clk, rst    : in  std_logic;
    axi_in      : in  axi_in_t;
    axi_out     : out axi_out_t;
    irq         : out std_logic;
    to_fabric   : out {to_t};
    from_fabric : in  {from_t}
  );
end entity;

architecture wrap of {regs} is
{signals}begin
  core : {core} port map (
    {port_map}
  );
{converts}end architecture;
"#,
        boundary = b.name,
        port_map = port_map.join(",\n    "),
    ))
}

/// A Rust value as a VHDL aggregate of its generated type, built from the
/// codec's bits rather than `unpack_*`, so a test constant checks `pack_*`
/// on its own. Fields of variants other than the tagged one are zeros, as
/// `unpack_*` leaves them.
pub fn literal<T: Facet<'static>>(value: &T) -> Result<String, LayoutError> {
    let ty = HwType::of(T::SHAPE)?;
    if matches!(ty, HwType::Unit) {
        return Ok(String::new());
    }
    let words = codec::encode(value, &ty);
    Ok(render(&ty, &words, &mut 0))
}

fn render(ty: &HwType, words: &[u32], cursor: &mut u32) -> String {
    let mut take = |width: u32| {
        let bits: Vec<bool> = (0..width)
            .map(|bit| {
                let at = *cursor + bit;
                (words[(at / 32) as usize] >> (at % 32)) & 1 == 1
            })
            .collect();
        *cursor += width;
        bits
    };
    let num = |bits: &[bool]| bits.iter().rev().fold(0usize, |n, &b| n << 1 | b as usize);
    match ty {
        HwType::Unit => unreachable!("never a field"),
        HwType::Bool => format!("'{}'", take(1)[0] as u8),
        HwType::Int { bits, .. } => {
            let s: String = take(*bits).iter().rev().map(|&b| if b { '1' } else { '0' }).collect();
            format!("\"{s}\"")
        }
        HwType::Enum { variants, .. } => variants[num(&take(ty.width()))].to_string(),
        HwType::Struct { fields, .. } => {
            let parts: Vec<_> = fields.iter().map(|(n, f)| format!("{n} => {}", render(f, words, cursor))).collect();
            format!("({})", parts.join(", "))
        }
        HwType::Array { elem, len } => {
            let parts: Vec<_> = (0..*len).map(|i| format!("{i} => {}", render(elem, words, cursor))).collect();
            format!("({})", parts.join(", "))
        }
        HwType::Tagged { variants, .. } => {
            let tag = num(&take(ty.tag_width()));
            let start = *cursor;
            let mut parts = vec![format!("tag => {}", variants[tag].0)];
            for (i, (v, payload)) in variants.iter().enumerate() {
                let Some(p) = payload else { continue };
                let value = if i == tag {
                    let mut at = start;
                    render(p, words, &mut at)
                } else {
                    let zeros = vec![0u32; p.width().div_ceil(32) as usize + 1];
                    render(p, &zeros, &mut 0)
                };
                parts.push(format!("{} => {value}", snake(v)));
            }
            *cursor = start + ty.payload_width();
            format!("({})", parts.join(", "))
        }
    }
}
