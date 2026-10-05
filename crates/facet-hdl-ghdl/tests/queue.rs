//! Queue ports end to end: host pushes jobs, the fabric multiplies by three
//! and pushes results back, and a host-set mask makes the fabric stall
//! pseudo-randomly (or freeze) so backpressure is exercised for real.

use facet::Facet;
use facet_hdl::{Boundary, FromFabricQueue, PushError, ToFabric, ToFabricQueue, Transport, bind, queue};
use facet_hdl_ghdl::{Sim, SimTransport};
use facet_hdl_vhdl::Vhdl;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// 64 bits: a 2-word entry, so a push commits on its second word.
#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Job {
    pub seq: u32,
    pub x: u32,
}

/// 96 bits: a 3-word entry, so a pop reads through the snapshot.
#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Done {
    pub seq: u32,
    pub y: u64,
}

const DEPTH: usize = 8;

/// Polls with no progress before a test calls an entry lost. Each poll is a
/// few bus transactions, so this is thousands of fabric cycles: far longer
/// than any stall the mask can produce.
const STUCK: u32 = 500;

#[derive(Facet)]
pub struct Pipe {
    pub jobs: ToFabricQueue<Job, DEPTH>,
    pub done: FromFabricQueue<Done, DEPTH>,
    /// The fabric moves an entry only on cycles where `lfsr & stall == 0`:
    /// 0 never stalls, all ones freezes it.
    pub stall: ToFabric<u32>,
}

const TOP: &str = r#"
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.facet_hdl_axil_pkg.all;
use work.pipe_pkg.all;

entity pipe_top is
  port (
    clk, rst_n : in std_logic;
    axi_in : in axil_m2s_t;
    axi_out : out axil_s2m_t;
    irq : out std_logic
  );
end entity;

architecture rtl of pipe_top is
  signal job : job_t;
  signal job_valid, job_ready, done_valid, done_ready, go : std_logic;
  signal stall : unsigned(31 downto 0);
  signal lfsr : unsigned(31 downto 0) := x"ACE1ACE1";
  signal x64 : unsigned(63 downto 0);
begin
  regs : entity work.pipe_regs
    port map (
      clk => clk, rst_n => rst_n, axi_in => axi_in, axi_out => axi_out, irq => irq,
      jobs => job, jobs_valid => job_valid, jobs_ready => job_ready,
      done => (seq => job.seq, y => shift_left(x64, 1) + x64),
      done_valid => done_valid, done_ready => done_ready,
      stall => stall, stall_written => open
    );

  x64 <= resize(job.x, 64);
  go <= '1' when (lfsr and stall) = 0 else '0';
  -- One job in, one result out, in the same cycle: the job is only taken
  -- when its result has somewhere to go.
  done_valid <= job_valid and go;
  job_ready <= done_ready and go;

  process (clk)
  begin
    if rising_edge(clk) then
      lfsr <= lfsr(30 downto 0) & (lfsr(31) xor lfsr(21) xor lfsr(1) xor lfsr(0));
    end if;
  end process;
end architecture;
"#;

fn sim() -> &'static Sim {
    static SIM: OnceLock<Sim> = OnceLock::new();
    SIM.get_or_init(|| {
        let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pipe-sim");
        std::fs::create_dir_all(&work).unwrap();
        let top = work.join("pipe_top.vhd");
        std::fs::write(&top, TOP).unwrap();
        let generated = Vhdl::of::<Pipe>().unwrap().write_to(&work.join("gen")).unwrap();
        Sim::builder(&work, "pipe_top")
            .sources(generated)
            .source(&top)
            .irq()
            .build()
            .unwrap()
    })
}

fn job(seq: u32) -> Job {
    Job {
        seq,
        x: seq.wrapping_mul(2_654_435_761),
    }
}

fn run_both_directions(stall_mask: u32) {
    const N: u32 = 300;
    let (transport, _clk) = sim().spawn().unwrap();
    let mut p: Pipe = bind(transport).unwrap();
    p.stall.write(&stall_mask).unwrap();

    let (mut sent, mut received, mut fulls, mut idle) = (0u32, 0u32, 0u32, 0u32);
    while received < N {
        let progress = (sent, received);
        // Push until the ring pushes back, then drain whatever came out.
        while sent < N {
            match p.jobs.push(&job(sent)) {
                Ok(()) => sent += 1,
                Err(PushError::Full) => {
                    fulls += 1;
                    break;
                }
                Err(e) => panic!("{e}"),
            }
        }
        while let Some(d) = p.done.pop().unwrap() {
            let want = job(received);
            assert_eq!(d.seq, received, "gap or duplicate");
            assert_eq!(d.y, want.x as u64 * 3, "corrupt entry {received}");
            received += 1;
        }
        idle = if (sent, received) == progress { idle + 1 } else { 0 };
        assert!(
            idle < STUCK,
            "stuck after {sent} sent, {received} received: an entry was lost"
        );
    }
    assert_eq!(p.done.pop().unwrap(), None, "nothing beyond what was sent");
    assert!(
        fulls > 0,
        "the run never hit backpressure, so it proved nothing about it"
    );
}

