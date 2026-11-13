//! Rust names VHDL can't take are refused when the VHDL is generated, naming
//! the culprit, rather than turning into a ghdl or Vivado error later. Spade
//! generation doesn't care.

use facet::Facet;
use facet_hdl::ToFabric;
use facet_hdl_spade::{Error, Spade};

#[derive(Facet)]
pub struct Wire {
    pub signal: u8,
}

#[derive(Facet)]
pub struct ReservedField {
    pub w: ToFabric<Wire>,
}

#[derive(Facet, Clone, Copy)]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
pub enum Mode {
    Ab,
    AB,
}

#[derive(Facet)]
pub struct CaseClash {
    pub m: ToFabric<Mode>,
}

fn vhdl_error<B: Facet<'static>>() -> String {
    let spade = Spade::of::<B>().expect("Spade doesn't mind these names");
    match spade.vhdl() {
        Err(e @ Error::Name(_)) => e.to_string(),
        Err(e) => panic!("wrong error: {e}"),
        Ok(_) => panic!("generated VHDL that can't compile"),
    }
}

#[test]
fn a_reserved_word_is_refused() {
    let e = vhdl_error::<ReservedField>();
    assert!(e.contains("`signal`") && e.contains("VHDL reserved word"), "{e}");
}

#[test]
fn names_differing_only_in_case_are_refused() {
    let e = vhdl_error::<CaseClash>();
    assert!(e.contains("`AB`") && e.contains("case"), "{e}");
}
