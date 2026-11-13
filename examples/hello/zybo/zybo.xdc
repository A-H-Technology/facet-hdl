# Original (legacy) Zybo: the 125 MHz clock from the Ethernet PHY and BTN0,
# active high (Digilent Zybo reference manual; chipsmith's Zybo examples).
set_property -dict {PACKAGE_PIN L16 IOSTANDARD LVCMOS33} [get_ports sysclk]
create_clock -period 8.000 -name sysclk [get_ports sysclk]
set_property -dict {PACKAGE_PIN R18 IOSTANDARD LVCMOS33} [get_ports btn0]
# The button is asynchronous and debounced in the fabric; nothing to time.
set_false_path -from [get_ports btn0]