#[test]
fn full_rate_both_ways_without_stalls() {
    run_both_directions(0);
}

#[test]
fn full_rate_both_ways_with_the_fabric_stalling() {
    // lfsr & 0b1011 == 0 on about one cycle in eight.
    run_both_directions(0b1011);
}

#[test]
fn a_full_queue_refuses_the_push_and_leaves_the_fabric_untouched() {
    let (transport, _clk) = sim().spawn().unwrap();
    let mut raw: SimTransport = transport.share();
    let mut p: Pipe = bind(transport).unwrap();
    let jobs = Boundary::of::<Pipe>().unwrap().ports[0].clone();

    p.stall.write(&u32::MAX).unwrap();
    for seq in 0..DEPTH as u32 {
        p.jobs.push(&job(seq)).unwrap();
    }
    let before = (
        raw.read(jobs.word + queue::HEAD).unwrap(),
        raw.read(jobs.word + queue::TAIL).unwrap(),
    );
    assert_eq!(before, (DEPTH as u32, 0));
    assert!(matches!(p.jobs.push(&job(99)), Err(PushError::Full)));
    let after = (
        raw.read(jobs.word + queue::HEAD).unwrap(),
        raw.read(jobs.word + queue::TAIL).unwrap(),
    );
    assert_eq!(after, before, "a refused push changes no fabric state");

    // Thaw: exactly the eight queued jobs come out, in order, then nothing.
    p.stall.write(&0).unwrap();
    let mut seqs = Vec::new();
    for _ in 0..STUCK {
        if let Some(d) = p.done.pop().unwrap() {
            seqs.push(d.seq);
        }
        if seqs.len() == DEPTH {
            break;
        }
    }
    assert_eq!(seqs, (0..DEPTH as u32).collect::<Vec<_>>());
    assert_eq!(p.done.pop().unwrap(), None);
}

#[test]
fn irq_is_high_exactly_while_results_are_waiting() {
    let (transport, _clk) = sim().spawn().unwrap();
    let mut line = transport.share().interrupt().expect("built with .irq()");
    let mut p: Pipe = bind(transport).unwrap();
    let short = Duration::from_millis(50);

    assert!(!line.wait(short).unwrap(), "nothing produced yet");
    p.jobs.push(&job(0)).unwrap();
    assert!(line.wait(Duration::from_secs(5)).unwrap(), "a result is waiting");
    assert!(
        line.wait(short).unwrap(),
        "level: still high, so a late waiter can't miss it"
    );
    assert_eq!(p.done.pop().unwrap().map(|d| d.seq), Some(0));
    assert!(!line.wait(short).unwrap(), "drained, so low again");
}

#[test]
fn pop_wait_wakes_on_the_interrupt_not_the_timeout() {
    let (transport, _clk) = sim().spawn().unwrap();
    let mut p: Pipe = bind(transport).unwrap();
    p.stall.write(&0b1011).unwrap();
    let timeout = Duration::from_secs(30);
    for seq in 0..20 {
        p.jobs.push(&job(seq)).unwrap();
        let started = Instant::now();
        let d = p.done.pop_wait(timeout).unwrap().expect("result before the timeout");
        assert_eq!(d.seq, seq);
        assert!(started.elapsed() < timeout / 10, "woke late: {:?}", started.elapsed());
    }
}

#[test]
fn pop_wait_gives_up_at_the_timeout() {
    let (transport, _clk) = sim().spawn().unwrap();
    let mut p: Pipe = bind(transport).unwrap();
    p.stall.write(&u32::MAX).unwrap();
    p.jobs.push(&job(0)).unwrap();
    let started = Instant::now();
    assert_eq!(p.done.pop_wait(Duration::from_millis(300)).unwrap(), None);
    assert!(started.elapsed() >= Duration::from_millis(300));
}
