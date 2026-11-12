//! Regenerates `hdl/generated/` from the declaration in `src/lib.rs`:
//! `cargo run --features gen --bin blinky-gen`.

fn main() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("hdl/generated");
    let spade = facet_hdl_spade::Spade::of::<blinky::Blinky>().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let _ = std::fs::remove_dir_all(&out);
    for f in spade.write_to(&out).expect("write generated Spade") {
        println!("{}", f.display());
    }
}
