-- hello_fabric.spade again, in VHDL, on the generated hello_regs
-- (hdl/generated/hello.vhd): the same two calls.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.facet_hdl_pkg.all;
use work.hello_pkg.all;

entity hello_fabric_vhdl is
  port (
    clk, rst, button : in  std_logic;
    axi_in           : in  axi_in_t;
    axi_out          : out axi_out_t;
    irq              : out std_logic
  );
end entity;

architecture rtl of hello_fabric_vhdl is
  signal t        : hello_to_fabric_t;
  signal f        : hello_from_fabric_t;
  signal was_down : std_logic;
begin
  regs : entity work.hello_regs port map (clk, rst, axi_in, axi_out, irq, to_fabric => t, from_fabric => f);

  -- add_one: answered in the cycle Rust's call arrives.
  f.add_one_resp       <= t.add_one_req.x + 1;
  f.add_one_resp_valid <= t.add_one_req_valid;
  f.add_one_req_ready  <= t.add_one_resp_ready;

  -- pressed: one call per press.
  process (clk) begin
    if rising_edge(clk) then
      was_down <= button and not rst;
    end if;
  end process;
  f.pressed_req_valid <= button and not was_down;
end architecture;
