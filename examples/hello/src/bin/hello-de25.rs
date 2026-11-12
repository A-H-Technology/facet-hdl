//! Compiles the DE25-Nano top (`hdl/de25_top.spade`) and the fabric to
//! `de25/build/spade.sv`, for `de25/build.sh`:
//! `cargo run --features gen --bin hello-de25`.

use facet_hdl_spade::{Source, Spade};
use std::fmt::Display;
use std::path::Path;

fn fail(e: impl Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |name: &str| std::fs::read_to_string(dir.join("hdl").join(name)).unwrap_or_else(|e| fail(e));
    let fabric = read("hello_fabric.spade");
    let sv = Spade::of::<hello::hello::Hello>()
        .unwrap_or_else(|e| fail(e))
        .compile_with(
            &read("de25_top.spade"),
            &[Source { module: &["hello_fabric"], name: "hello_fabric.spade", code: &fabric }],
        )
        .unwrap_or_else(|e| fail(e));
    let out = dir.join("de25/build/spade.sv");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, sv).unwrap();
    println!("{}", out.display());
}
