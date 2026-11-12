//! Values <-> the bit vector the fabric sees, driven by [`HwType`].
//!
//! Bit 0 of the vector is bit 0 of word 0; the vector is zero-padded up to a
//! whole number of 32-bit words. The Spade generator emits the mirror image of
//! exactly these rules, so this file is half of the wire format's definition.

use crate::hw::{HwType, TaggedOrigin};
use facet::{Facet, Partial, Peek};

pub const WORD_BITS: u32 = 32;

pub fn words_for(bits: u32) -> u32 {
    bits.div_ceil(WORD_BITS)
}

pub fn encode<T: Facet<'static>>(value: &T, ty: &HwType) -> Vec<u32> {
    let mut w = BitWriter {
        words: vec![0; words_for(ty.width()) as usize],
        cursor: 0,
    };
    encode_peek(Peek::new(value), ty, &mut w);
    w.words
}

pub fn decode<T: Facet<'static>>(words: &[u32], ty: &HwType) -> Result<T, DecodeError> {
    let mut r = BitReader {
        words,
        cursor: 0,
        path: Vec::new(),
    };
    let partial = Partial::alloc::<T>().map_err(reflect)?;
    let partial = decode_into(partial, ty, &mut r)?;
    partial.build().map_err(reflect)?.materialize().map_err(reflect)
}

// `HwType` was resolved from the very shape being peeked, so every downcast
// below matches by construction; a failure is a bug here, not bad input.
fn encode_peek(peek: Peek<'_, 'static>, ty: &HwType, w: &mut BitWriter) {
    const BUG: &str = "HwType out of sync with the shape it was resolved from";
    match ty {
        HwType::Unit => {}
        HwType::Bool => w.push(*peek.get::<bool>().expect(BUG) as u128, 1),
        HwType::Int { bits, signed } => {
            let raw: u128 = match (bits, signed) {
                (8, false) => *peek.get::<u8>().expect(BUG) as u128,
                (16, false) => *peek.get::<u16>().expect(BUG) as u128,
                (32, false) => *peek.get::<u32>().expect(BUG) as u128,
                (64, false) => *peek.get::<u64>().expect(BUG) as u128,
                (128, false) => *peek.get::<u128>().expect(BUG),
                (8, true) => *peek.get::<i8>().expect(BUG) as u8 as u128,
                (16, true) => *peek.get::<i16>().expect(BUG) as u16 as u128,
                (32, true) => *peek.get::<i32>().expect(BUG) as u32 as u128,
                (64, true) => *peek.get::<i64>().expect(BUG) as u64 as u128,
                (128, true) => *peek.get::<i128>().expect(BUG) as u128,
                _ => unreachable!("{BUG}"),
            };
            w.push(raw, *bits);
        }
        HwType::Enum { .. } => {
            let idx = peek.into_enum().expect(BUG).variant_index().expect(BUG);
            w.push(idx as u128, ty.width());
        }
        HwType::Struct { fields, .. } => {
            let s = peek.into_struct().expect(BUG);
            for (i, (_, fty)) in fields.iter().enumerate() {
                encode_peek(s.field(i).expect(BUG), fty, w);
            }
        }
        HwType::Array { elem, len } => {
            let l = peek.into_list_like().expect(BUG);
            for i in 0..*len {
                encode_peek(l.get(i).expect(BUG), elem, w);
            }
        }
        HwType::Tagged { origin, variants } => {
            let (idx, inner) = match origin {
                TaggedOrigin::Option => {
                    let o = peek.into_option().expect(BUG);
                    (usize::from(o.is_some()), o.value())
                }
                TaggedOrigin::Result => {
                    let r = peek.into_result().expect(BUG);
                    if r.is_ok() { (0, r.ok()) } else { (1, r.err()) }
                }
                TaggedOrigin::User(_) | TaggedOrigin::Call(_) => {
                    let e = peek.into_enum().expect(BUG);
                    let idx = e.variant_index().expect(BUG);
                    (idx, e.field(0).expect(BUG))
                }
            };
            w.push(idx as u128, ty.tag_width());
            let start = w.cursor;
            if let (Some(pty), Some(inner)) = (&variants[idx].1, inner) {
                encode_peek(inner, pty, w);
            }
            w.cursor = start + ty.payload_width();
        }
    }
}

