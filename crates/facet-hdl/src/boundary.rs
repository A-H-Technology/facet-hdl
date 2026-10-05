//! A boundary declaration resolved into a register map.

use crate::codec::words_for;
use crate::hw::{HwType, LayoutError};
use crate::port::{FromFabric, ToFabric};
use facet::{ConstTypeId, Facet, Shape, Type, UserType};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

/// Word 0 of every boundary is a read-only hash of its layout, so a host
/// built from one declaration refuses to talk to a bitstream built from another.
pub const FINGERPRINT_WORD: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Host writes, fabric reads.
    ToFabric,
    /// Fabric drives, host reads.
    FromFabric,
}

#[derive(Debug, Clone)]
pub struct PortDecl {
    pub name: &'static str,
    pub direction: Direction,
    pub ty: Arc<HwType>,
    /// Index of the port's first 32-bit word in the register window.
    pub word: u32,
    pub words: u32,
}

#[derive(Debug, Clone)]
pub struct Boundary {
    pub name: &'static str,
    pub ports: Vec<PortDecl>,
}

impl Boundary {
    pub fn of<B: Facet<'static>>() -> Result<Self, LayoutError> {
        Self::of_shape(B::SHAPE)
    }

    pub fn of_shape(shape: &'static Shape) -> Result<Self, LayoutError> {
        let Type::User(UserType::Struct(st)) = shape.ty else {
            return Err(LayoutError::NotABoundary(shape.to_string()));
        };
        let to = ToFabric::<()>::SHAPE.decl_id;
        let from = FromFabric::<()>::SHAPE.decl_id;

        let mut word = FINGERPRINT_WORD + 1;
        let mut ports = Vec::with_capacity(st.fields.len());
        for f in st.fields {
            let fs = f.shape();
            let direction = match fs.decl_id {
                d if d == to => Direction::ToFabric,
                d if d == from => Direction::FromFabric,
                _ => return Err(LayoutError::NotAPort(f.name.to_owned())),
            };
            let payload = fs.type_params[0].shape;
            let ty = HwType::of(payload).map_err(|e| match e {
                LayoutError::Unsupported { path, ty, why } => LayoutError::Unsupported {
                    path: if path.is_empty() {
                        f.name.to_owned()
                    } else {
                        format!("{}.{path}", f.name)
                    },
                    ty,
                    why,
                },
                e => e,
            })?;
            let words = words_for(ty.width());
            ports.push(PortDecl {
                name: f.name,
                direction,
                ty: Arc::new(ty),
                word,
                words,
            });
            word += words;
        }
        if ports.is_empty() {
            return Err(LayoutError::NotABoundary(shape.to_string()));
        }
        let boundary = Self {
            name: shape.type_identifier,
            ports,
        };
        boundary.named_types()?;
        Ok(boundary)
    }

    /// Total size of the register window, fingerprint included.
    pub fn words(&self) -> u32 {
        self.ports.last().map_or(1, |p| p.word + p.words)
    }

    /// FNV-1a over the canonical layout text: port order, names, directions,
    /// addresses and full structural types all feed in.
    pub fn fingerprint(&self) -> u32 {
        self.canonical()
            .bytes()
            .fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
    }

    pub fn canonical(&self) -> String {
        let mut s = String::new();
        for p in &self.ports {
            let _ = writeln!(s, "{}@{}+{}:{:?}:{}", p.name, p.word, p.words, p.direction, p.ty);
        }
        s
    }

    /// Every named type (struct/enum) the ports reach, deduplicated by type id
    /// and ordered so each comes after everything it contains. Same scheme as
    /// facet-zod's registry + toposort; generated declarations can be emitted
    /// in this order without forward references.
    pub fn named_types(&self) -> Result<Vec<&HwType>, LayoutError> {
        let mut out = Vec::new();
        let mut seen: HashMap<ConstTypeId, ()> = HashMap::new();
        let mut names: HashMap<&'static str, ConstTypeId> = HashMap::new();
        for p in &self.ports {
            collect(&p.ty, &mut out, &mut seen, &mut names)?;
        }
        Ok(out)
    }
}

fn collect<'a>(
    ty: &'a HwType,
    out: &mut Vec<&'a HwType>,
    seen: &mut HashMap<ConstTypeId, ()>,
    names: &mut HashMap<&'static str, ConstTypeId>,
) -> Result<(), LayoutError> {
    match ty {
        HwType::Bool | HwType::Int { .. } => Ok(()),
        HwType::Array { elem, .. } => collect(elem, out, seen, names),
        HwType::Enum { shape, .. } | HwType::Struct { shape, .. } => {
            if seen.insert(shape.id, ()).is_some() {
                return Ok(());
            }
            if let Some(prev) = names.insert(shape.type_identifier, shape.id)
                && prev != shape.id
            {
                return Err(LayoutError::NameClash(shape.type_identifier.to_owned()));
            }
            if let HwType::Struct { fields, .. } = ty {
                for (_, f) in fields {
                    collect(f, out, seen, names)?;
                }
            }
            out.push(ty);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Facet)]
    #[repr(u8)]
    enum Mode {
        A,
        B,
    }

    #[derive(Facet)]
    struct Inner {
        mode: Mode,
    }

    #[derive(Facet)]
    struct Outer {
        inner: Inner,
        modes: [Mode; 3],
        wide: u64,
    }

    #[derive(Facet)]
    struct Regs {
        ctl: ToFabric<Outer>,
        flag: FromFabric<bool>,
    }

    #[derive(Facet)]
    struct NotAllPorts {
        ctl: ToFabric<u8>,
        oops: u8,
    }

    #[derive(Facet)]
    struct Bad {
        name: ToFabric<String>,
    }

    #[test]
    fn lays_out_after_the_fingerprint() {
        let b = Boundary::of::<Regs>().unwrap();
        // Outer = 1 + 3*1 + 64 = 68 bits = 3 words
        assert_eq!((b.ports[0].word, b.ports[0].words), (1, 3));
        assert_eq!((b.ports[1].word, b.ports[1].words), (4, 1));
        assert_eq!(b.words(), 5);
    }

    #[test]
    fn named_types_come_after_their_contents() {
        let b = Boundary::of::<Regs>().unwrap();
        let names: Vec<_> = b.named_types().unwrap().iter().map(|t| t.name().unwrap()).collect();
        assert_eq!(names, ["Mode", "Inner", "Outer"]);
    }

    #[test]
    fn rejects_non_ports_and_unencodable_payloads() {
        assert!(matches!(Boundary::of::<NotAllPorts>(), Err(LayoutError::NotAPort(f)) if f == "oops"));
        let err = Boundary::of::<Bad>().unwrap_err().to_string();
        assert!(err.contains("`name`"), "{err}");
    }
}
