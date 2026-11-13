// hello on the original Zybo: the PS7 (ps.tcl's block design) with
// hello_zybo on its M_AXI_GP0 and IRQ_F2P[0], BTN0 as the button, all on
// 100 MHz made from the board's 125 MHz PL clock.
module zybo_top (
    input wire sysclk,
    input wire btn0,
    inout wire [14:0] DDR_addr, inout wire [2:0] DDR_ba, inout wire DDR_cas_n,
    inout wire DDR_ck_n, inout wire DDR_ck_p, inout wire DDR_cke, inout wire DDR_cs_n,
    inout wire [3:0] DDR_dm, inout wire [31:0] DDR_dq, inout wire [3:0] DDR_dqs_n,
    inout wire [3:0] DDR_dqs_p, inout wire DDR_odt, inout wire DDR_ras_n,
    inout wire DDR_reset_n, inout wire DDR_we_n,
    inout wire FIXED_IO_ddr_vrn, inout wire FIXED_IO_ddr_vrp, inout wire [53:0] FIXED_IO_mio,
    inout wire FIXED_IO_ps_clk, inout wire FIXED_IO_ps_porb, inout wire FIXED_IO_ps_srstb
);
    // 125 MHz misses timing by 0.2 ns: the PS7's GP0 outputs come late in the
    // cycle and the register file's read path behind RREADY is 13 levels deep.
    // 125 * 8 / 10 = 100 MHz, VCO at 1 GHz.
    wire fb, clk_pll, locked, clk;
    PLLE2_BASE #(.CLKIN1_PERIOD(8.0), .CLKFBOUT_MULT(8), .CLKOUT0_DIVIDE(10)) pll (
        .CLKIN1(sysclk), .CLKFBIN(fb), .CLKFBOUT(fb), .CLKOUT0(clk_pll), .LOCKED(locked),
        .RST(1'b0), .PWRDWN(1'b0),
        .CLKOUT1(), .CLKOUT2(), .CLKOUT3(), .CLKOUT4(), .CLKOUT5()
    );
    BUFG bufg (.I(clk_pll), .O(clk));

    // Nothing resets the PL after a load, so hold hello in reset until the
    // PLL locks and a few cycles after.
    reg [3:0] por = 4'hF;
    always @(posedge clk or negedge locked)
        if (!locked) por <= 4'hF; else por <= {por[2:0], 1'b0};
    wire rst = por[3];

    wire [31:0] araddr, awaddr, rdata, wdata;
    wire [11:0] arid, awid, bid, rid;
    wire [3:0] arlen, awlen;
    wire [1:0] bresp, rresp;
    wire arvalid, arready, awvalid, awready, wvalid, wready, bvalid, bready, rvalid, rready, rlast;
    wire irq;

    ps_wrapper ps (
        .DDR_0_addr(DDR_addr), .DDR_0_ba(DDR_ba), .DDR_0_cas_n(DDR_cas_n),
        .DDR_0_ck_n(DDR_ck_n), .DDR_0_ck_p(DDR_ck_p), .DDR_0_cke(DDR_cke), .DDR_0_cs_n(DDR_cs_n),
        .DDR_0_dm(DDR_dm), .DDR_0_dq(DDR_dq), .DDR_0_dqs_n(DDR_dqs_n), .DDR_0_dqs_p(DDR_dqs_p),
        .DDR_0_odt(DDR_odt), .DDR_0_ras_n(DDR_ras_n), .DDR_0_reset_n(DDR_reset_n), .DDR_0_we_n(DDR_we_n),
        .FIXED_IO_0_ddr_vrn(FIXED_IO_ddr_vrn), .FIXED_IO_0_ddr_vrp(FIXED_IO_ddr_vrp), .FIXED_IO_0_mio(FIXED_IO_mio),
        .FIXED_IO_0_ps_clk(FIXED_IO_ps_clk), .FIXED_IO_0_ps_porb(FIXED_IO_ps_porb), .FIXED_IO_0_ps_srstb(FIXED_IO_ps_srstb),
        .IRQ_F2P_0(irq),
        .M_AXI_GP0_ACLK_0(clk),
        .M_AXI_GP0_0_araddr(araddr), .M_AXI_GP0_0_arid(arid), .M_AXI_GP0_0_arlen(arlen),
        .M_AXI_GP0_0_arvalid(arvalid), .M_AXI_GP0_0_arready(arready),
        .M_AXI_GP0_0_awaddr(awaddr), .M_AXI_GP0_0_awid(awid), .M_AXI_GP0_0_awlen(awlen),
        .M_AXI_GP0_0_awvalid(awvalid), .M_AXI_GP0_0_awready(awready),
        .M_AXI_GP0_0_wdata(wdata), .M_AXI_GP0_0_wvalid(wvalid), .M_AXI_GP0_0_wready(wready),
        .M_AXI_GP0_0_bid(bid), .M_AXI_GP0_0_bresp(bresp), .M_AXI_GP0_0_bvalid(bvalid), .M_AXI_GP0_0_bready(bready),
        .M_AXI_GP0_0_rid(rid), .M_AXI_GP0_0_rdata(rdata), .M_AXI_GP0_0_rresp(rresp),
        .M_AXI_GP0_0_rlast(rlast), .M_AXI_GP0_0_rvalid(rvalid), .M_AXI_GP0_0_rready(rready)
    );

    // AxSIZE/AxBURST/WSTRB/WLAST/WID go unread: the slave takes every beat as
    // a counting 32-bit word, and AXI3's WID only matters to interleaving
    // writes, which GP0 doesn't do.
    hello_zybo hello (
        .clk(clk), .rst(rst), .btn0(btn0),
        .s_axi_awid(awid), .s_axi_awaddr(awaddr), .s_axi_awlen(awlen), .s_axi_awvalid(awvalid), .s_axi_awready(awready),
        .s_axi_wdata(wdata), .s_axi_wvalid(wvalid), .s_axi_wready(wready),
        .s_axi_bid(bid), .s_axi_bresp(bresp), .s_axi_bvalid(bvalid), .s_axi_bready(bready),
        .s_axi_arid(arid), .s_axi_araddr(araddr), .s_axi_arlen(arlen), .s_axi_arvalid(arvalid), .s_axi_arready(arready),
        .s_axi_rid(rid), .s_axi_rdata(rdata), .s_axi_rresp(rresp), .s_axi_rlast(rlast), .s_axi_rvalid(rvalid), .s_axi_rready(rready),
        .irq(irq)
    );
endmodule
