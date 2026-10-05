//! The real host API against the real fabric VHDL, under GHDL.

use blinky::*;
use facet::Facet;
use facet_hdl::{BindError, Boundary, HwType, ToFabric, Transport, bind, codec};
use facet_hdl_ghdl::{Sim, SimClock, SimTransport};
use facet_hdl_vhdl::Vhdl;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

fn hdl() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("hdl")
}

fn build(name: &str, result_latency: u32) -> Sim {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let generated = Vhdl::of::<Blinky>().unwrap().write_to(&work.join("gen")).unwrap();
    Sim::builder(&work, "blinky_core")
        // One cycle per millisecond keeps "uptime" and "period" cheap to simulate.
        .generic("CLK_HZ", "1000")
        .generic("RESULT_LATENCY", &result_latency.to_string())
        .sources(generated)
        .source(hdl().join("blinky_core.vhd"))
        .build()
        .expect("GHDL build (run inside `nix develop`)")
}

/// Built once per test binary; each test spawns its own simulator process.
fn sim() -> &'static Sim {
    static SIM: OnceLock<Sim> = OnceLock::new();
    SIM.get_or_init(|| build("blinky-sim", 0))
}

/// An adder slower than a 3-word AXI read round trip (~12 cycles).
fn slow_sim() -> &'static Sim {
    static SIM: OnceLock<Sim> = OnceLock::new();
    SIM.get_or_init(|| build("blinky-sim-slow", 200))
}

const WAIT: Duration = Duration::from_secs(5);

fn spawn() -> (Blinky, SimClock) {
    let (transport, clock): (SimTransport, SimClock) = sim().spawn().unwrap();
    (bind(transport).unwrap(), clock)
}

#[test]
fn checked_in_vhdl_matches_the_declaration() {
    let fresh = Path::new(env!("CARGO_TARGET_TMPDIR")).join("blinky-fresh");
    for path in Vhdl::of::<Blinky>().unwrap().write_to(&fresh).unwrap() {
        let name = path.file_name().unwrap();
        let checked_in = std::fs::read_to_string(hdl().join("generated").join(name)).unwrap_or_default();
        assert!(
            checked_in == std::fs::read_to_string(&path).unwrap(),
            "hdl/generated/{} is stale; run `cargo run --bin blinky-gen`",
            name.display()
        );
    }
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
        add(&b, &neg, WAIT).unwrap(),
        Sum {
            value: -(1i64 << 32),
            calls: 1
        }
    );
    assert_eq!(add(&b, &ops(40, 2), WAIT).unwrap(), Sum { value: 42, calls: 2 });
}

#[test]
fn a_read_straight_after_a_write_is_stale_when_the_fabric_is_slow() {
    let (transport, _clk) = slow_sim().spawn().unwrap();
    let b: Blinky = bind(transport).unwrap();

    // The pattern #2 is about: it "works" only while the fabric beats the bus.
    b.operands.write(&ops(40, 2)).unwrap();
    assert_eq!(
        b.sum.read().unwrap(),
        Sum { value: 0, calls: 0 },
        "the old value, silently"
    );

    // Waiting on the generation counter is right regardless of latency. The
    // (40, 2) request was still in flight and this write replaced it in the
    // mailbox, so it never completes: one generation, not two. Plain ports
    // can't hold two requests; that's what queue ports (#3) are for.
    assert_eq!(add(&b, &ops(1, 2), WAIT).unwrap(), Sum { value: 3, calls: 1 });
}

#[test]
fn led_patterns_follow_the_control_record() {
    let (b, clk) = spawn();
    b.leds
        .write(&LedControl {
            pattern: Pattern::Solid,
            mask: 0xA5,
            period_ms: 1,
        })
        .unwrap();
    clk.cycles(3).unwrap();
    let s = b.status.read().unwrap();
    assert_eq!((s.pattern, s.leds), (Pattern::Solid, 0xA5));

    // A 3-word read itself costs ~a dozen cycles, so the step must be slower than sampling.
    b.leds
        .write(&LedControl {
            pattern: Pattern::Chase,
            mask: 0xFF,
            period_ms: 40,
        })
        .unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..100 {
        clk.cycles(7).unwrap();
        let s = b.status.read().unwrap();
        assert_eq!(s.leds.count_ones(), 1, "chase lights exactly one LED: {:#010b}", s.leds);
        seen.insert(s.leds);
    }
    assert_eq!(seen.len(), 8, "every LED took a turn: {seen:?}");
}

#[test]
fn uptime_is_monotonic_across_snapshotted_reads() {
    let (b, clk) = spawn();
    let t0 = b.status.read().unwrap().uptime_ms;
    clk.cycles(100).unwrap();
    let t1 = b.status.read().unwrap().uptime_ms;
    assert!(t1 >= t0 + 100, "{t0} -> {t1}");
}

#[test]
fn a_host_built_from_another_declaration_is_refused() {
    #[derive(Facet)]
    struct Impostor {
        leds: ToFabric<u8>,
    }
    let (transport, _clk) = sim().spawn().unwrap();
    let err = bind::<Impostor>(transport).err().expect("bind must fail");
    assert!(matches!(err, BindError::Fingerprint { .. }), "{err}");
}

fn ops(a: u32, b: u32) -> Operands {
    Operands { a, b, negate: false }
}

#[test]
fn interleaved_words_from_a_second_accessor_tear_and_bind_refuses_it() {
    let (transport, _clk) = sim().spawn().unwrap();
    let mut other = transport.share();
    let b: Blinky = bind(transport).unwrap();
    let layout = Boundary::of::<Blinky>().unwrap();
    let port = layout.ports.iter().find(|p| p.name == "operands").unwrap();
    let theirs = codec::encode(&ops(1, 2), &HwType::of(Operands::SHAPE).unwrap());

    // The interleaving from #1, done through the raw transport since that's
    // all a second process has: they start, we write whole, they finish.
    other.write(port.word, theirs[0]).unwrap();
    b.operands.write(&ops(100, 200)).unwrap();
    other.write(port.word + 1, theirs[1]).unwrap();
    other.write(port.word + 2, theirs[2]).unwrap();
    assert_eq!(
        b.sum.read().unwrap().value,
        100 + 2,
        "our `a`, their `b`: a value nobody wrote"
    );

    assert!(matches!(bind::<Blinky>(other), Err(BindError::Busy { .. })));
}
