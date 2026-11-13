// hello on the DE25-Nano, built by chipsmith (no Quartus): hello_de25 on
// the HPS's lightweight bridge (its ports bound to the bridge's signals in
// chipsmith.toml), on CLOCK1_50, with KEY0 as the button and its interrupt
// on f2h_irq0[0]. Nothing resets the fabric after a load, so a counter holds
// hello in reset for its first cycles.
module de25_top (
    input  wire        CLOCK1_50,
    input  wire        KEY0,
    output wire        lw_clk,
    output wire        irq,
    input  wire [3:0]  awid,
    input  wire [28:0] awaddr,
    input  wire [7:0]  awlen,
    input  wire        awvalid,
    output wire        awready,
    input  wire [31:0] wdata,
    input  wire        wvalid,
    output wire        wready,
    output wire [3:0]  bid,
    output wire [1:0]  bresp,
    output wire        bvalid,
    input  wire        bready,
    input  wire [3:0]  arid,
    input  wire [28:0] araddr,
    input  wire [7:0]  arlen,
    input  wire        arvalid,
    output wire        arready,
    output wire [3:0]  rid,
    output wire [31:0] rdata,
    output wire [1:0]  rresp,
    output wire        rlast,
    output wire        rvalid,
    input  wire        rready
);
    wire clk = CLOCK1_50;
    assign lw_clk = clk;
    reg [3:0] por = 4'hF;
    always @(posedge clk) por <= {por[2:0], 1'b0};
    hello_de25 hello (
        .clk(clk), .rst(por[3]), .key0_n(KEY0),
        .s_axi_awid(awid), .s_axi_awaddr(awaddr), .s_axi_awlen(awlen),
        .s_axi_awvalid(awvalid), .s_axi_awready(awready),
        .s_axi_wdata(wdata), .s_axi_wvalid(wvalid), .s_axi_wready(wready),
        .s_axi_bid(bid), .s_axi_bresp(bresp), .s_axi_bvalid(bvalid), .s_axi_bready(bready),
        .s_axi_arid(arid), .s_axi_araddr(araddr), .s_axi_arlen(arlen),
        .s_axi_arvalid(arvalid), .s_axi_arready(arready),
        .s_axi_rid(rid), .s_axi_rdata(rdata), .s_axi_rresp(rresp),
        .s_axi_rlast(rlast), .s_axi_rvalid(rvalid), .s_axi_rready(rready),
        .irq(irq));
endmodule
