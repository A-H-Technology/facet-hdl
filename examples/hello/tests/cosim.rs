//! The real host code against the real fabric, under Icarus Verilog: the
//! Spade fabric, and the same fabric in Verilog and in VHDL on the generated
//! `hello_core`.

use facet_hdl::{bind, block_on_timeout};
use facet_hdl_iverilog::Sim;
use facet_hdl_spade::{FLAT_AXI_GLUE, FLAT_AXI_PORTS, Source, Spade};
use hello::hello::Hello;
use std::path::{Path, PathBuf};
use std::process::Command;
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

/// A Verilog top around `fabric` (a hand-written module with a `button`
/// input and the flat AXI ports), with the same stand-in button as [`sim`].
fn hdl_sim(name: &str, fabric: &str, sources: impl FnOnce(&Path) -> Vec<PathBuf>) -> Sim {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&work).unwrap();
    let core = Spade::of::<Hello>().unwrap().compile_core().unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(work.join("hello_core.sv"), core).unwrap();
    let axi = [
        "awid", "awaddr", "awlen", "awvalid", "awready", "wdata", "wvalid", "wready", "bid", "bresp", "bvalid",
        "bready", "arid", "araddr", "arlen", "arvalid", "arready", "rid", "rdata", "rresp", "rlast", "rvalid", "rready",
    ];
    let ports: String = axi.iter().map(|p| format!(", s_axi_{p}")).collect();
    let conns: String = axi.iter().map(|p| format!(", .s_axi_{p}(s_axi_{p})")).collect();
    let decls: String = axi
        .iter()
        .map(|p| {
            let dir = match *p {
                "awready" | "wready" | "bid" | "bresp" | "bvalid" | "arready" | "rid" | "rdata" | "rresp" | "rlast"
                | "rvalid" => "output",
                _ => "input",
            };
            let width = match *p {
                "awid" | "arid" | "bid" | "rid" => "[11:0] ",
                "awaddr" | "araddr" | "wdata" | "rdata" => "[31:0] ",
                "awlen" | "arlen" => "[7:0] ",
                "bresp" | "rresp" => "[1:0] ",
                _ => "",
            };
            format!("  {dir} wire {width}s_axi_{p};\n")
        })
        .collect();
    let top = format!(
        "module hello_top (clk, rst{ports}, irq);\n  input clk, rst;\n  output irq;\n{decls}  reg [31:0] cycle;\n  always @(posedge clk) cycle <= rst ? 0 : cycle + 1;\n  {fabric} f (.clk(clk), .rst(rst), .button(cycle >= 200 && cycle < 220){conns}, .irq(irq));\nendmodule\n"
    );
    std::fs::write(work.join("hello_top.v"), top).unwrap();
    let mut all = vec![work.join("hello_core.sv"), work.join("hello_top.v")];
    all.extend(sources(&work));
    Sim::builder(&work, "hello_top").sources(all).irq().build().unwrap()
}

/// Calls `add_one`, then waits for the one press the stand-in button makes.
fn exercise(sim: Sim) {
    let (transport, clk) = sim.spawn().unwrap();
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
fn rust_calls_the_fabric_and_the_fabric_calls_rust() {
    exercise(sim());
}

#[test]
fn the_same_fabric_in_verilog() {
    exercise(hdl_sim("hello-verilog", "hello_fabric_v", |_| vec![hdl().join("hello_fabric.v")]));
}

/// ghdl turns the VHDL into Verilog (leaving `hello_core` as an instance), so
/// one simulator runs it all.
#[test]
fn the_same_fabric_in_vhdl() {
    exercise(hdl_sim("hello-vhdl", "hello_fabric_vhdl", |work| {
        let pkg = work.join("hello_core_pkg.vhd");
        std::fs::write(&pkg, &Spade::of::<Hello>().unwrap().core_vhdl).unwrap();
        let out = Command::new("ghdl")
            .current_dir(work)
            .args(["--synth", "--std=08", "--out=verilog", "-Wno-binding"])
            .arg(&pkg)
            .arg(hdl().join("hello_fabric.vhd"))
            .args(["-e", "hello_fabric_vhdl"])
            .output()
            .expect("ghdl (in the dev shell)");
        assert!(out.status.success(), "ghdl: {}", String::from_utf8_lossy(&out.stderr));
        let v = work.join("hello_fabric_vhdl.v");
        std::fs::write(&v, out.stdout).unwrap();
        vec![v]
    }));
}

#[test]
fn checked_in_spade_matches_the_declaration() {
    let fresh = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hello-fresh");
    let spade = Spade::of::<Hello>().unwrap();
    let mut files = spade.write_to(&fresh).unwrap().to_vec();
    for (name, text) in [
        ("hello_core.sv", spade.compile_core().unwrap()),
        ("hello_core_pkg.vhd", spade.core_vhdl.clone()),
    ] {
        std::fs::write(fresh.join(name), text).unwrap();
        files.push(fresh.join(name));
    }
    for path in files {
        let name = path.file_name().unwrap();
        let checked_in = std::fs::read_to_string(hdl().join("generated").join(name)).unwrap_or_default();
        let fresh = std::fs::read_to_string(&path).unwrap();
        let same = match path.extension().and_then(|e| e.to_str()) {
            Some("sv") => spade_ids_ignored(&checked_in) == spade_ids_ignored(&fresh),
            _ => checked_in == fresh,
        };
        assert!(
            same,
            "hdl/generated/{} is stale; run `cargo run --features gen --bin hello-gen`",
            name.display()
        );
    }
}

/// Spade numbers its internal names (`_e_123`, `wrapping_add[2122]`)
/// differently from run to run, and orders their declarations by those
/// numbers. With the numbers blanked and the lines sorted, what's left is
/// the design.
fn spade_ids_ignored(sv: &str) -> Vec<String> {
    let mut lines: Vec<String> = sv
        .lines()
        .map(|line| {
            let mut out = String::new();
            let mut rest = line;
            while let Some(c) = rest.chars().next() {
                let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
                if let Some(after) = rest.strip_prefix("_e_").filter(|a| digits(a) > 0) {
                    out.push_str("_e_N");
                    rest = &after[digits(after)..];
                } else if let Some(after) = rest.strip_prefix('[').filter(|a| digits(a) > 0 && a[digits(a)..].starts_with(']')) {
                    out.push_str("[N]");
                    rest = &after[digits(after) + 1..];
                } else {
                    out.push(c);
                    rest = &rest[c.len_utf8()..];
                }
            }
            out
        })
        .collect();
    lines.sort();
    lines
}

/// The board tops (`hdl/<board>_top.spade`) only go through Quartus and
/// Vivado by hand (`<board>/build.sh`), so at least keep them compiling here.
#[test]
fn board_tops_compile() {
    let read = |name: &str| std::fs::read_to_string(hdl().join(name)).unwrap();
    let fabric = read("hello_fabric.spade");
    let debounce = read("debounce.spade");
    for board in ["de25", "zybo"] {
        let sv = Spade::of::<Hello>()
            .unwrap()
            .compile_with(
                &read(&format!("{board}_top.spade")),
                &[
                    Source { module: &["hello_fabric"], name: "hello_fabric.spade", code: &fabric },
                    Source { module: &["debounce"], name: "debounce.spade", code: &debounce },
                ],
            )
            .unwrap_or_else(|e| panic!("{board}: {e}"));
        assert!(sv.contains(&format!("module hello_{board} (")), "the shell instantiates hello_{board} by name");
    }
}
