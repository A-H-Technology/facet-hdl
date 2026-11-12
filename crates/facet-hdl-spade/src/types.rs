//! One Spade type per named Rust type, and `pack_<stem>`/`unpack_<stem>`
//! between each type and the `uint<W>` the wire carries. These mirror
//! `facet_hdl::codec` bit for bit: first field (or element, or the tag) at
//! the LSB, a payload zero-extended to the widest one.

use crate::names::{NameError, check, spade_type, stem};
use facet_hdl::{HwType, TaggedOrigin};
use std::fmt::Write;

/// Spade expression of type `uint<W>` holding `value` (an expression of `ty`).
pub fn pack(value: &str, ty: &HwType) -> String {
    match ty {
        HwType::Unit => unreachable!("() has no bits to pack"),
        HwType::Bool => format!("({value}).to_uint()"),
        HwType::Int { signed: false, .. } => value.to_owned(),
        HwType::Int { signed: true, .. } => format!("({value}).as_uint()"),
        _ => format!("pack_{}({value})", stem(ty)),
    }
}

/// Spade expression of type `ty` read back from `bits` (an expression of
/// type `uint<W>`).
pub fn unpack(bits: &str, ty: &HwType) -> String {
    match ty {
        HwType::Unit => "()".into(),
        HwType::Bool => format!("({bits} == 1)"),
        HwType::Int { signed: false, .. } => bits.to_owned(),
        HwType::Int { signed: true, .. } => format!("({bits}).as_int()"),
        _ => format!("unpack_{}({bits})", stem(ty)),
    }
}

/// `bits` (`uint<from>`) narrowed to `uint<to>` from bit `lo` up, as a
/// statement binding `name`. Same-width cases skip the shift and truncate.
fn slice(name: &str, bits: &str, from: u32, lo: u32, to: u32) -> String {
    if lo == 0 && to == from {
        format!("let {name}: uint<{to}> = {bits};")
    } else if lo == 0 {
        format!("let {name}: uint<{to}> = trunc({bits});")
    } else {
        format!("let {name}: uint<{to}> = trunc({bits} >> {lo});")
    }
}

/// `parts` (low to high) concatenated into one `uint`.
fn concat_low_first(parts: &[String]) -> String {
    let mut it = parts.iter().rev();
    let mut out = it.next().expect("at least one part").clone();
    for p in it {
        out = format!("{out}.concat({p})");
    }
    out
}

/// Every type that needs declaring or a codec, contents before containers,
/// once each.
pub fn collect<'a>(ty: &'a HwType, out: &mut Vec<&'a HwType>) {
    if matches!(ty, HwType::Bool | HwType::Int { .. } | HwType::Unit) {
        return;
    }
    for c in ty.children() {
        collect(c, out);
    }
    if !out.iter().any(|t| stem(t) == stem(ty)) {
        out.push(ty);
    }
}

pub fn generate(types: &[&HwType]) -> Result<String, NameError> {
    let mut out = String::new();
    for ty in types {
        declare(&mut out, ty)?;
        codec(&mut out, ty);
    }
    Ok(out)
}

fn declare(out: &mut String, ty: &HwType) -> Result<(), NameError> {
    match ty {
        HwType::Enum { shape, variants } => {
            check(shape.type_identifier, "a Rust type")?;
            writeln!(out, "pub enum {} {{", shape.type_identifier).unwrap();
            for v in variants {
                check(v, &format!("a variant of `{}`", shape.type_identifier))?;
                writeln!(out, "    {v},").unwrap();
            }
            writeln!(out, "}}\n").unwrap();
        }
        HwType::Struct { shape, fields } => {
            check(shape.type_identifier, "a Rust type")?;
            writeln!(out, "pub struct {} {{", shape.type_identifier).unwrap();
            for (name, f) in fields {
                check(name, &format!("a field of `{}`", shape.type_identifier))?;
                writeln!(out, "    {name}: {},", spade_type(f)).unwrap();
            }
            writeln!(out, "}}\n").unwrap();
        }
        HwType::Tagged {
            origin: TaggedOrigin::User(shape),
            variants,
        } => {
            check(shape.type_identifier, "a Rust type")?;
            writeln!(out, "pub enum {} {{", shape.type_identifier).unwrap();
            for (v, p) in variants {
                check(v, &format!("a variant of `{}`", shape.type_identifier))?;
                match p {
                    Some(p) => writeln!(out, "    {v} {{ value: {} }},", spade_type(p)).unwrap(),
                    None => writeln!(out, "    {v},").unwrap(),
                }
            }
            writeln!(out, "}}\n").unwrap();
        }
        // Arrays, Option, Result and CallResult already exist in Spade.
        _ => {}
    }
    Ok(())
}

/// How a tagged type's variants are written in patterns and constructors.
fn variant_path(origin: &TaggedOrigin, variant: &str) -> String {
    match origin {
        TaggedOrigin::User(shape) => format!("{}::{variant}", shape.type_identifier),
        TaggedOrigin::Option | TaggedOrigin::Result => variant.to_owned(),
        TaggedOrigin::Call(_) => format!("facet_hdl::CallResult::{variant}"),
    }
}

