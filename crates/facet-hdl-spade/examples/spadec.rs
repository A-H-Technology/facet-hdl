//! `cargo run --example spadec -- file.spade [lib.spade]`: compile with the
//! in-process Spade, print the SystemVerilog or the diagnostics. Each extra
//! file is the module named after its file stem.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let codes: Vec<String> = args.iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
    let mut sources = vec![facet_hdl_spade::Source {
        module: &[],
        name: &args[0],
        code: &codes[0],
    }];
    let stems: Vec<String> = args
        .iter()
        .map(|p| {
            std::path::Path::new(p)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let stems: Vec<[&str; 1]> = stems.iter().map(|s| [s.as_str()]).collect();
    for ((p, c), stem) in args.iter().zip(&codes).zip(&stems).skip(1) {
        sources.push(facet_hdl_spade::Source {
            module: stem,
            name: p,
            code: c,
        });
    }
    match facet_hdl_spade::compile(&sources) {
        Ok(sv) => print!("{sv}"),
        Err(e) => {
            eprint!("{e}");
            std::process::exit(1);
        }
    }
}
