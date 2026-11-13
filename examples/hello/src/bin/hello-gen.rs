//! Regenerates `hdl/generated/` from `src/lib.rs`:
//! `cargo run --features gen --bin hello-gen`.
//!
//! The Spade is for fabric written in Spade; `hello_core.sv` and
//! `hello_core_pkg.vhd` are for fabric written in Verilog or VHDL.

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}

fn main() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("hdl/generated");
    let spade = facet_hdl_spade::Spade::of::<hello::hello::Hello>().unwrap_or_else(|e| fail(e));
    let sv = spade.compile_core().unwrap_or_else(|e| fail(e));
    let _ = std::fs::remove_dir_all(&out);
    let mut files = spade.write_to(&out).expect("write generated Spade").to_vec();
    for (name, text) in [
        (format!("{}.sv", spade.core_name), sv),
        (format!("{}_pkg.vhd", spade.core_name), spade.core_vhdl.clone()),
    ] {
        let path = out.join(name);
        std::fs::write(&path, text).expect("write generated core");
        files.push(path);
    }
    for f in files {
        println!("{}", f.display());
    }
}
