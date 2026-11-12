//! `<boundary>_pkg`: one VHDL type per Rust type plus the packing functions
//! that mirror `facet_hdl::codec` bit for bit.

use crate::names::{NameError, Namespace, enum_literal, payload_field, tag_mark, type_mark, type_stem};
use facet_hdl::{Boundary, HwType};

/// Type declaration and the two conversion bodies for a VHDL enumeration;
/// the caller declares the functions' headers.
fn enumeration(head: &mut String, body: &mut String, mark: &str, width: &str, lits: &[String]) {
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
            format!(
                "{}({src}({hi} downto {lo}))",
                if *signed { "signed" } else { "unsigned" }
            )
        }
        _ => format!("to_{}({src}({hi} downto {lo}))", type_mark(ty)),
    }
}

/// Every type needing a declaration, children before parents, deduplicated
/// by VHDL name (named types were already checked for clashes by
/// [`Boundary::named_types`], so equal names mean equal types).
fn declarations(b: &Boundary) -> Vec<&HwType> {
    fn walk<'a>(ty: &'a HwType, out: &mut Vec<&'a HwType>) {
        if matches!(ty, HwType::Bool | HwType::Int { .. } | HwType::Unit) {
            return;
        }
        ty.children().into_iter().for_each(|c| walk(c, out));
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
                enumeration(&mut head, &mut body, &mark, &width, &lits);
            }
            HwType::Tagged { variants, .. } => {
                let tag = tag_mark(ty);
                let tag_width = format!("{st}_tag_width");
                let tw = ty.tag_width();
                ns.claim(&tag, format!("the tag of {origin}"))?;
                ns.claim(&tag_width, format!("the tag width of {origin}"))?;
                ns.claim(&format!("to_{tag}"), format!("the tag decoder of {origin}"))?;
                let lits: Vec<_> = variants.iter().map(|(v, _)| enum_literal(ty, v)).collect();
                for ((v, _), l) in variants.iter().zip(&lits) {
                    ns.claim(l, format!("variant `{v}` of {origin}"))?;
                }
                writeln!(head, "  constant {tag_width} : natural := {tw};").unwrap();
                enumeration(&mut head, &mut body, &tag, &tag_width, &lits);
                writeln!(head, "  function to_slv(v : {tag}) return std_logic_vector;").unwrap();
                writeln!(head, "  function to_{tag}(s : std_logic_vector) return {tag};\n").unwrap();

                // A record, not a union: VHDL has none. Every payload field is
                // decoded from the same bits, and only the one `tag` names is
                // meaningful, like reading the wrong arm of a C union.
                let mut record_scope = Namespace::default();
                record_scope.claim("tag", format!("the tag field of {origin}"))?;
                writeln!(head, "  type {mark} is record\n    tag : {tag};").unwrap();
                let (mut arms, mut unpacks) = (String::new(), String::new());
                for ((v, payload), lit) in variants.iter().zip(&lits) {
                    let Some(p) = payload else { continue };
                    let field = payload_field(v);
                    record_scope.claim(&field, format!("the payload of variant `{v}` of {origin}"))?;
                    writeln!(head, "    {field} : {};", type_mark(p)).unwrap();
                    let (l, h) = (tw.to_string(), (tw + p.width() - 1).to_string());
                    writeln!(
                        arms,
                        "      when {lit} =>\n        {}",
                        pack("r", &l, &h, &format!("v.{field}"), p)
                    )
                    .unwrap();
                    writeln!(unpacks, "    v.{field} := {};", unpack("n", &l, &h, p)).unwrap();
                }
                writeln!(head, "  end record;").unwrap();
                writeln!(
                    body,
                    "  function to_slv(v : {mark}) return std_logic_vector is\n    variable r : std_logic_vector({width} - 1 downto 0) := (others => '0');\n  begin\n    r({tag_width} - 1 downto 0) := to_slv(v.tag);\n    case v.tag is\n{arms}      when others => null;\n    end case;\n    return r;\n  end function;\n"
                )
                .unwrap();
                writeln!(
                    body,
                    "  function to_{mark}(s : std_logic_vector) return {mark} is\n    alias n : std_logic_vector(s'length - 1 downto 0) is s;\n    variable v : {mark};\n  begin\n    v.tag := to_{tag}(n({tag_width} - 1 downto 0));\n{unpacks}    return v;\n  end function;\n"
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
                writeln!(
                    head,
                    "  type {mark} is array (0 to {}) of {};",
                    len - 1,
                    type_mark(elem)
                )
                .unwrap();
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
            HwType::Bool | HwType::Int { .. } | HwType::Unit => unreachable!("not declared"),
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
