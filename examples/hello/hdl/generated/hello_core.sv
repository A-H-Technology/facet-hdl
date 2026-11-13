`ifdef COCOTB_SIM
    `define COCOTB_CODE(name) \
        string __top_module; \
        string __vcd_file; \
        initial begin \
            if ($value$plusargs("TOP_MODULE=%s", __top_module) && __top_module == `"name`" && $value$plusargs("VCD_FILENAME=%s", __vcd_file)) begin \
                $dumpfile (__vcd_file); \
                $dumpvars (0, \name ); \
            end \
        end
`else
    `define COCOTB_CODE(name)
`endif


module \std::cdc::sync2[2107]  (
        input clk_i,
        input input_i,
        output output__
    );
    `COCOTB_CODE( std::cdc::sync2[2107] )
    logic \clk ;
    assign \clk  = clk_i;
    logic \input ;
    assign \input  = input_i;
    (* src = "<compiler dir>/stdlib/cdc.spade:14,14" *)
    reg \sync1 ;
    (* src = "<compiler dir>/stdlib/cdc.spade:15,14" *)
    reg \sync2 ;
    always @(posedge \clk ) begin
        \sync1  <= \input ;
    end
    always @(posedge \clk ) begin
        \sync2  <= \sync1 ;
    end
    assign output__ = \sync2 ;
endmodule


module \std::cdc::sync2_bool  (
        input clk_i,
        input input_i,
        output output__
    );
    `COCOTB_CODE( std::cdc::sync2_bool )
    logic \clk ;
    assign \clk  = clk_i;
    logic \input ;
    assign \input  = input_i;
    (* src = "<compiler dir>/stdlib/cdc.spade:31,14" *)
    logic _e_496;
    (* src = "<compiler dir>/stdlib/cdc.spade:31,14" *)
    
    \std::cdc::sync2[2107]  \sync2_0 (.clk_i(\clk ), .input_i(\input ), .output__(_e_496));
    assign output__ = _e_496;
endmodule


module \std::conv::tri_to_bool  (
        input t_i,
        output output__
    );
    `COCOTB_CODE( std::conv::tri_to_bool )
    logic \t ;
    assign \t  = t_i;
    (* src = "<compiler dir>/stdlib/conv.spade:80,5" *)
    logic _e_786;
    (* src = "<compiler dir>/stdlib/conv.spade:80,5" *)
    
    \std::conv::impl#13::as_bool  \as_bool_0 (.self_i(\t ), .output__(_e_786));
    assign output__ = _e_786;
endmodule


module \std::io::rising_edge  (
        input clk_i,
        input value_i,
        output output__
    );
    `COCOTB_CODE( std::io::rising_edge )
    logic \clk ;
    assign \clk  = clk_i;
    logic \value ;
    assign \value  = value_i;
    (* src = "<compiler dir>/stdlib/io.spade:12,14" *)
    reg \old_value ;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_3711;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_3710;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_1028;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_3723;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_3724;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_3722;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_1026;
    always @(posedge \clk ) begin
        \old_value  <= \value ;
    end
    assign _e_3711 = {\old_value };
    assign _e_3710 = ~_e_3711;
    assign _e_1028 = _e_3710;
    assign _e_3723 = {\value };
    assign _e_3724 = {_e_1028};
    assign _e_3722 = _e_3723 & _e_3724;
    assign _e_1026 = _e_3722;
    assign output__ = _e_1026;
endmodule


module \std::io::falling_edge  (
        input clk_i,
        input value_i,
        output output__
    );
    `COCOTB_CODE( std::io::falling_edge )
    logic \clk ;
    assign \clk  = clk_i;
    logic \value ;
    assign \value  = value_i;
    (* src = "<compiler dir>/stdlib/io.spade:27,14" *)
    reg \old_value ;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_3737;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_3736;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_1035;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_3749;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_3750;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_3748;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_1034;
    always @(posedge \clk ) begin
        \old_value  <= \value ;
    end
    assign _e_3737 = {\value };
    assign _e_3736 = ~_e_3737;
    assign _e_1035 = _e_3736;
    assign _e_3749 = {_e_1035};
    assign _e_3750 = {\old_value };
    assign _e_3748 = _e_3749 & _e_3750;
    assign _e_1034 = _e_3748;
    assign output__ = _e_1034;
endmodule


module hello_core (
        input clk,
        input rst,
        input[11:0] s_axi_awid,
        input[31:0] s_axi_awaddr,
        input[7:0] s_axi_awlen,
        input s_axi_awvalid,
        output s_axi_awready,
        input[31:0] s_axi_wdata,
        input s_axi_wvalid,
        output s_axi_wready,
        output[11:0] s_axi_bid,
        output[1:0] s_axi_bresp,
        output s_axi_bvalid,
        input s_axi_bready,
        input[11:0] s_axi_arid,
        input[31:0] s_axi_araddr,
        input[7:0] s_axi_arlen,
        input s_axi_arvalid,
        output s_axi_arready,
        output[11:0] s_axi_rid,
        output[31:0] s_axi_rdata,
        output[1:0] s_axi_rresp,
        output s_axi_rlast,
        output s_axi_rvalid,
        input s_axi_rready,
        output irq,
        output add_one_req_valid,
        output[31:0] add_one_req_data,
        input add_one_req_ready,
        input add_one_resp_valid,
        input[31:0] add_one_resp_data,
        output add_one_resp_ready,
        input pressed_req_valid,
        output pressed_req_ready
    );
    `COCOTB_CODE( hello_core )
    logic \s_axi_awready_mut ;
    assign s_axi_awready = \s_axi_awready_mut ;
    logic \s_axi_wready_mut ;
    assign s_axi_wready = \s_axi_wready_mut ;
    logic[11:0] \s_axi_bid_mut ;
    assign s_axi_bid = \s_axi_bid_mut ;
    logic[1:0] \s_axi_bresp_mut ;
    assign s_axi_bresp = \s_axi_bresp_mut ;
    logic \s_axi_bvalid_mut ;
    assign s_axi_bvalid = \s_axi_bvalid_mut ;
    logic \s_axi_arready_mut ;
    assign s_axi_arready = \s_axi_arready_mut ;
    logic[11:0] \s_axi_rid_mut ;
    assign s_axi_rid = \s_axi_rid_mut ;
    logic[31:0] \s_axi_rdata_mut ;
    assign s_axi_rdata = \s_axi_rdata_mut ;
    logic[1:0] \s_axi_rresp_mut ;
    assign s_axi_rresp = \s_axi_rresp_mut ;
    logic \s_axi_rlast_mut ;
    assign s_axi_rlast = \s_axi_rlast_mut ;
    logic \s_axi_rvalid_mut ;
    assign s_axi_rvalid = \s_axi_rvalid_mut ;
    logic \irq_mut ;
    assign irq = \irq_mut ;
    logic \add_one_req_valid_mut ;
    assign add_one_req_valid = \add_one_req_valid_mut ;
    logic[31:0] \add_one_req_data_mut ;
    assign add_one_req_data = \add_one_req_data_mut ;
    logic \add_one_resp_ready_mut ;
    assign add_one_resp_ready = \add_one_resp_ready_mut ;
    logic \pressed_req_ready_mut ;
    assign pressed_req_ready = \pressed_req_ready_mut ;
    (* src = "top.spade:22,11" *)
    logic[140:0] \axi ;
    (* src = "top.spade:23,34" *)
    logic[65:0] _e_3418;
    (* src = "top.spade:23,34" *)
    logic[65:0] _e_3419_mut;
    (* src = "top.spade:23,34" *)
    logic[65:0] _e_1834;
    (* src = "top.spade:23,34" *)
    logic[65:0] _e_1834_mut;
    (* src = "top.spade:23,9" *)
    logic[65:0] \axi_out ;
    (* src = "top.spade:23,9" *)
    logic[65:0] \axi_out_inv_mut ;
    (* src = "top.spade:25,25" *)
    logic _e_1841;
    (* src = "top.spade:25,55" *)
    logic _e_1845;
    (* src = "top.spade:26,21" *)
    logic[11:0] _e_1849;
    (* src = "top.spade:26,46" *)
    logic[1:0] _e_1853;
    (* src = "top.spade:26,74" *)
    logic _e_1857;
    (* src = "top.spade:27,25" *)
    logic _e_1861;
    (* src = "top.spade:27,52" *)
    logic[11:0] _e_1865;
    (* src = "top.spade:27,77" *)
    logic[31:0] _e_1869;
    (* src = "top.spade:28,23" *)
    logic[1:0] _e_1873;
    (* src = "top.spade:28,50" *)
    logic _e_1877;
    (* src = "top.spade:28,78" *)
    logic _e_1881;
    (* src = "top.spade:29,13" *)
    logic[101:0] \p ;
    (* src = "top.spade:29,13" *)
    logic[34:0] \p_mut ;
    (* src = "top.spade:30,23" *)
    logic[65:0] _e_1890;
    (* src = "top.spade:31,15" *)
    logic _e_1894;
    (* src = "top.spade:32,35" *)
    logic[32:0] _e_1900;
    (* src = "top.spade:32,35" *)
    logic _e_1900_mut;
    (* src = "top.spade:32,35" *)
    logic[32:0] _e_1899;
    (* src = "top.spade:32,29" *)
    logic[32:0] _e_1903;
    (* src = "top.spade:32,56" *)
    logic[31:0] \_ ;
    logic _e_3443;
    localparam[0:0] _e_3444 = 1;
    logic _e_3445;
    localparam[0:0] _e_1904 = 1;
    (* src = "top.spade:32,29" *)
    logic[32:0] _e_1905;
    logic _e_3448;
    localparam[0:0] _e_1906 = 0;
    (* src = "top.spade:32,29" *)
    logic _e_1898;
    (* src = "top.spade:33,34" *)
    logic[32:0] _e_1910;
    (* src = "top.spade:33,28" *)
    logic[32:0] _e_1914;
    (* src = "top.spade:33,55" *)
    logic[31:0] \v ;
    logic _e_3460;
    localparam[0:0] _e_3461 = 1;
    logic _e_3462;
    (* src = "top.spade:33,66" *)
    logic[31:0] _e_1915;
    (* src = "top.spade:33,28" *)
    logic[32:0] _e_1917;
    logic _e_3465;
    localparam[31:0] _e_1918 = 32'd0;
    (* src = "top.spade:33,28" *)
    logic[31:0] _e_1909;
    (* src = "top.spade:34,9" *)
    logic _e_1920_mut;
    (* src = "top.spade:35,9" *)
    logic _e_1926;
    (* src = "top.spade:35,9" *)
    logic[32:0] _e_1926_mut;
    (* src = "top.spade:35,9" *)
    logic[32:0] _e_1925_mut;
    (* src = "top.spade:35,56" *)
    logic[32:0] _e_1931;
    (* src = "top.spade:35,89" *)
    logic[32:0] _e_1934;
    (* src = "top.spade:35,32" *)
    logic[32:0] _e_1928;
    (* src = "top.spade:36,31" *)
    logic _e_1938;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_3865;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_3864;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_1937;
    (* src = "top.spade:37,9" *)
    logic _e_1943;
    (* src = "top.spade:37,9" *)
    logic _e_1943_mut;
    (* src = "top.spade:37,9" *)
    logic _e_1942_mut;
    (* src = "top.spade:37,54" *)
    logic _e_1948;
    (* src = "top.spade:37,72" *)
    logic _e_1951;
    (* src = "top.spade:37,31" *)
    logic _e_1945;
    (* src = "top.spade:38,30" *)
    logic _e_1955;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_3889;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_3888;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_1954;
    logic[65:0] \o ;
    assign \axi  = {\s_axi_awid , \s_axi_awaddr , \s_axi_awlen , \s_axi_awvalid , \s_axi_wdata , \s_axi_wvalid , \s_axi_bready , \s_axi_arid , \s_axi_araddr , \s_axi_arlen , \s_axi_arvalid , \s_axi_rready };
    
    assign _e_3418 = _e_3419_mut;
    assign _e_1834 = {_e_3418};
    assign {_e_3419_mut} = _e_1834_mut;
    assign \axi_out  = _e_1834[65:0];
    assign _e_1834_mut[65:0] = \axi_out_inv_mut ;
    assign _e_1841 = \axi_out [65];
    assign \s_axi_awready_mut  = _e_1841;
    
    assign _e_1845 = \axi_out [64];
    assign \s_axi_wready_mut  = _e_1845;
    
    assign _e_1849 = \axi_out [63:52];
    assign \s_axi_bid_mut  = _e_1849;
    
    assign _e_1853 = \axi_out [51:50];
    assign \s_axi_bresp_mut  = _e_1853;
    
    assign _e_1857 = \axi_out [49];
    assign \s_axi_bvalid_mut  = _e_1857;
    
    assign _e_1861 = \axi_out [48];
    assign \s_axi_arready_mut  = _e_1861;
    
    assign _e_1865 = \axi_out [47:36];
    assign \s_axi_rid_mut  = _e_1865;
    
    assign _e_1869 = \axi_out [35:4];
    assign \s_axi_rdata_mut  = _e_1869;
    
    assign _e_1873 = \axi_out [3:2];
    assign \s_axi_rresp_mut  = _e_1873;
    
    assign _e_1877 = \axi_out [1];
    assign \s_axi_rlast_mut  = _e_1877;
    
    assign _e_1881 = \axi_out [0];
    assign \s_axi_rvalid_mut  = _e_1881;
    
    (* src = "top.spade:29,13" *)
    
    \hello::hello_regs  \hello_regs_0 (.clk_i(\clk ), .rst_i(\rst ), .axi_i(\axi ), .output__(\p ), .input__(\p_mut ));
    assign _e_1890 = \p [101:36];
    assign \axi_out_inv_mut  = _e_1890;
    
    assign _e_1894 = \p [35];
    assign \irq_mut  = _e_1894;
    
    assign _e_1900 = \p [34:2];
    assign \p_mut [34] = _e_1900_mut;
    assign _e_1899 = _e_1900[32:0];
    
    assign \_  = _e_1903[31:0];
    assign _e_3443 = _e_1903[32] == 1'd1;
    assign _e_3445 = _e_3443 && _e_3444;
    
    assign _e_3448 = _e_1905[32] == 1'd0;
    always_comb begin
        priority casez ({_e_3445, _e_3448})
            2'b1?: _e_1898 = _e_1904;
            2'b01: _e_1898 = _e_1906;
            2'b?: _e_1898 = 1'dx;
        endcase
    end
    assign _e_1903 = _e_1899;
    assign _e_1905 = _e_1899;
    assign \add_one_req_valid_mut  = _e_1898;
    
    assign _e_1910 = _e_1900[32:0];
    
    assign \v  = _e_1914[31:0];
    assign _e_3460 = _e_1914[32] == 1'd1;
    assign _e_3462 = _e_3460 && _e_3461;
    (* src = "top.spade:33,66" *)
    
    \hello::pack_add_one_args  \pack_add_one_args_0 (.v_i(\v ), .output__(_e_1915));
    
    assign _e_3465 = _e_1917[32] == 1'd0;
    always_comb begin
        priority casez ({_e_3462, _e_3465})
            2'b1?: _e_1909 = _e_1915;
            2'b01: _e_1909 = _e_1918;
            2'b?: _e_1909 = 32'dx;
        endcase
    end
    assign _e_1914 = _e_1910;
    assign _e_1917 = _e_1910;
    assign \add_one_req_data_mut  = _e_1909;
    
    assign _e_1900_mut = _e_1920_mut;
    assign _e_1920_mut = \add_one_req_ready ;
    
    assign _e_1926 = \p [1];
    assign \p_mut [33:1] = _e_1926_mut;
    assign _e_1926_mut[32:0] = _e_1925_mut;
    assign _e_1931 = {1'd1, \add_one_resp_data };
    assign _e_1934 = {1'd0, 32'bX};
    assign _e_1928 = \add_one_resp_valid  ? _e_1931 : _e_1934;
    assign _e_1925_mut = _e_1928;
    
    assign _e_1938 = _e_1926;
    assign _e_3865 = {_e_1938};
    assign _e_3864 = ~_e_3865;
    assign _e_1937 = _e_3864;
    assign \add_one_resp_ready_mut  = _e_1937;
    
    assign _e_1943 = \p [0];
    assign \p_mut [0] = _e_1943_mut;
    assign _e_1943_mut = _e_1942_mut;
    
    assign _e_1948 = {1'd1};
    assign _e_1951 = {1'd0};
    assign _e_1945 = \pressed_req_valid  ? _e_1948 : _e_1951;
    assign _e_1942_mut = _e_1945;
    
    assign _e_1955 = _e_1943;
    assign _e_3889 = {_e_1955};
    assign _e_3888 = ~_e_3889;
    assign _e_1954 = _e_3888;
    assign \pressed_req_ready_mut  = _e_1954;
    
    
    assign \o  = \axi_out ;
endmodule


module \facet_hdl::word_of  (
        input[31:0] addr_i,
        output[29:0] output__
    );
    `COCOTB_CODE( facet_hdl::word_of )
    logic[31:0] \addr ;
    assign \addr  = addr_i;
    localparam[31:0] _e_1963 = 32'd2;
    (* src = "facet_hdl.spade:69,11" *)
    logic[31:0] _e_1961;
    (* src = "facet_hdl.spade:69,5" *)
    logic[29:0] _e_1960;
    assign _e_1961 = \addr  >> _e_1963;
    assign _e_1960 = _e_1961[29:0];
    assign output__ = _e_1960;
endmodule


module \facet_hdl::axi4_slave  (
        input clk_i,
        input rst_i,
        input[140:0] i_i,
        input[31:0] rdata_i,
        output[159:0] output__
    );
    `COCOTB_CODE( facet_hdl::axi4_slave )
    logic \clk ;
    assign \clk  = clk_i;
    logic \rst ;
    assign \rst  = rst_i;
    logic[140:0] \i ;
    assign \i  = i_i;
    logic[31:0] \rdata ;
    assign \rdata  = rdata_i;
    (* src = "facet_hdl.spade:73,38" *)
    logic[84:0] _e_1968;
    (* src = "facet_hdl.spade:73,59" *)
    logic[84:0] _e_1969;
    (* src = "facet_hdl.spade:73,74" *)
    logic[84:0] _e_1972;
    logic _e_3422;
    (* src = "facet_hdl.spade:74,27" *)
    logic _e_1974;
    (* src = "facet_hdl.spade:75,26" *)
    logic[11:0] _e_1978;
    (* src = "facet_hdl.spade:75,42" *)
    logic[31:0] _e_1981;
    (* src = "facet_hdl.spade:75,34" *)
    logic[29:0] _e_1980;
    (* src = "facet_hdl.spade:75,53" *)
    logic[7:0] _e_1983;
    (* src = "facet_hdl.spade:75,13" *)
    logic[84:0] _e_1977;
    (* src = "facet_hdl.spade:76,19" *)
    logic _e_1986;
    (* src = "facet_hdl.spade:77,25" *)
    logic[11:0] _e_1990;
    (* src = "facet_hdl.spade:77,41" *)
    logic[31:0] _e_1993;
    (* src = "facet_hdl.spade:77,33" *)
    logic[29:0] _e_1992;
    (* src = "facet_hdl.spade:77,52" *)
    logic[7:0] _e_1995;
    (* src = "facet_hdl.spade:77,13" *)
    logic[84:0] _e_1989;
    (* src = "facet_hdl.spade:79,13" *)
    logic[84:0] _e_1998;
    (* src = "facet_hdl.spade:76,16" *)
    logic[84:0] _e_1985;
    (* src = "facet_hdl.spade:74,24" *)
    logic[84:0] _e_1973;
    (* src = "facet_hdl.spade:73,74" *)
    logic[84:0] _e_2002;
    (* src = "facet_hdl.spade:81,9" *)
    logic[11:0] \id ;
    (* src = "facet_hdl.spade:81,9" *)
    logic[29:0] \word ;
    (* src = "facet_hdl.spade:81,9" *)
    logic[7:0] \left ;
    logic _e_3451;
    localparam[0:0] _e_3452 = 1;
    localparam[0:0] _e_3453 = 1;
    localparam[0:0] _e_3454 = 1;
    logic _e_3455;
    logic _e_3456;
    logic _e_3457;
    (* src = "facet_hdl.spade:81,45" *)
    logic _e_2005;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_3907;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_3906;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_2004;
    localparam[7:0] _e_2012 = 0;
    (* src = "<compiler dir>/core/ops.spade:53,9" *)
    logic _e_2010;
    (* src = "facet_hdl.spade:84,13" *)
    logic[84:0] _e_2014;
    localparam[29:0] _e_2022 = 1;
    (* src = "facet_hdl.spade:86,36" *)
    logic[30:0] _e_2020;
    (* src = "facet_hdl.spade:86,30" *)
    logic[29:0] _e_2019;
    localparam[7:0] _e_2026 = 1;
    (* src = "facet_hdl.spade:86,53" *)
    logic[8:0] _e_2024;
    (* src = "facet_hdl.spade:86,47" *)
    logic[7:0] _e_2023;
    (* src = "facet_hdl.spade:86,13" *)
    logic[84:0] _e_2017;
    (* src = "facet_hdl.spade:83,16" *)
    logic[84:0] _e_2009;
    (* src = "facet_hdl.spade:81,41" *)
    logic[84:0] _e_2003;
    (* src = "facet_hdl.spade:73,74" *)
    logic[84:0] _e_2028;
    (* src = "facet_hdl.spade:88,9" *)
    logic[11:0] id_n1;
    logic _e_3468;
    localparam[0:0] _e_3469 = 1;
    logic _e_3470;
    (* src = "facet_hdl.spade:88,36" *)
    logic _e_2030;
    (* src = "facet_hdl.spade:88,47" *)
    logic[84:0] _e_2033;
    (* src = "facet_hdl.spade:88,33" *)
    logic[84:0] _e_2029;
    (* src = "facet_hdl.spade:73,74" *)
    logic[84:0] _e_2039;
    (* src = "facet_hdl.spade:89,9" *)
    logic[11:0] id_n2;
    (* src = "facet_hdl.spade:89,9" *)
    logic[29:0] word_n1;
    (* src = "facet_hdl.spade:89,9" *)
    logic[7:0] left_n1;
    logic _e_3473;
    localparam[0:0] _e_3474 = 1;
    localparam[0:0] _e_3475 = 1;
    localparam[0:0] _e_3476 = 1;
    logic _e_3477;
    logic _e_3478;
    logic _e_3479;
    (* src = "facet_hdl.spade:89,40" *)
    logic[84:0] _e_2040;
    (* src = "facet_hdl.spade:73,74" *)
    logic[84:0] _e_2049;
    (* src = "facet_hdl.spade:90,9" *)
    logic[11:0] id_n3;
    (* src = "facet_hdl.spade:90,9" *)
    logic[29:0] word_n2;
    (* src = "facet_hdl.spade:90,9" *)
    logic[7:0] left_n2;
    (* src = "facet_hdl.spade:90,9" *)
    logic[31:0] \data ;
    logic _e_3482;
    localparam[0:0] _e_3483 = 1;
    localparam[0:0] _e_3484 = 1;
    localparam[0:0] _e_3485 = 1;
    localparam[0:0] _e_3486 = 1;
    logic _e_3487;
    logic _e_3488;
    logic _e_3489;
    logic _e_3490;
    (* src = "facet_hdl.spade:90,54" *)
    logic _e_2052;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_3925;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_3924;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_2051;
    localparam[7:0] _e_2059 = 0;
    (* src = "<compiler dir>/core/ops.spade:53,9" *)
    logic _e_2057;
    (* src = "facet_hdl.spade:93,13" *)
    logic[84:0] _e_2061;
    localparam[29:0] _e_2068 = 1;
    (* src = "facet_hdl.spade:95,35" *)
    logic[30:0] _e_2066;
    (* src = "facet_hdl.spade:95,29" *)
    logic[29:0] _e_2065;
    localparam[7:0] _e_2072 = 1;
    (* src = "facet_hdl.spade:95,52" *)
    logic[8:0] _e_2070;
    (* src = "facet_hdl.spade:95,46" *)
    logic[7:0] _e_2069;
    (* src = "facet_hdl.spade:95,13" *)
    logic[84:0] _e_2063;
    (* src = "facet_hdl.spade:92,16" *)
    logic[84:0] _e_2056;
    (* src = "facet_hdl.spade:90,50" *)
    logic[84:0] _e_2050;
    (* src = "facet_hdl.spade:73,74" *)
    logic[84:0] _e_1970;
    (* src = "facet_hdl.spade:73,14" *)
    reg[84:0] \state ;
    (* src = "facet_hdl.spade:99,16" *)
    logic[84:0] _e_2075;
    logic _e_3511;
    localparam[0:0] _e_2076 = 1;
    (* src = "facet_hdl.spade:99,16" *)
    logic[84:0] \_ ;
    localparam[0:0] _e_3513 = 1;
    localparam[0:0] _e_2078 = 0;
    (* src = "facet_hdl.spade:99,16" *)
    logic \idle ;
    (* src = "facet_hdl.spade:100,25" *)
    logic[84:0] _e_2083;
    (* src = "facet_hdl.spade:100,39" *)
    logic[11:0] id_n4;
    logic _e_3516;
    localparam[0:0] _e_3517 = 1;
    logic _e_3518;
    localparam[0:0] _e_2085 = 1;
    (* src = "facet_hdl.spade:100,63" *)
    logic[12:0] _e_2084;
    (* src = "facet_hdl.spade:100,25" *)
    logic[84:0] __n1;
    localparam[0:0] _e_3520 = 1;
    localparam[0:0] _e_2089 = 0;
    localparam[11:0] _e_2090 = 0;
    (* src = "facet_hdl.spade:100,80" *)
    logic[12:0] _e_2088;
    (* src = "facet_hdl.spade:100,25" *)
    logic[12:0] _e_2080;
    (* src = "facet_hdl.spade:100,9" *)
    logic \bvalid ;
    (* src = "facet_hdl.spade:100,9" *)
    logic[11:0] \bid ;
    (* src = "facet_hdl.spade:101,43" *)
    logic[84:0] _e_2100;
    (* src = "facet_hdl.spade:102,9" *)
    logic[11:0] id_n5;
    (* src = "facet_hdl.spade:102,9" *)
    logic[29:0] word_n3;
    (* src = "facet_hdl.spade:102,9" *)
    logic[7:0] left_n3;
    (* src = "facet_hdl.spade:102,9" *)
    logic[31:0] data_n1;
    logic _e_3523;
    localparam[0:0] _e_3524 = 1;
    localparam[0:0] _e_3525 = 1;
    localparam[0:0] _e_3526 = 1;
    localparam[0:0] _e_3527 = 1;
    logic _e_3528;
    logic _e_3529;
    logic _e_3530;
    logic _e_3531;
    localparam[0:0] _e_2102 = 1;
    localparam[7:0] _e_2106 = 0;
    (* src = "<compiler dir>/core/ops.spade:53,9" *)
    logic _e_2104;
    (* src = "facet_hdl.spade:102,50" *)
    logic[45:0] _e_2101;
    (* src = "facet_hdl.spade:101,43" *)
    logic[84:0] __n2;
    localparam[0:0] _e_3541 = 1;
    localparam[0:0] _e_2110 = 0;
    localparam[11:0] _e_2111 = 0;
    localparam[0:0] _e_2112 = 0;
    localparam[31:0] _e_2113 = 32'd0;
    (* src = "facet_hdl.spade:103,14" *)
    logic[45:0] _e_2109;
    (* src = "facet_hdl.spade:101,43" *)
    logic[45:0] _e_2094;
    (* src = "facet_hdl.spade:101,9" *)
    logic \rvalid ;
    (* src = "facet_hdl.spade:101,9" *)
    logic[11:0] \rid ;
    (* src = "facet_hdl.spade:101,9" *)
    logic \rlast ;
    (* src = "facet_hdl.spade:101,9" *)
    logic[31:0] \rdata_out ;
    (* src = "facet_hdl.spade:105,17" *)
    logic[84:0] _e_2124;
    (* src = "facet_hdl.spade:106,9" *)
    logic[11:0] id_n6;
    (* src = "facet_hdl.spade:106,9" *)
    logic[29:0] word_n4;
    (* src = "facet_hdl.spade:106,9" *)
    logic[7:0] left_n4;
    logic _e_3544;
    localparam[0:0] _e_3545 = 1;
    localparam[0:0] _e_3546 = 1;
    localparam[0:0] _e_3547 = 1;
    logic _e_3548;
    logic _e_3549;
    logic _e_3550;
    (* src = "facet_hdl.spade:106,44" *)
    logic _e_2126;
    (* src = "facet_hdl.spade:106,75" *)
    logic[31:0] _e_2132;
    (* src = "facet_hdl.spade:106,60" *)
    logic[61:0] _e_2130;
    (* src = "facet_hdl.spade:106,55" *)
    logic[62:0] _e_2129;
    (* src = "facet_hdl.spade:106,94" *)
    logic[62:0] _e_2135;
    (* src = "facet_hdl.spade:106,41" *)
    logic[62:0] _e_2125;
    (* src = "facet_hdl.spade:105,17" *)
    logic[84:0] __n3;
    localparam[0:0] _e_3558 = 1;
    (* src = "facet_hdl.spade:107,14" *)
    logic[62:0] _e_2137;
    (* src = "facet_hdl.spade:105,17" *)
    logic[62:0] \write ;
    (* src = "facet_hdl.spade:109,16" *)
    logic[84:0] _e_2144;
    (* src = "facet_hdl.spade:110,9" *)
    logic[11:0] id_n7;
    (* src = "facet_hdl.spade:110,9" *)
    logic[29:0] word_n5;
    (* src = "facet_hdl.spade:110,9" *)
    logic[7:0] left_n5;
    logic _e_3565;
    localparam[0:0] _e_3566 = 1;
    localparam[0:0] _e_3567 = 1;
    localparam[0:0] _e_3568 = 1;
    logic _e_3569;
    logic _e_3570;
    logic _e_3571;
    (* src = "facet_hdl.spade:110,40" *)
    logic[30:0] _e_2145;
    (* src = "facet_hdl.spade:109,16" *)
    logic[84:0] __n4;
    localparam[0:0] _e_3573 = 1;
    (* src = "facet_hdl.spade:111,14" *)
    logic[30:0] _e_2148;
    (* src = "facet_hdl.spade:109,16" *)
    logic[30:0] \read ;
    (* src = "facet_hdl.spade:115,21" *)
    logic _e_2154;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_3949;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_3950;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_3948;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_2152;
    (* src = "facet_hdl.spade:116,13" *)
    logic[84:0] _e_2161;
    (* src = "facet_hdl.spade:116,27" *)
    logic[11:0] __n5;
    (* src = "facet_hdl.spade:116,27" *)
    logic[29:0] __n6;
    (* src = "facet_hdl.spade:116,27" *)
    logic[7:0] __n7;
    logic _e_3576;
    localparam[0:0] _e_3577 = 1;
    localparam[0:0] _e_3578 = 1;
    localparam[0:0] _e_3579 = 1;
    logic _e_3580;
    logic _e_3581;
    logic _e_3582;
    localparam[0:0] _e_2162 = 1;
    (* src = "facet_hdl.spade:116,13" *)
    logic[84:0] __n8;
    localparam[0:0] _e_3584 = 1;
    localparam[0:0] _e_2164 = 0;
    (* src = "facet_hdl.spade:116,13" *)
    logic _e_2156;
    localparam[1:0] _e_2166 = 0;
    (* src = "facet_hdl.spade:120,22" *)
    logic _e_2172;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_3963;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_3962;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_2171;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_3975;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_3976;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_3974;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_2169;
    (* src = "facet_hdl.spade:120,35" *)
    logic _e_2174;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_3989;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_3990;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_3988;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_2168;
    localparam[1:0] _e_2178 = 0;
    (* src = "facet_hdl.spade:114,9" *)
    logic[65:0] _e_2151;
    (* src = "facet_hdl.spade:113,5" *)
    logic[159:0] _e_2150;
    assign _e_1968 = {3'd0, 82'bX};
    assign _e_1969 = {3'd0, 82'bX};
    
    assign _e_3422 = _e_1972[84:82] == 3'd0;
    assign _e_1974 = \i [88];
    assign _e_1978 = \i [140:129];
    assign _e_1981 = \i [128:97];
    (* src = "facet_hdl.spade:75,34" *)
    
    \facet_hdl::word_of  \word_of_0 (.addr_i(_e_1981), .output__(_e_1980));
    assign _e_1983 = \i [96:89];
    assign _e_1977 = {3'd1, _e_1978, _e_1980, _e_1983, 32'bX};
    assign _e_1986 = \i [1];
    assign _e_1990 = \i [53:42];
    assign _e_1993 = \i [41:10];
    (* src = "facet_hdl.spade:77,33" *)
    
    \facet_hdl::word_of  \word_of_1 (.addr_i(_e_1993), .output__(_e_1992));
    assign _e_1995 = \i [9:2];
    assign _e_1989 = {3'd3, _e_1990, _e_1992, _e_1995, 32'bX};
    assign _e_1998 = {3'd0, 82'bX};
    assign _e_1985 = _e_1986 ? _e_1989 : _e_1998;
    assign _e_1973 = _e_1974 ? _e_1977 : _e_1985;
    
    assign \id  = _e_2002[81:70];
    assign \word  = _e_2002[69:40];
    assign \left  = _e_2002[39:32];
    assign _e_3451 = _e_2002[84:82] == 3'd1;
    assign _e_3455 = _e_3451 && _e_3452;
    assign _e_3456 = _e_3455 && _e_3453;
    assign _e_3457 = _e_3456 && _e_3454;
    assign _e_2005 = \i [55];
    assign _e_3907 = {_e_2005};
    assign _e_3906 = ~_e_3907;
    assign _e_2004 = _e_3906;
    assign _e_2010 = \left  == _e_2012;
    assign _e_2014 = {3'd2, \id , 70'bX};
    assign _e_2020 = \word  + _e_2022;
    assign _e_2019 = _e_2020[29:0];
    assign _e_2024 = \left  - _e_2026;
    assign _e_2023 = _e_2024[7:0];
    assign _e_2017 = {3'd1, \id , _e_2019, _e_2023, 32'bX};
    assign _e_2009 = _e_2010 ? _e_2014 : _e_2017;
    assign _e_2003 = _e_2004 ? \state  : _e_2009;
    
    assign id_n1 = _e_2028[81:70];
    assign _e_3468 = _e_2028[84:82] == 3'd2;
    assign _e_3470 = _e_3468 && _e_3469;
    assign _e_2030 = \i [54];
    assign _e_2033 = {3'd0, 82'bX};
    assign _e_2029 = _e_2030 ? _e_2033 : \state ;
    
    assign id_n2 = _e_2039[81:70];
    assign word_n1 = _e_2039[69:40];
    assign left_n1 = _e_2039[39:32];
    assign _e_3473 = _e_2039[84:82] == 3'd3;
    assign _e_3477 = _e_3473 && _e_3474;
    assign _e_3478 = _e_3477 && _e_3475;
    assign _e_3479 = _e_3478 && _e_3476;
    assign _e_2040 = {3'd4, id_n2, word_n1, left_n1, \rdata };
    
    assign id_n3 = _e_2049[81:70];
    assign word_n2 = _e_2049[69:40];
    assign left_n2 = _e_2049[39:32];
    assign \data  = _e_2049[31:0];
    assign _e_3482 = _e_2049[84:82] == 3'd4;
    assign _e_3487 = _e_3482 && _e_3483;
    assign _e_3488 = _e_3487 && _e_3484;
    assign _e_3489 = _e_3488 && _e_3485;
    assign _e_3490 = _e_3489 && _e_3486;
    assign _e_2052 = \i [0];
    assign _e_3925 = {_e_2052};
    assign _e_3924 = ~_e_3925;
    assign _e_2051 = _e_3924;
    assign _e_2057 = left_n2 == _e_2059;
    assign _e_2061 = {3'd0, 82'bX};
    assign _e_2066 = word_n2 + _e_2068;
    assign _e_2065 = _e_2066[29:0];
    assign _e_2070 = left_n2 - _e_2072;
    assign _e_2069 = _e_2070[7:0];
    assign _e_2063 = {3'd3, id_n3, _e_2065, _e_2069, 32'bX};
    assign _e_2056 = _e_2057 ? _e_2061 : _e_2063;
    assign _e_2050 = _e_2051 ? \state  : _e_2056;
    always_comb begin
        priority casez ({_e_3422, _e_3457, _e_3470, _e_3479, _e_3490})
            5'b1????: _e_1970 = _e_1973;
            5'b01???: _e_1970 = _e_2003;
            5'b001??: _e_1970 = _e_2029;
            5'b0001?: _e_1970 = _e_2040;
            5'b00001: _e_1970 = _e_2050;
            5'b?: _e_1970 = 85'dx;
        endcase
    end
    assign _e_1972 = \state ;
    assign _e_2002 = \state ;
    assign _e_2028 = \state ;
    assign _e_2039 = \state ;
    assign _e_2049 = \state ;
    initial begin
        \state  = 'b000XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX;
    end
    always @(posedge \clk ) begin
        if (\rst ) begin
            \state  <= _e_1968;
        end
        else begin
            \state  <= _e_1970;
        end
    end
    
    assign _e_3511 = _e_2075[84:82] == 3'd0;
    
    always_comb begin
        priority casez ({_e_3511, _e_3513})
            2'b1?: \idle  = _e_2076;
            2'b01: \idle  = _e_2078;
            2'b?: \idle  = 1'dx;
        endcase
    end
    assign _e_2075 = \state ;
    assign \_  = \state ;
    
    assign id_n4 = _e_2083[81:70];
    assign _e_3516 = _e_2083[84:82] == 3'd2;
    assign _e_3518 = _e_3516 && _e_3517;
    assign _e_2084 = {_e_2085, id_n4};
    
    assign _e_2088 = {_e_2089, _e_2090};
    always_comb begin
        priority casez ({_e_3518, _e_3520})
            2'b1?: _e_2080 = _e_2084;
            2'b01: _e_2080 = _e_2088;
            2'b?: _e_2080 = 13'dx;
        endcase
    end
    assign _e_2083 = \state ;
    assign __n1 = \state ;
    assign \bvalid  = _e_2080[12];
    assign \bid  = _e_2080[11:0];
    
    assign id_n5 = _e_2100[81:70];
    assign word_n3 = _e_2100[69:40];
    assign left_n3 = _e_2100[39:32];
    assign data_n1 = _e_2100[31:0];
    assign _e_3523 = _e_2100[84:82] == 3'd4;
    assign _e_3528 = _e_3523 && _e_3524;
    assign _e_3529 = _e_3528 && _e_3525;
    assign _e_3530 = _e_3529 && _e_3526;
    assign _e_3531 = _e_3530 && _e_3527;
    assign _e_2104 = left_n3 == _e_2106;
    assign _e_2101 = {_e_2102, id_n5, _e_2104, data_n1};
    
    assign _e_2109 = {_e_2110, _e_2111, _e_2112, _e_2113};
    always_comb begin
        priority casez ({_e_3531, _e_3541})
            2'b1?: _e_2094 = _e_2101;
            2'b01: _e_2094 = _e_2109;
            2'b?: _e_2094 = 46'dx;
        endcase
    end
    assign _e_2100 = \state ;
    assign __n2 = \state ;
    assign \rvalid  = _e_2094[45];
    assign \rid  = _e_2094[44:33];
    assign \rlast  = _e_2094[32];
    assign \rdata_out  = _e_2094[31:0];
    
    assign id_n6 = _e_2124[81:70];
    assign word_n4 = _e_2124[69:40];
    assign left_n4 = _e_2124[39:32];
    assign _e_3544 = _e_2124[84:82] == 3'd1;
    assign _e_3548 = _e_3544 && _e_3545;
    assign _e_3549 = _e_3548 && _e_3546;
    assign _e_3550 = _e_3549 && _e_3547;
    assign _e_2126 = \i [55];
    assign _e_2132 = \i [87:56];
    assign _e_2130 = {word_n4, _e_2132};
    assign _e_2129 = {1'd1, _e_2130};
    assign _e_2135 = {1'd0, 62'bX};
    assign _e_2125 = _e_2126 ? _e_2129 : _e_2135;
    
    assign _e_2137 = {1'd0, 62'bX};
    always_comb begin
        priority casez ({_e_3550, _e_3558})
            2'b1?: \write  = _e_2125;
            2'b01: \write  = _e_2137;
            2'b?: \write  = 63'dx;
        endcase
    end
    assign _e_2124 = \state ;
    assign __n3 = \state ;
    
    assign id_n7 = _e_2144[81:70];
    assign word_n5 = _e_2144[69:40];
    assign left_n5 = _e_2144[39:32];
    assign _e_3565 = _e_2144[84:82] == 3'd3;
    assign _e_3569 = _e_3565 && _e_3566;
    assign _e_3570 = _e_3569 && _e_3567;
    assign _e_3571 = _e_3570 && _e_3568;
    assign _e_2145 = {1'd1, word_n5};
    
    assign _e_2148 = {1'd0, 30'bX};
    always_comb begin
        priority casez ({_e_3571, _e_3573})
            2'b1?: \read  = _e_2145;
            2'b01: \read  = _e_2148;
            2'b?: \read  = 31'dx;
        endcase
    end
    assign _e_2144 = \state ;
    assign __n4 = \state ;
    assign _e_2154 = \i [88];
    assign _e_3949 = {\idle };
    assign _e_3950 = {_e_2154};
    assign _e_3948 = _e_3949 & _e_3950;
    assign _e_2152 = _e_3948;
    
    assign __n5 = _e_2161[81:70];
    assign __n6 = _e_2161[69:40];
    assign __n7 = _e_2161[39:32];
    assign _e_3576 = _e_2161[84:82] == 3'd1;
    assign _e_3580 = _e_3576 && _e_3577;
    assign _e_3581 = _e_3580 && _e_3578;
    assign _e_3582 = _e_3581 && _e_3579;
    
    always_comb begin
        priority casez ({_e_3582, _e_3584})
            2'b1?: _e_2156 = _e_2162;
            2'b01: _e_2156 = _e_2164;
            2'b?: _e_2156 = 1'dx;
        endcase
    end
    assign _e_2161 = \state ;
    assign __n8 = \state ;
    assign _e_2172 = \i [88];
    assign _e_3963 = {_e_2172};
    assign _e_3962 = ~_e_3963;
    assign _e_2171 = _e_3962;
    assign _e_3975 = {\idle };
    assign _e_3976 = {_e_2171};
    assign _e_3974 = _e_3975 & _e_3976;
    assign _e_2169 = _e_3974;
    assign _e_2174 = \i [1];
    assign _e_3989 = {_e_2169};
    assign _e_3990 = {_e_2174};
    assign _e_3988 = _e_3989 & _e_3990;
    assign _e_2168 = _e_3988;
    assign _e_2151 = {_e_2152, _e_2156, \bid , _e_2166, \bvalid , _e_2168, \rid , \rdata_out , _e_2178, \rlast , \rvalid };
    assign _e_2150 = {_e_2151, \write , \read };
    assign output__ = _e_2150;
endmodule


module \hello::pack_add_one_args  (
        input[31:0] v_i,
        output[31:0] output__
    );
    `COCOTB_CODE( hello::pack_add_one_args )
    logic[31:0] \v ;
    assign \v  = v_i;
    (* src = "hello.spade:14,24" *)
    logic[31:0] \f0 ;
    assign \f0  = \v [31:0];
    assign output__ = \f0 ;
endmodule


module \hello::unpack_add_one_args  (
        input[31:0] b_i,
        output[31:0] output__
    );
    `COCOTB_CODE( hello::unpack_add_one_args )
    logic[31:0] \b ;
    assign \b  = b_i;
    (* src = "hello.spade:20,5" *)
    logic[31:0] _e_2191;
    logic[31:0] \f0 ;
    assign _e_2191 = {\b };
    assign \f0  = \b ;
    assign output__ = _e_2191;
endmodule


module \hello::hello_regs  (
        input clk_i,
        input rst_i,
        input[140:0] axi_i,
        output[101:0] output__,
        input[34:0] input__
    );
    `COCOTB_CODE( hello::hello_regs )
    logic \clk ;
    assign \clk  = clk_i;
    logic \rst ;
    assign \rst  = rst_i;
    logic[140:0] \axi ;
    assign \axi  = axi_i;
    (* src = "hello.spade:36,15" *)
    logic[159:0] \bus ;
    (* src = "hello.spade:37,40" *)
    logic[62:0] _e_2201;
    (* src = "hello.spade:37,34" *)
    logic[62:0] _e_2204;
    (* src = "hello.spade:38,9" *)
    logic[61:0] \w ;
    logic _e_3425;
    localparam[0:0] _e_3426 = 1;
    logic _e_3427;
    localparam[0:0] _e_2206 = 1;
    (* src = "hello.spade:38,27" *)
    logic[29:0] _e_2207;
    (* src = "hello.spade:38,35" *)
    logic[31:0] _e_2209;
    (* src = "hello.spade:38,20" *)
    logic[62:0] _e_2205;
    (* src = "hello.spade:37,34" *)
    logic[62:0] _e_2211;
    logic _e_3430;
    localparam[0:0] _e_2213 = 0;
    localparam[29:0] _e_2214 = 0;
    localparam[31:0] _e_2215 = 32'd0;
    (* src = "hello.spade:39,17" *)
    logic[62:0] _e_2212;
    (* src = "hello.spade:37,34" *)
    logic[62:0] _e_2200;
    (* src = "hello.spade:37,9" *)
    logic \wr ;
    (* src = "hello.spade:37,9" *)
    logic[29:0] \wr_word ;
    (* src = "hello.spade:37,9" *)
    logic[31:0] \wr_data ;
    (* src = "hello.spade:41,31" *)
    logic[30:0] _e_2221;
    (* src = "hello.spade:41,25" *)
    logic[30:0] _e_2224;
    (* src = "hello.spade:42,9" *)
    logic[29:0] \word ;
    logic _e_3433;
    localparam[0:0] _e_3434 = 1;
    logic _e_3435;
    localparam[0:0] _e_2226 = 1;
    (* src = "hello.spade:42,23" *)
    logic[30:0] _e_2225;
    (* src = "hello.spade:41,25" *)
    logic[30:0] _e_2228;
    logic _e_3438;
    localparam[0:0] _e_2230 = 0;
    localparam[29:0] _e_2231 = 0;
    (* src = "hello.spade:43,17" *)
    logic[30:0] _e_2229;
    (* src = "hello.spade:41,25" *)
    logic[30:0] _e_2220;
    (* src = "hello.spade:41,9" *)
    logic \rd ;
    (* src = "hello.spade:41,9" *)
    logic[29:0] \rd_word ;
    (* src = "hello.spade:46,28" *)
    logic[31:0] _e_2236;
    localparam[31:0] _e_2239 = 32'd4;
    (* src = "<compiler dir>/core/ops.spade:500,9" *)
    logic \add_one_req_room ;
    (* src = "hello.spade:47,27" *)
    logic _e_3439;
    (* src = "hello.spade:47,27" *)
    logic _e_3440_mut;
    (* src = "hello.spade:47,27" *)
    logic \add_one_req_ack ;
    (* src = "hello.spade:47,27" *)
    logic \add_one_req_ack_mut ;
    (* src = "hello.spade:48,29" *)
    logic \add_one_req_ready ;
    localparam[29:0] _e_2251 = 3;
    (* src = "<compiler dir>/core/ops.spade:53,9" *)
    logic _e_2249;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4015;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4016;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4014;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_2247;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4029;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4030;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4028;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \add_one_req_push ;
    (* src = "hello.spade:50,5" *)
    logic[31:0] _e_2254[0:3];
    (* src = "hello.spade:51,9" *)
    logic[32:0] \add_one_req_rdp ;
    (* src = "hello.spade:51,9" *)
    logic[3:0] \add_one_req_rdp_mut ;
    (* src = "hello.spade:52,9" *)
    logic[35:0] \add_one_req_wrp_mut ;
    (* src = "hello.spade:54,60" *)
    logic[1:0] _e_2263;
    (* src = "hello.spade:54,59" *)
    logic[33:0] _e_2262;
    (* src = "hello.spade:54,54" *)
    logic[34:0] _e_2261;
    (* src = "hello.spade:54,104" *)
    logic[34:0] _e_2267;
    (* src = "hello.spade:54,32" *)
    logic[34:0] _e_2258;
    (* src = "hello.spade:55,70" *)
    logic[1:0] _e_2272;
    (* src = "hello.spade:55,65" *)
    logic[2:0] _e_2271;
    (* src = "hello.spade:55,39" *)
    logic[32:0] _e_2269;
    localparam[31:0] _e_2274 = 32'd0;
    (* src = "hello.spade:55,39" *)
    logic[31:0] \add_one_req_entry ;
    (* src = "<compiler dir>/core/ops.spade:58,9" *)
    logic _e_2277;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4049;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4050;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4048;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \add_one_req_pop ;
    localparam[31:0] _e_2285 = 32'd0;
    localparam[31:0] _e_2286 = 32'd0;
    localparam[31:0] _e_2292 = 32'd1;
    (* src = "hello.spade:57,90" *)
    logic[31:0] _e_2290;
    (* src = "hello.spade:57,68" *)
    logic[31:0] _e_2287;
    (* src = "hello.spade:57,14" *)
    reg[31:0] \add_one_req_head ;
    localparam[31:0] _e_2298 = 32'd0;
    localparam[31:0] _e_2299 = 32'd0;
    localparam[31:0] _e_2305 = 32'd1;
    (* src = "hello.spade:58,89" *)
    logic[31:0] _e_2303;
    (* src = "hello.spade:58,68" *)
    logic[31:0] _e_2300;
    (* src = "hello.spade:58,14" *)
    reg[31:0] \add_one_req_tail ;
    (* src = "hello.spade:60,29" *)
    logic[31:0] _e_2309;
    localparam[31:0] _e_2312 = 32'd4;
    (* src = "<compiler dir>/core/ops.spade:500,9" *)
    logic \add_one_resp_room ;
    (* src = "hello.spade:61,30" *)
    logic[32:0] _e_3491;
    (* src = "hello.spade:61,30" *)
    logic[32:0] _e_3492_mut;
    (* src = "hello.spade:61,30" *)
    logic[32:0] \add_one_resp_write ;
    (* src = "hello.spade:61,30" *)
    logic[32:0] \add_one_resp_write_mut ;
    (* src = "hello.spade:62,36" *)
    logic[32:0] _e_2317;
    (* src = "hello.spade:62,30" *)
    logic[32:0] _e_2320;
    (* src = "hello.spade:62,59" *)
    logic[31:0] \_ ;
    logic _e_3495;
    localparam[0:0] _e_3496 = 1;
    logic _e_3497;
    localparam[0:0] _e_2321 = 1;
    (* src = "hello.spade:62,30" *)
    logic[32:0] _e_2322;
    logic _e_3500;
    localparam[0:0] _e_2323 = 0;
    (* src = "hello.spade:62,30" *)
    logic \add_one_resp_valid ;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_4069;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_4068;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_2326;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4081;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4082;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4080;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \add_one_resp_ready_now ;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4095;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4096;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4094;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \add_one_resp_push ;
    localparam[29:0] _e_2339 = 4;
    (* src = "<compiler dir>/core/ops.spade:53,9" *)
    logic _e_2337;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4115;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4116;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4114;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_2335;
    (* src = "hello.spade:68,50" *)
    logic[31:0] _e_2341;
    (* src = "hello.spade:68,93" *)
    logic[31:0] _e_2344;
    (* src = "<compiler dir>/core/ops.spade:505,9" *)
    logic _e_2340;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4135;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4136;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4134;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \add_one_resp_pop ;
    localparam[31:0] _e_2351 = 32'd0;
    localparam[31:0] _e_2352 = 32'd0;
    localparam[31:0] _e_2358 = 32'd1;
    (* src = "hello.spade:69,92" *)
    logic[31:0] _e_2356;
    (* src = "hello.spade:69,69" *)
    logic[31:0] _e_2353;
    (* src = "hello.spade:69,14" *)
    reg[31:0] \add_one_resp_head ;
    localparam[31:0] _e_2364 = 32'd0;
    localparam[31:0] _e_2365 = 32'd0;
    (* src = "hello.spade:70,69" *)
    logic[31:0] _e_2366;
    (* src = "hello.spade:70,14" *)
    reg[31:0] \add_one_resp_tail ;
    (* src = "hello.spade:71,5" *)
    logic[31:0] _e_2372[0:3];
    (* src = "hello.spade:72,9" *)
    logic[32:0] \add_one_resp_rdp ;
    (* src = "hello.spade:72,9" *)
    logic[3:0] \add_one_resp_rdp_mut ;
    (* src = "hello.spade:73,9" *)
    logic[35:0] \add_one_resp_wrp_mut ;
    (* src = "hello.spade:75,62" *)
    logic[1:0] _e_2381;
    (* src = "hello.spade:75,94" *)
    logic[32:0] _e_2384;
    (* src = "hello.spade:75,88" *)
    logic[32:0] _e_2387;
    (* src = "hello.spade:75,117" *)
    logic[31:0] \add_one_resp_x ;
    logic _e_3534;
    localparam[0:0] _e_3535 = 1;
    logic _e_3536;
    (* src = "hello.spade:75,88" *)
    logic[32:0] _e_2389;
    logic _e_3539;
    localparam[31:0] _e_2390 = 32'd0;
    (* src = "hello.spade:75,88" *)
    logic[31:0] _e_2383;
    (* src = "hello.spade:75,61" *)
    logic[33:0] _e_2380;
    (* src = "hello.spade:75,56" *)
    logic[34:0] _e_2379;
    (* src = "hello.spade:75,180" *)
    logic[34:0] _e_2392;
    (* src = "hello.spade:75,33" *)
    logic[34:0] _e_2376;
    (* src = "hello.spade:76,72" *)
    logic[1:0] _e_2397;
    (* src = "hello.spade:76,67" *)
    logic[2:0] _e_2396;
    (* src = "hello.spade:76,40" *)
    logic[32:0] _e_2394;
    localparam[31:0] _e_2399 = 32'd0;
    (* src = "hello.spade:76,40" *)
    logic[31:0] \add_one_resp_entry ;
    (* src = "hello.spade:78,28" *)
    logic[31:0] _e_2402;
    localparam[31:0] _e_2405 = 32'd4;
    (* src = "<compiler dir>/core/ops.spade:500,9" *)
    logic \pressed_req_room ;
    (* src = "hello.spade:79,29" *)
    logic _e_3551;
    (* src = "hello.spade:79,29" *)
    logic _e_3552_mut;
    (* src = "hello.spade:79,29" *)
    logic \pressed_req_write ;
    (* src = "hello.spade:79,29" *)
    logic \pressed_req_write_mut ;
    (* src = "hello.spade:80,35" *)
    logic _e_2410;
    (* src = "hello.spade:80,29" *)
    logic _e_2413;
    logic _e_3555;
    localparam[0:0] _e_3556 = 1;
    logic _e_3559;
    localparam[0:0] _e_2414 = 1;
    (* src = "hello.spade:80,29" *)
    logic _e_2415;
    logic _e_3562;
    localparam[0:0] _e_2416 = 0;
    (* src = "hello.spade:80,29" *)
    logic \pressed_req_valid ;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_4155;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_4154;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_2419;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4167;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4168;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4166;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \pressed_req_ready_now ;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4181;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4182;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4180;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \pressed_req_push ;
    localparam[29:0] _e_2432 = 7;
    (* src = "<compiler dir>/core/ops.spade:53,9" *)
    logic _e_2430;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4201;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4202;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4200;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_2428;
    (* src = "hello.spade:86,49" *)
    logic[31:0] _e_2434;
    (* src = "hello.spade:86,91" *)
    logic[31:0] _e_2437;
    (* src = "<compiler dir>/core/ops.spade:505,9" *)
    logic _e_2433;
    (* src = "<compiler dir>/core/ops.spade:343,29" *)
    logic _e_4221;
    (* src = "<compiler dir>/core/ops.spade:343,37" *)
    logic _e_4222;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic _e_4220;
    (* src = "<compiler dir>/core/ops.spade:343,9" *)
    logic \pressed_req_pop ;
    localparam[31:0] _e_2444 = 32'd0;
    localparam[31:0] _e_2445 = 32'd0;
    localparam[31:0] _e_2451 = 32'd1;
    (* src = "hello.spade:87,90" *)
    logic[31:0] _e_2449;
    (* src = "hello.spade:87,68" *)
    logic[31:0] _e_2446;
    (* src = "hello.spade:87,14" *)
    reg[31:0] \pressed_req_head ;
    localparam[31:0] _e_2457 = 32'd0;
    localparam[31:0] _e_2458 = 32'd0;
    (* src = "hello.spade:88,68" *)
    logic[31:0] _e_2459;
    (* src = "hello.spade:88,14" *)
    reg[31:0] \pressed_req_tail ;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2467;
    localparam[29:0] _e_3587 = 0;
    logic _e_3586;
    localparam[31:0] _e_2468 = 32'd21893230;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2469;
    localparam[29:0] _e_3590 = 1;
    logic _e_3589;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2471;
    localparam[29:0] _e_3593 = 2;
    logic _e_3592;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2473;
    localparam[29:0] _e_3596 = 4;
    logic _e_3595;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2475;
    localparam[29:0] _e_3599 = 5;
    logic _e_3598;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2477;
    localparam[29:0] _e_3602 = 6;
    logic _e_3601;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2479;
    localparam[29:0] _e_3605 = 7;
    logic _e_3604;
    (* src = "hello.spade:90,27" *)
    logic[29:0] _e_2481;
    localparam[29:0] _e_3608 = 8;
    logic _e_3607;
    (* src = "hello.spade:90,27" *)
    logic[29:0] __n2;
    localparam[0:0] _e_3610 = 1;
    localparam[31:0] _e_2484 = 32'd0;
    (* src = "hello.spade:90,27" *)
    logic[31:0] \rdata ;
    (* src = "hello.spade:101,16" *)
    logic[65:0] _e_2487;
    (* src = "<compiler dir>/core/ops.spade:58,9" *)
    logic _e_2490;
    (* src = "<compiler dir>/core/ops.spade:58,9" *)
    logic _e_2493;
    (* src = "<compiler dir>/core/ops.spade:387,28" *)
    logic _e_4247;
    (* src = "<compiler dir>/core/ops.spade:387,36" *)
    logic _e_4248;
    (* src = "<compiler dir>/core/ops.spade:387,9" *)
    logic _e_4246;
    (* src = "<compiler dir>/core/ops.spade:387,9" *)
    logic _e_2489;
    (* src = "<compiler dir>/core/ops.spade:58,9" *)
    logic _e_2498;
    (* src = "hello.spade:101,168" *)
    logic[31:0] _e_2503;
    (* src = "hello.spade:101,163" *)
    logic[32:0] _e_2502;
    (* src = "hello.spade:101,217" *)
    logic[32:0] _e_2506;
    (* src = "hello.spade:101,121" *)
    logic[32:0] _e_2497;
    (* src = "hello.spade:101,225" *)
    logic _e_2507_mut;
    (* src = "hello.spade:101,105" *)
    logic[32:0] _e_2496;
    (* src = "hello.spade:101,105" *)
    logic _e_2496_mut;
    (* src = "hello.spade:101,262" *)
    logic[32:0] _e_2510_mut;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_4267;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_4266;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_2512;
    (* src = "hello.spade:101,245" *)
    logic _e_2509;
    (* src = "hello.spade:101,245" *)
    logic[32:0] _e_2509_mut;
    (* src = "hello.spade:101,327" *)
    logic _e_2515_mut;
    (* src = "<compiler dir>/core/ops.spade:301,29" *)
    logic _e_4279;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_4278;
    (* src = "<compiler dir>/core/ops.spade:301,9" *)
    logic _e_2517;
    (* src = "hello.spade:101,310" *)
    logic _e_2514;
    (* src = "hello.spade:101,310" *)
    logic _e_2514_mut;
    (* src = "hello.spade:101,5" *)
    logic[101:0] _e_2486;
    (* src = "hello.spade:101,5" *)
    logic[34:0] _e_2486_mut;
    (* src = "hello.spade:36,15" *)
    
    \facet_hdl::axi4_slave  \axi4_slave_0 (.clk_i(\clk ), .rst_i(\rst ), .i_i(\axi ), .rdata_i(\rdata ), .output__(\bus ));
    assign _e_2201 = \bus [93:31];
    
    assign \w  = _e_2204[61:0];
    assign _e_3425 = _e_2204[62] == 1'd1;
    assign _e_3427 = _e_3425 && _e_3426;
    assign _e_2207 = \w [61:32];
    assign _e_2209 = \w [31:0];
    assign _e_2205 = {_e_2206, _e_2207, _e_2209};
    
    assign _e_3430 = _e_2211[62] == 1'd0;
    assign _e_2212 = {_e_2213, _e_2214, _e_2215};
    always_comb begin
        priority casez ({_e_3427, _e_3430})
            2'b1?: _e_2200 = _e_2205;
            2'b01: _e_2200 = _e_2212;
            2'b?: _e_2200 = 63'dx;
        endcase
    end
    assign _e_2204 = _e_2201;
    assign _e_2211 = _e_2201;
    assign \wr  = _e_2200[62];
    assign \wr_word  = _e_2200[61:32];
    assign \wr_data  = _e_2200[31:0];
    assign _e_2221 = \bus [30:0];
    
    assign \word  = _e_2224[29:0];
    assign _e_3433 = _e_2224[30] == 1'd1;
    assign _e_3435 = _e_3433 && _e_3434;
    assign _e_2225 = {_e_2226, \word };
    
    assign _e_3438 = _e_2228[30] == 1'd0;
    assign _e_2229 = {_e_2230, _e_2231};
    always_comb begin
        priority casez ({_e_3435, _e_3438})
            2'b1?: _e_2220 = _e_2225;
            2'b01: _e_2220 = _e_2229;
            2'b?: _e_2220 = 31'dx;
        endcase
    end
    assign _e_2224 = _e_2221;
    assign _e_2228 = _e_2221;
    assign \rd  = _e_2220[30];
    assign \rd_word  = _e_2220[29:0];
    (* src = "hello.spade:46,28" *)
    
    \core::ops::impl#93::wrapping_sub[2117]  \wrapping_sub_0 (.self_i(\add_one_req_head ), .rhs_i(\add_one_req_tail ), .output__(_e_2236));
    assign \add_one_req_room  = _e_2236 < _e_2239;
    
    assign _e_3439 = _e_3440_mut;
    assign \add_one_req_ack  = {_e_3439};
    assign {_e_3440_mut} = \add_one_req_ack_mut ;
    assign \add_one_req_ready  = \add_one_req_ack ;
    assign _e_2249 = \wr_word  == _e_2251;
    assign _e_4015 = {\wr };
    assign _e_4016 = {_e_2249};
    assign _e_4014 = _e_4015 & _e_4016;
    assign _e_2247 = _e_4014;
    assign _e_4029 = {_e_2247};
    assign _e_4030 = {\add_one_req_room };
    assign _e_4028 = _e_4029 & _e_4030;
    assign \add_one_req_push  = _e_4028;
    assign \add_one_req_rdp [32] = \add_one_req_rdp_mut [2] ? 1'b1 : 1'b0;
    assign \add_one_req_rdp [31:0] = \add_one_req_rdp_mut [2] ? _e_2254[\add_one_req_rdp_mut [1:0]] : 'sbx;
    always @(posedge \add_one_req_wrp_mut [35]) begin
        if (\add_one_req_wrp_mut [34]) begin
            _e_2254[\add_one_req_wrp_mut [33:32]] <= \add_one_req_wrp_mut [31:0];
        end
    end
    assign _e_2263 = \add_one_req_head [1:0];
    assign _e_2262 = {_e_2263, \wr_data };
    assign _e_2261 = {1'd1, _e_2262};
    assign _e_2267 = {1'd0, 34'bX};
    assign _e_2258 = \add_one_req_push  ? _e_2261 : _e_2267;
    (* src = "hello.spade:54,5" *)
    
    \core::mem::impl#73::write[2124]  \write_0 (.self_o(\add_one_req_wrp_mut ), .clk_i(\clk ), .req_i(_e_2258));
    assign _e_2272 = \add_one_req_tail [1:0];
    assign _e_2271 = {1'd1, _e_2272};
    (* src = "hello.spade:55,39" *)
    
    \core::mem::impl#70::read_comb[2125]  \read_comb_0 (.self_i(\add_one_req_rdp ), .self_o(\add_one_req_rdp_mut ), .req_i(_e_2271), .output__(_e_2269));
    (* src = "hello.spade:55,39" *)
    
    \std::option::impl#41::unwrap_or[2126]  \unwrap_or_0 (.self_i(_e_2269), .default_i(_e_2274), .output__(\add_one_req_entry ));
    assign _e_2277 = \add_one_req_head  != \add_one_req_tail ;
    assign _e_4049 = {_e_2277};
    assign _e_4050 = {\add_one_req_ready };
    assign _e_4048 = _e_4049 & _e_4050;
    assign \add_one_req_pop  = _e_4048;
    (* src = "hello.spade:57,90" *)
    
    \core::ops::impl#91::wrapping_add[2129]  \wrapping_add_0 (.self_i(\add_one_req_head ), .rhs_i(_e_2292), .output__(_e_2290));
    assign _e_2287 = \add_one_req_push  ? _e_2290 : \add_one_req_head ;
    initial begin
        \add_one_req_head  = 'b00000000000000000000000000000000;
    end
    always @(posedge \clk ) begin
        if (\rst ) begin
            \add_one_req_head  <= _e_2285;
        end
        else begin
            \add_one_req_head  <= _e_2287;
        end
    end
    (* src = "hello.spade:58,89" *)
    
    \core::ops::impl#91::wrapping_add[2129]  \wrapping_add_1 (.self_i(\add_one_req_tail ), .rhs_i(_e_2305), .output__(_e_2303));
    assign _e_2300 = \add_one_req_pop  ? _e_2303 : \add_one_req_tail ;
    initial begin
        \add_one_req_tail  = 'b00000000000000000000000000000000;
    end
    always @(posedge \clk ) begin
        if (\rst ) begin
            \add_one_req_tail  <= _e_2298;
        end
        else begin
            \add_one_req_tail  <= _e_2300;
        end
    end
    (* src = "hello.spade:60,29" *)
    
    \core::ops::impl#93::wrapping_sub[2117]  \wrapping_sub_1 (.self_i(\add_one_resp_head ), .rhs_i(\add_one_resp_tail ), .output__(_e_2309));
    assign \add_one_resp_room  = _e_2309 < _e_2312;
    
    assign _e_3491 = _e_3492_mut;
    assign \add_one_resp_write  = {_e_3491};
    assign {_e_3492_mut} = \add_one_resp_write_mut ;
    assign _e_2317 = \add_one_resp_write [32:0];
    
    assign \_  = _e_2320[31:0];
    assign _e_3495 = _e_2320[32] == 1'd1;
    assign _e_3497 = _e_3495 && _e_3496;
    
    assign _e_3500 = _e_2322[32] == 1'd0;
    always_comb begin
        priority casez ({_e_3497, _e_3500})
            2'b1?: \add_one_resp_valid  = _e_2321;
            2'b01: \add_one_resp_valid  = _e_2323;
            2'b?: \add_one_resp_valid  = 1'dx;
        endcase
    end
    assign _e_2320 = _e_2317;
    assign _e_2322 = _e_2317;
    assign _e_4069 = {\rst };
    assign _e_4068 = ~_e_4069;
    assign _e_2326 = _e_4068;
    assign _e_4081 = {_e_2326};
    assign _e_4082 = {\add_one_resp_room };
    assign _e_4080 = _e_4081 & _e_4082;
    assign \add_one_resp_ready_now  = _e_4080;
    assign _e_4095 = {\add_one_resp_valid };
    assign _e_4096 = {\add_one_resp_ready_now };
    assign _e_4094 = _e_4095 & _e_4096;
    assign \add_one_resp_push  = _e_4094;
    assign _e_2337 = \wr_word  == _e_2339;
    assign _e_4115 = {\wr };
    assign _e_4116 = {_e_2337};
    assign _e_4114 = _e_4115 & _e_4116;
    assign _e_2335 = _e_4114;
    (* src = "hello.spade:68,50" *)
    
    \core::ops::impl#93::wrapping_sub[2117]  \wrapping_sub_2 (.self_i(\wr_data ), .rhs_i(\add_one_resp_tail ), .output__(_e_2341));
    (* src = "hello.spade:68,93" *)
    
    \core::ops::impl#93::wrapping_sub[2117]  \wrapping_sub_3 (.self_i(\add_one_resp_head ), .rhs_i(\add_one_resp_tail ), .output__(_e_2344));
    assign _e_2340 = _e_2341 <= _e_2344;
    assign _e_4135 = {_e_2335};
    assign _e_4136 = {_e_2340};
    assign _e_4134 = _e_4135 & _e_4136;
    assign \add_one_resp_pop  = _e_4134;
    (* src = "hello.spade:69,92" *)
    
    \core::ops::impl#91::wrapping_add[2129]  \wrapping_add_2 (.self_i(\add_one_resp_head ), .rhs_i(_e_2358), .output__(_e_2356));
    assign _e_2353 = \add_one_resp_push  ? _e_2356 : \add_one_resp_head ;
    initial begin
        \add_one_resp_head  = 'b00000000000000000000000000000000;
    end
    always @(posedge \clk ) begin
        if (\rst ) begin
            \add_one_resp_head  <= _e_2351;
        end
        else begin
            \add_one_resp_head  <= _e_2353;
        end
    end
    assign _e_2366 = \add_one_resp_pop  ? \wr_data  : \add_one_resp_tail ;
    initial begin
        \add_one_resp_tail  = 'b00000000000000000000000000000000;
    end
    always @(posedge \clk ) begin
        if (\rst ) begin
            \add_one_resp_tail  <= _e_2364;
        end
        else begin
            \add_one_resp_tail  <= _e_2366;
        end
    end
    assign \add_one_resp_rdp [32] = \add_one_resp_rdp_mut [2] ? 1'b1 : 1'b0;
    assign \add_one_resp_rdp [31:0] = \add_one_resp_rdp_mut [2] ? _e_2372[\add_one_resp_rdp_mut [1:0]] : 'sbx;
    always @(posedge \add_one_resp_wrp_mut [35]) begin
        if (\add_one_resp_wrp_mut [34]) begin
            _e_2372[\add_one_resp_wrp_mut [33:32]] <= \add_one_resp_wrp_mut [31:0];
        end
    end
    assign _e_2381 = \add_one_resp_head [1:0];
    assign _e_2384 = \add_one_resp_write [32:0];
    
    assign \add_one_resp_x  = _e_2387[31:0];
    assign _e_3534 = _e_2387[32] == 1'd1;
    assign _e_3536 = _e_3534 && _e_3535;
    
    assign _e_3539 = _e_2389[32] == 1'd0;
    always_comb begin
        priority casez ({_e_3536, _e_3539})
            2'b1?: _e_2383 = \add_one_resp_x ;
            2'b01: _e_2383 = _e_2390;
            2'b?: _e_2383 = 32'dx;
        endcase
    end
    assign _e_2387 = _e_2384;
    assign _e_2389 = _e_2384;
    assign _e_2380 = {_e_2381, _e_2383};
    assign _e_2379 = {1'd1, _e_2380};
    assign _e_2392 = {1'd0, 34'bX};
    assign _e_2376 = \add_one_resp_push  ? _e_2379 : _e_2392;
    (* src = "hello.spade:75,5" *)
    
    \core::mem::impl#73::write[2124]  \write_1 (.self_o(\add_one_resp_wrp_mut ), .clk_i(\clk ), .req_i(_e_2376));
    assign _e_2397 = \add_one_resp_tail [1:0];
    assign _e_2396 = {1'd1, _e_2397};
    (* src = "hello.spade:76,40" *)
    
    \core::mem::impl#70::read_comb[2125]  \read_comb_1 (.self_i(\add_one_resp_rdp ), .self_o(\add_one_resp_rdp_mut ), .req_i(_e_2396), .output__(_e_2394));
    (* src = "hello.spade:76,40" *)
    
    \std::option::impl#41::unwrap_or[2126]  \unwrap_or_1 (.self_i(_e_2394), .default_i(_e_2399), .output__(\add_one_resp_entry ));
    (* src = "hello.spade:78,28" *)
    
    \core::ops::impl#93::wrapping_sub[2117]  \wrapping_sub_4 (.self_i(\pressed_req_head ), .rhs_i(\pressed_req_tail ), .output__(_e_2402));
    assign \pressed_req_room  = _e_2402 < _e_2405;
    
    assign _e_3551 = _e_3552_mut;
    assign \pressed_req_write  = {_e_3551};
    assign {_e_3552_mut} = \pressed_req_write_mut ;
    assign _e_2410 = \pressed_req_write ;
    
    
    assign _e_3555 = _e_2413 == 1'd1;
    assign _e_3559 = _e_3555 && _e_3556;
    
    assign _e_3562 = _e_2415 == 1'd0;
    always_comb begin
        priority casez ({_e_3559, _e_3562})
            2'b1?: \pressed_req_valid  = _e_2414;
            2'b01: \pressed_req_valid  = _e_2416;
            2'b?: \pressed_req_valid  = 1'dx;
        endcase
    end
    assign _e_2413 = _e_2410;
    assign _e_2415 = _e_2410;
    assign _e_4155 = {\rst };
    assign _e_4154 = ~_e_4155;
    assign _e_2419 = _e_4154;
    assign _e_4167 = {_e_2419};
    assign _e_4168 = {\pressed_req_room };
    assign _e_4166 = _e_4167 & _e_4168;
    assign \pressed_req_ready_now  = _e_4166;
    assign _e_4181 = {\pressed_req_valid };
    assign _e_4182 = {\pressed_req_ready_now };
    assign _e_4180 = _e_4181 & _e_4182;
    assign \pressed_req_push  = _e_4180;
    assign _e_2430 = \wr_word  == _e_2432;
    assign _e_4201 = {\wr };
    assign _e_4202 = {_e_2430};
    assign _e_4200 = _e_4201 & _e_4202;
    assign _e_2428 = _e_4200;
    (* src = "hello.spade:86,49" *)
    
    \core::ops::impl#93::wrapping_sub[2117]  \wrapping_sub_5 (.self_i(\wr_data ), .rhs_i(\pressed_req_tail ), .output__(_e_2434));
    (* src = "hello.spade:86,91" *)
    
    \core::ops::impl#93::wrapping_sub[2117]  \wrapping_sub_6 (.self_i(\pressed_req_head ), .rhs_i(\pressed_req_tail ), .output__(_e_2437));
    assign _e_2433 = _e_2434 <= _e_2437;
    assign _e_4221 = {_e_2428};
    assign _e_4222 = {_e_2433};
    assign _e_4220 = _e_4221 & _e_4222;
    assign \pressed_req_pop  = _e_4220;
    (* src = "hello.spade:87,90" *)
    
    \core::ops::impl#91::wrapping_add[2129]  \wrapping_add_3 (.self_i(\pressed_req_head ), .rhs_i(_e_2451), .output__(_e_2449));
    assign _e_2446 = \pressed_req_push  ? _e_2449 : \pressed_req_head ;
    initial begin
        \pressed_req_head  = 'b00000000000000000000000000000000;
    end
    always @(posedge \clk ) begin
        if (\rst ) begin
            \pressed_req_head  <= _e_2444;
        end
        else begin
            \pressed_req_head  <= _e_2446;
        end
    end
    assign _e_2459 = \pressed_req_pop  ? \wr_data  : \pressed_req_tail ;
    initial begin
        \pressed_req_tail  = 'b00000000000000000000000000000000;
    end
    always @(posedge \clk ) begin
        if (\rst ) begin
            \pressed_req_tail  <= _e_2457;
        end
        else begin
            \pressed_req_tail  <= _e_2459;
        end
    end
    
    assign _e_3586 = _e_2467 == _e_3587;
    
    assign _e_3589 = _e_2469 == _e_3590;
    
    assign _e_3592 = _e_2471 == _e_3593;
    
    assign _e_3595 = _e_2473 == _e_3596;
    
    assign _e_3598 = _e_2475 == _e_3599;
    
    assign _e_3601 = _e_2477 == _e_3602;
    
    assign _e_3604 = _e_2479 == _e_3605;
    
    assign _e_3607 = _e_2481 == _e_3608;
    
    always_comb begin
        priority casez ({_e_3586, _e_3589, _e_3592, _e_3595, _e_3598, _e_3601, _e_3604, _e_3607, _e_3610})
            9'b1????????: \rdata  = _e_2468;
            9'b01???????: \rdata  = \add_one_req_tail ;
            9'b001??????: \rdata  = \add_one_req_head ;
            9'b0001?????: \rdata  = \add_one_resp_tail ;
            9'b00001????: \rdata  = \add_one_resp_head ;
            9'b000001???: \rdata  = \add_one_resp_entry ;
            9'b0000001??: \rdata  = \pressed_req_tail ;
            9'b00000001?: \rdata  = \pressed_req_head ;
            9'b000000001: \rdata  = _e_2484;
            9'b?: \rdata  = 32'dx;
        endcase
    end
    assign _e_2467 = \rd_word ;
    assign _e_2469 = \rd_word ;
    assign _e_2471 = \rd_word ;
    assign _e_2473 = \rd_word ;
    assign _e_2475 = \rd_word ;
    assign _e_2477 = \rd_word ;
    assign _e_2479 = \rd_word ;
    assign _e_2481 = \rd_word ;
    assign __n2 = \rd_word ;
    assign _e_2487 = \bus [159:94];
    assign _e_2490 = \add_one_resp_head  != \add_one_resp_tail ;
    assign _e_2493 = \pressed_req_head  != \pressed_req_tail ;
    assign _e_4247 = {_e_2490};
    assign _e_4248 = {_e_2493};
    assign _e_4246 = _e_4247 | _e_4248;
    assign _e_2489 = _e_4246;
    assign _e_2498 = \add_one_req_head  != \add_one_req_tail ;
    (* src = "hello.spade:101,168" *)
    
    \hello::unpack_add_one_args  \unpack_add_one_args_0 (.b_i(\add_one_req_entry ), .output__(_e_2503));
    assign _e_2502 = {1'd1, _e_2503};
    assign _e_2506 = {1'd0, 32'bX};
    assign _e_2497 = _e_2498 ? _e_2502 : _e_2506;
    assign \add_one_req_ack_mut  = _e_2507_mut;
    assign _e_2496 = {_e_2497};
    assign {_e_2507_mut} = _e_2496_mut;
    assign \add_one_resp_write_mut [32:0] = _e_2510_mut;
    assign _e_4267 = {\add_one_resp_ready_now };
    assign _e_4266 = ~_e_4267;
    assign _e_2512 = _e_4266;
    assign _e_2509 = {_e_2512};
    assign {_e_2510_mut} = _e_2509_mut;
    assign \pressed_req_write_mut  = _e_2515_mut;
    assign _e_4279 = {\pressed_req_ready_now };
    assign _e_4278 = ~_e_4279;
    assign _e_2517 = _e_4278;
    assign _e_2514 = {_e_2517};
    assign {_e_2515_mut} = _e_2514_mut;
    assign _e_2486 = {_e_2487, _e_2489, _e_2496, _e_2509, _e_2514};
    assign {_e_2496_mut, _e_2509_mut, _e_2514_mut} = _e_2486_mut;
    assign output__ = _e_2486;
    assign _e_2486_mut = input__;
endmodule


module \std::conv::impl#11::as_clock  (
        input self_i,
        output output__
    );
    `COCOTB_CODE( std::conv::impl#11::as_clock )
    logic \self ;
    assign \self  = self_i;
    logic _e_830;
    assign _e_830 = \self ;
    assign output__ = _e_830;
endmodule


module \std::conv::impl#11::as_uint  (
        input self_i,
        output output__
    );
    `COCOTB_CODE( std::conv::impl#11::as_uint )
    logic \self ;
    assign \self  = self_i;
    logic _e_834;
    assign _e_834 = \self ;
    assign output__ = _e_834;
endmodule


module \std::conv::impl#12::as_bool  (
        input self_i,
        output output__
    );
    `COCOTB_CODE( std::conv::impl#12::as_bool )
    logic \self ;
    assign \self  = self_i;
    logic _e_845;
    assign _e_845 = \self ;
    assign output__ = _e_845;
endmodule


module \std::conv::impl#13::as_bool  (
        input self_i,
        output output__
    );
    `COCOTB_CODE( std::conv::impl#13::as_bool )
    logic \self ;
    assign \self  = self_i;
    logic _e_853;
    assign _e_853 = \self ;
    assign output__ = _e_853;
endmodule


module \std::default::impl#18::default  (
        output output__
    );
    `COCOTB_CODE( std::default::impl#18::default )
    localparam[0:0] _e_929 = 0;
    assign output__ = _e_929;
endmodule


module \std::default::impl#19::default  (
        
    );
    `COCOTB_CODE( std::default::impl#19::default )
    
endmodule


module \std::option::impl#41::unwrap_or[2126]  (
        input[32:0] self_i,
        input[31:0] default_i,
        output[31:0] output__
    );
    `COCOTB_CODE( std::option::impl#41::unwrap_or[2126] )
    logic[32:0] \self ;
    assign \self  = self_i;
    logic[31:0] \default ;
    assign \default  = default_i;
    (* src = "<compiler dir>/stdlib/option.spade:73,9" *)
    logic[32:0] _e_1521;
    (* src = "<compiler dir>/stdlib/option.spade:74,13" *)
    logic[31:0] \val ;
    logic _e_3503;
    localparam[0:0] _e_3504 = 1;
    logic _e_3505;
    (* src = "<compiler dir>/stdlib/option.spade:73,9" *)
    logic[32:0] _e_1523;
    logic _e_3508;
    (* src = "<compiler dir>/stdlib/option.spade:73,9" *)
    logic[31:0] _e_1518;
    
    assign \val  = _e_1521[31:0];
    assign _e_3503 = _e_1521[32] == 1'd1;
    assign _e_3505 = _e_3503 && _e_3504;
    
    assign _e_3508 = _e_1523[32] == 1'd0;
    always_comb begin
        priority casez ({_e_3505, _e_3508})
            2'b1?: _e_1518 = \val ;
            2'b01: _e_1518 = \default ;
            2'b?: _e_1518 = 32'dx;
        endcase
    end
    assign _e_1521 = \self ;
    assign _e_1523 = \self ;
    assign output__ = _e_1518;
endmodule


module \core::mem::impl#70::read_comb[2125]  (
        input[32:0] self_i, output[3:0] self_o,
        input[2:0] req_i,
        output[32:0] output__
    );
    `COCOTB_CODE( core::mem::impl#70::read_comb[2125] )
    logic[32:0] \self ;
    assign \self  = self_i;
    logic[3:0] \self_mut ;
    assign self_o = \self_mut ;
    logic[2:0] \req ;
    assign \req  = req_i;
    (* src = "<compiler dir>/core/mem.spade:36,13" *)
    logic _e_2520_mut;
    localparam[0:0] _e_2524 = 1'bx;
    logic _e_2523;
    (* src = "<compiler dir>/core/mem.spade:37,13" *)
    logic[2:0] _e_2526_mut;
    (* src = "<compiler dir>/core/mem.spade:39,9" *)
    logic[32:0] _e_2530;
    assign \self_mut [3] = _e_2520_mut;
    assign _e_2523 = _e_2524;
    assign _e_2520_mut = _e_2523;
    
    assign \self_mut [2:0] = _e_2526_mut;
    assign _e_2526_mut = \req ;
    
    assign _e_2530 = \self [32:0];
    assign output__ = _e_2530;
endmodule


module \core::mem::impl#73::write[2124]  (
        output[35:0] self_o,
        input clk_i,
        input[34:0] req_i
    );
    `COCOTB_CODE( core::mem::impl#73::write[2124] )
    logic[35:0] \self_mut ;
    assign self_o = \self_mut ;
    logic \clk ;
    assign \clk  = clk_i;
    logic[34:0] \req ;
    assign \req  = req_i;
    (* src = "<compiler dir>/core/mem.spade:64,13" *)
    logic _e_2549_mut;
    (* src = "<compiler dir>/core/mem.spade:65,13" *)
    logic[34:0] _e_2553_mut;
    assign \self_mut [35] = _e_2549_mut;
    assign _e_2549_mut = \clk ;
    
    assign \self_mut [34:0] = _e_2553_mut;
    assign _e_2553_mut = \req ;
    
    
endmodule


module \core::ops::impl#91::wrapping_add[2129]  (
        input[31:0] self_i,
        input[31:0] rhs_i,
        output[31:0] output__
    );
    `COCOTB_CODE( core::ops::impl#91::wrapping_add[2129] )
    logic[31:0] \self ;
    assign \self  = self_i;
    logic[31:0] \rhs ;
    assign \rhs  = rhs_i;
    (* src = "<compiler dir>/core/ops.spade:212,62" *)
    logic[32:0] _e_2873;
    logic[32:0] \bits ;
    (* src = "<compiler dir>/core/ops.spade:213,34" *)
    logic[31:0] _e_2878;
    logic[31:0] _e_2877;
    assign _e_2873 = \self  + \rhs ;
    assign \bits  = _e_2873;
    assign _e_2878 = \bits [31-:32];
    assign _e_2877 = _e_2878;
    assign output__ = _e_2877;
endmodule


module \core::ops::impl#93::wrapping_sub[2117]  (
        input[31:0] self_i,
        input[31:0] rhs_i,
        output[31:0] output__
    );
    `COCOTB_CODE( core::ops::impl#93::wrapping_sub[2117] )
    logic[31:0] \self ;
    assign \self  = self_i;
    logic[31:0] \rhs ;
    assign \rhs  = rhs_i;
    (* src = "<compiler dir>/core/ops.spade:234,62" *)
    logic[32:0] _e_2895;
    logic[32:0] \bits ;
    (* src = "<compiler dir>/core/ops.spade:235,34" *)
    logic[31:0] _e_2900;
    logic[31:0] _e_2899;
    assign _e_2895 = \self  - \rhs ;
    assign \bits  = _e_2895;
    assign _e_2900 = \bits [31-:32];
    assign _e_2899 = _e_2900;
    assign output__ = _e_2899;
endmodule

`default_nettype none