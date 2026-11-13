# facet-hdl

Spike: declare the boundary between an SoC FPGA's ARM side (the HPS) and
its fabric **once**, in Rust, and get both sides from it. The hello world,
`examples/hello`, is one call each way:

```rust
#[facet_hdl::rpc]
pub mod hello {
    /// Runs in the fabric; Rust calls it.
    #[import]
    pub fn add_one(x: u32) -> u32;

    /// Runs here; the fabric calls it each time the button goes down.
    #[export(oneway)]
    pub fn pressed() {
        println!("hello from the fabric!");
    }
}
```

The fabric half is Spade, written against types generated from that module:

```spade
use hello::hello_regs;

pub entity hello_fabric(clk: clock, rst: bool, axi: facet_hdl::AxiIn, button: bool)
    -> (facet_hdl::AxiOut, bool)
{
    let p = inst hello_regs(clk, rst, axi);

    // add_one: answered in the cycle Rust's call arrives.
    set p.add_one_resp.write = match p.add_one_req.read {
        Some(args) => Some(args.x +. 1),
        None => None,
    };
    set p.add_one_req.ack = !p.add_one_resp.full;

    // pressed: one call per press.
    reg(clk) was_down: bool reset(rst: false) initial(false) = button;
    set p.pressed_req.write = if button && !was_down { Some(()) } else { None };

    (p.axi_out, p.irq)
}
```

And the HPS side (`src/bin/hello.rs`) is:

```rust
let fabric: Hello = bind(transport)?;
println!("add_one(41) = {}", block_on(fabric.add_one(41))?);
block_on(fabric.serve())?; // prints "hello from the fabric!" on every press
```

Imports are `async`: a call into the fabric finishes whenever the fabric
answers, so it's a future, and `serve` is one too. They run on any executor
(tokio, smol, or the bundled `block_on` above), and on `&self`, so a host can
serve the fabric's calls and make its own on the same boundary at once.

Calls are built on ports, which you can also declare directly:

```rust
#[derive(Facet)]
pub struct Adder {
    pub operands: ToFabric<Operands>,   // host writes, fabric reads
    pub sum: FromFabric<Sum>,           // fabric drives, host reads
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
gets `push(&T) -> Result<(), PushError>` / `pop() -> Option<T>`, or
`send(&T).await` / `recv().await` to wait for room or for an entry,
and the fabric gets the end of a queue shaped like Spade's own
`std::mem::FifoRead`/`FifoWrite`: `facet_hdl::Read<T> { read: Option<T>,
ack: inv bool }` for a host -> fabric queue, `facet_hdl::Write<T> { write:
inv Option<T>, full: bool }` for a fabric -> host one.

- **Spade** (`facet-hdl-spade`): one Spade type per Rust type (structs,
  enums, arrays, `Option`, `Result`), `pack_*`/`unpack_*` functions between
  each type and its bits, and a `<boundary>_regs(clk, rst, axi)` entity that
  returns one `<Boundary>Ports` struct: the bus's answer, the `irq` line, and
  a field per port (a value for a register, a `Read`/`Write` end for a
  queue). In front of it sits an AXI4 slave (`facet_hdl.spade`) shaped
  for the DE25-Nano's lightweight bridge and the Zynq's M_AXI_GP0: 32-bit
  data, 12-bit IDs echoed, bursts. Your fabric logic is Spade
  and instantiates the register file; the bus is two struct ports
  (`facet_hdl::AxiIn`/`AxiOut`), and flat `s_axi_*` signals only appear on
  the top level. The Spade compiler runs in-process (`Spade::compile`),
  pinned to v0.21.0, so there's no swim step. Its output is SystemVerilog,
  which chipsmith reads.
- **Verilog or VHDL** instead of Spade: `<boundary>_core` is the same
  register file with flat ports, compiled to SystemVerilog
  (`Spade::compile_core`), plus a VHDL package declaring it as a component
  (`Spade::core_vhdl`). Each queue is a valid/ready stream (`<port>_valid`,
  `<port>_data`, `<port>_ready`; an entry moves on a clock edge where both
  are high), each register a plain vector, and payloads are the packed bits
  of the [wire format](#wire-format). hello ships them checked in
  (`hdl/generated/hello_core.sv`, `hello_core_pkg.vhd`), so that side needs
  no Rust, and has its fabric in both languages too
  (`hdl/hello_fabric.v`, `hdl/hello_fabric.vhd`).
- **Host handles** (`facet-hdl`): `bind::<Adder>(transport)` returns that same
  struct with live handles: `b.leds.write(&ctl)?`, `b.status.read()?`. There's
  no `read` on a `ToFabric` and no `write` on a `FromFabric`.
- **A fingerprint** of the layout sits in word 0 of the register window.
  `bind` refuses a bitstream that was generated from a different declaration.

Targets are mercury (Terasic DE25-Nano, Agilex 5) over the lightweight
HPS-to-FPGA bridge and venus (original Digilent Zybo, Zynq-7010) over
M_AXI_GP0, but the pieces are separate crates:

| crate | role |
| --- | --- |
| `facet-hdl-macros` | `#[facet_hdl::rpc]` (re-exported from `facet-hdl`), parsed with unsynn |
| `facet-hdl` | `ToFabric`/`FromFabric` registers, `ToFabricQueue`/`FromFabricQueue` channels, shape → `HwType`, bit codec, `Boundary` layout, `Transport`, `bind` |
| `facet-hdl-spade` | Spade generator, the AXI4 slave, the in-process compiler; `literal(&value)` renders a Rust value as a Spade constant |
| `facet-hdl-iverilog` | Co-simulation: Icarus Verilog runs the compiled fabric, and a generated testbench turns stdin lines into AXI4 transactions, a new ID each. `SimTransport` makes the real host code drive it; `vvp sim.vvp +vcd=<file>` dumps waveforms |
| `facet-hdl-linux` | Linux transports: `DevMem` (uncached `/dev/mem` mapping of the bridge, `AGILEX5_LWH2F = 0x2000_0000`, polls) and `Uio` (a `generic-uio` node: the mapping plus the `irq` line, so `pop_wait` sleeps) |
| `examples/hello` | The hello world above: declaration, the fabric (`hdl/hello_fabric.spade`, and the same in `.v` and `.vhd`), the generator bin, the HPS CLI, and its co-sims |

