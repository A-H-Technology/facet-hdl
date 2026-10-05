-- The hand-written half of the blinky fabric. Everything it exchanges with
-- the HPS arrives as the typed records generated from src/lib.rs.
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.blinky_pkg.all;

entity blinky_core is
  generic (
    ADDR_WIDTH : positive := 6;
    CLK_HZ     : positive := 100_000_000
  );
  port (
    clk   : in std_logic;
    rst_n : in std_logic;

    s_axi_awaddr  : in  std_logic_vector(ADDR_WIDTH - 1 downto 0);
    s_axi_awvalid : in  std_logic;
    s_axi_awready : out std_logic;
    s_axi_wdata   : in  std_logic_vector(31 downto 0);
    s_axi_wstrb   : in  std_logic_vector(3 downto 0);
    s_axi_wvalid  : in  std_logic;
    s_axi_wready  : out std_logic;
    s_axi_bresp   : out std_logic_vector(1 downto 0);
    s_axi_bvalid  : out std_logic;
    s_axi_bready  : in  std_logic;
    s_axi_araddr  : in  std_logic_vector(ADDR_WIDTH - 1 downto 0);
    s_axi_arvalid : in  std_logic;
    s_axi_arready : out std_logic;
    s_axi_rdata   : out std_logic_vector(31 downto 0);
    s_axi_rresp   : out std_logic_vector(1 downto 0);
    s_axi_rvalid  : out std_logic;
    s_axi_rready  : in  std_logic;

    led : out std_logic_vector(7 downto 0)
  );
end entity;

architecture rtl of blinky_core is
  constant MS_CYCLES : positive := CLK_HZ / 1000;

  signal leds             : led_control_t;
  signal operands         : operands_t;
  signal operands_written : std_logic;

  signal ms_div  : natural range 0 to MS_CYCLES - 1 := 0;
  signal ms_tick : std_logic := '0';
  signal uptime  : unsigned(63 downto 0) := (others => '0');
  signal step_ms : unsigned(15 downto 0) := (others => '0');
  signal phase   : std_logic := '0';
  signal chase   : unsigned(2 downto 0) := (others => '0');
  signal led_i   : unsigned(7 downto 0) := (others => '0');
  signal value   : signed(63 downto 0) := (others => '0');
  signal calls   : unsigned(15 downto 0) := (others => '0');
begin
  regs : entity work.blinky_regs
    generic map (ADDR_WIDTH => ADDR_WIDTH)
    port map (
      clk => clk, rst_n => rst_n,
      s_axi_awaddr => s_axi_awaddr, s_axi_awvalid => s_axi_awvalid, s_axi_awready => s_axi_awready,
      s_axi_wdata => s_axi_wdata, s_axi_wstrb => s_axi_wstrb, s_axi_wvalid => s_axi_wvalid, s_axi_wready => s_axi_wready,
      s_axi_bresp => s_axi_bresp, s_axi_bvalid => s_axi_bvalid, s_axi_bready => s_axi_bready,
      s_axi_araddr => s_axi_araddr, s_axi_arvalid => s_axi_arvalid, s_axi_arready => s_axi_arready,
      s_axi_rdata => s_axi_rdata, s_axi_rresp => s_axi_rresp, s_axi_rvalid => s_axi_rvalid, s_axi_rready => s_axi_rready,
      leds => leds,
      leds_written => open,
      status => (pattern => leds.pattern, leds => led_i, uptime_ms => uptime),
      operands => operands,
      operands_written => operands_written,
      sum => (value => value, calls => calls)
    );

  led <= std_logic_vector(led_i);

  process (clk)
    variable total : unsigned(63 downto 0);
  begin
    if rising_edge(clk) then
      ms_tick <= '0';
      if rst_n = '0' then
        ms_div  <= 0;
        uptime  <= (others => '0');
        step_ms <= (others => '0');
        phase   <= '0';
        chase   <= (others => '0');
        led_i   <= (others => '0');
        value   <= (others => '0');
        calls   <= (others => '0');
      else
        if ms_div = MS_CYCLES - 1 then
          ms_div  <= 0;
          ms_tick <= '1';
          uptime  <= uptime + 1;
        else
          ms_div <= ms_div + 1;
        end if;

        if ms_tick = '1' then
          if step_ms + 1 >= leds.period_ms then
            step_ms <= (others => '0');
            phase   <= not phase;
            chase   <= chase + 1;
          else
            step_ms <= step_ms + 1;
          end if;
        end if;

        case leds.pattern is
          when pattern_off   => led_i <= (others => '0');
          when pattern_solid => led_i <= leds.mask;
          when pattern_blink =>
            if phase = '1' then
              led_i <= leds.mask;
            else
              led_i <= (others => '0');
            end if;
          when pattern_chase => led_i <= leds.mask and shift_left(to_unsigned(1, 8), to_integer(chase));
        end case;

        if operands_written = '1' then
          calls <= calls + 1;
          total := resize(operands.a, 64) + resize(operands.b, 64);
          if operands.negate = '1' then
            value <= -signed(total);
          else
            value <= signed(total);
          end if;
        end if;
      end if;
    end if;
  end process;
end architecture;