fn decode_into(p: Partial<'static>, ty: &HwType, r: &mut BitReader<'_>) -> Result<Partial<'static>, DecodeError> {
    Ok(match ty {
        HwType::Unit => p.set(()).map_err(reflect)?,
        HwType::Bool => p.set(r.take(1) != 0).map_err(reflect)?,
        HwType::Int { bits, signed } => {
            let raw = r.take(*bits);
            match (bits, signed) {
                (8, false) => p.set(raw as u8),
                (16, false) => p.set(raw as u16),
                (32, false) => p.set(raw as u32),
                (64, false) => p.set(raw as u64),
                (128, false) => p.set(raw),
                (8, true) => p.set(raw as u8 as i8),
                (16, true) => p.set(raw as u16 as i16),
                (32, true) => p.set(raw as u32 as i32),
                (64, true) => p.set(raw as u64 as i64),
                (128, true) => p.set(raw as i128),
                _ => unreachable!(),
            }
            .map_err(reflect)?
        }
        HwType::Enum { shape, variants } => {
            let idx = r.take(ty.width()) as usize;
            if idx >= variants.len() {
                return Err(DecodeError::Discriminant {
                    path: r.path.join("."),
                    ty: shape.type_identifier,
                    index: idx,
                });
            }
            p.select_nth_variant(idx).map_err(reflect)?
        }
        HwType::Struct { fields, .. } => {
            let mut p = p;
            for (i, (name, fty)) in fields.iter().enumerate() {
                r.path.push((*name).to_owned());
                p = decode_into(p.begin_nth_field(i).map_err(reflect)?, fty, r)?
                    .end()
                    .map_err(reflect)?;
                r.path.pop();
            }
            p
        }
        HwType::Array { elem, len } => {
            let mut p = p;
            for i in 0..*len {
                r.path.push(format!("[{i}]"));
                p = decode_into(p.begin_nth_field(i).map_err(reflect)?, elem, r)?
                    .end()
                    .map_err(reflect)?;
                r.path.pop();
            }
            p
        }
        HwType::Tagged { origin, variants } => {
            let idx = r.take(ty.tag_width()) as usize;
            let Some((name, payload)) = variants.get(idx) else {
                return Err(DecodeError::Discriminant {
                    path: r.path.join("."),
                    ty: match origin {
                        TaggedOrigin::User(shape) => shape.type_identifier,
                        TaggedOrigin::Option => "Option",
                        TaggedOrigin::Result => "Result",
                        TaggedOrigin::Call(_) => "CallResult",
                    },
                    index: idx,
                });
            };
            let start = r.cursor;
            // `()` payloads were resolved to `None`, but the Rust value still
            // has the field, so it gets built from its default.
            let fill = |p: Partial<'static>, r: &mut BitReader<'_>| -> Result<Partial<'static>, DecodeError> {
                r.path.push((*name).to_owned());
                let p = match payload {
                    Some(pty) => decode_into(p, pty, r)?,
                    None => p.set_default().map_err(reflect)?,
                };
                r.path.pop();
                p.end().map_err(reflect)
            };
            let p = match (origin, idx) {
                (TaggedOrigin::Option, 0) => p.set_default().map_err(reflect)?,
                (TaggedOrigin::Option, _) => fill(p.begin_some().map_err(reflect)?, r)?,
                (TaggedOrigin::Result, 0) => fill(p.begin_ok().map_err(reflect)?, r)?,
                (TaggedOrigin::Result, _) => fill(p.begin_err().map_err(reflect)?, r)?,
                (TaggedOrigin::User(shape) | TaggedOrigin::Call(shape), _) => {
                    let p = p.select_nth_variant(idx).map_err(reflect)?;
                    let has_field = matches!(
                        shape.ty,
                        facet::Type::User(facet::UserType::Enum(en)) if !en.variants[idx].data.fields.is_empty()
                    );
                    if has_field {
                        fill(p.begin_nth_field(0).map_err(reflect)?, r)?
                    } else {
                        p
                    }
                }
            };
            // Bits above a narrower payload are padding; the fabric may leave
            // them dirty, so they're skipped rather than checked.
            r.cursor = start + ty.payload_width();
            p
        }
    })
}

struct BitWriter {
    words: Vec<u32>,
    cursor: u32,
}

impl BitWriter {
    fn push(&mut self, value: u128, width: u32) {
        for bit in 0..width {
            if (value >> bit) & 1 == 1 {
                let at = self.cursor + bit;
                self.words[(at / WORD_BITS) as usize] |= 1 << (at % WORD_BITS);
            }
        }
        self.cursor += width;
    }
}

struct BitReader<'a> {
    words: &'a [u32],
    cursor: u32,
    path: Vec<String>,
}

impl BitReader<'_> {
    fn take(&mut self, width: u32) -> u128 {
        let mut v = 0u128;
        for bit in 0..width {
            let at = self.cursor + bit;
            v |= (((self.words[(at / WORD_BITS) as usize] >> (at % WORD_BITS)) & 1) as u128) << bit;
        }
        self.cursor += width;
        v
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// The fabric drove a variant index the Rust enum doesn't have.
    #[error("fabric sent variant index {index} for `{ty}` at `{path}`, which has no such variant")]
    Discriminant {
        path: String,
        ty: &'static str,
        index: usize,
    },
    #[error("reflection failed while decoding (a facet-hdl bug): {0}")]
    Reflect(String),
}

