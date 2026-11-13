# The PS7 as a block design, only for its M_AXI_GP0 and IRQ_F2P: the
# bitstream carries none of the PS's own settings (DDR, MIO, clocks), which
# the board's boot loader already applied, so nothing here needs the board
# preset. Sourced by build.tcl with a project open.
create_bd_design ps
set ps7 [create_bd_cell -type ip -vlnv xilinx.com:ip:processing_system7 ps7]
set_property -dict [list \
    CONFIG.PCW_USE_M_AXI_GP0 1 \
    CONFIG.PCW_USE_FABRIC_INTERRUPT 1 \
    CONFIG.PCW_IRQ_F2P_INTR 1 \
    CONFIG.PCW_EN_CLK0_PORT 0 \
    CONFIG.PCW_EN_RST0_PORT 0 \
] $ps7
make_bd_intf_pins_external [get_bd_intf_pins ps7/DDR] [get_bd_intf_pins ps7/FIXED_IO] [get_bd_intf_pins ps7/M_AXI_GP0]
make_bd_pins_external [get_bd_pins ps7/M_AXI_GP0_ACLK] [get_bd_pins ps7/IRQ_F2P]
# GP0 runs on 100 MHz from the board's 125 MHz PL clock (zybo_top.v), not
# FCLK_CLK0: nothing in Linux's device tree claims FCLK0, so
# clk_disable_unused may stop it.
set_property CONFIG.FREQ_HZ 100000000 [get_bd_ports M_AXI_GP0_ACLK_0]
set_property CONFIG.ASSOCIATED_BUSIF M_AXI_GP0_0 [get_bd_ports M_AXI_GP0_ACLK_0]
set_property CONFIG.SENSITIVITY LEVEL_HIGH [get_bd_ports IRQ_F2P_0]
# GP0's window starts at 0x4000_0000; hello's registers are its first words.
assign_bd_address -offset 0x40000000 -range 1G [get_bd_addr_segs M_AXI_GP0_0/Reg]
validate_bd_design
save_bd_design
make_wrapper -files [get_files ps.bd] -top -import
generate_target all [get_files ps.bd]