```sh
nix develop --command cargo test                                 # incl. the iverilog co-sims
nix develop --command cargo run --features gen --bin hello-gen   # regenerate examples/hello/hdl/generated
nix build .#hello-hps                                            # static aarch64-musl hello + hello-async for mercury
nix build .#hello-zybo                                           # static armv7-musl hello + hello-async for venus
nix develop --command cargo run --features gen --bin hello-top -- de25  # a board's top as examples/hello/<board>/build/spade.sv
examples/hello/de25/build.sh                                     # Quartus: build/hello.core.rbf (see "On the boards")
examples/hello/zybo/build.sh                                     # Vivado: build/hello.bit
chipsmith -C examples/hello/zybo/chipsmith build                 # or chipsmith, no Vivado: its build/hello.bit
```

## Calls: the fabric calls Rust, Rust calls the fabric

Mark functions in a module. `#[export]` fns are Rust the fabric calls,
`#[import]` fns are bodiless declarations of HDL that Rust calls:

```rust
#[facet_hdl::rpc]
pub mod calc {
    #[export(depth = 8)]            // up to 8 calls in flight
    pub fn add(a: u32, b: u32) -> Option<u32> { a.checked_add(b) }

    #[export]                       // () still answers: done, or panicked
    pub fn apply(cfg: Config) { /* ... */ }

    #[export(oneway)]               // no answer at all; must return ()
    pub fn log(ev: Event) { /* ... */ }

    #[import]                       // implemented in HDL
    pub fn scale(x: u16) -> u32;

    #[import]                       // no arguments, () answer: just handshakes
    pub fn flush();
}

let calc: calc::Calc = bind(transport)?;
let y = calc.scale(7).await?;   // Rust -> fabric
calc.serve().await?;            // fabric -> Rust, forever (or poll())
```

The macro adds an argument struct per fn (`AddArgs { a, b }`) and a boundary
named after the module (`Calc`) with two queues per fn, `<fn>_req` and
`<fn>_resp`:

