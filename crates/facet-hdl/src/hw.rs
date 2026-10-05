//! The hardware view of a Rust type: what a [`Shape`] means as a bit vector.

use facet::{ConstTypeId, Facet, NumericType, PrimitiveType, SequenceType, Shape, Type, UserType};
use std::fmt;

/// A type that has a fixed bit-level representation on the fabric.
///
/// Only types where every bit pattern of the encoding is either a valid value
/// or a detectable error can appear here, which is why `char`, floats, slices
/// and data-carrying enums are rejected rather than approximated.
#[derive(Debug, Clone)]
pub enum HwType {
    Bool,
    Int {
        bits: u32,
        signed: bool,
    },
    /// Fieldless enum, encoded as the variant's declaration index.
    Enum {
        shape: &'static Shape,
        variants: Vec<&'static str>,
    },
    /// Fields packed in declaration order, first field at the LSB.
    Struct {
        shape: &'static Shape,
        fields: Vec<(&'static str, HwType)>,
    },
    /// Elements packed in index order, element 0 at the LSB.
    Array {
        elem: Box<HwType>,
        len: usize,
    },
}

impl HwType {
    pub fn of(shape: &'static Shape) -> Result<Self, LayoutError> {
        Self::resolve(shape, &mut Vec::new(), &mut Vec::new())
    }

    /// `open` holds the named types currently being resolved, because a type
    /// that contains itself has no finite width.
    fn resolve(
        shape: &'static Shape,
        path: &mut Vec<String>,
        open: &mut Vec<ConstTypeId>,
    ) -> Result<Self, LayoutError> {
        let unsupported = |path: &Vec<String>, why: &str| LayoutError::Unsupported {
            path: path.join("."),
            ty: shape.to_string(),
            why: why.to_owned(),
        };
        match shape.ty {
            Type::Primitive(PrimitiveType::Boolean) => Ok(Self::Bool),
            Type::Primitive(PrimitiveType::Numeric(NumericType::Integer { signed })) => {
                let bytes = shape
                    .layout
                    .sized_layout()
                    .map_err(|_| unsupported(path, "unsized integer"))?
                    .size();
                if shape.id == usize::SHAPE.id || shape.id == isize::SHAPE.id {
                    return Err(unsupported(
                        path,
                        "usize/isize differ between host and fabric; pick an explicit width",
                    ));
                }
                Ok(Self::Int {
                    bits: bytes as u32 * 8,
                    signed,
                })
            }
            Type::Sequence(SequenceType::Array(arr)) => {
                if arr.n == 0 {
                    return Err(unsupported(path, "zero-length arrays have no bits"));
                }
                path.push("[]".into());
                let elem = Self::resolve(arr.t, path, open)?;
                path.pop();
                Ok(Self::Array {
                    elem: Box::new(elem),
                    len: arr.n,
                })
            }
            Type::User(UserType::Enum(en)) => {
                if let Some(v) = en.variants.iter().find(|v| !v.data.fields.is_empty()) {
                    return Err(unsupported(
                        path,
                        &format!("variant `{}` carries data; only fieldless enums are supported", v.name),
                    ));
                }
                if en.variants.len() < 2 {
                    return Err(unsupported(path, "an enum needs at least two variants to occupy a bit"));
                }
                Ok(Self::Enum {
                    shape,
                    variants: en.variants.iter().map(|v| v.name).collect(),
                })
            }
            Type::User(UserType::Struct(st)) => {
                if open.contains(&shape.id) {
                    return Err(unsupported(path, "a type that contains itself has no finite width"));
                }
                open.push(shape.id);
                let mut fields = Vec::with_capacity(st.fields.len());
                for f in st.fields {
                    path.push(f.name.to_owned());
                    fields.push((f.name, Self::resolve(f.shape(), path, open)?));
                    path.pop();
                }
                open.pop();
                if fields.is_empty() {
                    return Err(unsupported(path, "a struct with no fields has no bits"));
                }
                Ok(Self::Struct { shape, fields })
            }
            _ => Err(unsupported(path, "no fixed-width hardware encoding")),
        }
    }

    pub fn width(&self) -> u32 {
        match self {
            Self::Bool => 1,
            Self::Int { bits, .. } => *bits,
            Self::Enum { variants, .. } => enum_width(variants.len()),
            Self::Struct { fields, .. } => fields.iter().map(|(_, t)| t.width()).sum(),
            Self::Array { elem, len } => elem.width() * *len as u32,
        }
    }

    /// The name this type goes by in generated code, if it is a named type.
    pub fn name(&self) -> Option<&'static str> {
        match self {
            Self::Enum { shape, .. } | Self::Struct { shape, .. } => Some(shape.type_identifier),
            _ => None,
        }
    }
}

pub(crate) fn enum_width(variants: usize) -> u32 {
    usize::BITS - (variants - 1).leading_zeros()
}

/// Canonical structural description; two boundaries that print the same
/// encode the same bits at the same addresses.
impl fmt::Display for HwType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => write!(f, "bool"),
            Self::Int { bits, signed } => write!(f, "{}{bits}", if *signed { 'i' } else { 'u' }),
            Self::Enum { shape, variants } => write!(f, "{}{{{}}}", shape.type_identifier, variants.join(",")),
            Self::Struct { shape, fields } => {
                write!(f, "{}{{", shape.type_identifier)?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{name}:{ty}")?;
                }
                write!(f, "}}")
            }
            Self::Array { elem, len } => write!(f, "[{elem};{len}]"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("`{ty}` at `{path}` can't cross the fabric boundary: {why}")]
    Unsupported { path: String, ty: String, why: String },
    #[error("`{0}` is not a boundary: it must be a struct whose fields are all ports")]
    NotABoundary(String),
    #[error("boundary field `{0}` is not a port (ToFabric, FromFabric, ToFabricQueue or FromFabricQueue)")]
    NotAPort(String),
    #[error("two different types are both called `{0}`; generated code would conflate them")]
    NameClash(String),
    #[error("queue `{port}` has depth {depth}; it must be a power of two from 2 to 65536")]
    QueueDepth { port: &'static str, depth: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_widths() {
        assert_eq!([2, 3, 4, 5, 8, 9].map(enum_width), [1, 2, 2, 3, 3, 4]);
    }
}
