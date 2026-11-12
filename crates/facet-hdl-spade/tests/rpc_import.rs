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
        r#"use dev::{{dev_regs, ScaleArgs, SetLedArgs, SlowArgs}};
use facet_hdl::CallResult;

#[no_mangle(all)]
entity dev_top(
    clk: clock,
    rst: bool,
{FLAT_AXI_PORTS}
    irq: inv bool,
) {{
{FLAT_AXI_GLUE}
    decl led_on, slow_busy, slow_x, slow_wait, ticks_sent, last;
    let (scale_req, scale_req_inv) = port();
    let (scale_req_valid, scale_req_valid_inv) = port();
    let (scale_resp_ready, scale_resp_ready_inv) = port();
    let (ping_req_valid, ping_req_valid_inv) = port();
    let (ping_resp_ready, ping_resp_ready_inv) = port();
    let (set_led_req, set_led_req_inv) = port();
    let (set_led_req_valid, set_led_req_valid_inv) = port();
    let (led_req_valid, led_req_valid_inv) = port();
    let (led_resp_ready, led_resp_ready_inv) = port();
    let (slow_req, slow_req_inv) = port();
    let (slow_req_valid, slow_req_valid_inv) = port();
    let (slow_resp_ready, slow_resp_ready_inv) = port();
    let (tick_req_ready, tick_req_ready_inv) = port();
    let (tick_resp, tick_resp_inv) = port();
    let (tick_resp_valid, tick_resp_valid_inv) = port();
    let (last_tick_req_valid, last_tick_req_valid_inv) = port();
    let (last_tick_resp_ready, last_tick_resp_ready_inv) = port();
    let _: bool = tick_req_ready;

    // scale: x * 3, answered in the cycle it's asked.
    let sx: ScaleArgs = scale_req;
    let x32: uint<32> = zext(sx.x);
    let scaled: uint<32> = trunc(x32 + (x32 << 1));

    // set_led / led: a register, written oneway, read back by a call.
    let sl: SetLedArgs = set_led_req;
    reg(clk) led_on: bool initial(false) = if set_led_req_valid {{ sl.lit }} else {{ led_on }};

    // slow: x + 1, but only SLOW cycles after the call.
    let slow_ready = !slow_busy;
    let slow_done = slow_busy && slow_wait == 2000;
    let sa: SlowArgs = slow_req;
    reg(clk) slow_busy: bool initial(false) =
        if slow_req_valid && slow_ready {{ true }} else if slow_done && slow_resp_ready {{ false }} else {{ slow_busy }};
    reg(clk) slow_x: uint<8> initial(0) = if slow_req_valid && slow_ready {{ sa.x }} else {{ slow_x }};
    reg(clk) slow_wait: uint<16> initial(0) =
        if slow_req_valid && slow_ready {{ 0 }} else if slow_busy && slow_wait < 2000 {{ trunc(slow_wait + 1) }} else {{ slow_wait }};

    // tick: three calls into Rust, keeping the last answer.
    reg(clk) ticks_sent: uint<2> initial(0) = if ticks_sent < 3 && tick_req_ready {{ trunc(ticks_sent + 1) }} else {{ ticks_sent }};
    reg(clk) last: uint<8> initial(0) = match (tick_resp_valid, tick_resp) {{
        (true, CallResult::Returned(v)) => v,
        _ => last,
    }};

    let _ = inst dev_regs(
        clk, rst, axi, axi_out_inv, irq,
        scale_req_inv, scale_req_valid_inv, scale_resp_ready,
        scaled, scale_req_valid, scale_resp_ready_inv,
        ping_req_valid_inv, ping_resp_ready,
        ping_req_valid, ping_resp_ready_inv,
        set_led_req_inv, set_led_req_valid_inv, true,
        led_req_valid_inv, led_resp_ready,
        led_on, led_req_valid, led_resp_ready_inv,
        slow_req_inv, slow_req_valid_inv, slow_ready,
        slow_x.wrapping_add(1), slow_done, slow_resp_ready_inv,
        ticks_sent < 3, tick_req_ready_inv,
        tick_resp_inv, tick_resp_valid_inv, true,
        last_tick_req_valid_inv, last_tick_resp_ready,
        last, last_tick_req_valid, last_tick_resp_ready_inv,
    );
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
