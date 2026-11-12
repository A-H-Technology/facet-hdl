//! The async side of hello on real hardware, on tokio: `serve` runs while
//! overlapping `add_one` calls share the binding, and calls given up on with
//! a zero timeout must not hand their answer to the next call.
//!
//! hello's fabric answers add_one in the cycle the call arrives, and a
//! call's first poll writes the request and reads the answer with no await
//! between, so on the DE25 the zero timeout never wins: it reports 0
//! abandoned and the dropped-call path stays covered only by the co-sim's
//! `slow` import (facet-hdl-spade/tests/rpc_import.rs).
//!
//!   hello-async [--base 0x20000000 | --uio NAME]

use facet_hdl::{Transport, bind};
use facet_hdl_linux::{AGILEX5_LWH2F, DevMem, Uio};
use futures::future::join_all;
use hello::hello::Hello;
use std::process::ExitCode;
use std::time::Duration;
use tokio::time::timeout;

type Error = Box<dyn std::error::Error>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] => match DevMem::open_for::<Hello>(AGILEX5_LWH2F) {
            Ok(t) => run(t).await,
            Err(e) => Err(e.into()),
        },
        ["--base", addr] => match parse_addr(addr).and_then(|b| Ok(DevMem::open_for::<Hello>(b)?)) {
            Ok(t) => run(t).await,
            Err(e) => Err(e),
        },
        ["--uio", name] => match Uio::open_for::<Hello>(name) {
            Ok(t) => run(t).await,
            Err(e) => Err(e.into()),
        },
        _ => Err("usage: hello-async [--base ADDR | --uio NAME]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("hello-async: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_addr(s: &str) -> Result<u64, Error> {
    Ok(match s.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16)?,
        None => s.parse()?,
    })
}

async fn run(transport: impl Transport + 'static) -> Result<(), Error> {
    let fabric: Hello = bind(transport)?;
    let checks = async {
        // More calls in flight than the queue holds, so some wait for room.
        let xs: Vec<u32> = (0..64u32).map(|i| i.wrapping_mul(0x9E37_79B9)).collect();
        let answers = join_all(xs.iter().map(|&x| fabric.add_one(x))).await;
        let mut wrong = 0;
        for (x, a) in xs.iter().zip(answers) {
            if a? != x.wrapping_add(1) {
                wrong += 1;
            }
        }
        println!("overlapping: {} calls, {wrong} wrong", xs.len());
        if wrong > 0 {
            return Err::<(), Error>("an overlapping call got another's answer".into());
        }

        // Count how often a zero timeout gives up before the answer is read.
        // Each try is followed by a call with a different argument, which
        // would see x + 1 of the abandoned one if its answer leaked.
        let (mut abandoned, mut finished, mut leaked) = (0, 0, 0);
        for i in 0..10_000u32 {
            let gone = 1_000_000 + i;
            match timeout(Duration::ZERO, fabric.add_one(gone)).await {
                Err(_) => abandoned += 1,
                Ok(r) => {
                    finished += 1;
                    if r? != gone + 1 {
                        leaked += 1;
                    }
                }
            }
            let next = 2_000_000 + i;
            let got = timeout(Duration::from_millis(100), fabric.add_one(next))
                .await
                .map_err(|_| "add_one hung after an abandoned call")??;
            if got != next + 1 {
                leaked += 1;
                eprintln!("add_one({next}) = {got}");
            }
        }
        println!("zero timeout: {abandoned} abandoned, {finished} finished anyway, {leaked} wrong answers");
        if leaked > 0 {
            return Err("an abandoned call's answer reached a later call".into());
        }
        if abandoned == 0 {
            println!("no call was abandoned: the fabric answered every one first, so that path went untested");
        }
        Ok(())
    };
    tokio::select! {
        r = fabric.serve() => match r? {},
        r = checks => r,
    }
}
