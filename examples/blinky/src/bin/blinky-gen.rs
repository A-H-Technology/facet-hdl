//! Regenerates `hdl/generated/` from the declaration in `src/lib.rs`.

fn main() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("hdl/generated");
    let vhdl = facet_hdl_vhdl::Vhdl::of::<blinky::Blinky>().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    for f in vhdl.write_to(&out).expect("write generated VHDL") {
        println!("{}", f.display());
    }
}
