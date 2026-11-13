#!/usr/bin/env bash
# Builds hello's DE25-Nano core with Quartus Prime Pro, because chipsmith
# can't map the lightweight bridge's ports yet. Remove this (and hpsf_top.sv)
# for a chipsmith.toml once a DE25 manifest takes the lwhps2fpga_* signals.
#
#   cargo run --features gen --bin hello-top -- de25   # writes build/spade.sv
#   examples/hello/de25/build.sh                        # writes build/hello.core.rbf
#
# The project is agilex5-re's designs/hpsf with hpsf_top.sv swapped, so the
# HPS IO hash stays phase 1's; the script refuses a core where it doesn't.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
hpsf=${AGILEX5_RE:-/pool/dev/agilex5-re}/designs/hpsf
spl=${AGILEX5_RE:-/pool/dev/agilex5-re}/specimens/hps-first/out/spl11/spl11.hex
want_hash=D921CDF7CADFEECA11572B24600BFD337FFD119E47601CA5075AA911EEC3D07A
proj=$here/build/quartus

[ -f "$here/build/spade.sv" ] || { echo "no build/spade.sv: run hello-top -- de25 first" >&2; exit 1; }
rm -rf "$proj" && mkdir -p "$proj"
cp -r "$hpsf"/{hpsf.qpf,hpsf.qsf,hpsf.sdc,hps_wrapper.sv,ip,pins} "$proj"/
cp "$here/hpsf_top.sv" "$proj"/
# Quartus Prime Pro 26.1.1's quartus_syn segfaults (HDB_INSTANCE_NAME::
# get_schema_flag) when it infers a RAM inside a module whose name is an
# escaped identifier like Spade's `\hello::hello_regs `: the register file's
# queues are such RAMs. The same SV with plain names synthesizes and infers
# both. So give every escaped name that isn't already a plain identifier a
# plain one. No public Altera tracker to link; drop this when a Quartus
# release synthesizes build/spade.sv as Spade writes it.
python3 -I - "$here/build/spade.sv" "$proj/spade.sv" <<'PY'
import re, sys
src = open(sys.argv[1]).read()
keywords = {"input", "output", "inout", "module", "reg", "wire", "logic", "begin", "end",
            "assign", "always", "initial", "case", "default", "if", "else", "for", "string"}
plain = {}
def rename(m):
    name = m.group(1)
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_$]*", name) and name not in keywords:
        return name + " "  # `\x ` and `x` are already the same identifier
    new = plain.setdefault(name, "esc_" + re.sub(r"\W", "_", name))
    return new + " "
out = re.sub(r"\\(\S+) ", rename, src)
if len(set(plain.values())) != len(plain):
    sys.exit("two escaped names map to the same plain one")
open(sys.argv[2], "w").write(out)
PY
cat >> "$proj/hpsf.qsf" <<'QSF'
set_global_assignment -name SYSTEMVERILOG_FILE spade.sv
# KEY[0], active low (Terasic DE25-Nano manual; agilex5-re designs/pins).
set_location_assignment PIN_C8 -to KEY0
set_instance_assignment -name IO_STANDARD "3.3-V LVCMOS" -to KEY0
QSF
# The key is asynchronous and debounced in the fabric; nothing to time.
echo 'set_false_path -from [get_ports KEY0]' >> "$proj/hpsf.sdc"
cp "$spl" "$proj/spl.hex"

cd "$proj"
export CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro ORACLE_LICENSE_MAC=44:8a:5b:87:88:20 LM_LICENSE_FILE=$HOME/.altera/agilex5.dat
q=${AGILEX5_RE:-/pool/dev/agilex5-re}/tools/quartus.sh
"$q" quartus_sh --flow compile hpsf
# The SPL only goes into the .hps.rbf, which is never used: the board keeps
# its own phase 1 in QSPI. The .core.rbf is the same whichever SPL.
"$q" quartus_pfg -c output_files/hpsf.sof hpsf.rbf -o hps_path=spl.hex -o hps=1
rm -f hpsf.hps.rbf
info=$("$q" quartus_pfg -i hpsf.core.rbf)
echo "$info" | grep -i hash
echo "$info" | grep -qi "$want_hash" || { echo "HPS IO hash isn't phase 1's ($want_hash): don't load this" >&2; exit 1; }
cp hpsf.core.rbf "$here/build/hello.core.rbf"
echo "$here/build/hello.core.rbf"