- **Export** `add`: the fabric pushes an `AddArgs` onto `add_req` and pops a
  `CallResult<T>` from `add_resp`, either `Returned(T)` or `Panicked`. In
  Spade that's `facet_hdl::CallResult<Option<uint<32>>>`, so the fabric
  matches on it like Rust would. The fns stay as written, so Rust can still
  call them.
- **Import** `scale`: the declaration becomes a method,
  `async fn scale(&self, x) -> Result<u32, PortError>`. The fabric pops a
  `ScaleArgs` from `scale_req` and pushes a plain `uint<32>` onto
  `scale_resp`, since HDL can't panic.
- `oneway` drops `_resp`. An exported oneway fn's panic is only reported by
  the panic hook; an imported one's future resolves once the call is queued.
- A fn with no arguments, or an answer of `()`, is a queue of `()`: the
  fabric reads or writes `Some(())`, and no data is stored. The handshake is
  the message.

Calls to one import can overlap (`join_all` over eight `scale`s puts eight
in flight); past `depth` they wait for room. The ring is FIFO, so answer
`k` belongs to request `k`, and an import's `_resp` is an `rpc::Answers`
rather than a `FromFabricQueue`: each call claims its answer by that number,
and whoever pops another call's answer first parks it for them.

There's no timeout parameter: put your runtime's timeout around the call
(or use `block_on_timeout`). Dropping the future abandons the call. The
request may already be with the fabric, and its answer still comes, but it
gets dropped on arrival instead of being handed to a later call. Answers
owed to a previous process are dropped the same way.

