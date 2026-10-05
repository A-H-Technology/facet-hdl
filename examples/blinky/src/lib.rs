//! The whole HPS<->fabric contract for the blinky demo. The VHDL in
//! `hdl/generated/` and the host's handles both come from this file.

use facet::Facet;
use facet_hdl::{FromFabric, PortError, ToFabric};
use std::time::Duration;

#[derive(Facet, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pattern {
    Off,
    Solid,
    Blink,
    Chase,
}

#[derive(Facet, Debug, Clone, PartialEq, Eq)]
pub struct LedControl {
    pub pattern: Pattern,
    /// Which LEDs take part in the pattern.
    pub mask: u8,
    /// Milliseconds per Blink half-period or Chase step.
    pub period_ms: u16,
}

#[derive(Facet, Debug, Clone, PartialEq, Eq)]
pub struct LedStatus {
    pub pattern: Pattern,
    pub leds: u8,
    pub uptime_ms: u64,
}

#[derive(Facet, Debug, Clone, PartialEq, Eq)]
pub struct Operands {
    pub a: u32,
    pub b: u32,
    pub negate: bool,
}

#[derive(Facet, Debug, Clone, PartialEq, Eq)]
pub struct Sum {
    pub value: i64,
    /// Generation counter: bumped together with `value` once the sum for a
    /// new `operands` write is valid. Wait for it to change; see [`add`].
    pub calls: u16,
}

#[derive(Facet)]
pub struct Blinky {
    pub leds: ToFabric<LedControl>,
    pub status: FromFabric<LedStatus>,
    pub operands: ToFabric<Operands>,
    pub sum: FromFabric<Sum>,
}

/// One request/response round trip. Reading `sum` straight after writing
/// `operands` would only be right if the fabric happened to finish within the
/// read's bus latency; waiting for `calls` to move makes it right regardless.
/// Needs the binding to be the only writer of `operands`, which `bind`'s
/// window lock guarantees.
pub fn add(b: &Blinky, operands: &Operands, timeout: Duration) -> Result<Sum, PortError> {
    let before = b.sum.read()?.calls;
    b.operands.write(operands)?;
    b.sum.read_until(timeout, |s| s.calls != before)
}
