//! Register ports (`ToFabric`/`FromFabric`) end to end: multi-word commits,
//! read snapshots, the generation-counter pattern a request/response over
//! plain registers needs, the fingerprint, and window ownership.

use facet::Facet;
use facet_hdl::{BindError, Boundary, FromFabric, HwType, PortError, ToFabric, Transport, bind, codec};
use facet_hdl_iverilog::{Sim, SimClock, SimTransport};
use facet_hdl_spade::{FLAT_AXI_GLUE, FLAT_AXI_PORTS, Spade};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

/// 65 bits: three words, so a write commits on its third.
#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Operands {
    pub a: u32,
    pub b: u32,
    pub negate: bool,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Sum {
    pub value: i64,
    /// Generation counter: bumped together with `value` once the sum for a
    /// new `operands` write is valid.
    pub calls: u16,
}

#[derive(Facet)]
pub struct Adder {
    pub operands: ToFabric<Operands>,
    pub sum: FromFabric<Sum>,
    /// Cycles since reset: 64 bits, so a read snapshots it.
    pub uptime: FromFabric<u64>,
}

/// One round trip over plain registers. Reading `sum` straight after writing
/// `operands` is only right if the fabric beats the read; waiting for
/// `calls` to move is right regardless.
fn add(b: &Adder, operands: &Operands) -> Result<Sum, PortError> {
    let before = b.sum.read()?.calls;
    b.operands.write(operands)?;
    b.sum.read_until(Duration::from_secs(5), |s| s.calls != before)
}

fn ops(a: u32, b: u32) -> Operands {
    Operands { a, b, negate: false }
}

/// `latency` extra cycles before `sum` reflects an `operands` write: a
/// stand-in for a real multi-cycle computation.
fn build(name: &str, latency: u16) -> Sim {
    let top = format!(
        r#"use adder::{{adder_regs, Sum}};

#[no_mangle(all)]
entity adder_top(
    clk: clock,
    rst: bool,
{FLAT_AXI_PORTS}
    irq: inv bool,
) {{
{FLAT_AXI_GLUE}
    let p = inst adder_regs(clk, rst, axi);
    set axi_out_inv = p.axi_out;
    set irq = p.irq;

    reg(clk) cycles: uint<64> reset(rst: 0) initial(0) = cycles +. 1;
    set p.uptime = cycles;

    decl result, countdown, pending, value, calls;
    let a: uint<64> = zext(p.operands.a);
    let b: uint<64> = zext(p.operands.b);
    let total = (a +. b).as_int();
    reg(clk) result: int<64> reset(rst: 0) initial(0) = if p.operands_written {{
        if p.operands.negate {{ total.wrapping_neg() }} else {{ total }}
    }} else {{
        result
    }};
    // value and calls move together, so a host that sees the new
    // generation also sees its value.
    let done = !p.operands_written && pending && countdown == 0;
    reg(clk) countdown: uint<16> reset(rst: 0) initial(0) =
        if p.operands_written {{ {latency} }} else if pending && countdown != 0 {{ trunc(countdown - 1) }} else {{ countdown }};
    reg(clk) pending: bool reset(rst: false) initial(false) =
        if p.operands_written {{ true }} else if done {{ false }} else {{ pending }};
    reg(clk) value: int<64> reset(rst: 0) initial(0) = if done {{ result }} else {{ value }};
    reg(clk) calls: uint<16> reset(rst: 0) initial(0) = if done {{ calls +. 1 }} else {{ calls }};
    set p.sum = Sum(value, calls);
}}
"#
    );
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&work).unwrap();
    let sv = Spade::of::<Adder>()
        .unwrap()
        .compile(&top)
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(work.join("spade.sv"), sv).unwrap();
    Sim::builder(&work, "adder_top")
        .source(work.join("spade.sv"))
        .build()
        .unwrap()
}

fn sim() -> &'static Sim {
    static SIM: OnceLock<Sim> = OnceLock::new();
    SIM.get_or_init(|| build("adder-sim", 0))
}

/// An adder slower than a 3-word AXI read round trip.
fn slow_sim() -> &'static Sim {
    static SIM: OnceLock<Sim> = OnceLock::new();
    SIM.get_or_init(|| build("adder-sim-slow", 200))
}

fn spawn() -> (Adder, SimClock) {
    let (transport, clock): (SimTransport, SimClock) = sim().spawn().unwrap();
    (bind(transport).unwrap(), clock)
}

#[test]
fn multi_word_write_commits_once_and_the_sum_comes_back_signed() {
    let (b, _clk) = spawn();
    let neg = Operands {
        a: u32::MAX,
        b: 1,
        negate: true,
    };
    assert_eq!(
        add(&b, &neg).unwrap(),
        Sum {
            value: -(1i64 << 32),
            calls: 1
        }
    );
    assert_eq!(add(&b, &ops(40, 2)).unwrap(), Sum { value: 42, calls: 2 });
}

#[test]
fn a_read_straight_after_a_write_is_stale_when_the_fabric_is_slow() {
    let (transport, _clk) = slow_sim().spawn().unwrap();
    let b: Adder = bind(transport).unwrap();

    b.operands.write(&ops(40, 2)).unwrap();
    assert_eq!(
        b.sum.read().unwrap(),
        Sum { value: 0, calls: 0 },
        "the old value, silently"
    );

    // The (40, 2) request was still in flight and this write replaced it in
    // the mailbox, so it never completes: one generation, not two. Plain
    // registers can't hold two requests; that's what queues are for.
    assert_eq!(add(&b, &ops(1, 2)).unwrap(), Sum { value: 3, calls: 1 });
}

#[test]
fn uptime_is_monotonic_across_snapshotted_reads() {
    let (b, clk) = spawn();
    let t0 = b.uptime.read().unwrap();
    clk.cycles(100).unwrap();
    let t1 = b.uptime.read().unwrap();
    assert!(t1 >= t0 + 100, "{t0} -> {t1}");
}

#[test]
fn a_host_built_from_another_declaration_is_refused() {
    #[derive(Facet)]
    struct Impostor {
        operands: ToFabric<u8>,
    }
    let (transport, _clk) = sim().spawn().unwrap();
    let err = bind::<Impostor>(transport).err().expect("bind must fail");
    assert!(matches!(err, BindError::Fingerprint { .. }), "{err}");
}

#[test]
fn interleaved_words_from_a_second_accessor_tear_and_bind_refuses_it() {
    let (transport, _clk) = sim().spawn().unwrap();
    let mut other = transport.share();
    let b: Adder = bind(transport).unwrap();
    let layout = Boundary::of::<Adder>().unwrap();
    let port = layout.ports.iter().find(|p| p.name == "operands").unwrap();
    let theirs = codec::encode(&ops(1, 2), &HwType::of(Operands::SHAPE).unwrap());

    // What a second process could do through the raw window: start a
    // write, let ours land whole, then finish theirs.
    other.write(port.word, theirs[0]).unwrap();
    b.operands.write(&ops(100, 200)).unwrap();
    other.write(port.word + 1, theirs[1]).unwrap();
    other.write(port.word + 2, theirs[2]).unwrap();
    assert_eq!(
        b.sum.read().unwrap().value,
        100 + 2,
        "our `a`, their `b`: a value nobody wrote"
    );

    assert!(matches!(bind::<Adder>(other), Err(BindError::Busy { .. })));
}