One thread per binding waits on the irq line and wakes whichever futures
are waiting, so nothing needs a reactor. It only touches the line while
someone is waiting. If the fabric stops draining an export's `_resp`,
`serve` blocks on it (there's no "room" interrupt) and stops answering other
fns until it drains.

Limits for now: no generics, no `self`, parameters are `name: Type`, and
exports are sync fns.

## Wire format

These are the rules `codec.rs` and the generated Spade both implement. The
Spade side packs and unpacks explicitly, so the format doesn't depend on how
Spade lays its own types out.

- `bool` = 1 bit. `u8..u128`/`i8..i128` = their width, two's complement.
  A fieldless enum is its declaration index in `ceil(log2(n))` bits. Structs
  are their fields in declaration order with the first field at the LSB.
  Arrays are their elements with index 0 at the LSB.
- An enum whose variants carry data (`Option<T>`, `Result<T, E>`, or your own
  `Cmd::Set(Config)`) is a tag, the variant's declaration index in
  `ceil(log2(n))` bits, then a payload area as wide as the widest payload.
  The payload starts at the area's LSB and the bits above it are zero; a
  decoder ignores them. A `()` payload takes no bits, so `Result<(), E>` is
  just the tag and `E`. A variant carries at most one field: wrap several in
  a struct. `None` is variant 0 and `Some` 1; `Ok` is 0 and `Err` 1.
  In Spade it's the same enum: `Option`/`Result` are Spade's own, and a user
  enum's payload variant is declared `Set { value: Config }`.
- Rejected with an error that names the field path: `usize`, floats, `char`,
  strings, slices, variants with several fields, zero-sized things,
  recursive types.
  Names that are Spade keywords (`reg`, `port`, `set`, ...), or ports whose
  generated internals would collide, are rejected too.
- `()` can only be a whole queue entry, which has no entry words and no
  storage in the fabric. The host pops by writing the tail as usual,
  and pushes by writing the new absolute head to the head word. There's no
  entry word to commit on, and writing the absolute count, like a pop does,
  makes a retried write harmless.
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
  waits for it with `FromFabric::read_until`; `add` in
  `crates/facet-hdl-spade/tests/registers.rs` shows the pattern.
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
  already high when the wait starts. `FromFabricQueue::recv()` (and its
  sync twin `pop_wait(timeout)`) sleeps on it when the transport offers an
  `Interrupt` (`Uio`, and the iverilog sim built with `.irq()`), and polls
  every 50 µs otherwise (`DevMem`).
  - There is one line for the whole boundary. A waiter woken by another
    queue's entry re-checks its own queue and sleeps again. While that other
    entry sits unconsumed, though, the line stays high and the waiter
    degrades to polling. Keep every `FromFabricQueue` drained.
  - Nothing signals "a `ToFabricQueue` has room again". A level "not full"
    line would be high nearly all the time, so a full push returns
    `PushError::Full`, and `send` re-polls every 50 µs until there's room.
- Every AXI access answers OKAY: unmapped words read 0 and ignore writes,
  since an error response can take the kernel down. Each beat is one 32-bit
  word; AxSIZE and WSTRB are ignored, and every burst counts up.
- Generated registers start at their reset values (`initial`), as FPGA flops
  power up, so fabric logic that isn't reset with the register file never
  sees undefined state.

## Verified

Every co-sim runs the real host API against Spade compiled in-process and
simulated by Icarus Verilog. Each was also checked by planting a bug and
watching it fail.

- The unit tests cover the codec, layout, macro expansion and its errors,
  and bind. There's an in-memory fake fabric for fingerprint mismatch and
  bad discriminants.
- AXI4 slave on its own: single beats and 9- and 16-beat bursts, every
  response's ID checked by the testbench. A slave that doesn't echo BID, or
  whose bursts don't advance, fails.
- Echo co-sim covers nested structs, arrays of structs/enums/bools, 2-D
  arrays, `u128`, `i8`/`i64`, and tagged types: `Option<u16>`,
  `Result<(), Tri>`, an array of a user enum with payloads of three widths,
  and `Option<Result<Inner, bool>>`. A golden value written as a Spade
  literal checks packing and unpacking *separately*, so a symmetric bug can't
  hide behind the loopback. A planted field-order bug fails the packing
  check, and a payload read one bit low fails the unpacking one.
- Hello co-sim: Rust's `add_one(41)` comes back 42 (and `u32::MAX`
  wraps to 0), and one 20-cycle button press calls `pressed` exactly once.
  A fabric answering `x` instead of `x + 1` fails, and so does one without
  the edge detection (more than one call per press). The checked-in
  `hdl/generated/` has to match the declaration (for `hello_core.sv`, up
  to the numbering of Spade's internal names, which changes run to run).
- The same hello co-sim against `hello_fabric.v` and against
  `hello_fabric.vhd`, each on the generated `hello_core`. ghdl
  (`--synth --out=verilog`) turns the VHDL into Verilog so iverilog runs it
  all. A VHDL fabric answering `x + 2` fails with 43. Neither has been on a
  board: the board tops still use the Spade fabric.
- Register co-sim (an adder over plain registers): bind/fingerprint, 3-word
  signed sums, a snapshotted 64-bit cycle counter, a 200-cycle adder showing
  write-then-read returning a stale value and the generation-counter wait
  returning the right one, and a torn commit through an unowned second
  accessor.
- Queue co-sim: a Spade pipeline (`jobs` → ×3 → `done`) runs 300 sequenced
  jobs both ways at full rate, once free-running and once with an LFSR
  stalling it. It checks for gaps, duplicates and corrupt 2- and 3-word
  entries, and backpressure has to actually happen. Another test pushes into
  a frozen consumer: the ninth push returns `Full`, and the fabric's counters
  are unchanged. A fabric→host ring planted to accept pushes when full fails
  both rate tests as a `Desync`.
- Interrupt: the line is low while the results queue is empty, high while a
  result waits, stays high until it's popped, then goes low. `pop_wait`
  wakes far inside its timeout and returns `None` once the timeout passes.
  With `irq` planted stuck low, the line test fails and `pop_wait` only
  wakes at its 30 s timeout.
- Window ownership: a second `bind` is refused in-process and from another
  process, and a holder killed with `kill -9` frees the window.
- RPC co-sim: the fabric, in Spade, is the caller. It fires 64 `add` calls
  back to back, 8 in flight, alongside 16 `check` calls, one of which panics
  in Rust, checks every answer by matching on `CallResult`/`Option`/`Result`,
  and reports its tally through a oneway call. Planting an off-by-one in
  Rust's `add` makes it report 56 mismatches, the 56 calls that don't
  overflow. Its counters have no reset on purpose: with the register file's
  queues ready during reset, calls get lost and the test fails.
- Import co-sim, on tokio: Rust calls the Spade fabric's `scale` (also 8
  calls in flight at once), `ping` (no arguments, `()` answer), oneway
  `set_led` read back through `led`, while `serve` answers the fabric's three
  calls to the no-argument export `tick` alongside. `slow` answers 2000
  cycles late, one at a time:
  - `slow(30)` and `slow(40)` both go out, then `slow(40)` is awaited first
    and has to park `slow(30)`'s answer on the way. If other calls' answers
    are dropped, `slow(30)` never returns. If answers are handed out in pop
    order, `slow(40)` gets 31.
  - `slow(1)` is given up on after its request went out, and `slow(2)` must
    still return 3. The same holds across a rebind, for an answer owed to
    the dropped binding.
  - With pop-order hand-out planted, that returns `slow(1)`'s 2.
