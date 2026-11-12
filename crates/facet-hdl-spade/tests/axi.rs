//! The hand-written AXI4 slave on its own: 16 words of memory behind the word
//! bus, driven with single beats and bursts, every response's ID checked.

use facet_hdl::Transport;
use facet_hdl_iverilog::Sim;
use facet_hdl_spade::{Source, compile};
use std::path::Path;

#[test]
fn single_beats_and_bursts_with_ids() {
    let sv = compile(&[
        Source {
            module: &[],
            name: "axi_mem.spade",
            code: include_str!("axi_mem.spade"),
        },
        Source {
            module: &["facet_hdl"],
            name: "facet_hdl.spade",
            code: facet_hdl_spade::LIBRARY,
        },
    ])
    .unwrap();
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("axi-mem");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("spade.sv"), sv).unwrap();
    let sim = Sim::builder(&work, "mem_top")
        .source(work.join("spade.sv"))
        .build()
        .unwrap();
    let (mut t, _clk) = sim.spawn().unwrap();

    for w in 0..16 {
        t.write(w, 0x1000_0000 + w * 7).unwrap();
    }
    for w in 0..16 {
        assert_eq!(t.read(w).unwrap(), 0x1000_0000 + w * 7, "word {w}");
    }
    let burst: Vec<u32> = (0..9).map(|i| 0xA5A5_0000 | i).collect();
    t.write_burst(3, &burst).unwrap();
    assert_eq!(t.read_burst(3, 9).unwrap(), burst);
    assert_eq!(t.read(2).unwrap(), 0x1000_0000 + 2 * 7, "the burst stayed in its words");
    assert_eq!(
        t.read(12).unwrap(),
        0x1000_0000 + 12 * 7,
        "the burst stayed in its words"
    );
    // A whole-window read in one burst: 16 beats.
    let all = t.read_burst(0, 16).unwrap();
    assert_eq!(all[5], 0xA5A5_0002);
}
