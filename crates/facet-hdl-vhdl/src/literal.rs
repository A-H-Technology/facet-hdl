//! A Rust value as a VHDL expression of its generated type, e.g. for reset
//! values or test constants that must agree with the host.

use crate::names::enum_literal;
use facet::Facet;
use facet_hdl::{HwType, LayoutError, codec};

pub fn literal<T: Facet<'static>>(value: &T) -> Result<String, LayoutError> {
    let ty = HwType::of(T::SHAPE)?;
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
        HwType::Bool => if take(1) == 1 { "'1'" } else { "'0'" }.into(),
        HwType::Int { bits, signed } => {
            let digits = (*bits as usize).div_ceil(4);
            let raw = take(*bits);
            format!("{}'({bits}x\"{raw:0digits$X}\")", if *signed { "signed" } else { "unsigned" })
        }
        HwType::Enum { variants, .. } => enum_literal(ty, variants[take(ty.width()) as usize]),
        HwType::Struct { fields, .. } => {
            let parts: Vec<_> = fields.iter().map(|(n, f)| format!("{n} => {}", render(f, words, cursor))).collect();
            format!("({})", parts.join(", "))
        }
        HwType::Array { elem, len } => {
            let parts: Vec<_> = (0..*len).map(|i| format!("{i} => {}", render(elem, words, cursor))).collect();
            format!("({})", parts.join(", "))
        }
    }
}
