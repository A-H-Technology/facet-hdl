// hello on the original Zybo, built by chipsmith (no Vivado): the PS7 primitive with
// hello_zybo on M_AXI_GP0 and IRQF2P[0], BTN0 as the button, at 100 MHz
// from the 125 MHz PL clock (facet-hdl examples/hello/zybo/zybo_top.v, with
// the PS7 itself in place of Vivado's block design).
module zybo_top (input wire sysclk, input wire btn0);
    wire clk, locked;
    XC7_PLL #(.INCLK0_INPUT_FREQUENCY(8000), .CLK0_MULTIPLY_BY(4), .CLK0_DIVIDE_BY(5))
        pll (.INCLK(sysclk), .ARESET(1'b0), .CLK0(clk), .LOCKED(locked));

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

    PS7 ps7 (
        .MAXIGP0ACLK(clk),
        .MAXIGP0ARADDR(araddr), .MAXIGP0ARID(arid), .MAXIGP0ARLEN(arlen),
        .MAXIGP0ARVALID(arvalid), .MAXIGP0ARREADY(arready),
        .MAXIGP0AWADDR(awaddr), .MAXIGP0AWID(awid), .MAXIGP0AWLEN(awlen),
        .MAXIGP0AWVALID(awvalid), .MAXIGP0AWREADY(awready),
        .MAXIGP0WDATA(wdata), .MAXIGP0WVALID(wvalid), .MAXIGP0WREADY(wready),
        .MAXIGP0BID(bid), .MAXIGP0BRESP(bresp), .MAXIGP0BVALID(bvalid), .MAXIGP0BREADY(bready),
        .MAXIGP0RID(rid), .MAXIGP0RDATA(rdata), .MAXIGP0RRESP(rresp),
        .MAXIGP0RLAST(rlast), .MAXIGP0RVALID(rvalid), .MAXIGP0RREADY(rready),
        .IRQF2P({19'd0, irq})
    );

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
