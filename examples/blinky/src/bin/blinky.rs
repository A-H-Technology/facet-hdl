//! Host side of the demo, for the HPS. Same `Blinky` struct the VHDL was
//! generated from; `bind` refuses to run against a bitstream that wasn't.
//!
//!   blinky [--base 0x20000000 | --uio NAME] status
//!   blinky [--base ..] leds <off|solid|blink|chase> [mask] [period_ms]
//!   blinky [--base ..] add <a> <b> [--negate]

use blinky::{Blinky, LedControl, Operands, Pattern};
use facet_hdl::{Transport, bind};
use facet_hdl_linux::{AGILEX5_LWH2F, DevMem, Uio};
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str =
    "usage: blinky [--base ADDR | --uio NAME] status | leds PATTERN [MASK] [PERIOD_MS] | add A B [--negate]";

/// How to reach the register window.
enum Window {
    /// Physical address through /dev/mem.
    Base(u64),
    /// A generic-uio device-tree node, found by name.
    Uio(String),
}

enum Cmd {
    Status,
    Leds(LedControl),
    Add(Operands),
}

fn main() -> ExitCode {
    let result = parse(std::env::args().skip(1).collect()).and_then(|(window, cmd)| match window {
        Window::Base(base) => run(DevMem::open_for::<Blinky>(base)?, cmd),
        Window::Uio(name) => run(Uio::open_for::<Blinky>(&name)?, cmd),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("blinky: {e}");
            ExitCode::FAILURE
        }
    }
}

type Error = Box<dyn std::error::Error>;

fn int(s: &str) -> Result<u64, Error> {
    Ok(match s.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16)?,
        None => s.parse()?,
    })
}

// Fully parsed before anything is mapped: on arm64 a stray access into an
// undecoded part of the bridge window can come back as an SError, which takes
// the kernel down rather than this process.
fn parse(mut args: Vec<String>) -> Result<(Window, Cmd), Error> {
    let window = match args.first().map(String::as_str) {
        Some(flag @ ("--base" | "--uio")) => {
            let value = args.get(1).ok_or(format!("{flag} needs a value"))?.clone();
            let window = if flag == "--base" {
                Window::Base(int(&value)?)
            } else {
                Window::Uio(value)
            };
            args.drain(..2);
            window
        }
        _ => Window::Base(AGILEX5_LWH2F),
    };
    let negate = args.iter().any(|a| a == "--negate");
    args.retain(|a| a != "--negate");
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let cmd = match args.as_slice() {
        ["status"] => Cmd::Status,
        ["leds", pattern, rest @ ..] if rest.len() <= 2 => Cmd::Leds(LedControl {
            pattern: match *pattern {
                "off" => Pattern::Off,
                "solid" => Pattern::Solid,
                "blink" => Pattern::Blink,
                "chase" => Pattern::Chase,
                p => return Err(format!("unknown pattern {p:?}").into()),
            },
            mask: rest.first().map(|s| int(s)).transpose()?.unwrap_or(0xFF).try_into()?,
            period_ms: rest.get(1).map(|s| int(s)).transpose()?.unwrap_or(250).try_into()?,
        }),
        ["add", a, b] => Cmd::Add(Operands {
            a: int(a)?.try_into()?,
            b: int(b)?.try_into()?,
            negate,
        }),
        _ => return Err(USAGE.into()),
    };
    Ok((window, cmd))
}

fn run(transport: impl Transport + 'static, cmd: Cmd) -> Result<(), Error> {
    let b: Blinky = bind(transport)?;
    match cmd {
        Cmd::Status => println!("{:#?}", b.status.read()?),
        Cmd::Leds(ctl) => {
            b.leds.write(&ctl)?;
            println!("{:#?}", b.status.read()?);
        }
        Cmd::Add(ops) => println!("{:#?}", blinky::add(&b, &ops, Duration::from_millis(100))?),
    }
    Ok(())
}
