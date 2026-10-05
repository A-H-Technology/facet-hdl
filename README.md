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

#[derive(Facet)]
pub struct Pipe {
    pub jobs: ToFabricQueue<Job, 64>,     // host -> fabric channel
    pub done: FromFabricQueue<Done, 64>,  // fabric -> host channel
}
```

Two kinds of port, each in both directions. `ToFabric`/`FromFabric` are
registers: one value, last writer wins. `ToFabricQueue`/`FromFabricQueue` are
SPSC channels: a ring of `N` entries that never drops or repeats one. The host
gets `push(&T) -> Result<(), PushError>` / `pop() -> Option<T>` on `&mut self`,
and the fabric gets a `<name>`, `<name>_valid`, `<name>_ready` handshake.

- **VHDL** (`facet-hdl-vhdl`): a package with one VHDL type per Rust type
  (records, enums, arrays) plus `to_slv`/`to_<type>` packing functions, and an
  AXI4-Lite register file entity whose ports are those typed records. The bus
  itself is two records from `facet_hdl_axil_pkg` (`axi_in : axil_m2s_t`,
  `axi_out : axil_s2m_t`), so your logic carries it as two ports and passes it
  straight through. Flat `s_axi_*` signals only appear at the Platform Designer
  edge.
- **Host handles** (`facet-hdl`): `bind::<Blinky>(transport)` returns that same
  struct with live handles: `b.leds.write(&ctl)?`, `b.status.read()?`. There's
  no `read` on a `ToFabric` and no `write` on a `FromFabric`.
- **A fingerprint** of the layout sits in word 0 of the register window.
  `bind` refuses a bitstream that was generated from a different declaration.

Target is mercury (Terasic DE25-Nano, Agilex 5) over the lightweight
HPS-to-FPGA bridge, but the pieces are separate crates:

| crate | role |
| --- | --- |
| `facet-hdl` | `ToFabric`/`FromFabric` registers, `ToFabricQueue`/`FromFabricQueue` channels, shape → `HwType`, bit codec, `Boundary` layout, `Transport`, `bind` |
| `facet-hdl-vhdl` | VHDL-2008 package and AXI4-Lite register entity; `literal(&value)` renders a Rust value as a VHDL constant |
| `facet-hdl-ghdl` | Co-simulation: GHDL runs the fabric, and a generated testbench turns stdin lines into AXI transactions. `SimTransport` makes the real host code drive it |
| `facet-hdl-linux` | Linux transports: `DevMem` (uncached `/dev/mem` mapping of the bridge, `AGILEX5_LWH2F = 0x2000_0000`, polls) and `Uio` (a `generic-uio` node: the mapping plus the `irq` line, so `pop_wait` sleeps) |
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
- Each register port occupies `ceil(width/32)` consecutive 32-bit words, in
  field order, starting at word 1. A queue port occupies two counter words
  (`+0` tail, `+1` head) followed by one entry's worth of words.
- A **ToFabric** write commits when its last word is written. Earlier words
  are buffered, so the fabric never sees a torn value, and `<port>_written`
  pulses once per commit.
- A **FromFabric** read snapshots the whole value when its first word is read,
  and the later words come from that snapshot.
- A `FromFabric` read returns what the fabric drives *now*. It is not a
  response to an earlier `ToFabric` write, however close together the two
  are. Request/response over plain ports needs a field the fabric bumps once
  the answer is valid (a generation counter, like `Sum::calls`), and the host
  waits for it with `FromFabric::read_until`; `blinky::add` shows the pattern.
  A `ToFabric` port is a mailbox, so a write replaces a request still in
  flight. Queue ports are for traffic that can't lose messages.
- A **queue** has a free-running u32 head (producer count) and tail (consumer
  count). Each counter has exactly one writer, so the bus needs no locks or
  atomics. `used = head - tail`, which keeps full and empty distinct. The depth
  is a power of two from 2 to 65536, and it's part of the fingerprint.
  - Host → fabric: the host writes the entry window, and its last word
    enqueues. The host caches the fabric's tail and re-reads it only when the
    ring looks full, so a push normally costs no reads at all.
  - Fabric → host: the entry window shows the oldest entry, snapshotted on its
    first word. To pop, the host writes the new **absolute** tail, never a
    pop-on-read: a debug dump or a retried read must not eat entries, and the
    fabric ignores a tail write that isn't a step forward.
  - Full and empty only show up in the counters, never as an AXI error. An
    error response can arrive as an SError and panic the kernel. If the
    fabric's counters ever disagree with the host's (`head - tail > N`, or a
    counter the host owns moving on its own), that's `PortError::Desync`; it
    is never used as an index. `bind` starts from the fabric's counters, so a
    restarted host resumes where the ring really is.
- Words from two accessors interleaving on one port would commit or return
  a value neither of them meant. So a binding owns its whole register window:
  `bind` takes an exclusive `flock` on the transport's lock file and holds it
  until the binding is dropped. DevMem and Uio both use
  `/run/lock/facet-hdl-window-<phys>.lock`, so they exclude each other too. A second `bind`, in the same
  process or another one, gets `BindError::Busy`. The kernel releases the lock
  when the holder dies, `kill -9` included. Within one binding, a mutex keeps
  each port access contiguous. Raw `devmem` pokes bypass all of this; only a
  kernel driver owning the window could stop them.
- The register file's `irq` output is **level**-sensitive: high while any
  `FromFabricQueue` holds an entry. A consumer that finds its queue empty and
  then waits can't miss an entry that landed in between, because the line is
  already high when the wait starts. `FromFabricQueue::pop_wait(timeout)`
  sleeps on it when the transport offers an `Interrupt` (`Uio`, and the GHDL
  sim built with `.irq()`), and polls otherwise (`DevMem`).
  - There is one line for the whole boundary. A waiter woken by another
    queue's entry re-checks its own queue and sleeps again. While that other
    entry sits unconsumed, though, the line stays high and the waiter
    degrades to polling. Keep every `FromFabricQueue` drained.
  - Nothing signals "a `ToFabricQueue` has room again". A level "not full"
    line would be high nearly all the time, so a full push returns
    `PushError::Full` and the producer retries.

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
- Queue co-sim: a fabric pipeline (`jobs` → ×3 → `done`) runs 300 sequenced
  jobs both ways at full rate. It's run once free-running and once with an
  LFSR making the fabric stall, and checks for gaps, duplicates and corrupt
  2- and 3-word entries. Backpressure has to actually happen during the run.
  Another test pushes into a frozen consumer: the ninth push returns `Full`,
  and the fabric's counters are unchanged afterwards. A planted bug (the
  fabric consuming while not ready) fails all three tests.
- Window ownership: a second `bind` is refused in-process and from another
  process, and a holder killed with `kill -9` frees the window. A co-sim
  produces a real torn commit through an unowned second accessor.
- Latency: a co-sim with a 200-cycle adder shows write-then-read returning a
  stale value, and `blinky::add` returning the right one.
- Interrupt co-sim: the line is low while the results queue is empty, high
  while a result waits, stays high until it's popped, then goes low again.
  `pop_wait` wakes far inside its timeout and returns `None` once the timeout
  passes. With `irq` planted stuck low, the line test fails and `pop_wait`
  only wakes at its 30 s timeout.
- `Uio`'s register path against a faked `/sys/class/uio` and a plain file in
  place of `/dev/uioN`: lookup by name, the mapping, bounds, a node whose
  `reg` is smaller than the boundary, and the shared lock path. **Not
  verified:** `UioIrq` (unmask by writing 1, `poll`, read the count). That
  needs a real `uio_pdrv_genirq` device.

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
3. **For `Uio` and its interrupt:** a device-tree node for the component with
   `compatible = "generic-uio"`, `reg` covering the window, and the `irq`
   wired to an FPGA-to-HPS interrupt as a level-high SPI. The kernel also
   needs `uio_pdrv_genirq.of_id=generic-uio` on its command line. The module
   is already `=m` in Terasic's kernel config. The SPI number depends on the
   Platform Designer system, so none is assumed here. This belongs in
   nixos-fpga's `hardware.fpga` overlay.
4. **Then:** `blinky --uio <node name> status`, or `blinky --base
   0x2000_0000+<component offset> status` without the node. Don't point
   `--base` at an offset nothing decodes. A DECERR on arm64 can arrive as an
   SError and panic the kernel.

Candidates for single-sourcing next: the Platform Designer `_hw.tcl` for the
register entity, and that device-tree node. Address span, interface and
interrupt come straight from `Boundary` for both. Reset values could come from
`Default`.
