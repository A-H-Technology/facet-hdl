//! `#[facet_hdl::rpc]` end to end: the fabric is the caller. It fires 64
//! `add` calls back to back (up to 8 in flight) and 16 `check` calls
//! alongside them, one of which panics in Rust, checks every answer tag by
//! tag, and finally reports its verdict through a `oneway` call.

use facet::Facet;
use facet_hdl::bind;
use facet_hdl_ghdl::Sim;
use facet_hdl_vhdl::Vhdl;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub static REPORTS: Mutex<Vec<Report>> = Mutex::new(Vec::new());
pub static ADDS: Mutex<u32> = Mutex::new(0);

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Report {
    pub checked: u16,
    pub mismatches: u16,
}

#[facet_hdl::rpc]
pub mod calc {
    use super::*;

    /// Overflow comes back as `None`, so the fabric sees an `Option`.
    #[export(depth = 8)]
    pub fn add(a: u32, b: u32) -> Option<u32> {
        *ADDS.lock().unwrap() += 1;
        a.checked_add(b)
    }

    /// 13 panics; other odd numbers are refused.
    #[export]
    pub fn check(x: u8) -> Result<(), u8> {
        if x == 13 {
            panic!("unlucky (this panic is part of the test)");
        }
        if x % 2 == 1 { Err(x) } else { Ok(()) }
    }

    #[export(oneway)]
    pub fn report(r: Report) {
        REPORTS.lock().unwrap().push(r);
    }
}

const TOP: &str = r#"
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.facet_hdl_axil_pkg.all;
use work.calc_pkg.all;

entity calc_top is
  port (
    clk, rst_n : in std_logic;
    axi_in : in axil_m2s_t;
    axi_out : out axil_s2m_t;
    irq : out std_logic
  );
end entity;

architecture rtl of calc_top is
  constant N_ADD : natural := 64;
  constant N_CHECK : natural := 16;

  -- Every 8th call overflows, so `None` comes back.
  function add_args(i : natural) return add_args_t is
  begin
    if i mod 8 = 7 then
      return (a => x"FFFFFFF0", b => x"00000100");
    end if;
    return (a => to_unsigned(i * 7919 + 1, 32), b => to_unsigned(i * 104729, 32));
  end function;

  function add_right(r : call_option_u32_t; i : natural) return boolean is
    constant args : add_args_t := add_args(i);
    constant sum : unsigned(32 downto 0) := resize(args.a, 33) + resize(args.b, 33);
  begin
    if r.tag /= call_option_u32_returned then
      return false;
    elsif sum(32) = '1' then
      return r.returned.tag = option_u32_none;
    else
      return r.returned.tag = option_u32_some and r.returned.some = sum(31 downto 0);
    end if;
  end function;

  function check_right(r : call_result_unit_u8_t; x : natural) return boolean is
  begin
    if x = 13 then
      return r.tag = call_result_unit_u8_panicked;
    elsif x mod 2 = 1 then
      return r.tag = call_result_unit_u8_returned and r.returned.tag = result_unit_u8_err
        and r.returned.err = to_unsigned(x, 8);
    else
      return r.tag = call_result_unit_u8_returned and r.returned.tag = result_unit_u8_ok;
    end if;
  end function;

  signal add_req_valid, add_req_ready, add_resp_valid, add_resp_ready : std_logic;
  signal check_req_valid, check_req_ready, check_resp_valid, check_resp_ready : std_logic;
  signal report_req_valid, report_req_ready : std_logic;
  signal add_resp : call_option_u32_t;
  signal check_resp : call_result_unit_u8_t;
  -- Issued and answered counts per fn: issuing runs ahead of answers.
  signal add_sent, add_got, check_sent, check_got : natural := 0;
  signal add_bad, check_bad : natural := 0;
  signal reported : boolean := false;
begin
  regs : entity work.calc_regs
    port map (
      clk => clk, rst_n => rst_n, axi_in => axi_in, axi_out => axi_out, irq => irq,
      add_req => add_args(add_sent), add_req_valid => add_req_valid, add_req_ready => add_req_ready,
      add_resp => add_resp, add_resp_valid => add_resp_valid, add_resp_ready => add_resp_ready,
      check_req => (x => to_unsigned(check_sent, 8)),
      check_req_valid => check_req_valid, check_req_ready => check_req_ready,
      check_resp => check_resp, check_resp_valid => check_resp_valid, check_resp_ready => check_resp_ready,
      report_req => (r => (checked => to_unsigned(add_got + check_got, 16),
                           mismatches => to_unsigned(add_bad + check_bad, 16))),
      report_req_valid => report_req_valid, report_req_ready => report_req_ready
    );

  add_req_valid <= '1' when add_sent < N_ADD else '0';
  add_resp_ready <= '1' when add_got < N_ADD else '0';
  check_req_valid <= '1' when check_sent < N_CHECK else '0';
  check_resp_ready <= '1' when check_got < N_CHECK else '0';
  report_req_valid <= '1' when add_got = N_ADD and check_got = N_CHECK and not reported else '0';

  process (clk)
  begin
    if rising_edge(clk) then
      if add_req_valid = '1' and add_req_ready = '1' then
        add_sent <= add_sent + 1;
      end if;
      if add_resp_valid = '1' and add_resp_ready = '1' then
        if not add_right(add_resp, add_got) then
          add_bad <= add_bad + 1;
        end if;
        add_got <= add_got + 1;
      end if;
      if check_req_valid = '1' and check_req_ready = '1' then
        check_sent <= check_sent + 1;
      end if;
      if check_resp_valid = '1' and check_resp_ready = '1' then
        if not check_right(check_resp, check_got) then
          check_bad <= check_bad + 1;
        end if;
        check_got <= check_got + 1;
      end if;
      if report_req_valid = '1' and report_req_ready = '1' then
        reported <= true;
      end if;
    end if;
  end process;
end architecture;
"#;

#[test]
fn the_fabric_calls_rust() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-sim");
    std::fs::create_dir_all(&work).unwrap();
    let top = work.join("calc_top.vhd");
    std::fs::write(&top, TOP).unwrap();
    let generated = Vhdl::of::<calc::Calc>().unwrap().write_to(&work.join("gen")).unwrap();
    let sim = Sim::builder(&work, "calc_top")
        .sources(generated)
        .source(&top)
        .irq()
        .build()
        .unwrap();
    let (transport, _clk) = sim.spawn().unwrap();
    let mut calc: calc::Calc = bind(transport).unwrap();

    let deadline = Instant::now() + Duration::from_secs(60);
    while REPORTS.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the fabric never reported");
        if calc.poll().unwrap() == 0 {
            facet_hdl::rpc::wait(&calc.add_req, Duration::from_millis(10)).unwrap();
        }
    }
    assert_eq!(
        REPORTS.lock().unwrap().as_slice(),
        [Report {
            checked: 64 + 16,
            mismatches: 0
        }],
        "the fabric saw wrong answers"
    );
    assert_eq!(*ADDS.lock().unwrap(), 64, "every add ran exactly once");
}
