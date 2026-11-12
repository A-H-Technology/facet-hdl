//! `#[import]`: Rust calls HDL. Also `()` entries both ways: `ping` takes no
//! arguments and answers `()`, `tick` is an export with no arguments, and
//! `last_tick` is an import with none. `slow` answers late, to show that a
//! call that was given up on can't hand its answer to the next one.

use facet_hdl::bind;
use futures::future::join_all;
use futures::poll;
use std::pin::pin;
use facet_hdl_iverilog::Sim;
use facet_hdl_spade::{FLAT_AXI_GLUE, FLAT_AXI_PORTS, Spade};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::timeout;

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

#[tokio::test]
async fn rust_calls_the_fabric() {
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
    let again = transport.share();
    let dev: dev::Dev = bind(transport).unwrap();
    let t = Duration::from_secs(5);

    // The fabric calls `tick` three times on its own; `serve` answers them
    // while the calls below run on the same boundary.
    let calls = async {
        let deadline = Instant::now() + t;
        while *TICKS.lock().unwrap() < 3 {
            assert!(Instant::now() < deadline, "the fabric didn't call tick three times");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        assert_eq!(dev.last_tick().await.unwrap(), 3, "the fabric saw tick's answers");

        for x in [0, 1, 1000, u16::MAX] {
            assert_eq!(dev.scale(x).await.unwrap(), 3 * x as u32);
        }
        // Eight in flight at once, each handed its own answer.
        let xs: Vec<u16> = (0..8).map(|i| i * 1000 + 7).collect();
        let answers = join_all(xs.iter().map(|&x| dev.scale(x))).await;
        for (x, a) in xs.iter().zip(answers) {
            assert_eq!(a.unwrap(), 3 * *x as u32);
        }

        for _ in 0..10 {
            dev.ping().await.unwrap();
        }

        for on in [true, false, true] {
            dev.set_led(on).await.unwrap();
            assert_eq!(dev.led().await.unwrap(), on);
        }

    };
    tokio::select! {
        r = dev.serve() => panic!("serve ended: {:?}", r.map(|n| match n {})),
        r = timeout(Duration::from_secs(60), calls) => r.expect("the calls hung"),
    }

    // From here on `serve` is gone, so the simulation only advances while
    // one of our calls waits: thousands of cycles can't pass between two
    // polls.

    // Claimed out of order: `later` pops `first`'s answer on the way to its
    // own and has to keep it for `first`. slow answers one at a time, 2000
    // cycles each.
    {
        let (mut first, mut later) = (pin!(dev.slow(30)), pin!(dev.slow(40)));
        assert!(poll!(&mut first).is_pending(), "sent, not yet answered");
        assert!(poll!(&mut later).is_pending(), "sent, not yet answered");
        assert_eq!(timeout(t, later).await.expect("hung").unwrap(), 41);
        assert_eq!(timeout(t, first).await.expect("first's answer was lost").unwrap(), 31);
    }

    // Nothing advances the simulation now, so
    // slow's answer can't be there yet when the timeout gives up. Its answer
    // still comes, and must not become the next call's.
    let given_up = timeout(Duration::ZERO, dev.slow(1)).await;
    assert!(given_up.is_err(), "{given_up:?}");
    assert_eq!(
        timeout(t, dev.slow(2)).await.expect("hung").unwrap(),
        3,
        "slow(1)'s late answer was taken as slow(2)'s"
    );

    // Same again across a host restart: the old binding's owed answer reaches
    // the new one and must be dropped there too.
    let given_up = timeout(Duration::ZERO, dev.slow(10)).await;
    assert!(given_up.is_err(), "{given_up:?}");
    drop(dev);
    let dev: dev::Dev = bind(again).unwrap();
    assert_eq!(
        timeout(t, dev.slow(20)).await.expect("hung").unwrap(),
        21,
        "the previous host's slow(10) answered this one"
    );
}
