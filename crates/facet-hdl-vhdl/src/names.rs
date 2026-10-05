//! Rust names -> VHDL identifiers, and the one namespace they all share.

use facet_hdl::HwType;
use std::collections::HashMap;

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

/// Type mark used wherever a value of `ty` is declared.
pub fn type_mark(ty: &HwType) -> String {
    match ty {
        HwType::Bool => "std_logic".into(),
        HwType::Int { bits, signed } => {
            format!("{}({} downto 0)", if *signed { "signed" } else { "unsigned" }, bits - 1)
        }
        HwType::Enum { .. } | HwType::Struct { .. } | HwType::Array { .. } => {
            format!("{}_t", type_stem(ty))
        }
    }
}

/// The part of a declared type's name before `_t`; also names its width constant.
pub fn type_stem(ty: &HwType) -> String {
    match ty {
        HwType::Bool => "bool".into(),
        HwType::Int { bits, signed } => format!("{}{bits}", if *signed { 'i' } else { 'u' }),
        HwType::Enum { shape, .. } | HwType::Struct { shape, .. } => snake(shape.type_identifier),
        HwType::Array { elem, len } => format!("{}_array{len}", type_stem(elem)),
    }
}

pub fn enum_literal(ty: &HwType, variant: &str) -> String {
    format!("{}_{}", type_stem(ty), snake(variant))
}

/// VHDL is case-insensitive and has one flat namespace per design unit, so a
/// Rust field `status` and type `Status` would collide; every generated name
/// goes through here and a clash is reported with both origins.
#[derive(Default)]
pub struct Namespace {
    taken: HashMap<String, String>,
}

impl Namespace {
    pub fn claim(&mut self, ident: &str, origin: impl Into<String>) -> Result<(), NameError> {
        let origin = origin.into();
        check_ident(ident, &origin)?;
        let key = ident.to_ascii_lowercase();
        match self.taken.get(&key) {
            Some(prev) if *prev != origin => Err(NameError::Clash {
                ident: ident.to_owned(),
                first: prev.clone(),
                second: origin,
            }),
            _ => {
                self.taken.insert(key, origin);
                Ok(())
            }
        }
    }
}

fn check_ident(ident: &str, origin: &str) -> Result<(), NameError> {
    let bad = |why: &str| NameError::Invalid { ident: ident.to_owned(), origin: origin.to_owned(), why: why.to_owned() };
    if !ident.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return Err(bad("must start with a letter"));
    }
    if !ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(bad("only letters, digits and `_` are allowed"));
    }
    if ident.contains("__") || ident.ends_with('_') {
        return Err(bad("VHDL forbids `__` and a trailing `_`"));
    }
    if RESERVED.contains(&ident.to_ascii_lowercase().as_str()) {
        return Err(bad("reserved word in VHDL-2008"));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum NameError {
    #[error("`{ident}` (from {origin}) is not a legal VHDL identifier: {why}")]
    Invalid { ident: String, origin: String, why: String },
    #[error("`{ident}` would be declared twice in VHDL: by {first} and by {second}")]
    Clash { ident: String, first: String, second: String },
}

const RESERVED: &[&str] = &[
    "abs", "access", "after", "alias", "all", "and", "architecture", "array", "assert", "assume",
    "assume_guarantee", "attribute", "begin", "block", "body", "buffer", "bus", "case", "component",
    "configuration", "constant", "context", "cover", "default", "disconnect", "downto", "else",
    "elsif", "end", "entity", "exit", "fairness", "file", "for", "force", "function", "generate",
    "generic", "group", "guarded", "if", "impure", "in", "inertial", "inout", "is", "label",
    "library", "linkage", "literal", "loop", "map", "mod", "nand", "new", "next", "nor", "not",
    "null", "of", "on", "open", "or", "others", "out", "package", "parameter", "port", "postponed",
    "procedure", "process", "property", "protected", "pure", "range", "record", "register",
    "reject", "release", "rem", "report", "restrict", "restrict_guarantee", "return", "rol", "ror",
    "select", "sequence", "severity", "shared", "signal", "sla", "sll", "sra", "srl", "strong",
    "subtype", "then", "to", "transport", "type", "unaffected", "units", "until", "use",
    "variable", "vmode", "vprop", "vunit", "wait", "when", "while", "with", "xnor", "xor",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_cases_type_names() {
        assert_eq!(snake("LedStatus"), "led_status");
        assert_eq!(snake("Blinky"), "blinky");
    }

    #[test]
    fn catches_case_insensitive_clashes_and_keywords() {
        let mut ns = Namespace::default();
        ns.claim("status", "port `status`").unwrap();
        assert!(ns.claim("STATUS", "type `Status`").is_err());
        assert!(ns.claim("next", "field `next`").is_err());
        assert!(ns.claim("a__b", "field `a__b`").is_err());
    }
}
