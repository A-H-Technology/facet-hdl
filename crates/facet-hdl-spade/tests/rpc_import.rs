//! `#[import]`: Rust calls HDL. Also `()` entries both ways: `ping` takes no
//! arguments and answers `()`, `tick` is an export with no arguments, and
//! `last_tick` is an import with none. `slow` answers late, to show that a
//! call that timed out can't hand its answer to the next one.

use facet_hdl::{PortError, bind};
use facet_hdl_iverilog::Sim;
use facet_hdl_spade::{FLAT_AXI_GLUE, FLAT_AXI_PORTS, Spade};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub static TICKS: Mutex<u8> = Mutex::new(0);

#[facet_hdl::rpc]
pub mod dev {
    use super::*;

    #[import(depth = 8)]
    pub fn scale(x: u16) -> u32;

    #[import]
    pub fn ping();

    #[import(oneway)]
    pub fn set_led(lit: bool);

    #[import]
    pub fn led() -> bool;

    /// Answers `x + 1`, but only after `SLOW` cycles.
    #[import]
    pub fn slow(x: u8) -> u8;

    #[export]
    pub fn tick() -> u8 {
        let mut t = TICKS.lock().unwrap();
        *t += 1;
        *t
    }

    /// The last answer the fabric got from `tick`.
    #[import]
    pub fn last_tick() -> u8;
}

/// The fabric: implements the imports and calls `tick` three times on its own.
fn top() -> String {
    format!(
        r#"use dev::dev_regs;
use facet_hdl::CallResult;

#[no_mangle(all)]
entity dev_top(
    clk: clock,
    rst: bool,
{FLAT_AXI_PORTS}
    irq: inv bool,
) {{
{FLAT_AXI_GLUE}
    let p = inst dev_regs(clk, rst, axi);
    set axi_out_inv = p.axi_out;
    set irq = p.irq;
    decl led_on, slow_busy, slow_x, slow_wait, ticks_sent, last;

    // scale: x * 3, answered in the cycle it's asked.
    set p.scale_resp.write = match p.scale_req.read {{
        Some(args) => {{ let x: uint<32> = zext(args.x); Some(x +. (x << 1)) }},
        None => None,
    }};
    set p.scale_req.ack = !p.scale_resp.full;

    // ping: no arguments, nothing to say back but "done".
    set p.ping_resp.write = p.ping_req.read;
    set p.ping_req.ack = !p.ping_resp.full;

    // set_led / led: a register, written oneway, read back by a call.
    set p.set_led_req.ack = true;
    reg(clk) led_on: bool initial(false) = match p.set_led_req.read {{ Some(args) => args.lit, None => led_on }};
    set p.led_resp.write = match p.led_req.read {{ Some(_) => Some(led_on), None => None }};
    set p.led_req.ack = !p.led_resp.full;

    // slow: x + 1, but only 2000 cycles after the call.
    let take = !slow_busy && match p.slow_req.read {{ Some(_) => true, None => false }};
    let answer = slow_busy && slow_wait == 2000;
    set p.slow_req.ack = take;
    set p.slow_resp.write = if answer {{ Some(slow_x +. 1) }} else {{ None }};
    reg(clk) slow_busy: bool initial(false) = if take {{ true }} else if answer && !p.slow_resp.full {{ false }} else {{ slow_busy }};
    reg(clk) slow_x: uint<8> initial(0) = match (take, p.slow_req.read) {{ (true, Some(args)) => args.x, _ => slow_x }};
    reg(clk) slow_wait: uint<16> initial(0) = if take {{ 0 }} else if slow_busy && slow_wait < 2000 {{ slow_wait +. 1 }} else {{ slow_wait }};

    // tick: three calls into Rust, keeping the last answer for last_tick.
    set p.tick_req.write = if ticks_sent < 3 {{ Some(()) }} else {{ None }};
    reg(clk) ticks_sent: uint<2> initial(0) = if ticks_sent < 3 && !p.tick_req.full {{ ticks_sent +. 1 }} else {{ ticks_sent }};
    set p.tick_resp.ack = true;
    reg(clk) last: uint<8> initial(0) = match p.tick_resp.read {{ Some(CallResult::Returned(v)) => v, _ => last }};
    set p.last_tick_resp.write = match p.last_tick_req.read {{ Some(_) => Some(last), None => None }};
    set p.last_tick_req.ack = !p.last_tick_resp.full;
}}
"#
    )
}

#[test]
fn rust_calls_the_fabric() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("spade-dev-sim");
    std::fs::create_dir_all(&work).unwrap();
    let sv = Spade::of::<dev::Dev>()
        .unwrap()
        .compile(&top())
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(work.join("spade.sv"), sv).unwrap();
    let sim = Sim::builder(&work, "dev_top")
        .source(work.join("spade.sv"))
        .irq()
        .build()
        .unwrap();
    let (transport, _clk) = sim.spawn().unwrap();
    let mut dev: dev::Dev = bind(transport).unwrap();
    let t = Duration::from_secs(5);

    // The fabric calls `tick` three times on its own; answer them.
    let deadline = Instant::now() + t;
    while *TICKS.lock().unwrap() < 3 {
        assert!(Instant::now() < deadline, "the fabric didn't call tick three times");
        dev.poll().unwrap();
    }
    assert_eq!(dev.last_tick(t).unwrap(), 3, "the fabric saw tick's answers");

    for x in [0, 1, 1000, u16::MAX] {
        assert_eq!(dev.scale(x, t).unwrap(), 3 * x as u32);
    }
    // Eight in flight, straight through the queues.
    for x in 0..8u16 {
        dev.scale_req.push(&dev::ScaleArgs { x }).unwrap();
    }
    for x in 0..8u16 {
        assert_eq!(dev.scale_resp.pop_wait(t).unwrap(), Some(3 * x as u32));
    }

    for _ in 0..10 {
        dev.ping(t).unwrap();
    }

    for on in [true, false, true] {
        dev.set_led(on, t).unwrap();
        assert_eq!(dev.led(t).unwrap(), on);
    }

    assert!(matches!(dev.slow(1, Duration::ZERO), Err(PortError::Timeout(_))));
    assert_eq!(
        dev.slow(2, t).unwrap(),
        3,
        "slow(1)'s late answer was taken as slow(2)'s"
    );
}
