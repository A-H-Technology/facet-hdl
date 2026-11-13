//! `<boundary>_core`: the register file with every port flattened to plain
//! wires, so fabric written in Verilog or VHDL can instantiate it like any
//! other module. Payloads are the packed bits `facet_hdl::codec` defines
//! (first field at the LSB), and queues are valid/ready streams:
//!
//! - host -> fabric queue `q`: `q_valid`, `q_data` out, `q_ready` in. The
//!   entry is taken on a clock edge where both are high.
//! - fabric -> host queue `q`: `q_valid`, `q_data` in, `q_ready` out. Same
//!   rule; `q_ready` is low while the queue is full or in reset.
//! - host -> fabric register `r`: `r_data` out, plus `r_written`, high for the one
//!   cycle after the host commits a new value.
//! - fabric -> host register `r`: `r_data` in, read whenever the host asks.
//!
//! A `()` queue has no `_data`.

use crate::names::{NameError, check, snake};
use crate::types::{pack, unpack};
use crate::{FLAT_AXI_GLUE, FLAT_AXI_PORTS};
use facet_hdl::{Boundary, Direction, PortKind};
use std::collections::HashSet;
use std::fmt::Write;

pub fn core_name(b: &Boundary) -> String {
    format!("{}_core", snake(b.name))
}

/// A port of the flat module. `width: None` is a single bit (`bool` in Spade,
/// `std_logic` in VHDL); `Some(w)` is a `w`-bit vector.
#[derive(Clone)]
pub(crate) struct Pin {
    pub name: String,
    pub output: bool,
    pub width: Option<u32>,
}

/// The AXI side, mirroring [`FLAT_AXI_PORTS`].
pub(crate) const AXI_PINS: &[(&str, bool, Option<u32>)] = &[
    ("s_axi_awid", false, Some(12)),
    ("s_axi_awaddr", false, Some(32)),
    ("s_axi_awlen", false, Some(8)),
    ("s_axi_awvalid", false, None),
    ("s_axi_awready", true, None),
    ("s_axi_wdata", false, Some(32)),
    ("s_axi_wvalid", false, None),
    ("s_axi_wready", true, None),
    ("s_axi_bid", true, Some(12)),
    ("s_axi_bresp", true, Some(2)),
    ("s_axi_bvalid", true, None),
    ("s_axi_bready", false, None),
    ("s_axi_arid", false, Some(12)),
    ("s_axi_araddr", false, Some(32)),
    ("s_axi_arlen", false, Some(8)),
    ("s_axi_arvalid", false, None),
    ("s_axi_arready", true, None),
    ("s_axi_rid", true, Some(12)),
    ("s_axi_rdata", true, Some(32)),
    ("s_axi_rresp", true, Some(2)),
    ("s_axi_rlast", true, None),
    ("s_axi_rvalid", true, None),
    ("s_axi_rready", false, None),
];

/// What [`generate`] gives: the Spade entity, and every port of the module
/// it compiles to.
pub(crate) struct Core {
    pub name: String,
    pub spade: String,
    pub pins: Vec<Pin>,
}

