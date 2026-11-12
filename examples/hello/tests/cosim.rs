//! The real host code against the real fabric Spade, under Icarus Verilog.

use facet_hdl::{bind, block_on_timeout};
use facet_hdl_iverilog::Sim;
use facet_hdl_spade::{FLAT_AXI_GLUE, FLAT_AXI_PORTS, Source, Spade};
use hello::hello::Hello;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn hdl() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("hdl")
}

/// The fabric with a stand-in for the button: held down for cycles 200 to
/// 219, so exactly one press.
fn sim() -> Sim {
    let top = format!(
        r#"#[no_mangle(all)]
entity hello_top(
    clk: clock,
    rst: bool,
{FLAT_AXI_PORTS}
    irq: inv bool,
) {{
{FLAT_AXI_GLUE}
    reg(clk) cycle: uint<32> reset(rst: 0) initial(0) = cycle +. 1;
    let (out, interrupt) = inst hello_fabric::hello_fabric(clk, rst, axi, cycle >= 200 && cycle < 220);
    set axi_out_inv = out;
    set irq = interrupt;
}}
"#
    );
    let fabric = std::fs::read_to_string(hdl().join("hello_fabric.spade")).unwrap();
    let sv = Spade::of::<Hello>()
        .unwrap()
        .compile_with(
            &top,
            &[Source {
                module: &["hello_fabric"],
                name: "hello_fabric.spade",
                code: &fabric,
            }],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hello-sim");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("spade.sv"), sv).unwrap();
    Sim::builder(&work, "hello_top")
        .source(work.join("spade.sv"))
        .irq()
        .build()
        .unwrap()
}

#[test]
fn rust_calls_the_fabric_and_the_fabric_calls_rust() {
    let (transport, clk) = sim().spawn().unwrap();
    let fabric: Hello = bind(transport).unwrap();
    let t = Duration::from_secs(5);
    let add_one = |x| block_on_timeout(fabric.add_one(x), t).expect("no answer").unwrap();

    assert_eq!(add_one(41), 42);
    assert_eq!(add_one(u32::MAX), 0, "wraps, like the Spade says");

    // One press, one call: answer it, then nothing more comes.
    let deadline = Instant::now() + t;
    let mut presses = 0;
    while presses == 0 {
        assert!(Instant::now() < deadline, "the press never arrived");
        presses += fabric.poll().unwrap();
    }
    clk.cycles(500).unwrap();
    presses += fabric.poll().unwrap();
    assert_eq!(presses, 1, "one press, one call");
}

#[test]
fn checked_in_spade_matches_the_declaration() {
    let fresh = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hello-fresh");
    for path in Spade::of::<Hello>().unwrap().write_to(&fresh).unwrap() {
        let name = path.file_name().unwrap();
        let checked_in = std::fs::read_to_string(hdl().join("generated").join(name)).unwrap_or_default();
        assert!(
            checked_in == std::fs::read_to_string(&path).unwrap(),
            "hdl/generated/{} is stale; run `cargo run --features gen --bin hello-gen`",
            name.display()
        );
    }
}
