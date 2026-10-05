//! `<boundary>_pkg`: one VHDL type per Rust type plus the packing functions
//! that mirror `facet_hdl::codec` bit for bit.

use crate::names::{Namespace, NameError, enum_literal, type_mark, type_stem};
use facet_hdl::{Boundary, HwType};
use std::fmt::Write;

/// Statement storing the slv form of `value` into `target(hi downto lo)`.
pub fn pack(target: &str, lo: &str, hi: &str, value: &str, ty: &HwType) -> String {
    match ty {
        HwType::Bool => format!("{target}({lo}) := {value};"),
        HwType::Int { .. } => format!("{target}({hi} downto {lo}) := std_logic_vector({value});"),
        _ => format!("{target}({hi} downto {lo}) := to_slv({value});"),
    }
}

/// Expression reading a `ty` back out of `src(hi downto lo)`.
pub fn unpack(src: &str, lo: &str, hi: &str, ty: &HwType) -> String {
    match ty {
        HwType::Bool => format!("{src}({lo})"),
        HwType::Int { signed, .. } => {
            format!("{}({src}({hi} downto {lo}))", if *signed { "signed" } else { "unsigned" })
        }
        _ => format!("to_{}({src}({hi} downto {lo}))", type_mark(ty)),
    }
}

/// Every type needing a declaration, children before parents, deduplicated
/// by VHDL name (named types were already checked for clashes by
/// [`Boundary::named_types`], so equal names mean equal types).
fn declarations(b: &Boundary) -> Vec<&HwType> {
    fn walk<'a>(ty: &'a HwType, out: &mut Vec<&'a HwType>) {
        match ty {
            HwType::Bool | HwType::Int { .. } => return,
            HwType::Array { elem, .. } => walk(elem, out),
            HwType::Struct { fields, .. } => fields.iter().for_each(|(_, f)| walk(f, out)),
            HwType::Enum { .. } => {}
        }
        if !out.iter().any(|t| type_mark(t) == type_mark(ty)) {
            out.push(ty);
        }
    }
    let mut out = Vec::new();
    for p in &b.ports {
        walk(&p.ty, &mut out);
    }
    out
}

pub fn package_name(b: &Boundary) -> String {
    format!("{}_pkg", crate::names::snake(b.name))
}

