//! Host side, for the HPS: calls the fabric's `add_one`, then answers the
//! fabric's `pressed` calls until killed.
//!
//!   hello [--base 0x20000000 | --uio NAME]

use facet_hdl::{Transport, bind};
use facet_hdl_linux::{AGILEX5_LWH2F, DevMem, Uio};
use hello::hello::Hello;
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] => DevMem::open_for::<Hello>(AGILEX5_LWH2F)
            .map_err(Into::into)
            .and_then(run),
        ["--base", addr] => parse_addr(addr)
            .and_then(|base| Ok(DevMem::open_for::<Hello>(base)?))
            .and_then(run),
        ["--uio", name] => Uio::open_for::<Hello>(name).map_err(Into::into).and_then(run),
        _ => Err("usage: hello [--base ADDR | --uio NAME]".into()),
    };
    if let Err(e) = result {
        eprintln!("hello: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

type Error = Box<dyn std::error::Error>;

fn parse_addr(s: &str) -> Result<u64, Error> {
    Ok(match s.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16)?,
        None => s.parse()?,
    })
}

fn run(transport: impl Transport + 'static) -> Result<(), Error> {
    let mut fabric: Hello = bind(transport)?;
    println!("add_one(41) = {}", fabric.add_one(41, Duration::from_millis(100))?);
    println!("press the button");
    match fabric.serve()? {}
}