- `Uio`'s register path against a faked `/sys/class/uio` and a plain file in
  place of `/dev/uioN`: lookup by name, the mapping, bounds, a node whose
  `reg` is smaller than the boundary, and the shared lock path. `UioIrq`
  (unmask by writing 1, `poll`, read the count) needs a real
  `uio_pdrv_genirq` device, so it's only checked on venus (see below).

## On the boards

### mercury (DE25-Nano)

On 2026-11-12, hello ran on the DE25-Nano, built by Quartus Prime Pro 26.1.1
rather than chipsmith: chipsmith's place and route doesn't map the
lightweight bridge's `lwhps2fpga_*` ports yet (only `f2h_irq0[0]` and
`[3]`). `examples/hello/de25` is agilex5-re's `designs/hpsf` with its
read-only bridge responder swapped for `hello_de25` (`hdl/de25_top.spade`:
the fabric, KEY[0] synchronised and debounced for 10.5 ms, irq on
`f2h_irq0[0]`); the HPS IP and pins are hpsf's, so the core keeps the
board's phase 1 HPS IO hash (D921CDF7…C3D07A) and `chipsmith flash --ssh`
loads it. No bridge driver or firmware call is needed: a core load releases
the bridges (nixos-fpga 5240fbc).

`build.sh` renames Spade's escaped module names (`\hello::hello_regs `)
to plain ones first: Quartus' synthesis segfaults inferring the register
file's queue RAMs inside such a module (reproduced in 30 s on `spade.sv`
alone; plain names infer both RAMs and pass).

Seen on the board:

- **bind and add_one:** `hello --base 0x20000000` binds (word 0 reads
  `0x014E106E`, the fingerprint) and prints `add_one(41) = 42`.
- **Idle:** in 23 minutes with nobody at the board, no `pressed` call, so
  the key input doesn't glitch. Polling `DevMem` costs about 18% of a core
  (4:01 of CPU in 22:52). GICD_ISPENDR1 bit 17 (hello's irq) reads 0 with
  no call pending.
- **Overlapping calls on tokio:** `hello-async` runs `serve` with 64
  `add_one` calls at once on one binding: all 64 right.
- **Zero-timeout calls:** 10,000 tries, none abandoned: the fabric answers
  in the cycle the request lands, and a call's first poll writes and reads
  back with no await between. So a dropped call's late answer is still only
  verified in the import co-sim's `slow`.

Not yet seen on mercury:

1. **A key press.** Nobody has pressed KEY[0] yet, so "one `pressed` per
   press, none while held" isn't verified on hardware (the co-sim's 20-cycle
   press is; the debouncer isn't simulated). Same on venus.
2. **`Uio` and its interrupt on mercury** (seen on venus, below). mercury has the node since 2026-11-13
   (nixos-fpga 6b864d2, `hardware.terasic.de25-nano.lwBridgeUio`, on by
   default): `/dev/uio0` named `lwhps2fpga`, mapping 0x2000_0000 (64 KiB),
   interrupt f2h_irq0[0] = GIC SPI 17, level-high, with `uio_pdrv_genirq`
   bound by `of_id=generic-uio`. Its interrupt path is checked with
   chipsmith's de25-irq core (191 blocking reads in 10 s, 191 GIC
   interrupts). The device is root-only (0600). Still to run: `hello --uio`
   with hello's core loaded, expecting about 0% CPU while idle. Being level,
   hello's irq has to fall once serviced, or the line refires at once.
   Avoid `systemctl reboot` on mercury, which hung the HPS once on
   2026-11-13; reload phase 1 over JTAG or power-cycle instead.
