//! Values <-> the bit vector the fabric sees, driven by [`HwType`].
//!
//! Bit 0 of the vector is bit 0 of word 0; the vector is zero-padded up to a
//! whole number of 32-bit words. The VHDL generator emits the mirror image of
//! exactly these rules, so this file is half of the wire format's definition.

use crate::hw::HwType;
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
    }
}

fn decode_into(p: Partial<'static>, ty: &HwType, r: &mut BitReader<'_>) -> Result<Partial<'static>, DecodeError> {
    Ok(match ty {
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

    #[test]
    fn rejects_out_of_range_discriminant() {
        let ty = HwType::of(Mode::SHAPE).unwrap();
        let err = decode::<Mode>(&[3], &ty).unwrap_err();
        assert!(matches!(err, DecodeError::Discriminant { index: 3, .. }), "{err}");
    }
}