/// `module` is the module the boundary's Spade is compiled as.
pub(crate) fn generate(b: &Boundary, module: &str) -> Result<Core, NameError> {
    // The codec functions live in the boundary's module, not where this
    // entity is compiled.
    let qualify = |e: String| match e.starts_with("pack_") || e.starts_with("unpack_") {
        true => format!("{module}::{e}"),
        false => e,
    };
    let pack = |v: &str, ty| qualify(pack(v, ty));
    let unpack = |v: &str, ty| qualify(unpack(v, ty));
    let name = core_name(b);
    let mut taken: HashSet<String> = ["clk", "rst", "irq", "axi", "axi_out", "axi_out_inv", "o", "p"]
        .into_iter()
        .map(String::from)
        .chain(AXI_PINS.iter().map(|(n, ..)| n.to_string()))
        .collect();
    let mut pins: Vec<Pin> = Vec::new();
    let mut body = String::new();
    let mut pin = |name: String, output: bool, width: Option<u32>, origin: &str| -> Result<String, NameError> {
        check(&name, origin)?;
        if !taken.insert(name.clone()) {
            return Err(NameError::Clash {
                ident: name,
                origin: origin.to_owned(),
            });
        }
        pins.push(Pin {
            name: name.clone(),
            output,
            width,
        });
        Ok(name)
    };

    for p in &b.ports {
        let n = p.name;
        let origin = format!("the flat wires of port `{n}`");
        let w = p.ty.width();
        let data = (w > 0).then_some(w);
        match (&p.kind, &p.direction) {
            (PortKind::Queue { .. }, Direction::ToFabric) => {
                let valid = pin(format!("{n}_valid"), true, None, &origin)?;
                writeln!(body, "    set {valid} = match p.{n}.read {{ Some(_) => true, None => false }};").unwrap();
                if let Some(w) = data {
                    let d = pin(format!("{n}_data"), true, Some(w), &origin)?;
                    writeln!(
                        body,
                        "    set {d} = match p.{n}.read {{ Some(v) => {}, None => 0 }};",
                        pack("v", &p.ty)
                    )
                    .unwrap();
                }
                let ready = pin(format!("{n}_ready"), false, None, &origin)?;
                writeln!(body, "    set p.{n}.ack = {ready};").unwrap();
            }
            (PortKind::Queue { .. }, Direction::FromFabric) => {
                let valid = pin(format!("{n}_valid"), false, None, &origin)?;
                let value = match data {
                    Some(w) => unpack(&pin(format!("{n}_data"), false, Some(w), &origin)?, &p.ty),
                    None => "()".into(),
                };
                let ready = pin(format!("{n}_ready"), true, None, &origin)?;
                writeln!(
                    body,
                    "    set p.{n}.write = if {valid} {{ Some({value}) }} else {{ None }};\n    set {ready} = !p.{n}.full;"
                )
                .unwrap();
            }
            (PortKind::Register, Direction::ToFabric) => {
                let value = pin(format!("{n}_data"), true, Some(w), &origin)?;
                let written = pin(format!("{n}_written"), true, None, &origin)?;
                writeln!(
                    body,
                    "    set {value} = {};\n    set {written} = p.{n}_written;",
                    pack(&format!("p.{n}"), &p.ty)
                )
                .unwrap();
            }
            (PortKind::Register, Direction::FromFabric) => {
                let value = pin(format!("{n}_data"), false, Some(w), &origin)?;
                writeln!(body, "    set p.{n} = {};", unpack(&value, &p.ty)).unwrap();
            }
        }
    }

    let flat: String = pins
        .iter()
        .map(|p| {
            let ty = p.width.map_or("bool".into(), |w| format!("uint<{w}>"));
            format!("    {}: {}{ty},\n", p.name, if p.output { "inv " } else { "" })
        })
        .collect();
    let spade = format!(
        r#"// Generated by facet-hdl from `{boundary}`: `{module}_regs` with flat ports,
// for fabric written in Verilog or VHDL. Do not edit.
#[no_mangle(all)]
entity {name}(
    clk: clock,
    rst: bool,
{FLAT_AXI_PORTS}
    irq: inv bool,
{flat}) {{
{FLAT_AXI_GLUE}    let p = inst {module}::{module}_regs(clk, rst, axi);
    set axi_out_inv = p.axi_out;
    set irq = p.irq;
{body}}}
"#,
        boundary = b.name,
    );

    let all: Vec<Pin> = [("clk", false, None), ("rst", false, None)]
        .iter()
        .chain(AXI_PINS)
        .chain(&[("irq", true, None)])
        .map(|&(n, output, width)| Pin {
            name: n.into(),
            output,
            width,
        })
        .chain(pins)
        .collect();
    Ok(Core { name, spade, pins: all })
}
