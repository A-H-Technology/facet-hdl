//! `#[facet_hdl::rpc]` end to end: the fabric is the caller. It fires 64
//! `add` calls back to back (up to 8 in flight) and 16 `check` calls
//! alongside them, one of which panics in Rust, checks every answer tag by
//! tag, and finally reports its verdict through a `oneway` call.

use facet::Facet;
use facet_hdl::bind;
use facet_hdl_iverilog::Sim;
use facet_hdl_spade::{FLAT_AXI_GLUE, FLAT_AXI_PORTS, Spade};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub static REPORTS: Mutex<Vec<Report>> = Mutex::new(Vec::new());
pub static ADDS: Mutex<u32> = Mutex::new(0);

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Report {
    pub checked: u16,
    pub mismatches: u16,
}

#[facet_hdl::rpc]
pub mod calc {
    use super::*;

    /// Overflow comes back as `None`, so the fabric sees an `Option`.
    #[export(depth = 8)]
    pub fn add(a: u32, b: u32) -> Option<u32> {
        *ADDS.lock().unwrap() += 1;
        a.checked_add(b)
    }

    /// 13 panics; other odd numbers are refused.
    #[export]
    pub fn check(x: u8) -> Result<(), u8> {
        if x == 13 {
            panic!("unlucky (this panic is part of the test)");
        }
        if x % 2 == 1 { Err(x) } else { Ok(()) }
    }

    #[export(oneway)]
    pub fn report(r: Report) {
        REPORTS.lock().unwrap().push(r);
    }
}

/// The fabric: issues every call as soon as there's room, checks every
/// answer as it arrives, and reports the tally. Its counters have no reset,
/// on purpose: a register file that took calls during reset would lose them.
fn top() -> String {
    format!(
        r#"use calc::{{calc_regs, AddArgs, CheckArgs, Report, ReportArgs}};
use facet_hdl::CallResult;

// Every 8th call overflows, so `None` comes back.
fn add_args(i: uint<8>) -> AddArgs {{
    if (i & 7) == 7 {{
        AddArgs(0xFFFF_FFF0, 0x100)
    }} else {{
        let n: uint<32> = zext(i);
        AddArgs(n.wrapping_mul(7919).wrapping_add(1), n.wrapping_mul(104729))
    }}
}}

fn add_right(r: CallResult<Option<uint<32>>>, i: uint<8>) -> bool {{
    let args = add_args(i);
    let sum: uint<33> = args.a + args.b;
    let overflow = (sum >> 32) == 1;
    let expect: uint<32> = trunc(sum);
    match r {{
        CallResult::Returned(Some(v)) => !overflow && v == expect,
        CallResult::Returned(None) => overflow,
        CallResult::Panicked => false,
    }}
}}

// 13 panics in Rust; other odd numbers are refused.
fn check_right(r: CallResult<Result<(), uint<8>>>, x: uint<8>) -> bool {{
    match r {{
        CallResult::Panicked => x == 13,
        CallResult::Returned(Err(e)) => x != 13 && (x & 1) == 1 && e == x,
        CallResult::Returned(Ok(_)) => (x & 1) == 0,
    }}
}}

#[no_mangle(all)]
entity calc_top(
    clk: clock,
    rst: bool,
{FLAT_AXI_PORTS}
    irq: inv bool,
) {{
{FLAT_AXI_GLUE}
    let p = inst calc_regs(clk, rst, axi);
    set axi_out_inv = p.axi_out;
    set irq = p.irq;
    decl add_sent, add_got, check_sent, check_got, bad, reported;

    // Call as fast as the queues take the calls.
    set p.add_req.write = if add_sent < 64 {{ Some(add_args(add_sent)) }} else {{ None }};
    set p.check_req.write = if check_sent < 16 {{ Some(CheckArgs(check_sent)) }} else {{ None }};
    let add_sent_now = add_sent < 64 && !p.add_req.full;
    let check_sent_now = check_sent < 16 && !p.check_req.full;
    reg(clk) add_sent: uint<8> initial(0) = if add_sent_now {{ add_sent +. 1 }} else {{ add_sent }};
    reg(clk) check_sent: uint<8> initial(0) = if check_sent_now {{ check_sent +. 1 }} else {{ check_sent }};

    // Check every answer as it arrives.
    set p.add_resp.ack = true;
    set p.check_resp.ack = true;
    let add_wrong = match p.add_resp.read {{ Some(r) => !add_right(r, add_got), None => false }};
    let check_wrong = match p.check_resp.read {{ Some(r) => !check_right(r, check_got), None => false }};
    let add_answered = match p.add_resp.read {{ Some(_) => true, None => false }};
    let check_answered = match p.check_resp.read {{ Some(_) => true, None => false }};
    reg(clk) add_got: uint<8> initial(0) = if add_answered {{ add_got +. 1 }} else {{ add_got }};
    reg(clk) check_got: uint<8> initial(0) = if check_answered {{ check_got +. 1 }} else {{ check_got }};
    reg(clk) bad: uint<16> initial(0) =
        bad +. (if add_wrong {{ 1 }} else {{ 0 }}) +. (if check_wrong {{ 1 }} else {{ 0 }});

    // Then tell Rust how it went.
    let done = add_got == 64 && check_got == 16 && !reported;
    let checked: uint<16> = zext(add_got + check_got);
    set p.report_req.write = if done {{ Some(ReportArgs(Report(checked, bad))) }} else {{ None }};
    reg(clk) reported: bool initial(false) = reported || (done && !p.report_req.full);
}}
"#
    )
}

#[test]
fn the_fabric_calls_rust() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("spade-calc-sim");
    std::fs::create_dir_all(&work).unwrap();
    let sv = Spade::of::<calc::Calc>()
        .unwrap()
        .compile(&top())
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(work.join("spade.sv"), sv).unwrap();
    let sim = Sim::builder(&work, "calc_top")
        .source(work.join("spade.sv"))
        .irq()
        .build()
        .unwrap();
    let (transport, _clk) = sim.spawn().unwrap();
    let calc: calc::Calc = bind(transport).unwrap();

    let deadline = Instant::now() + Duration::from_secs(60);
    while REPORTS.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the fabric never reported");
        let idle = facet_hdl::rpc::Idle::start(&calc.add_req);
        if calc.poll().unwrap() == 0 {
            facet_hdl::block_on_timeout(idle.wait(), Duration::from_millis(10));
        }
    }
    assert_eq!(
        REPORTS.lock().unwrap().as_slice(),
        [Report {
            checked: 64 + 16,
            mismatches: 0
        }],
        "the fabric saw wrong answers"
    );
    assert_eq!(*ADDS.lock().unwrap(), 64, "every add ran exactly once");
}
