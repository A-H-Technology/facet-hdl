// hello_fabric.spade again, in plain Verilog: the same register file
// (generated hello_core), the same two calls.

`default_nettype none
module hello_fabric_v (
  input  wire        clk,
  input  wire        rst,
  input  wire        button,
  input  wire [11:0] s_axi_awid,
  input  wire [31:0] s_axi_awaddr,
  input  wire [7:0]  s_axi_awlen,
  input  wire        s_axi_awvalid,
  output wire        s_axi_awready,
  input  wire [31:0] s_axi_wdata,
  input  wire        s_axi_wvalid,
  output wire        s_axi_wready,
  output wire [11:0] s_axi_bid,
  output wire [1:0]  s_axi_bresp,
  output wire        s_axi_bvalid,
  input  wire        s_axi_bready,
  input  wire [11:0] s_axi_arid,
  input  wire [31:0] s_axi_araddr,
  input  wire [7:0]  s_axi_arlen,
  input  wire        s_axi_arvalid,
  output wire        s_axi_arready,
  output wire [11:0] s_axi_rid,
  output wire [31:0] s_axi_rdata,
  output wire [1:0]  s_axi_rresp,
  output wire        s_axi_rlast,
  output wire        s_axi_rvalid,
  input  wire        s_axi_rready,
  output wire        irq
);
  wire        call_valid, answer_ready;
  wire [31:0] call_x;
  reg         was_down;

  always @(posedge clk)
    was_down <= rst ? 1'b0 : button;

  hello_core core (
    .clk(clk), .rst(rst),
    .s_axi_awid(s_axi_awid), .s_axi_awaddr(s_axi_awaddr), .s_axi_awlen(s_axi_awlen),
    .s_axi_awvalid(s_axi_awvalid), .s_axi_awready(s_axi_awready),
    .s_axi_wdata(s_axi_wdata), .s_axi_wvalid(s_axi_wvalid), .s_axi_wready(s_axi_wready),
    .s_axi_bid(s_axi_bid), .s_axi_bresp(s_axi_bresp), .s_axi_bvalid(s_axi_bvalid), .s_axi_bready(s_axi_bready),
    .s_axi_arid(s_axi_arid), .s_axi_araddr(s_axi_araddr), .s_axi_arlen(s_axi_arlen),
    .s_axi_arvalid(s_axi_arvalid), .s_axi_arready(s_axi_arready),
    .s_axi_rid(s_axi_rid), .s_axi_rdata(s_axi_rdata), .s_axi_rresp(s_axi_rresp),
    .s_axi_rlast(s_axi_rlast), .s_axi_rvalid(s_axi_rvalid), .s_axi_rready(s_axi_rready),
    .irq(irq),

    // add_one: take the call only when its answer can go out the same cycle.
    .add_one_req_valid(call_valid), .add_one_req_data(call_x), .add_one_req_ready(answer_ready),
    .add_one_resp_valid(call_valid), .add_one_resp_data(call_x + 32'd1), .add_one_resp_ready(answer_ready),

    // pressed: one call per press. A press while the queue is full is lost,
    // as in the Spade version.
    .pressed_req_valid(button && !was_down), .pressed_req_ready()
  );
endmodule
