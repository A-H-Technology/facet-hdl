//! A Rust value as a Spade expression of its generated type, built from
//! constructors rather than `unpack_*`, so a test constant checks `pack_*` on
//! its own.

use crate::names::spade_type;
use facet::Facet;
use facet_hdl::{HwType, LayoutError, TaggedOrigin, codec};

pub fn literal<T: Facet<'static>>(value: &T) -> Result<String, LayoutError> {
    let ty = HwType::of(T::SHAPE)?;
    if matches!(ty, HwType::Unit) {
        return Ok("()".into());
    }
    let words = codec::encode(value, &ty);
    let mut cursor = 0;
    Ok(render(&ty, &words, &mut cursor))
}

// Reads back the codec's own bits rather than re-walking the value, so this
// stays a pure function of the wire format.
fn render(ty: &HwType, words: &[u32], cursor: &mut u32) -> String {
    let mut take = |width: u32| {
        let mut v = 0u128;
        for bit in 0..width {
            let at = *cursor + bit;
            v |= (((words[(at / 32) as usize] >> (at % 32)) & 1) as u128) << bit;
        }
        *cursor += width;
        v
    };
    match ty {
        HwType::Unit => "()".into(),
        HwType::Bool => (take(1) == 1).to_string(),
        HwType::Int { bits, signed: false } => format!("{:#x}u{bits}", take(*bits)),
        HwType::Int { bits, signed: true } => format!("({:#x}u{bits}).as_int()", take(*bits)),
        HwType::Enum { shape, variants } => {
            format!("{}::{}", shape.type_identifier, variants[take(ty.width()) as usize])
        }
        HwType::Struct { shape, fields } => {
            let parts: Vec<_> = fields.iter().map(|(_, f)| render(f, words, cursor)).collect();
            format!("{}({})", shape.type_identifier, parts.join(", "))
        }
        HwType::Array { elem, len } => {
            let parts: Vec<_> = (0..*len).map(|_| render(elem, words, cursor)).collect();
            format!("[{}]", parts.join(", "))
        }
        HwType::Tagged { origin, variants } => {
            let tag = take(ty.tag_width()) as usize;
            let start = *cursor;
            let (name, payload) = &variants[tag];
            let inner = payload.as_ref().map(|p| render(p, words, cursor));
            *cursor = start + ty.payload_width();
            let typed = |v: String| format!("{{ let x: {} = {v}; x }}", spade_type(ty));
            match (origin, inner) {
                (TaggedOrigin::User(shape), Some(i)) => format!("{}::{name}({i})", shape.type_identifier),
                (TaggedOrigin::User(shape), None) => format!("{}::{name}", shape.type_identifier),
                // `None` alone doesn't say which `Option` it is.
                (TaggedOrigin::Option, None) => typed("None".into()),
                (TaggedOrigin::Option, Some(i)) => format!("Some({i})"),
                (TaggedOrigin::Result, i) => typed(format!("{name}({})", i.unwrap_or_else(|| "()".into()))),
                (TaggedOrigin::Call(_), i) => typed(match i {
                    Some(i) => format!("facet_hdl::CallResult::{name}({i})"),
                    None if tag == 0 => format!("facet_hdl::CallResult::{name}(())"),
                    None => format!("facet_hdl::CallResult::{name}"),
                }),
            }
        }
    }
}
