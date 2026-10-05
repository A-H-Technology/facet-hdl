//! VHDL-2008 from a [`facet_hdl::Boundary`]: a package with one type per Rust
//! type, and an AXI4-Lite register file entity whose ports are those types.

mod axil;
mod literal;
mod names;
mod package;
mod regs;

use facet::Facet;
use facet_hdl::{Boundary, LayoutError};
use std::path::{Path, PathBuf};

pub use literal::literal;
pub use names::NameError;
pub use package::package_name;
pub use regs::entity_name;

pub struct Vhdl {
    pub package_name: String,
    pub package: String,
    pub entity_name: String,
    pub entity: String,
}

impl Vhdl {
    pub fn of<B: Facet<'static>>() -> Result<Self, Error> {
        Self::from_boundary(&Boundary::of::<B>()?)
    }

    pub fn from_boundary(b: &Boundary) -> Result<Self, Error> {
        let mut ns = names::Namespace::default();
        for n in axil::NAMES {
            ns.claim(n, "the AXI4-Lite package")?;
        }
        Ok(Self {
            package_name: package_name(b),
            package: package::generate(b, &mut ns)?,
            entity_name: entity_name(b),
            entity: regs::generate(b, &mut ns)?,
        })
    }

    /// Writes the bus package, `<pkg>.vhd` and `<entity>.vhd` into `dir`, in
    /// that (compile) order.
    pub fn write_to(&self, dir: &Path) -> std::io::Result<[PathBuf; 3]> {
        std::fs::create_dir_all(dir)?;
        let files = [
            (axil::PACKAGE_NAME, axil::PACKAGE),
            (&self.package_name, &self.package),
            (&self.entity_name, &self.entity),
        ]
        .map(|(name, text)| (dir.join(format!("{name}.vhd")), text));
        for (path, text) in &files {
            std::fs::write(path, text)?;
        }
        Ok(files.map(|(path, _)| path))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Layout(#[from] LayoutError),
    #[error(transparent)]
    Name(#[from] NameError),
}
