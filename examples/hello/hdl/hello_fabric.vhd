-- hello_fabric.spade again, in VHDL: the same register file (generated
-- hello_core, declared in hello_core_pkg), the same two calls.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.hello_core_pkg.all;

entity hello_fabric_vhdl is
  port (
    clk, rst, button : in  std_logic;
    s_axi_awid       : in  std_logic_vector(11 downto 0);
    s_axi_awaddr     : in  std_logic_vector(31 downto 0);
    s_axi_awlen      : in  std_logic_vector(7 downto 0);
    s_axi_awvalid    : in  std_logic;
    s_axi_awready    : out std_logic;
    s_axi_wdata      : in  std_logic_vector(31 downto 0);
    s_axi_wvalid     : in  std_logic;
    s_axi_wready     : out std_logic;
    s_axi_bid        : out std_logic_vector(11 downto 0);
    s_axi_bresp      : out std_logic_vector(1 downto 0);
    s_axi_bvalid     : out std_logic;
    s_axi_bready     : in  std_logic;
    s_axi_arid       : in  std_logic_vector(11 downto 0);
    s_axi_araddr     : in  std_logic_vector(31 downto 0);
    s_axi_arlen      : in  std_logic_vector(7 downto 0);
    s_axi_arvalid    : in  std_logic;
    s_axi_arready    : out std_logic;
    s_axi_rid        : out std_logic_vector(11 downto 0);
    s_axi_rdata      : out std_logic_vector(31 downto 0);
    s_axi_rresp      : out std_logic_vector(1 downto 0);
    s_axi_rlast      : out std_logic;
    s_axi_rvalid     : out std_logic;
    s_axi_rready     : in  std_logic;
    irq              : out std_logic
  );
end entity;

architecture rtl of hello_fabric_vhdl is
  signal call_valid, answer_ready, was_down, press : std_logic;
  signal call_x, answer                            : std_logic_vector(31 downto 0);
begin
  process (clk) begin
    if rising_edge(clk) then
      was_down <= button and not rst;
    end if;
  end process;
  press  <= button and not was_down;
  answer <= std_logic_vector(unsigned(call_x) + 1);

  core : hello_core port map (
    clk => clk, rst => rst,
    s_axi_awid => s_axi_awid, s_axi_awaddr => s_axi_awaddr, s_axi_awlen => s_axi_awlen,
    s_axi_awvalid => s_axi_awvalid, s_axi_awready => s_axi_awready,
    s_axi_wdata => s_axi_wdata, s_axi_wvalid => s_axi_wvalid, s_axi_wready => s_axi_wready,
    s_axi_bid => s_axi_bid, s_axi_bresp => s_axi_bresp, s_axi_bvalid => s_axi_bvalid, s_axi_bready => s_axi_bready,
    s_axi_arid => s_axi_arid, s_axi_araddr => s_axi_araddr, s_axi_arlen => s_axi_arlen,
    s_axi_arvalid => s_axi_arvalid, s_axi_arready => s_axi_arready,
    s_axi_rid => s_axi_rid, s_axi_rdata => s_axi_rdata, s_axi_rresp => s_axi_rresp,
    s_axi_rlast => s_axi_rlast, s_axi_rvalid => s_axi_rvalid, s_axi_rready => s_axi_rready,
    irq => irq,

    -- add_one: take the call only when its answer can go out the same cycle.
    add_one_req_valid => call_valid, add_one_req_data => call_x, add_one_req_ready => answer_ready,
    add_one_resp_valid => call_valid, add_one_resp_data => answer, add_one_resp_ready => answer_ready,

    -- pressed: one call per press. A press while the queue is full is lost,
    -- as in the Spade version.
    pressed_req_valid => press, pressed_req_ready => open
  );
end architecture;
