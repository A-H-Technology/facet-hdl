//! Compiles a board's top (`hdl/<board>_top.spade`) and the fabric to
//! `<board>/build/spade.sv`, for that board's `build.sh`:
//! `cargo run --features gen --bin hello-top -- de25` (or `zybo`).

use facet_hdl_spade::{Source, Spade};
use std::fmt::Display;
use std::path::Path;

fn fail(e: impl Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}

fn main() {
    let board = match std::env::args().nth(1).as_deref() {
        Some(b @ ("de25" | "zybo")) => b.to_owned(),
        _ => fail("usage: hello-top de25|zybo"),
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |name: &str| std::fs::read_to_string(dir.join("hdl").join(name)).unwrap_or_else(|e| fail(e));
    let fabric = read("hello_fabric.spade");
    let debounce = read("debounce.spade");
    let sv = Spade::of::<hello::hello::Hello>()
        .unwrap_or_else(|e| fail(e))
        .compile_with(
            &read(&format!("{board}_top.spade")),
            &[
                Source { module: &["hello_fabric"], name: "hello_fabric.spade", code: &fabric },
                Source { module: &["debounce"], name: "debounce.spade", code: &debounce },
            ],
        )
        .unwrap_or_else(|e| fail(e));
    let out = dir.join(board).join("build/spade.sv");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, sv).unwrap();
    println!("{}", out.display());
}