3. **A late answer on hardware.** It needs an import that answers late, as
   the co-sim's `slow` does.
4. **chipsmith building it on the DE25.** On the Zybo it does (below).
   For the DE25, chipsmith e4ff99c takes the lightweight bridge in the
   manifest by Quartus' names (`araddr = "lwhps2fpga_araddr"`,
   `lwhps2fpga_clk`, the design wired straight to the HPS's AXI4 ports, no
   adapter) and `phase1 = "de25-nano"` for the shell and hash (a3d0431);
   chipsmith's examples/de25-lw builds but hasn't been on the board. Then
   `de25/build.sh` and `hpsf_top.sv` go.
5. **The 12-bit-ID core.** The slave's IDs widened from 4 to 12 bits for
   GP0 after the run above; the DE25 top zero-extends the bridge's 4. That
   core builds, but hasn't been loaded on mercury.

### venus (Zybo)

On 2026-11-13, hello ran on the original Zybo, built by Vivado 2026.1:
chipsmith gives every design a PS7 with its pins tied off and can't route
M_AXI_GP0 or IRQ_F2P yet. `examples/hello/zybo` is a block design holding
only the PS7 with GP0 and IRQ_F2P[0] (`ps.tcl`; the PS's own DDR/MIO/clock
settings aren't in a PL bitstream, so no board preset), `zybo_top.v` around
it and `hello_zybo` (`hdl/zybo_top.spade`: GP0's address minus its
0x4000_0000 base, BTN0 debounced). hello and GP0 run on 100 MHz from a PLL
off the 125 MHz pin (L16), not FCLK_CLK0, which Linux may gate. 125 MHz
missed by 0.2 ns: the PS7's GP0 outputs arrive late and the register
file's read path behind RREADY is 13 levels deep. At 100 MHz the worst
slack is +0.89 ns. `chipsmith flash --ssh` loads the `.bit`.

32-bit ARM needed a fix in facet-core (`vendor/facet-core`, see
`vendor/README.md`): it marked wide pointers with the address's top bit,
which ARM Linux's stack (0xBExx_xxxx) has set, so `bind` panicked filling
the boundary. qemu-user never showed it.

Seen on the board:

- **bind and add_one:** `hello --base 0x4000_0000` and `hello --uio gp0`
  both print `add_one(41) = 42`.
- **`Uio` sleeps:** `/dev/uio0` "gp0" (nixos-fpga a7e4c62: 64 KiB at
  0x4000_0000, IRQF2P[0] = GIC 61, level-high). `hello --uio gp0` used 5
  ticks of CPU in its first 3 minutes, nearly all of it startup, with its
  two threads asleep in `futex_do_wait` and `poll_schedule_timeout`.
  `--base` polling took 19 s of CPU in 50 s (38% of an A9 core).
- **Overlapping calls on tokio:** `hello-async`, over `--base` and over
  `--uio gp0`: 64 at once, all right; 10,000 zero-timeout calls, none
  abandoned, for the same reason as on mercury.

Built by chipsmith (2f263d0) instead, the same day: `examples/hello/zybo/
chipsmith` is `zybo_top.v` with the PS7 primitive in place of the block
design and chipsmith's PLL, over the same `build/spade.sv`. chipsmith took
the Spade SystemVerilog as it is (14 modules, escaped names), made 822
LUTs, 12 RAM32Ms and 135 CARRY4s, and estimates 8.67 ns at 100 MHz with GP0
timed as Vivado times it (no clock skew modelled). On venus `hello --uio
gp0` printed `add_one(41) = 42`, and `hello-async` over `--uio gp0` and
`--base` ran 64 overlapping calls and 10,000 zero-timeout ones with 0 wrong
answers. Two builds from the same sources give the same `.bit`. The button
is still unpressed.

## Licence

EUPL-1.2, see [LICENSE](LICENSE), from c1db5fe on. Earlier commits were
published under MIT OR Apache-2.0 and stay available under those terms.
