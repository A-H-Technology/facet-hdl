-- facet-hdl's VHDL library: the AXI4 slave port of every `<boundary>_regs`,
-- as one record each way. The field names are the AXI signal names without
-- `s_axi_`; see `flat_axi_ports`/`flat_axi_glue` in facet-hdl-spade for a
-- top level that has the flat signals.
library ieee;
use ieee.std_logic_1164.all;

package facet_hdl_pkg is
  type axi_in_t is record
    awid : std_logic_vector(11 downto 0);
    awaddr : std_logic_vector(31 downto 0);
    awlen : std_logic_vector(7 downto 0);
    awvalid : std_logic;
    wdata : std_logic_vector(31 downto 0);
    wvalid : std_logic;
    bready : std_logic;
    arid : std_logic_vector(11 downto 0);
    araddr : std_logic_vector(31 downto 0);
    arlen : std_logic_vector(7 downto 0);
    arvalid : std_logic;
    rready : std_logic;
  end record;

  type axi_out_t is record
    awready : std_logic;
    wready : std_logic;
    bid : std_logic_vector(11 downto 0);
    bresp : std_logic_vector(1 downto 0);
    bvalid : std_logic;
    arready : std_logic;
    rid : std_logic_vector(11 downto 0);
    rdata : std_logic_vector(31 downto 0);
    rresp : std_logic_vector(1 downto 0);
    rlast : std_logic;
    rvalid : std_logic;
  end record;
end package;
