//! Declare the HPS<->FPGA boundary once, as a facet type:
//!
//! ```
//! use facet::Facet;
//! use facet_hdl::{FromFabric, ToFabric};
//!
//! #[derive(Facet)]
//! #[repr(u8)]
//! pub enum Pattern { Off, Solid, Blink }
//!
//! #[derive(Facet)]
//! pub struct Leds { pub pattern: Pattern, pub mask: u8 }
//!
//! #[derive(Facet)]
//! pub struct Regs {
//!     pub leds: ToFabric<Leds>,
//!     pub ticks: FromFabric<u32>,
//! }
//!
//! let layout = facet_hdl::Boundary::of::<Regs>().unwrap();
//! assert_eq!(layout.words(), 3);
//! ```
//!
//! That one struct is both what the VHDL generator reads and, after
//! [`bind`], the host's handle to the running fabric.

mod boundary;
pub mod codec;
mod hw;
mod port;

pub use boundary::{Boundary, Direction, FINGERPRINT_WORD, PortDecl};
pub use hw::{HwType, LayoutError};
pub use port::{BindError, FromFabric, PortError, ToFabric, Transport, bind};
