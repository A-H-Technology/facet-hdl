//! Rust names -> Spade names. Types, fields and ports keep their Rust names;
//! anonymous types (arrays, `Option`, `Result`, `CallResult`) get a stem for
//! their pack/unpack functions.

use facet_hdl::{HwType, TaggedOrigin};

/// `LedStatus` -> `led_status`.
pub fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The Spade type a value of `ty` is declared with.
pub fn spade_type(ty: &HwType) -> String {
    match ty {
        HwType::Unit => "()".into(),
        HwType::Bool => "bool".into(),
        HwType::Int { bits, signed } => format!("{}<{bits}>", if *signed { "int" } else { "uint" }),
        HwType::Enum { shape, .. } | HwType::Struct { shape, .. } => shape.type_identifier.into(),
        HwType::Array { elem, len } => format!("[{}; {len}]", spade_type(elem)),
        HwType::Tagged { origin, variants } => {
            let payload = |i: usize| variants[i].1.as_ref().map_or("()".into(), spade_type);
            match origin {
                TaggedOrigin::User(shape) => shape.type_identifier.into(),
                TaggedOrigin::Option => format!("Option<{}>", payload(1)),
                TaggedOrigin::Result => format!("Result<{}, {}>", payload(0), payload(1)),
                TaggedOrigin::Call(_) => format!("facet_hdl::CallResult<{}>", payload(0)),
            }
        }
    }
}

/// Names a type's pack/unpack functions: `pack_<stem>`.
pub fn stem(ty: &HwType) -> String {
    match ty {
        HwType::Unit => "unit".into(),
        HwType::Bool => "bool".into(),
        HwType::Int { bits, signed } => format!("{}{bits}", if *signed { 'i' } else { 'u' }),
        HwType::Enum { shape, .. } | HwType::Struct { shape, .. } => snake(shape.type_identifier),
        HwType::Array { elem, len } => format!("{}_array{len}", stem(elem)),
        HwType::Tagged { origin, variants } => {
            let payload = |i: usize| variants[i].1.as_ref().map_or("unit".into(), stem);
            match origin {
                TaggedOrigin::User(shape) => snake(shape.type_identifier),
                TaggedOrigin::Option => format!("option_{}", payload(1)),
                TaggedOrigin::Result => format!("result_{}_{}", payload(0), payload(1)),
                TaggedOrigin::Call(_) => format!("call_{}", payload(0)),
            }
        }
    }
}

/// Spade's own words, which can't name a field, port or type.
const KEYWORDS: &[&str] = &[
    "as", "assert", "clock", "comptime", "decl", "else", "entity", "enum", "extern", "false", "fn", "for", "gen", "if",
    "impl", "initial", "inst", "inv", "let", "match", "memory", "mod", "mut", "pipeline", "port", "pub", "reg",
    "reset", "self", "Self", "set", "stage", "struct", "trait", "true", "type", "unsafe", "use", "where", "wire",
];

pub fn check(ident: &str, origin: &str) -> Result<(), NameError> {
    if KEYWORDS.contains(&ident) {
        return Err(NameError::Keyword {
            ident: ident.to_owned(),
            origin: origin.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum NameError {
    #[error("`{ident}` (from {origin}) is a Spade keyword; rename it")]
    Keyword { ident: String, origin: String },
    #[error("`{ident}` (from {origin}) is already taken in the generated register file; rename the port")]
    Clash { ident: String, origin: String },
}