pub fn generate(b: &Boundary, ns: &mut Namespace) -> Result<String, NameError> {
    let pkg = package_name(b);
    let stem = crate::names::snake(b.name);
    ns.claim(&pkg, format!("the package for `{}`", b.name))?;
    ns.claim(&format!("{stem}_fingerprint"), "the fingerprint constant")?;
    ns.claim(&format!("{stem}_words"), "the window-size constant")?;

    let decls = declarations(b);
    let mut head = String::new();
    let mut body = String::new();

    for ty in &decls {
        let mark = type_mark(ty);
        let st = type_stem(ty);
        let width = format!("{st}_width");
        let origin = match ty.name() {
            Some(n) => format!("Rust type `{n}`"),
            None => format!("array type `{mark}`"),
        };
        ns.claim(&mark, origin.clone())?;
        ns.claim(&width, format!("the width of {origin}"))?;
        ns.claim(&format!("to_{mark}"), format!("the decoder for {origin}"))?;

        match ty {
            HwType::Enum { variants, .. } => {
                let lits: Vec<_> = variants.iter().map(|v| enum_literal(ty, v)).collect();
                for (v, l) in variants.iter().zip(&lits) {
                    ns.claim(l, format!("variant `{v}` of {origin}"))?;
                }
                writeln!(head, "  type {mark} is ({});", lits.join(", ")).unwrap();
                writeln!(
                    body,
                    "  function to_slv(v : {mark}) return std_logic_vector is\n  begin\n    return std_logic_vector(to_unsigned({mark}'pos(v), {width}));\n  end function;\n"
                )
                .unwrap();
                let mut arms = String::new();
                for (i, l) in lits.iter().enumerate() {
                    writeln!(arms, "      when {i} => return {l};").unwrap();
                }
                writeln!(
                    body,
                    "  -- Out-of-range codes can't come from the host (the Rust side only\n  -- encodes real variants); fold them onto the first variant.\n  function to_{mark}(s : std_logic_vector) return {mark} is\n    alias n : std_logic_vector(s'length - 1 downto 0) is s;\n  begin\n    case to_integer(unsigned(n)) is\n{arms}      when others => return {};\n    end case;\n  end function;\n",
                    lits[0]
                )
                .unwrap();
            }
            HwType::Struct { fields, .. } => {
                writeln!(head, "  type {mark} is record").unwrap();
                for (name, f) in fields {
                    writeln!(head, "    {name} : {};", type_mark(f)).unwrap();
                }
                writeln!(head, "  end record;").unwrap();
                let (mut packs, mut unpacks, mut lo) = (String::new(), String::new(), 0u32);
                let mut record_scope = Namespace::default();
                for (name, f) in fields {
                    record_scope.claim(name, format!("field `{name}` of {origin}"))?;
                    let (l, h) = (lo.to_string(), (lo + f.width() - 1).to_string());
                    writeln!(packs, "    {}", pack("r", &l, &h, &format!("v.{name}"), f)).unwrap();
                    writeln!(unpacks, "    v.{name} := {};", unpack("n", &l, &h, f)).unwrap();
                    lo += f.width();
                }
                writeln!(
                    body,
                    "  function to_slv(v : {mark}) return std_logic_vector is\n    variable r : std_logic_vector({width} - 1 downto 0);\n  begin\n{packs}    return r;\n  end function;\n"
                )
                .unwrap();
                writeln!(
                    body,
                    "  function to_{mark}(s : std_logic_vector) return {mark} is\n    alias n : std_logic_vector(s'length - 1 downto 0) is s;\n    variable v : {mark};\n  begin\n{unpacks}    return v;\n  end function;\n"
                )
                .unwrap();
            }
            HwType::Array { elem, len } => {
                writeln!(head, "  type {mark} is array (0 to {}) of {};", len - 1, type_mark(elem)).unwrap();
                let w = elem.width();
                let (l, h) = (format!("i * {w}"), format!("(i + 1) * {w} - 1"));
                writeln!(
                    body,
                    "  function to_slv(v : {mark}) return std_logic_vector is\n    variable r : std_logic_vector({width} - 1 downto 0);\n  begin\n    for i in 0 to {} loop\n      {}\n    end loop;\n    return r;\n  end function;\n",
                    len - 1,
                    pack("r", &l, &h, "v(i)", elem)
                )
                .unwrap();
                writeln!(
                    body,
                    "  function to_{mark}(s : std_logic_vector) return {mark} is\n    alias n : std_logic_vector(s'length - 1 downto 0) is s;\n    variable v : {mark};\n  begin\n    for i in 0 to {} loop\n      v(i) := {};\n    end loop;\n    return v;\n  end function;\n",
                    len - 1,
                    unpack("n", &l, &h, elem)
                )
                .unwrap();
            }
            HwType::Bool | HwType::Int { .. } => unreachable!("not declared"),
        }
        writeln!(head, "  constant {width} : natural := {};", ty.width()).unwrap();
        writeln!(head, "  function to_slv(v : {mark}) return std_logic_vector;").unwrap();
        writeln!(head, "  function to_{mark}(s : std_logic_vector) return {mark};\n").unwrap();
    }

    let mut ports = String::new();
    for p in &b.ports {
        for (suffix, value) in [("word", p.word), ("words", p.words)] {
            let c = format!("{}_{suffix}", p.name);
            ns.claim(&c, format!("the address constant of port `{}`", p.name))?;
            writeln!(ports, "  constant {c} : natural := {value};").unwrap();
        }
    }

    Ok(format!(
        "-- Generated by facet-hdl from `{name}`. Do not edit; change the Rust\n-- declaration and regenerate.\n--\n-- Layout:\n{layout}\nlibrary ieee;\nuse ieee.std_logic_1164.all;\nuse ieee.numeric_std.all;\n\npackage {pkg} is\n  constant {stem}_fingerprint : std_logic_vector(31 downto 0) := x\"{fp:08x}\";\n  constant {stem}_words : natural := {words};\n\n{head}{ports}end package;\n\npackage body {pkg} is\n{body}end package body;\n",
        name = b.name,
        layout = b.canonical().lines().map(|l| format!("--   {l}\n")).collect::<String>(),
        fp = b.fingerprint(),
        words = b.words(),
    ))
}
