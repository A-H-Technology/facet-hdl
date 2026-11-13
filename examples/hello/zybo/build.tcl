# Run by build.sh in build/vivado.
create_project -in_memory -part xc7z010clg400-1
set_property target_language Verilog [current_project]
source ../../ps.tcl
read_verilog ../../zybo_top.v
read_verilog -sv ../spade.sv
read_xdc ../../zybo.xdc
synth_design -top zybo_top
opt_design
place_design
route_design
report_timing_summary -file timing.rpt
report_utilization -file utilization.rpt
write_bitstream -force ../hello.bit
