#!/usr/bin/env bash
# Builds hello's Zybo bitstream with Vivado, because chipsmith ties the PS7's
# pins off and can't route M_AXI_GP0 or IRQ_F2P yet. Remove this, build.tcl,
# ps.tcl and zybo_top.v for a chipsmith.toml once chipsmith packs a PS7 with
# real connections.
#
#   cargo run --features gen --bin hello-top -- zybo   # writes build/spade.sv
#   examples/hello/zybo/build.sh                        # writes build/hello.bit
#   chipsmith flash --ssh root@venus examples/hello/zybo/build/hello.bit
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
[ -f "$here/build/spade.sv" ] || { echo "no build/spade.sv: run hello-top -- zybo first" >&2; exit 1; }
rm -rf "$here/build/vivado" && mkdir -p "$here/build/vivado"
cd "$here/build/vivado"
${XC7_RE:-/pool/dev/xc7-re}/tools/vivado.sh -mode batch -nojournal -log vivado.log -source ../../build.tcl
grep -A3 "Design Timing Summary" -m1 timing.rpt >/dev/null && sed -n '/WNS(ns)/,+2p' timing.rpt
echo "$here/build/hello.bit"