fn codec(out: &mut String, ty: &HwType) {
    let (st, t, w) = (stem(ty), spade_type(ty), ty.width());
    let (mut packs, mut unpacks) = (String::new(), String::new());
    match ty {
        HwType::Enum { shape, variants } => {
            let name = shape.type_identifier;
            writeln!(packs, "    match v {{").unwrap();
            writeln!(unpacks, "    match b {{").unwrap();
            for (i, v) in variants.iter().enumerate() {
                writeln!(packs, "        {name}::{v} => {i},").unwrap();
                writeln!(unpacks, "        {i} => {name}::{v},").unwrap();
            }
            // Out-of-range codes can't come from the host (it only encodes
            // real variants); fold them onto the first.
            writeln!(unpacks, "        _ => {name}::{},", variants[0]).unwrap();
            writeln!(packs, "    }}").unwrap();
            writeln!(unpacks, "    }}").unwrap();
        }
        HwType::Struct { shape, fields } => {
            let mut parts = Vec::new();
            let mut lo = 0;
            let mut args = Vec::new();
            for (i, (name, f)) in fields.iter().enumerate() {
                writeln!(
                    packs,
                    "    let f{i}: uint<{}> = {};",
                    f.width(),
                    pack(&format!("v.{name}"), f)
                )
                .unwrap();
                parts.push(format!("f{i}"));
                writeln!(unpacks, "    {}", slice(&format!("f{i}"), "b", w, lo, f.width())).unwrap();
                args.push(unpack(&format!("f{i}"), f));
                lo += f.width();
            }
            writeln!(packs, "    {}", concat_low_first(&parts)).unwrap();
            writeln!(unpacks, "    {}({})", shape.type_identifier, args.join(", ")).unwrap();
        }
        HwType::Array { elem, len } => {
            let mut parts = Vec::new();
            let mut elems = Vec::new();
            for i in 0..*len {
                let ew = elem.width();
                writeln!(packs, "    let e{i}: uint<{ew}> = {};", pack(&format!("v[{i}]"), elem)).unwrap();
                parts.push(format!("e{i}"));
                writeln!(unpacks, "    {}", slice(&format!("e{i}"), "b", w, i as u32 * ew, ew)).unwrap();
                elems.push(unpack(&format!("e{i}"), elem));
            }
            writeln!(packs, "    {}", concat_low_first(&parts)).unwrap();
            writeln!(unpacks, "    [{}]", elems.join(", ")).unwrap();
        }
        HwType::Tagged { origin, variants } => {
            let (tw, pw) = (ty.tag_width(), ty.payload_width());
            writeln!(packs, "    match v {{").unwrap();
            writeln!(unpacks, "    {}", slice("tag", "b", w, 0, tw)).unwrap();
            if pw > 0 {
                writeln!(unpacks, "    {}", slice("p", "b", w, tw, pw)).unwrap();
            }
            writeln!(unpacks, "    match tag {{").unwrap();
            for (i, (v, payload)) in variants.iter().enumerate() {
                let path = variant_path(origin, v);
                let tag = format!("{i}u{tw}");
                let pad = |inner: &str| {
                    if pw == 0 {
                        tag.clone()
                    } else {
                        format!("{{ let z: uint<{pw}> = {inner}; z.concat({tag}) }}")
                    }
                };
                // A `()` payload is still a field in Spade (`Ok(())`); it
                // just has no bits.
                let has_field = match origin {
                    TaggedOrigin::User(_) => payload.is_some(),
                    TaggedOrigin::Option => i == 1,
                    TaggedOrigin::Result => true,
                    TaggedOrigin::Call(_) => i == 0,
                };
                match payload {
                    Some(p) => {
                        let packed = pack("x", p);
                        let z = if p.width() == pw {
                            packed
                        } else {
                            format!("zext({packed})")
                        };
                        writeln!(packs, "        {path}(x) => {},", pad(&z)).unwrap();
                        let narrow = if p.width() == pw {
                            "p".to_owned()
                        } else {
                            format!("{{ let n: uint<{}> = trunc(p); n }}", p.width())
                        };
                        writeln!(unpacks, "        {i} => {path}({}),", unpack(&narrow, p)).unwrap();
                    }
                    None if has_field => {
                        writeln!(packs, "        {path}(_) => {},", pad("0")).unwrap();
                        writeln!(unpacks, "        {i} => {path}(()),").unwrap();
                    }
                    None => {
                        writeln!(packs, "        {path} => {},", pad("0")).unwrap();
                        writeln!(unpacks, "        {i} => {path},").unwrap();
                    }
                }
            }
            writeln!(packs, "    }}").unwrap();
            // The first variant again, for tags past the last one.
            let first = unpacks
                .lines()
                .find(|l| l.trim_start().starts_with("0 => "))
                .expect("variant 0")
                .trim_start()
                .trim_start_matches("0 => ")
                .to_owned();
            writeln!(unpacks, "        _ => {first}").unwrap();
            writeln!(unpacks, "    }}").unwrap();
        }
        HwType::Bool | HwType::Int { .. } | HwType::Unit => unreachable!("no codec functions"),
    }
    writeln!(out, "pub fn pack_{st}(v: {t}) -> uint<{w}> {{\n{packs}}}\n").unwrap();
    writeln!(out, "pub fn unpack_{st}(b: uint<{w}>) -> {t} {{\n{unpacks}}}\n").unwrap();
}
