//! The Spade compiler, in-process: Spade source in, SystemVerilog out.

use spade_codespan_reporting::term::termcolor::Buffer;
use spade_common::name::Path;
use spade_common::namespace::ModuleNamespace;

/// One Spade file: the module path it's compiled under (`[]` for the root)
/// and its source.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub module: &'a [&'a str],
    pub name: &'a str,
    pub code: &'a str,
}

/// Compiles `sources` with Spade's stdlib and returns the SystemVerilog, or
/// the compiler's diagnostics as text.
pub fn compile(sources: &[Source<'_>]) -> Result<String, String> {
    // The compiler recurses deeply on big expressions; a test thread's 2 MiB
    // stack isn't enough, so it gets a thread of its own.
    const STACK: usize = 512 << 20;
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .name("spade".into())
            .stack_size(STACK)
            .spawn_scoped(s, || compile_here(sources))
            .expect("spawning the compiler thread")
            .join()
            .unwrap_or_else(|p| std::panic::resume_unwind(p))
    })
}

fn compile_here(sources: &[Source<'_>]) -> Result<String, String> {
    let out = tempfile_path();
    let mut buffer = Buffer::no_color();
    let opts = spade::Opt {
        error_buffer: &mut buffer,
        outfile: Some(out.clone()),
        mir_output: None,
        verilator_wrapper_output: None,
        state_dump_file: None,
        item_list_file: None,
        print_parse_traceback: None,
        opt_passes: vec![],
    };
    let files = sources
        .iter()
        .map(|s| {
            (
                ModuleNamespace {
                    namespace: Path::from_strs(s.module),
                    base_namespace: Path::from_strs(s.module),
                    file: s.name.to_owned(),
                    working_dir: None,
                },
                s.name.to_owned(),
                s.code.to_owned(),
            )
        })
        .collect();
    let result = spade::compile(
        files,
        spade::CompilationGoal::Codegen,
        true,
        opts,
        spade_diagnostics::DiagHandler::new(Box::new(spade_diagnostics::emitter::CodespanEmitter)),
    );
    let diagnostics = String::from_utf8_lossy(buffer.as_slice()).into_owned();
    let sv = std::fs::read_to_string(&out);
    let _ = std::fs::remove_file(&out);
    match (result, sv) {
        (Ok(_), Ok(sv)) => Ok(sv),
        (Ok(_), Err(e)) => Err(format!("spade wrote no output: {e}\n{diagnostics}")),
        (Err(_), _) => Err(diagnostics),
    }
}

fn tempfile_path() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "facet-hdl-spade-{}-{}.sv",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_and_reports_errors() {
        let sv = compile(&[Source {
            module: &[],
            name: "t.spade",
            code: "#[no_mangle(all)] entity t(a: uint<8>, b: inv uint<8>) { set b = trunc(a + 1); }",
        }])
        .unwrap();
        assert!(sv.contains("module t"), "{sv}");
        let err = compile(&[Source {
            module: &[],
            name: "t.spade",
            code: "entity t(a: uint<8>) -> uint<4> { a }",
        }])
        .unwrap_err();
        assert!(err.contains("error"), "{err}");
    }
}
