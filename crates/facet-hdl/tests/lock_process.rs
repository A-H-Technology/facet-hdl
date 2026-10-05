//! Kept out of `bind.rs` on purpose: spawning a child forks this process,
//! and between fork and exec the child holds a copy of every open fd,
//! including other tests' lock files, which makes their release racy.

mod common;

use common::Mem;
use facet::Facet;
use facet_hdl::{BindError, FromFabric, ToFabric, bind};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Facet)]
pub struct Regs {
    pub ctl: ToFabric<u32>,
    pub status: FromFabric<u32>,
}

fn window_at(lock: PathBuf) -> Mem {
    Mem::for_boundary::<Regs>(lock).0
}

const HOLD: &str = "FACET_HDL_TEST_HOLD_LOCK";

/// Not a test on its own: the cross-process test re-runs this binary with
/// `HOLD` set, and this becomes the other process holding the window.
#[test]
fn lock_holder_subprocess() {
    let Ok(lock) = std::env::var(HOLD) else { return };
    let mem = window_at(lock.into());
    let _held: Regs = bind(mem).unwrap();
    println!("LOCKED");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[test]
fn another_process_holding_the_window_blocks_bind_until_it_is_killed() {
    let lock = std::env::temp_dir().join(format!("facet-hdl-test-{}-xproc.lock", std::process::id()));
    let mut holder = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_holder_subprocess", "--nocapture", "--test-threads=1"])
        .env(HOLD, &lock)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let ready = BufReader::new(holder.stdout.take().unwrap())
        .lines()
        .any(|l| l.unwrap().contains("LOCKED"));
    assert!(ready, "holder exited before taking the lock");

    let refused = bind::<Regs>(window_at(lock.clone()));
    holder.kill().unwrap(); // SIGKILL: no destructors, no cleanup
    holder.wait().unwrap();
    assert!(matches!(refused, Err(BindError::Busy { .. })), "bind while held");
    let _ours: Regs = bind(window_at(lock)).expect("the kernel drops a dead holder's flock");
}
