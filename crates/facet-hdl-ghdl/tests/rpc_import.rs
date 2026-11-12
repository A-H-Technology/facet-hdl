//! `#[import]`: Rust calls HDL. Also `()` entries both ways: `ping` takes no
//! arguments and answers `()`, `tick` is an export with no arguments, and
//! `last_tick` is an import with none. `slow` answers late, to show that a
//! call that timed out can't hand its answer to the next one.

use facet_hdl::{PortError, bind};
use facet_hdl_ghdl::Sim;
use facet_hdl_vhdl::Vhdl;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub static TICKS: Mutex<u8> = Mutex::new(0);

#[facet_hdl::rpc]
pub mod dev {
    use super::*;

    #[import(depth = 8)]
    pub fn scale(x: u16) -> u32;

    #[import]
    pub fn ping();

    #[import(oneway)]
    pub fn set_led(lit: bool);

    #[import]
    pub fn led() -> bool;

    /// Answers `x + 1`, but only after `SLOW` cycles.
    #[import]
    pub fn slow(x: u8) -> u8;

    #[export]
    pub fn tick() -> u8 {
        let mut t = TICKS.lock().unwrap();
        *t += 1;
        *t
    }

    /// The last answer the fabric got from `tick`.
    #[import]
    pub fn last_tick() -> u8;
}

const TOP: &str = r#"
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.facet_hdl_axil_pkg.all;
use work.dev_pkg.all;

entity dev_top is
  port (
    clk, rst_n : in std_logic;
    axi_in : in axil_m2s_t;
    axi_out : out axil_s2m_t;
    irq : out std_logic
  );
end entity;

architecture rtl of dev_top is
  constant SLOW : natural := 2000;

  signal scale_req : scale_args_t;
  signal scale_req_valid, scale_req_ready, scale_resp_ready : std_logic;
  signal ping_req_valid, ping_resp_ready : std_logic;
  signal set_led_req : set_led_args_t;
  signal set_led_req_valid : std_logic;
  signal led_req_valid, led_resp_ready : std_logic;
  signal slow_req : slow_args_t;
  signal slow_req_valid, slow_req_ready, slow_resp_valid, slow_resp_ready : std_logic;
  signal tick_req_valid, tick_req_ready, tick_resp_valid, tick_resp_ready : std_logic;
  signal tick_resp : call_u8_t;
  signal last_tick_req_valid, last_tick_resp_ready : std_logic;

  signal led_on : std_logic := '0';
  signal slow_busy : boolean := false;
  signal slow_x : unsigned(7 downto 0);
  signal slow_wait : natural := 0;
  signal ticks_sent : natural := 0;
  signal last : unsigned(7 downto 0) := (others => '0');
  signal x32 : unsigned(31 downto 0);
begin
  regs : entity work.dev_regs
    port map (
      clk => clk, rst_n => rst_n, axi_in => axi_in, axi_out => axi_out, irq => irq,
      scale_req => scale_req, scale_req_valid => scale_req_valid, scale_req_ready => scale_req_ready,
      scale_resp => x32 + shift_left(x32, 1), scale_resp_valid => scale_req_valid,
      scale_resp_ready => scale_resp_ready,
      -- No data either way: the handshakes are the call and the answer.
      ping_req_valid => ping_req_valid, ping_req_ready => ping_resp_ready,
      ping_resp_valid => ping_req_valid, ping_resp_ready => ping_resp_ready,
      set_led_req => set_led_req, set_led_req_valid => set_led_req_valid, set_led_req_ready => '1',
      led_req_valid => led_req_valid, led_req_ready => led_resp_ready,
      led_resp => led_on, led_resp_valid => led_req_valid, led_resp_ready => led_resp_ready,
      slow_req => slow_req, slow_req_valid => slow_req_valid, slow_req_ready => slow_req_ready,
      slow_resp => slow_x + 1, slow_resp_valid => slow_resp_valid, slow_resp_ready => slow_resp_ready,
      tick_req_valid => tick_req_valid, tick_req_ready => tick_req_ready,
      tick_resp => tick_resp, tick_resp_valid => tick_resp_valid, tick_resp_ready => '1',
      last_tick_req_valid => last_tick_req_valid, last_tick_req_ready => last_tick_resp_ready,
      last_tick_resp => last, last_tick_resp_valid => last_tick_req_valid,
      last_tick_resp_ready => last_tick_resp_ready
    );

  x32 <= resize(scale_req.x, 32);
  scale_req_ready <= scale_resp_ready;
  slow_req_ready <= '1' when not slow_busy else '0';
  slow_resp_valid <= '1' when slow_busy and slow_wait = SLOW else '0';
  tick_req_valid <= '1' when ticks_sent < 3 else '0';

  process (clk)
  begin
    if rising_edge(clk) then
      if set_led_req_valid = '1' then
        led_on <= set_led_req.lit;
      end if;
      if slow_req_valid = '1' and slow_req_ready = '1' then
        slow_busy <= true;
        slow_x <= slow_req.x;
        slow_wait <= 0;
      elsif slow_busy and slow_wait < SLOW then
        slow_wait <= slow_wait + 1;
      elsif slow_resp_valid = '1' and slow_resp_ready = '1' then
        slow_busy <= false;
      end if;
      if tick_req_valid = '1' and tick_req_ready = '1' then
        ticks_sent <= ticks_sent + 1;
      end if;
      if tick_resp_valid = '1' and tick_resp.tag = call_u8_returned then
        last <= tick_resp.returned;
      end if;
    end if;
  end process;
end architecture;
"#;

#[test]
fn rust_calls_the_fabric() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dev-sim");
    std::fs::create_dir_all(&work).unwrap();
    let top = work.join("dev_top.vhd");
    std::fs::write(&top, TOP).unwrap();
    let generated = Vhdl::of::<dev::Dev>().unwrap().write_to(&work.join("gen")).unwrap();
    let sim = Sim::builder(&work, "dev_top")
        .sources(generated)
        .source(&top)
        .irq()
        .build()
        .unwrap();
    let (transport, _clk) = sim.spawn().unwrap();
    let mut dev: dev::Dev = bind(transport).unwrap();
    let t = Duration::from_secs(5);

    // The fabric calls `tick` three times on its own; answer them.
    let deadline = Instant::now() + t;
    while *TICKS.lock().unwrap() < 3 {
        assert!(Instant::now() < deadline, "the fabric didn't call tick three times");
        dev.poll().unwrap();
    }
    assert_eq!(dev.last_tick(t).unwrap(), 3, "the fabric saw tick's answers");

    for x in [0, 1, 1000, u16::MAX] {
        assert_eq!(dev.scale(x, t).unwrap(), 3 * x as u32);
    }
    // Eight in flight, straight through the queues.
    for x in 0..8u16 {
        dev.scale_req.push(&dev::ScaleArgs { x }).unwrap();
    }
    for x in 0..8u16 {
        assert_eq!(dev.scale_resp.pop_wait(t).unwrap(), Some(3 * x as u32));
    }

    for _ in 0..10 {
        dev.ping(t).unwrap();
    }

    for on in [true, false, true] {
        dev.set_led(on, t).unwrap();
        assert_eq!(dev.led(t).unwrap(), on);
    }

    assert!(matches!(dev.slow(1, Duration::ZERO), Err(PortError::Timeout(_))));
    assert_eq!(
        dev.slow(2, t).unwrap(),
        3,
        "slow(1)'s late answer was taken as slow(2)'s"
    );
}