fn reflect(e: impl std::fmt::Display) -> DecodeError {
    DecodeError::Reflect(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Facet, Debug, PartialEq, Clone)]
    #[repr(u8)]
    enum Mode {
        Off,
        Solid,
        Blink,
    }

    #[derive(Facet, Debug, PartialEq, Clone)]
    struct Mixed {
        on: bool,
        mode: Mode,
        level: i8,
        pair: [u16; 2],
        wide: u64,
    }

    fn roundtrip<T: Facet<'static> + PartialEq + std::fmt::Debug>(v: T) -> Vec<u32> {
        let ty = HwType::of(T::SHAPE).unwrap();
        let words = encode(&v, &ty);
        assert_eq!(words.len() as u32, words_for(ty.width()));
        assert_eq!(decode::<T>(&words, &ty).unwrap(), v);
        words
    }

    #[test]
    fn packs_lsb_first_in_declaration_order() {
        let v = Mixed {
            on: true,
            mode: Mode::Blink,
            level: -1,
            pair: [0xABCD, 0x1234],
            wide: u64::MAX,
        };
        // on(1) mode(2) level(8) pair(32) wide(64) = 107 bits -> 4 words
        let words = roundtrip(v);
        assert_eq!(words[0] & 0b111, 0b101, "on=1 at bit 0, Blink=2 at bits 1..3");
        assert_eq!((words[0] >> 3) & 0xFF, 0xFF, "level=-1 two's complement at bits 3..11");
        assert_eq!((words[0] >> 11) & 0xFFFF, 0xABCD, "pair[0] next");
        assert_eq!(words[3] >> 11, 0, "padding past bit 107 stays zero");
    }

    #[derive(Facet, Debug, PartialEq, Clone)]
    #[repr(u8)]
    enum Cmd {
        Stop,
        Set(Mixed),
        Nudge(i8),
    }

    #[derive(Facet, Debug, PartialEq, Clone)]
    struct Tagged {
        maybe: Option<u8>,
        res: Result<(), Mode>,
        cmds: [Cmd; 2],
        nested: Option<Result<u16, bool>>,
    }

    #[test]
    fn option_is_tag_then_payload() {
        let ty = HwType::of(<Option<u8>>::SHAPE).unwrap();
        assert_eq!(ty.width(), 9);
        assert_eq!(roundtrip(Some(0xA5u8)), [0xA5 << 1 | 1]);
        assert_eq!(roundtrip(None::<u8>), [0]);
    }

    #[test]
    fn unit_payloads_take_no_bits() {
        let ty = HwType::of(<Result<(), Mode>>::SHAPE).unwrap();
        assert_eq!(ty.width(), 1 + 2, "tag + Mode");
        assert_eq!(roundtrip(Ok::<(), Mode>(())), [0]);
        assert_eq!(roundtrip(Err::<(), Mode>(Mode::Blink)), [0b101]);
    }

    #[test]
    fn user_enum_payloads_share_one_area() {
        let ty = HwType::of(Cmd::SHAPE).unwrap();
        assert_eq!(ty.payload_width(), 107, "Mixed is the widest payload");
        assert_eq!(ty.width(), 2 + 107);
        // Nudge(-1): tag 2, then i8 0xFF, then zeros up to bit 109.
        let words = roundtrip(Cmd::Nudge(-1));
        assert_eq!(words, [0b10 | 0xFF << 2, 0, 0, 0]);
        roundtrip(Cmd::Stop);
        roundtrip(Cmd::Set(Mixed {
            on: true,
            mode: Mode::Solid,
            level: -100,
            pair: [1, 2],
            wide: 0xDEAD_BEEF_0000_0001,
        }));
    }

    #[test]
    fn tagged_nests_anywhere() {
        roundtrip(Tagged {
            maybe: Some(7),
            res: Err(Mode::Off),
            cmds: [Cmd::Nudge(3), Cmd::Stop],
            nested: Some(Err(true)),
        });
        roundtrip(Tagged {
            maybe: None,
            res: Ok(()),
            cmds: [Cmd::Stop, Cmd::Nudge(-128)],
            nested: Some(Ok(0xBEEF)),
        });
    }

    #[test]
    fn padding_above_a_short_payload_is_ignored() {
        let ty = HwType::of(Cmd::SHAPE).unwrap();
        let mut words = encode(&Cmd::Nudge(5), &ty);
        words[3] |= 1 << 20;
        assert_eq!(decode::<Cmd>(&words, &ty).unwrap(), Cmd::Nudge(5));
    }

    #[test]
    fn rejects_out_of_range_tag() {
        let ty = HwType::of(Cmd::SHAPE).unwrap();
        let err = decode::<Cmd>(&[3, 0, 0, 0], &ty).unwrap_err();
        assert!(
            matches!(
                err,
                DecodeError::Discriminant {
                    index: 3,
                    ty: "Cmd",
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn rejects_multi_field_variants() {
        #[derive(Facet)]
        #[repr(u8)]
        #[allow(dead_code)]
        enum Two {
            A(u8, u8),
            B,
        }
        let err = HwType::of(Two::SHAPE).unwrap_err().to_string();
        assert!(err.contains("wrap them in one struct"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_discriminant() {
        let ty = HwType::of(Mode::SHAPE).unwrap();
        let err = decode::<Mode>(&[3], &ty).unwrap_err();
        assert!(matches!(err, DecodeError::Discriminant { index: 3, .. }), "{err}");
    }
}
