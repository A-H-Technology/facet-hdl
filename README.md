# facet-hdl

Spike: declare the boundary between an SoC FPGA's ARM side (the HPS) and
its fabric **once**, as Rust types with `#[derive(Facet)]`. Everything else
comes from that declaration:

```rust
#[derive(Facet)]
pub struct Blinky {
    pub leds: ToFabric<LedControl>,     // host writes, fabric reads
    pub status: FromFabric<LedStatus>,  // fabric drives, host reads
    pub operands: ToFabric<Operands>,
    pub sum: FromFabric<Sum>,
}
```

- **VHDL** (`facet-hdl-vhdl`): a package with one VHDL type per Rust type
  (records, enums, arrays) plus `to_slv`/`to_<type>` packing functions, and an
  AXI4-Lite register file entity whose ports are those typed records.
- **Host handles** (`facet-hdl`): `bind::<Blinky>(transport)` returns that same
  struct with live handles: `b.leds.write(&ctl)?`, `b.status.read()?`. There's
  no `read` on a `ToFabric` and no `write` on a `FromFabric`.
- **A fingerprint** of the layout sits in word 0 of the register window.
  `bind` refuses a bitstream that was generated from a different declaration.

Target is mercury (Terasic DE25-Nano, Agilex 5) over the lightweight
HPS-to-FPGA bridge, but the pieces are separate crates:

| crate | role |
| --- | --- |
| `facet-hdl` | `ToFabric`/`FromFabric`, shape → `HwType`, bit codec, `Boundary` layout, `Transport`, `bind` |
| `facet-hdl-vhdl` | VHDL-2008 package and AXI4-Lite register entity; `literal(&value)` renders a Rust value as a VHDL constant |
| `facet-hdl-ghdl` | Co-simulation: GHDL runs the fabric, and a generated testbench turns stdin lines into AXI transactions. `SimTransport` makes the real host code drive it |
| `facet-hdl-devmem` | `Transport` over an uncached `/dev/mem` mapping of the bridge (`AGILEX5_LWH2F = 0x2000_0000`) |
| `examples/blinky` | Declaration, hand-written fabric logic (`hdl/blinky_core.vhd`), the generator bin, the HPS CLI, and co-sim tests |

```sh
nix develop --command cargo test                  # incl. GHDL co-sim
nix develop --command cargo run --bin blinky-gen  # regenerate examples/blinky/hdl/generated
nix build .#blinky-hps                            # static aarch64-musl CLI for mercury
```

## Wire format

These are the rules `codec.rs` and the generated VHDL both implement.

- `bool` = 1 bit. `u8..u128`/`i8..i128` = their width, two's complement.
  A fieldless enum is its declaration index in `ceil(log2(n))` bits. Structs
  are their fields in declaration order with the first field at the LSB.
  Arrays are their elements with index 0 at the LSB.
- Rejected with an error that names the field path: `usize`, floats, `char`,
  strings, slices, data-carrying enums, zero-sized things, recursive types.
  Generated names that collide or are illegal in VHDL (`next`, `a__b`, a port
  `status` next to a type `Status`, ...) are rejected too.
- Each port occupies `ceil(width/32)` consecutive 32-bit words, in field order,
  starting at word 1.
- A **ToFabric** write commits when its last word is written. Earlier words
  are buffered, so the fabric never sees a torn value, and `<port>_written`
  pulses once per commit.
- A **FromFabric** read snapshots the whole value when its first word is read,
  and the later words come from that snapshot.
- The host holds a per-bus lock across each port access to keep that
  ordering intact.

## Verified

- The unit tests cover the codec, layout, naming and bind. There's an
  in-memory fake fabric for fingerprint mismatch and bad discriminants.
- GHDL co-sim of the blinky design runs the real host API end to end. It
  checks bind/fingerprint, 3-word signed sums, LED patterns and snapshotted
  64-bit uptime.
- Echo co-sim covers nested structs, arrays of structs/enums/bools, 2-D
  arrays, `u128` and `i8`/`i64`. A golden constant rendered from Rust checks
  VHDL pack and unpack *separately*, so a symmetric bug can't hide behind the
  loopback. A planted field-order bug does fail it.

## Not yet: running on mercury

Nothing has touched hardware. `blinky-hps` builds, but mercury was off the
network when this was written. Before it can do anything there:

1. **Bitstream.** Agilex 5 needs Quartus Prime **Pro**, which chipsmith doesn't
   drive yet; it does Lite and 13.0sp1. The design also needs a Platform
   Designer system with the HPS, normally Terasic's DE25-Nano GHRD. Our
   `blinky_core` goes onto the LWH2F bridge as an AXI4-Lite slave, plus pin
   assignments for the LEDs.
2. **Loading it.** QSPI currently holds the stock image. nixos-fpga's planned
   `hardware.fpga` (fpga-manager + overlay) is the declarative route.
3. **Then:** `blinky --base 0x2000_0000+<component offset> status`. Don't
   point it at an offset nothing decodes. A DECERR on arm64 can arrive as an
   SError and panic the kernel.

Candidates for single-sourcing next: the Platform Designer `_hw.tcl` for the
register entity (address span and interface come straight from `Boundary`),
interrupt lines (`FromFabric` change notification), and reset values from
`Default`.
